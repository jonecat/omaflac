use ratatui::style::{Color, Modifier, Style};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;

#[derive(Debug, Clone)]
pub struct OmarchyTheme {
    pub name: String,
    pub is_dark: bool,
    pub last_checked: SystemTime,
    pub last_modified: Option<SystemTime>,

    // Semantic colors from Omarchy colors.toml
    pub accent: Color,
    pub selection: Color,
    pub muted: Color,

    pub background: Color,
    pub dark_background: Color,
    pub darker_background: Color,
    pub lighter_background: Color,

    pub foreground: Color,
    pub dark_foreground: Color,
    pub light_foreground: Color,
    pub bright_foreground: Color,

    pub red: Color,
    pub green: Color,
    pub yellow: Color,
    pub blue: Color,
    pub magenta: Color,
    pub cyan: Color,
    pub orange: Color,

    pub bright_red: Color,
    pub bright_green: Color,
    pub bright_yellow: Color,
    pub bright_blue: Color,
    pub bright_magenta: Color,
    pub bright_cyan: Color,
}

impl Default for OmarchyTheme {
    fn default() -> Self {
        // Fallback default theme (Omarchy signature green & dark slate)
        Self {
            name: "Omarchy Default".to_string(),
            is_dark: true,
            last_checked: SystemTime::now(),
            last_modified: None,

            accent: Color::Rgb(158, 206, 106),          // #9ec66a (Omarchy Green)
            selection: Color::Rgb(37, 46, 86),          // #252e56
            muted: Color::Rgb(109, 125, 182),           // #6d7db6

            background: Color::Rgb(6, 11, 30),          // #060b1e
            dark_background: Color::Rgb(4, 8, 22),      // #040816
            darker_background: Color::Rgb(3, 6, 16),    // #030610
            lighter_background: Color::Rgb(19, 26, 58), // #131a3a

            foreground: Color::Rgb(255, 206, 173),      // #ffcead
            dark_foreground: Color::Rgb(109, 125, 182), // #6d7db6
            light_foreground: Color::Rgb(201, 184, 166),// #c9b8a6
            bright_foreground: Color::Rgb(255, 206, 173),

            red: Color::Rgb(237, 91, 90),
            green: Color::Rgb(146, 165, 147),
            yellow: Color::Rgb(233, 187, 79),
            orange: Color::Rgb(235, 139, 84),
            blue: Color::Rgb(125, 130, 217),
            magenta: Color::Rgb(200, 157, 193),
            cyan: Color::Rgb(163, 191, 209),

            bright_red: Color::Rgb(250, 170, 169),
            bright_green: Color::Rgb(196, 207, 196),
            bright_yellow: Color::Rgb(247, 220, 156),
            bright_blue: Color::Rgb(194, 196, 240),
            bright_magenta: Color::Rgb(234, 215, 231),
            bright_cyan: Color::Rgb(223, 234, 240),
        }
    }
}

impl OmarchyTheme {
    pub fn colors_path() -> Option<PathBuf> {
        let home = dirs::home_dir()?;
        let p1 = home.join(".local/state/omarchy/current/theme/colors.toml");
        if p1.exists() {
            return Some(p1);
        }
        let p2 = home.join(".config/omarchy/current/theme/colors.toml");
        if p2.exists() {
            return Some(p2);
        }
        None
    }

    pub fn theme_name_path() -> Option<PathBuf> {
        let home = dirs::home_dir()?;
        let p1 = home.join(".local/state/omarchy/current/theme.name");
        if p1.exists() {
            return Some(p1);
        }
        None
    }

    pub fn load() -> Self {
        let mut theme = Self::default();

        // 1. Read theme name
        if let Some(p) = Self::theme_name_path() {
            if let Ok(raw_name) = fs::read_to_string(p) {
                let trimmed = raw_name.trim();
                if !trimmed.is_empty() {
                    // Title-case the name (e.g. "ethereal" -> "Ethereal")
                    let capitalized: String = trimmed
                        .split('-')
                        .map(|word| {
                            let mut chars = word.chars();
                            match chars.next() {
                                None => String::new(),
                                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                            }
                        })
                        .collect::<Vec<_>>()
                        .join(" ");
                    theme.name = capitalized;
                }
            }
        }

        // 2. Read colors.toml
        if let Some(path) = Self::colors_path() {
            if let Ok(metadata) = fs::metadata(&path) {
                theme.last_modified = metadata.modified().ok();
            }

            if let Ok(content) = fs::read_to_string(&path) {
                let map = Self::parse_colors_file(&content);
                theme.apply_color_map(&map);
            }
        }

        theme.last_checked = SystemTime::now();
        theme
    }

