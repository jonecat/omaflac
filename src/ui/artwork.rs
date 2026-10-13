use crate::library::scanner::Scanner;
use crate::library::track::Track;
use crate::ui::theme::OmarchyTheme;
use lofty::file::TaggedFileExt;
use lofty::probe::Probe;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{StatefulWidget, Widget};
use ratatui_image::picker::Picker;
use ratatui_image::protocol::StatefulProtocol;
use ratatui_image::StatefulImage;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

pub const SPINNER_CHARS: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ArtSize {
    Full,
    Thumb,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtRenderResult {
    Rendered,
    Loading,
    NotFound,
}

#[derive(Clone)]
pub struct ArtworkCache {
    picker: Arc<Mutex<Picker>>,
    images: Arc<Mutex<HashMap<PathBuf, Option<Arc<image::DynamicImage>>>>>,
    in_flight: Arc<Mutex<HashSet<PathBuf>>>,
    protocols: Arc<Mutex<HashMap<(PathBuf, ArtSize), StatefulProtocol>>>,
}

impl Default for ArtworkCache {
    fn default() -> Self {
        // Query terminal stdio for graphics capabilities (Ghostty Kitty protocol / Sixel / Halfblocks)
        let mut options = ratatui_image::picker::cap_parser::QueryStdioOptions::default();
        options.timeout = std::time::Duration::from_millis(100);
        let picker = Picker::from_query_stdio_with_options(options).unwrap_or_else(|_| {
            let mut p = Picker::halfblocks();
            let is_kitty_compat = std::env::var("GHOSTTY_RESOURCES_DIR").is_ok()
                || std::env::var("TERM_PROGRAM").map(|v| v.contains("ghostty") || v.contains("kitty")).unwrap_or(false)
                || std::env::var("TERM").map(|t| t.contains("kitty") || t.contains("ghostty")).unwrap_or(false)
                || std::env::var("KITTY_WINDOW_ID").is_ok();
            if is_kitty_compat {
                p.set_protocol_type(ratatui_image::picker::ProtocolType::Kitty);
            }
            p
        });
        Self {
            picker: Arc::new(Mutex::new(picker)),
            images: Arc::new(Mutex::new(HashMap::new())),
            in_flight: Arc::new(Mutex::new(HashSet::new())),
            protocols: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl ArtworkCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_loading(&self, path: &Path) -> bool {
        if let Ok(flight) = self.in_flight.lock() {
            flight.contains(path)
        } else {
            false
        }
    }

    pub fn thumb_width_for_height(&self, height_rows: u16) -> u16 {
        if let Ok(picker) = self.picker.lock() {
            let fs = picker.font_size();
            if fs.width > 0 && fs.height > 0 {
                let target_px = (height_rows as u32) * (fs.height as u32);
                let cols = (target_px + fs.width as u32 - 1) / (fs.width as u32);
                return (cols as u16).max(height_rows * 2);
            }
        }
        height_rows * 2 + 1
    }

    pub fn render_track_artwork(
        &self,
        track: &Track,
        size: ArtSize,
        area: Rect,
        buf: &mut Buffer,
    ) -> ArtRenderResult {
        let resize_mode = match size {
            ArtSize::Thumb => ratatui_image::Resize::Scale(Some(ratatui_image::FilterType::CatmullRom)),
            ArtSize::Full => ratatui_image::Resize::Fit(Some(ratatui_image::FilterType::CatmullRom)),
        };

        // 1. Check if resize protocol already exists in cache
        if let Ok(mut protos) = self.protocols.lock() {
            if let Some(proto) = protos.get_mut(&(track.path.clone(), size)) {
                let widget = StatefulImage::default().resize(resize_mode);
                widget.render(area, buf, proto);
                return ArtRenderResult::Rendered;
            }
        }

        // 2. Check if decoded image is already cached in memory
        let cached_image = {
            if let Ok(img_map) = self.images.lock() {
                img_map.get(&track.path).cloned()
            } else {
                None
            }
        };

        match cached_image {
            Some(Some(arc_image)) => {
                // Decoded image is in memory: create protocol for this size
                if let Ok(picker) = self.picker.lock() {
                    let img_for_proto = if size == ArtSize::Thumb {
                        let w = arc_image.width();
                        let h = arc_image.height();
                        let side = w.min(h);
                        if side > 0 && w != h {
                            arc_image.crop_imm((w - side) / 2, (h - side) / 2, side, side)
                        } else {
                            (*arc_image).clone()
                        }
                    } else {
                        (*arc_image).clone()
                    };
                    let mut proto = picker.new_resize_protocol(img_for_proto);
                    let widget = StatefulImage::default().resize(resize_mode);
                    widget.render(area, buf, &mut proto);
                    if let Ok(mut protos) = self.protocols.lock() {
                        protos.insert((track.path.clone(), size), proto);
                    }
                    return ArtRenderResult::Rendered;
                }
                ArtRenderResult::NotFound
            }
            Some(None) => {
                // Already checked and track has no artwork
                ArtRenderResult::NotFound
            }
            None => {
                // Not in image cache: start asynchronous load in background thread
                let mut should_spawn = false;
                if let Ok(mut flight) = self.in_flight.lock() {
                    if !flight.contains(&track.path) {
                        flight.insert(track.path.clone());
                        should_spawn = true;
                    }
                }

                if should_spawn {
                    let path = track.path.clone();
                    let track_clone = track.clone();
                    let images_clone = Arc::clone(&self.images);
                    let in_flight_clone = Arc::clone(&self.in_flight);

                    std::thread::Builder::new()
                        .name("art-loader".into())
                        .spawn(move || {
                            let maybe_img = Self::load_image_for_track(&track_clone).map(Arc::new);
                            if let Ok(mut map) = images_clone.lock() {
                                map.insert(path.clone(), maybe_img);
                            }
                            if let Ok(mut flight) = in_flight_clone.lock() {
                                flight.remove(&path);
                            }
                        })
                        .ok();
                }

                ArtRenderResult::Loading
            }
        }
    }

    pub fn load_image_for_track(track: &Track) -> Option<image::DynamicImage> {
        // 1. Try embedded tag pictures
        if let Ok(tagged_file) = Probe::open(&track.path).and_then(|p| p.read()) {
            let tag = tagged_file.primary_tag().or_else(|| tagged_file.first_tag());
            if let Some(t) = tag {
                if let Some(pic) = t.pictures().first() {
                    if let Ok(img) = image::load_from_memory(pic.data()) {
                        return Some(img);
                    }
                }
            }
        }

        // 2. Try parent folder covers (cover.jpg, folder.jpg, album.jpg, etc.)
        if let Some(parent) = track.path.parent() {
            if let Some(cover_path) = Scanner::find_folder_cover(parent) {
                if let Ok(img) = image::open(&cover_path) {
                    return Some(img);
                }
            }
        }

        None
    }
}

pub struct ArtworkWidget<'a> {
    track: Option<&'a Track>,
    cache: &'a ArtworkCache,
    theme: Option<&'a OmarchyTheme>,
    size: ArtSize,
}

impl<'a> ArtworkWidget<'a> {
    pub fn new(
        track: Option<&'a Track>,
        cache: &'a ArtworkCache,
        theme: Option<&'a OmarchyTheme>,
    ) -> Self {
        Self {
            track,
            cache,
            theme,
            size: ArtSize::Full,
        }
    }

    pub fn with_size(mut self, size: ArtSize) -> Self {
        self.size = size;
        self
    }
}

impl<'a> Widget for ArtworkWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let bg_color = self
            .theme
            .map(|t| t.dark_background)
            .unwrap_or(Color::Rgb(20, 24, 45));

        let accent_color = self
            .theme
            .map(|t| t.accent)
            .unwrap_or(Color::Rgb(158, 206, 106));

        let cyan_color = self
            .theme
            .map(|t| t.cyan)
            .unwrap_or(Color::Rgb(125, 207, 255));

        let current_spinner = {
            let ms = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis();
            let idx = (ms / 90) as usize % SPINNER_CHARS.len();
            SPINNER_CHARS[idx]
        };

        if let Some(track) = self.track {
            match self.cache.render_track_artwork(track, self.size, area, buf) {
                ArtRenderResult::Rendered => return,
                ArtRenderResult::Loading => {
                    // Draw loading state box with spinner
                    for y in 0..area.height {
                        for x in 0..area.width {
                            let cell = &mut buf[(area.left() + x, area.top() + y)];
                            cell.set_symbol(" ");
                            cell.set_bg(bg_color);
                        }
                    }

                    let center_y = area.top() + area.height / 2;
                    let center_x = area.left() + area.width / 2;

                    if self.size == ArtSize::Thumb {
                        if center_x < area.right() && center_y < area.bottom() {
                            buf.set_string(
                                center_x.saturating_sub(1),
                                center_y,
                                format!("{} ♪", current_spinner),
                                Style::default().fg(cyan_color).add_modifier(Modifier::BOLD),
                            );
                        }
                    } else {
                        // Full artwork loading box
                        let spin_str = format!("{}  Loading Album Artwork...", current_spinner);
                        let msg_x = center_x.saturating_sub(spin_str.chars().count() as u16 / 2);
                        if msg_x < area.right() && center_y < area.bottom() {
                            buf.set_string(
                                msg_x,
                                center_y,
                                &spin_str,
                                Style::default().fg(cyan_color).add_modifier(Modifier::BOLD),
                            );
                        }
                    }
                    return;
                }
                ArtRenderResult::NotFound => {
                    // Fallthrough to placeholder below
                }
            }
        }

        // Placeholder artwork if no cover art is found or no track selected
        for y in 0..area.height {
            for x in 0..area.width {
                let cell = &mut buf[(area.left() + x, area.top() + y)];
                cell.set_symbol(" ");
                cell.set_bg(bg_color);
            }
        }

        // Draw decorative vinyl/note icon in center of placeholder
        let center_x = area.left() + area.width / 2;
        let center_y = area.top() + area.height / 2;
        if center_x < area.right() && center_y < area.bottom() {
            let note = if self.size == ArtSize::Thumb { "♫" } else { "♪  No Art  ♪" };
            let note_x = center_x.saturating_sub(note.chars().count() as u16 / 2);
            buf.set_string(
                note_x,
                center_y,
                note,
                Style::default().fg(accent_color),
            );
        }
    }
}
