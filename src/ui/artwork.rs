use crate::library::scanner::Scanner;
use crate::library::track::Track;
use lofty::file::TaggedFileExt;
use lofty::probe::Probe;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::widgets::Widget;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct ArtworkCache {
    cache: Arc<Mutex<HashMap<(PathBuf, u16, u16), Vec<HalfBlockPixel>>>>,
}

#[derive(Clone, Copy)]
pub struct HalfBlockPixel {
    pub top_color: Color,
    pub bottom_color: Color,
}

impl Default for ArtworkCache {
    fn default() -> Self {
        Self {
            cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl ArtworkCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_or_render(
        &self,
        track: &Track,
        width: u16,
        height: u16,
    ) -> Option<Vec<HalfBlockPixel>> {
        if width == 0 || height == 0 {
            return None;
        }

        let key = (track.path.clone(), width, height);
        if let Ok(c) = self.cache.lock() {
            if let Some(pixels) = c.get(&key) {
                return Some(pixels.clone());
            }
        }

        // Try extracting and rendering artwork
        let image = Self::load_image_for_track(track)?;
        let pixels = Self::render_to_halfblocks(&image, width as u32, height as u32);

        if let Ok(mut c) = self.cache.lock() {
            c.insert(key, pixels.clone());
        }

        Some(pixels)
    }

    fn load_image_for_track(track: &Track) -> Option<image::DynamicImage> {
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

        // 2. Try parent folder covers
        if let Some(parent) = track.path.parent() {
            if let Some(cover_path) = Scanner::find_folder_cover(parent) {
                if let Ok(img) = image::open(&cover_path) {
                    return Some(img);
                }
            }
        }

        None
    }

    fn render_to_halfblocks(img: &image::DynamicImage, width: u32, height: u32) -> Vec<HalfBlockPixel> {
        // height in terminal rows corresponds to (height * 2) pixels vertically
        let pixel_height = height * 2;
        let resized = img.resize_exact(width, pixel_height, image::imageops::FilterType::Triangle);
        let rgb_img = resized.to_rgb8();

        let mut pixels = Vec::with_capacity((width * height) as usize);

        for y in 0..height {
            let top_y = y * 2;
            let bottom_y = top_y + 1;

            for x in 0..width {
                let top_p = rgb_img.get_pixel(x, top_y);
                let bottom_p = rgb_img.get_pixel(x, bottom_y);

                let top_color = Color::Rgb(top_p[0], top_p[1], top_p[2]);
                let bottom_color = Color::Rgb(bottom_p[0], bottom_p[1], bottom_p[2]);

                pixels.push(HalfBlockPixel {
                    top_color,
                    bottom_color,
                });
            }
        }

        pixels
    }
}

pub struct ArtworkWidget<'a> {
    track: Option<&'a Track>,
    cache: &'a ArtworkCache,
}

impl<'a> ArtworkWidget<'a> {
    pub fn new(track: Option<&'a Track>, cache: &'a ArtworkCache) -> Self {
        Self { track, cache }
    }
}

impl<'a> Widget for ArtworkWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        if let Some(track) = self.track {
            if let Some(pixels) = self.cache.get_or_render(track, area.width, area.height) {
                let mut idx = 0;
                for y in 0..area.height {
                    for x in 0..area.width {
                        if idx < pixels.len() {
                            let p = pixels[idx];
                            let cell = &mut buf[(area.left() + x, area.top() + y)];
                            cell.set_symbol("▀");
                            cell.set_fg(p.top_color);
                            cell.set_bg(p.bottom_color);
                            idx += 1;
                        }
                    }
                }
                return;
            }
        }

        // Placeholder artwork if no cover art is found
        let bg_color = Color::Rgb(36, 40, 59);

        for y in 0..area.height {
            for x in 0..area.width {
                let cell = &mut buf[(area.left() + x, area.top() + y)];
                cell.set_symbol(" ");
                cell.set_bg(bg_color);
            }
        }

        // Draw decorative musical notes in center of placeholder
        let center_x = area.left() + area.width / 2;
        let center_y = area.top() + area.height / 2;
        if center_x < area.right() && center_y < area.bottom() {
            let note = "♫";
            buf.set_string(
                center_x.saturating_sub(1),
                center_y,
                note,
                ratatui::style::Style::default().fg(Color::Rgb(158, 206, 106)),
            );
        }
    }
}