    pub fn refresh_if_changed(&mut self) -> bool {
        let now = SystemTime::now();
        if let Ok(elapsed) = now.duration_since(self.last_checked) {
            if elapsed.as_millis() < 500 {
                return false;
            }
        }
        self.last_checked = now;

        if let Some(path) = Self::colors_path() {
            if let Ok(meta) = fs::metadata(&path) {
                if let Ok(mtime) = meta.modified() {
                    if self.last_modified != Some(mtime) {
                        *self = Self::load();
                        return true;
                    }
                }
            }
        }
        false
    }

    fn parse_colors_file(content: &str) -> HashMap<String, String> {
        let mut map = HashMap::new();
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            if let Some((k, v)) = trimmed.split_once('=') {
                let key = k.trim().trim_matches('"').trim_matches('\'').to_string();
                let val_raw = v.trim();
                let val_clean = if let Some(after) = val_raw.strip_prefix('"') {
                    after.split('"').next().unwrap_or("").to_string()
                } else if let Some(after) = val_raw.strip_prefix('\'') {
                    after.split('\'').next().unwrap_or("").to_string()
                } else {
                    val_raw.split('#').next().unwrap_or("").trim().to_string()
                };
                map.insert(key, val_clean);
            }
        }
        map
    }

    fn apply_color_map(&mut self, map: &HashMap<String, String>) {
        if let Some(mode) = map.get("mode").or_else(|| map.get("theme_type")) {
            self.is_dark = mode != "light";
        }

        let get_color = |key: &str| -> Option<Color> {
            map.get(key).and_then(|v| Self::parse_hex(v))
        };

        if let Some(c) = get_color("accent") { self.accent = c; }
        if let Some(c) = get_color("selection").or_else(|| get_color("selection_background")) { self.selection = c; }
        if let Some(c) = get_color("muted").or_else(|| get_color("dark_foreground")) { self.muted = c; }

        if let Some(c) = get_color("background") { self.background = c; }
        if let Some(c) = get_color("dark_background").or_else(|| get_color("dark_bg")) { self.dark_background = c; }
        if let Some(c) = get_color("darker_background").or_else(|| get_color("darker_bg")) { self.darker_background = c; }
        if let Some(c) = get_color("lighter_background").or_else(|| get_color("lighter_bg")) { self.lighter_background = c; }

        if let Some(c) = get_color("foreground") { self.foreground = c; }
        if let Some(c) = get_color("dark_foreground").or_else(|| get_color("dark_fg")) { self.dark_foreground = c; }
        if let Some(c) = get_color("light_foreground").or_else(|| get_color("light_fg")) { self.light_foreground = c; }
        if let Some(c) = get_color("bright_foreground").or_else(|| get_color("bright_fg")) { self.bright_foreground = c; }

        if let Some(c) = get_color("red") { self.red = c; }
        if let Some(c) = get_color("green") { self.green = c; }
        if let Some(c) = get_color("yellow") { self.yellow = c; }
        if let Some(c) = get_color("blue") { self.blue = c; }
        if let Some(c) = get_color("magenta").or_else(|| get_color("purple")) { self.magenta = c; }
        if let Some(c) = get_color("cyan") { self.cyan = c; }
        if let Some(c) = get_color("orange") { self.orange = c; }

        if let Some(c) = get_color("bright_red") { self.bright_red = c; }
        if let Some(c) = get_color("bright_green") { self.bright_green = c; }
        if let Some(c) = get_color("bright_yellow") { self.bright_yellow = c; }
        if let Some(c) = get_color("bright_blue") { self.bright_blue = c; }
        if let Some(c) = get_color("bright_magenta").or_else(|| get_color("bright_purple")) { self.bright_magenta = c; }
        if let Some(c) = get_color("bright_cyan") { self.bright_cyan = c; }
    }

    pub fn parse_hex(hex: &str) -> Option<Color> {
        let clean = hex.trim().trim_matches('"').trim_matches('\'').trim_start_matches('#');
        if clean.len() == 6 {
            let r = u8::from_str_radix(&clean[0..2], 16).ok()?;
            let g = u8::from_str_radix(&clean[2..4], 16).ok()?;
            let b = u8::from_str_radix(&clean[4..6], 16).ok()?;
            Some(Color::Rgb(r, g, b))
        } else {
            None
        }
    }

    // Helper styling functions for UI components
    pub fn block_style(&self) -> Style {
        Style::default().fg(self.muted)
    }

    pub fn block_focus_style(&self) -> Style {
        Style::default().fg(self.accent).add_modifier(Modifier::BOLD)
    }

    pub fn primary_text(&self) -> Style {
        Style::default().fg(self.foreground)
    }

    pub fn secondary_text(&self) -> Style {
        Style::default().fg(self.light_foreground)
    }

    pub fn muted_text(&self) -> Style {
        Style::default().fg(self.muted)
    }

    pub fn accent_text(&self) -> Style {
        Style::default().fg(self.accent).add_modifier(Modifier::BOLD)
    }

    pub fn selected_row(&self) -> Style {
        Style::default()
            .bg(self.selection)
            .fg(self.bright_foreground)
            .add_modifier(Modifier::BOLD)
    }

    pub fn focused_title_style(&self) -> Style {
        Style::default()
            .bg(self.accent)
            .fg(self.dark_background)
            .add_modifier(Modifier::BOLD)
    }

    pub fn unfocused_title_style(&self) -> Style {
        Style::default().fg(self.muted)
    }

    pub fn card_style(&self) -> Style {
        Style::default().bg(self.lighter_background).fg(self.foreground)
    }

    /// Render a btop-style scrollbar along the right border of a pane when `total_items > visible_rows`.
    pub fn render_scrollbar(
        &self,
        buf: &mut ratatui::buffer::Buffer,
        area: ratatui::layout::Rect,
        content_top: u16,
        content_height: u16,
        scroll_offset: usize,
        visible_rows: usize,
        total_items: usize,
    ) {
        if total_items <= visible_rows || visible_rows == 0 || content_height < 2 || area.width < 2 {
            return;
        }

        let x = area.right().saturating_sub(1);
        let max_scroll = total_items.saturating_sub(visible_rows);
        let clamped_offset = scroll_offset.min(max_scroll);

        if content_height >= 4 {
            let top_y = content_top;
            let bot_y = content_top + content_height - 1;

            let up_style = if clamped_offset > 0 {
                Style::default().fg(self.accent).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(self.muted)
            };
            let down_style = if clamped_offset < max_scroll {
                Style::default().fg(self.accent).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(self.muted)
            };

            buf.set_string(x, top_y, "▲", up_style);
            buf.set_string(x, bot_y, "▼", down_style);

            let track_h = (content_height - 2) as usize;
            let thumb_h = ((visible_rows as f32 / total_items as f32) * track_h as f32).round() as usize;
            let thumb_h = thumb_h.clamp(1, track_h.saturating_sub(1).max(1));
            let avail_travel = track_h.saturating_sub(thumb_h);
            let thumb_top = if max_scroll > 0 {
                ((clamped_offset as f32 / max_scroll as f32) * avail_travel as f32).round() as usize
            } else {
                0
            };

            for i in 0..track_h {
                let y = content_top + 1 + i as u16;
                if i >= thumb_top && i < thumb_top + thumb_h {
                    buf.set_string(x, y, "█", Style::default().fg(self.accent));
                } else {
                    buf.set_string(x, y, "│", Style::default().fg(self.muted));
                }
            }
        } else {
            let track_h = content_height as usize;
            let avail_travel = track_h.saturating_sub(1);
            let thumb_top = if max_scroll > 0 {
                ((clamped_offset as f32 / max_scroll as f32) * avail_travel as f32).round() as usize
            } else {
                0
            };
            for i in 0..track_h {
                let y = content_top + i as u16;
                if i == thumb_top {
                    buf.set_string(x, y, "█", Style::default().fg(self.accent));
                } else {
                    buf.set_string(x, y, "│", Style::default().fg(self.muted));
                }
            }
        }
    }
}
