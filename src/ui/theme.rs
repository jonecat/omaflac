use ratatui::style::{Color, Modifier, Style};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub const PRESET_THEMES: &[&str] = &[
    "auto",
    "gotham",
    "tokyonight",
    "catppuccin-mocha",
    "catppuccin-latte",
    "gruvbox-dark",
    "nord",
    "dracula",
    "rose-pine",
    "solarized-dark",
    "solarized-light",
    "omarchy",
];

#[derive(Debug, Clone)]
pub struct OmarchyTheme {
    pub name: String,
    pub preferred_name: String,
    pub is_dark: bool,
    pub last_checked: SystemTime,
    pub last_modified: Option<SystemTime>,
    pub watched_paths: Vec<PathBuf>,

    // Semantic colors
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
        Self::preset_omarchy_default()
    }
}

impl OmarchyTheme {
    pub fn list_presets() -> &'static [&'static str] {
        PRESET_THEMES
    }

    pub fn next_preset(current: &str) -> &'static str {
        let cur_clean = current.to_lowercase();
        let idx = PRESET_THEMES
            .iter()
            .position(|&p| p == cur_clean)
            .unwrap_or(0);
        let next_idx = (idx + 1) % PRESET_THEMES.len();
        PRESET_THEMES[next_idx]
    }

    pub fn colors_path() -> Option<PathBuf> {
        Self::omarchy_colors_path()
    }

    pub fn omarchy_colors_path() -> Option<PathBuf> {
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

    pub fn custom_theme_path() -> Option<PathBuf> {
        let config_dir = dirs::config_dir()?;
        let path = config_dir.join("omaflac/theme.toml");
        if path.exists() {
            Some(path)
        } else {
            None
        }
    }

    pub fn load() -> Self {
        Self::load_with_preference(Some("auto"))
    }

    pub fn load_with_preference(preferred: Option<&str>) -> Self {
        let pref = preferred.unwrap_or("auto").trim().to_lowercase();

        // 1. If explicitly requested a preset (e.g. for unit tests), load it
        if pref != "auto" && !pref.is_empty() {
            if let Some(preset) = Self::load_preset(&pref) {
                return preset;
            }
        }

        // 2. Check for custom user theme at ~/.config/omaflac/theme.toml
        if let Some(custom) = Self::detect_custom_theme() {
            return custom;
        }

        // 3. Check for Omarchy Linux system theme (~/.local/state/omarchy/...)
        if let Some(omarchy) = Self::detect_omarchy_theme() {
            return omarchy;
        }

        // 4. Try active running terminal emulator first
        if let Some(active) = Self::detect_active_terminal_theme() {
            return active;
        }

        // 5. Fallback auto-detect from terminal configs (Ghostty, Kitty, Alacritty, WezTerm, Foot)
        if let Some(ghostty) = Self::detect_ghostty_theme() {
            return ghostty;
        }
        if let Some(kitty) = Self::detect_kitty_theme() {
            return kitty;
        }
        if let Some(alacritty) = Self::detect_alacritty_theme() {
            return alacritty;
        }
        if let Some(wezterm) = Self::detect_wezterm_theme() {
            return wezterm;
        }
        if let Some(foot) = Self::detect_foot_theme() {
            return foot;
        }

        // 6. Query terminal dynamically via OSC 10 / OSC 11
        if let Some(osc) = Self::detect_from_terminal_osc() {
            return osc;
        }

        // 7. Default fallback
        let mut def = Self::preset_omarchy_default();
        def.preferred_name = "auto".to_string();
        def
    }

    pub fn refresh_if_changed(&mut self) -> bool {
        let now = SystemTime::now();
        if let Ok(elapsed) = now.duration_since(self.last_checked) {
            if elapsed.as_millis() < 500 {
                return false;
            }
        }
        self.last_checked = now;

        // If locked to a static preset, nothing to watch
        if self.preferred_name != "auto" && !self.preferred_name.is_empty() {
            return false;
        }

        let mut changed = false;
        for path in &self.watched_paths {
            if let Ok(meta) = fs::metadata(path) {
                if let Ok(mtime) = meta.modified() {
                    if self.last_modified.map(|lm| mtime > lm).unwrap_or(true) {
                        changed = true;
                        break;
                    }
                }
            }
        }

        // Also check if Omarchy theme or custom theme newly appeared
        if !changed {
            if let Some(p) = Self::custom_theme_path() {
                if !self.watched_paths.contains(&p) {
                    changed = true;
                }
            }
            if let Some(p) = Self::omarchy_colors_path() {
                if !self.watched_paths.contains(&p) {
                    changed = true;
                }
            }
        }

        if changed {
            *self = Self::load_with_preference(Some(&self.preferred_name));
            return true;
        }

        false
    }

    // =========================================================================
    // Theme Detectors
    // =========================================================================

    fn detect_custom_theme() -> Option<Self> {
        let path = Self::custom_theme_path()?;
        let meta = fs::metadata(&path).ok()?;
        let mtime = meta.modified().ok();
        let content = fs::read_to_string(&path).ok()?;
        let map = Self::parse_colors_file(&content);

        let mut theme = Self::preset_omarchy_default();
        theme.name = "Custom (theme.toml)".to_string();
        theme.preferred_name = "auto".to_string();
        theme.last_modified = mtime;
        theme.watched_paths = vec![path];
        theme.apply_color_map(&map);
        Some(theme)
    }

    fn detect_omarchy_theme() -> Option<Self> {
        let colors_p = Self::omarchy_colors_path()?;
        let meta = fs::metadata(&colors_p).ok()?;
        let mtime = meta.modified().ok();
        let content = fs::read_to_string(&colors_p).ok()?;
        let map = Self::parse_colors_file(&content);

        let mut theme = Self::preset_omarchy_default();
        theme.preferred_name = "auto".to_string();
        theme.name = "Omarchy Default".to_string();

        let mut watched = vec![colors_p];

        if let Some(np) = Self::theme_name_path() {
            watched.push(np.clone());
            if let Ok(raw_name) = fs::read_to_string(np) {
                let trimmed = raw_name.trim();
                if !trimmed.is_empty() {
                    let cap: String = trimmed
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
                    theme.name = format!("Omarchy ({})", cap);
                }
            }
        }

        theme.last_modified = mtime;
        theme.watched_paths = watched;
        theme.apply_color_map(&map);
        Some(theme)
    }

    fn detect_ghostty_theme() -> Option<Self> {
        let home = dirs::home_dir()?;
        let config_candidates = [
            home.join(".config/ghostty/config"),
            home.join("Library/Application Support/com.mitchellh.ghostty/config"),
        ];

        let mut found_config = None;
        for c in &config_candidates {
            if c.exists() {
                found_config = Some(c.clone());
                break;
            }
        }

        let config_path = found_config?;
        let config_content = fs::read_to_string(&config_path).ok()?;
        let config_mtime = fs::metadata(&config_path)
            .ok()
            .and_then(|m| m.modified().ok());

        let mut watched_paths = vec![config_path.clone()];

        // Parse key-values from Ghostty config
        let mut theme_name_opt: Option<String> = None;
        let mut direct_bg: Option<Color> = None;
        let mut direct_fg: Option<Color> = None;
        let mut direct_sel: Option<Color> = None;
        let mut direct_palette: [Option<Color>; 16] = [None; 16];

        for line in config_content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            if let Some((k, v)) = trimmed.split_once('=') {
                let key = k.trim().to_lowercase();
                let val = v.trim().trim_matches('"').trim_matches('\'').trim();
                if key == "theme" {
                    theme_name_opt = Some(val.to_string());
                } else if key == "background" {
                    direct_bg = Self::parse_hex(val);
                } else if key == "foreground" {
                    direct_fg = Self::parse_hex(val);
                } else if key == "selection-background" || key == "selection_background" {
                    direct_sel = Self::parse_hex(val);
                } else if key == "palette" {
                    if let Some((idx_s, hex_s)) = val.split_once('=') {
                        if let Ok(idx) = idx_s.trim().parse::<usize>() {
                            if idx < 16 {
                                direct_palette[idx] = Self::parse_hex(hex_s.trim());
                            }
                        }
                    }
                }
            }
        }

        let mut theme_display = "Ghostty".to_string();
        let mut theme_bg = direct_bg;
        let mut theme_fg = direct_fg;
        let mut theme_sel = direct_sel;
        let mut theme_palette = direct_palette;

        if let Some(t_name) = theme_name_opt {
            let search_dirs = [
                home.join(".config/ghostty/themes"),
                home.join("Library/Application Support/com.mitchellh.ghostty/themes"),
                PathBuf::from("/Applications/Ghostty.app/Contents/Resources/ghostty/themes"),
                PathBuf::from("/usr/share/ghostty/themes"),
                PathBuf::from("/usr/local/share/ghostty/themes"),
            ];

            let mut theme_file = None;
            for dir in &search_dirs {
                if let Some(p) = Self::find_case_insensitive_file(dir, &t_name) {
                    theme_file = Some(p);
                    break;
                }
            }

            if let Some(tf) = theme_file {
                if let Ok(meta) = fs::metadata(&tf) {
                    if meta.modified().is_ok() {
                        theme_display = format!(
                            "Ghostty: {}",
                            tf.file_name()
                                .and_then(|s| s.to_str())
                                .unwrap_or(&t_name)
                        );
                        watched_paths.push(tf.clone());
                        if let Ok(t_content) = fs::read_to_string(&tf) {
                            let (f_bg, f_fg, f_sel, f_pal) = Self::parse_ghostty_theme_content(&t_content);
                            if theme_bg.is_none() { theme_bg = f_bg; }
                            if theme_fg.is_none() { theme_fg = f_fg; }
                            if theme_sel.is_none() { theme_sel = f_sel; }
                            for i in 0..16 {
                                if theme_palette[i].is_none() {
                                    theme_palette[i] = f_pal[i];
                                }
                            }
                        }
                    }
                }
            } else {
                // If not found as a file, check if it matches a known preset
                if let Some(preset) = Self::load_preset(&t_name) {
                    let mut p = preset;
                    p.name = format!("Ghostty: {}", p.name);
                    p.preferred_name = "auto".to_string();
                    p.watched_paths = watched_paths;
                    p.last_modified = config_mtime;
                    return Some(p);
                }
            }
        }

        if theme_bg.is_some() || theme_palette.iter().any(|c| c.is_some()) {
            Some(Self::build_from_palette(
                theme_display,
                theme_bg,
                theme_fg,
                theme_sel,
                theme_palette,
                watched_paths,
                config_mtime,
                "auto".to_string(),
            ))
        } else {
            None
        }
    }

    fn parse_ghostty_theme_content(
        content: &str,
    ) -> (Option<Color>, Option<Color>, Option<Color>, [Option<Color>; 16]) {
        let mut bg = None;
        let mut fg = None;
        let mut sel = None;
        let mut pal = [None; 16];

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            if let Some((k, v)) = trimmed.split_once('=') {
                let key = k.trim().to_lowercase();
                let val = v.trim().trim_matches('"').trim_matches('\'').trim();
                if key == "background" {
                    bg = Self::parse_hex(val);
                } else if key == "foreground" {
                    fg = Self::parse_hex(val);
                } else if key == "selection-background" || key == "selection_background" {
                    sel = Self::parse_hex(val);
                } else if key == "palette" {
                    if let Some((idx_s, hex_s)) = val.split_once('=') {
                        if let Ok(idx) = idx_s.trim().parse::<usize>() {
                            if idx < 16 {
                                pal[idx] = Self::parse_hex(hex_s.trim());
                            }
                        }
                    }
                }
            }
        }

        (bg, fg, sel, pal)
    }

    fn detect_kitty_theme() -> Option<Self> {
        let home = dirs::home_dir()?;
        let candidates = [
            home.join(".config/kitty/current-theme.conf"),
            home.join(".config/kitty/kitty.conf"),
        ];

        let mut config_path = None;
        for c in &candidates {
            if c.exists() {
                config_path = Some(c.clone());
                break;
            }
        }

        let path = config_path?;
        let content = fs::read_to_string(&path).ok()?;
        let mtime = fs::metadata(&path).ok().and_then(|m| m.modified().ok());

        let mut bg = None;
        let mut fg = None;
        let mut sel = None;
        let mut pal = [None; 16];

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.len() >= 2 {
                let key = parts[0].to_lowercase();
                let val = parts[1];
                if key == "background" {
                    bg = Self::parse_hex(val);
                } else if key == "foreground" {
                    fg = Self::parse_hex(val);
                } else if key == "selection_background" {
                    sel = Self::parse_hex(val);
                } else if key.starts_with("color") {
                    if let Ok(idx) = key.strip_prefix("color").unwrap_or("").parse::<usize>() {
                        if idx < 16 {
                            pal[idx] = Self::parse_hex(val);
                        }
                    }
                }
            }
        }

        if bg.is_some() || pal.iter().any(|c| c.is_some()) {
            Some(Self::build_from_palette(
                "Kitty Theme".to_string(),
                bg,
                fg,
                sel,
                pal,
                vec![path],
                mtime,
                "auto".to_string(),
            ))
        } else {
            None
        }
    }

    fn detect_alacritty_theme() -> Option<Self> {
        let home = dirs::home_dir()?;
        let candidates = [
            home.join(".config/alacritty/alacritty.toml"),
            home.join(".config/alacritty.toml"),
            home.join(".alacritty.toml"),
        ];

        let mut config_path = None;
        for c in &candidates {
            if c.exists() {
                config_path = Some(c.clone());
                break;
            }
        }

        let path = config_path?;
        let content = fs::read_to_string(&path).ok()?;
        let mtime = fs::metadata(&path).ok().and_then(|m| m.modified().ok());

        let mut bg = None;
        let mut fg = None;
        let mut sel = None;
        let mut pal = [None; 16];

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            if let Some((k, v)) = trimmed.split_once('=') {
                let key = k.trim().to_lowercase();
                let val = v.trim().trim_matches('"').trim_matches('\'').trim();
                if key == "background" {
                    bg = Self::parse_hex(val);
                } else if key == "foreground" {
                    fg = Self::parse_hex(val);
                } else if key.contains("selection") && key.contains("background") {
                    sel = Self::parse_hex(val);
                } else if key == "black" {
                    pal[0] = Self::parse_hex(val);
                } else if key == "red" {
                    pal[1] = Self::parse_hex(val);
                } else if key == "green" {
                    pal[2] = Self::parse_hex(val);
                } else if key == "yellow" {
                    pal[3] = Self::parse_hex(val);
                } else if key == "blue" {
                    pal[4] = Self::parse_hex(val);
                } else if key == "magenta" {
                    pal[5] = Self::parse_hex(val);
                } else if key == "cyan" {
                    pal[6] = Self::parse_hex(val);
                } else if key == "white" {
                    pal[7] = Self::parse_hex(val);
                }
            }
        }

        if bg.is_some() || pal.iter().any(|c| c.is_some()) {
            Some(Self::build_from_palette(
                "Alacritty Theme".to_string(),
                bg,
                fg,
                sel,
                pal,
                vec![path],
                mtime,
                "auto".to_string(),
            ))
        } else {
            None
        }
    }

    fn detect_wezterm_theme() -> Option<Self> {
        let home = dirs::home_dir()?;
        let candidates = [home.join(".config/wezterm/wezterm.lua"), home.join(".wezterm.lua")];

        for path in &candidates {
            if let Ok(content) = fs::read_to_string(path) {
                for line in content.lines() {
                    let trimmed = line.trim();
                    if trimmed.starts_with("--") {
                        continue;
                    }
                    if trimmed.contains("color_scheme") {
                        if let Some((_, v)) = trimmed.split_once('=') {
                            let scheme = v
                                .trim()
                                .trim_matches('"')
                                .trim_matches('\'')
                                .trim_matches(',')
                                .trim();
                            if let Some(preset) = Self::load_preset(scheme) {
                                let mut p = preset;
                                p.name = format!("WezTerm ({})", p.name);
                                p.preferred_name = "auto".to_string();
                                p.watched_paths = vec![path.clone()];
                                p.last_modified = fs::metadata(path).ok().and_then(|m| m.modified().ok());
                                return Some(p);
                            }
                        }
                    }
                }
            }
        }
        None
    }

    fn detect_active_terminal_theme() -> Option<Self> {
        let is_ghostty = std::env::var("GHOSTTY_RESOURCES_DIR").is_ok()
            || std::env::var("TERM_PROGRAM").map(|s| s.eq_ignore_ascii_case("ghostty")).unwrap_or(false);
        if is_ghostty {
            if let Some(theme) = Self::detect_ghostty_theme() {
                return Some(theme);
            }
        }

        let is_kitty = std::env::var("KITTY_WINDOW_ID").is_ok()
            || std::env::var("KITTY_PID").is_ok()
            || std::env::var("TERM").map(|s| s == "xterm-kitty").unwrap_or(false);
        if is_kitty {
            if let Some(theme) = Self::detect_kitty_theme() {
                return Some(theme);
            }
        }

        let is_alacritty = std::env::var("ALACRITTY_WINDOW_ID").is_ok()
            || std::env::var("ALACRITTY_SOCKET").is_ok()
            || std::env::var("ALACRITTY_LOG").is_ok()
            || std::env::var("TERM").map(|s| s == "alacritty").unwrap_or(false);
        if is_alacritty {
            if let Some(theme) = Self::detect_alacritty_theme() {
                return Some(theme);
            }
        }

        let is_wezterm = std::env::var("WEZTERM_PANE").is_ok()
            || std::env::var("WEZTERM_EXECUTABLE").is_ok()
            || std::env::var("TERM_PROGRAM").map(|s| s.eq_ignore_ascii_case("wezterm")).unwrap_or(false);
        if is_wezterm {
            if let Some(theme) = Self::detect_wezterm_theme() {
                return Some(theme);
            }
        }

        let is_foot = std::env::var("FOOT_TERMINAL").is_ok()
            || std::env::var("TERM").map(|s| s.starts_with("foot")).unwrap_or(false);
        if is_foot {
            if let Some(theme) = Self::detect_foot_theme() {
                return Some(theme);
            }
        }

        None
    }

    fn detect_foot_theme() -> Option<Self> {
        let home = dirs::home_dir()?;
        let candidates = [
            home.join(".config/foot/foot.ini"),
            home.join(".local/state/omarchy/current/theme/foot.ini"),
        ];

        let mut config_path = None;
        for c in &candidates {
            if c.exists() {
                config_path = Some(c.clone());
                break;
            }
        }

        let path = config_path?;
        let content = fs::read_to_string(&path).ok()?;
        let mtime = fs::metadata(&path).ok().and_then(|m| m.modified().ok());

        let mut bg = None;
        let mut fg = None;
        let mut sel = None;
        let mut pal = [None; 16];
        let mut in_colors = false;

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') || trimmed.is_empty() {
                continue;
            }
            if trimmed.starts_with('[') && trimmed.ends_with(']') {
                in_colors = trimmed.eq_ignore_ascii_case("[colors]");
                continue;
            }
            if in_colors {
                if let Some((k, v)) = trimmed.split_once('=') {
                    let key = k.trim().to_lowercase();
                    let val = v.trim().trim_matches('"').trim_matches('\'').trim();
                    if key == "background" {
                        bg = Self::parse_hex(val);
                    } else if key == "foreground" {
                        fg = Self::parse_hex(val);
                    } else if key == "selection-background" || key == "selection_background" {
                        sel = Self::parse_hex(val);
                    } else if let Some(idx_s) = key.strip_prefix("regular") {
                        if let Ok(idx) = idx_s.parse::<usize>() {
                            if idx < 8 {
                                pal[idx] = Self::parse_hex(val);
                            }
                        }
                    } else if let Some(idx_s) = key.strip_prefix("bright") {
                        if let Ok(idx) = idx_s.parse::<usize>() {
                            if idx < 8 {
                                pal[idx + 8] = Self::parse_hex(val);
                            }
                        }
                    }
                }
            }
        }

        if bg.is_some() || pal.iter().any(|c| c.is_some()) {
            Some(Self::build_from_palette(
                "Foot Theme".to_string(),
                bg,
                fg,
                sel,
                pal,
                vec![path],
                mtime,
                "auto".to_string(),
            ))
        } else {
            None
        }
    }

    fn detect_from_terminal_osc() -> Option<Self> {
        // Query the live terminal colors via OSC 10 / OSC 11
        let opts = terminal_colorsaurus::QueryOptions::default();
        let palette = terminal_colorsaurus::color_palette(opts.clone()).ok()?;
        let (bg_r, bg_g, bg_b) = palette.background.scale_to_8bit();
        let (fg_r, fg_g, fg_b) = palette.foreground.scale_to_8bit();

        let mode = terminal_colorsaurus::theme_mode(opts)
            .unwrap_or(terminal_colorsaurus::ThemeMode::Dark);
        let is_dark = mode == terminal_colorsaurus::ThemeMode::Dark;

        let bg = Color::Rgb(bg_r, bg_g, bg_b);
        let fg = Color::Rgb(fg_r, fg_g, fg_b);

        let mut pal = [None; 16];
        pal[0] = Some(bg);
        pal[7] = Some(fg);

        let mut theme = Self::build_from_palette(
            "Terminal (Auto-Detected)".to_string(),
            Some(bg),
            Some(fg),
            None,
            pal,
            vec![],
            None,
            "auto".to_string(),
        );
        theme.is_dark = is_dark;
        Some(theme)
    }

    // =========================================================================
    // Palette Builder & Parser Helpers
    // =========================================================================

    fn find_case_insensitive_file(dir: &Path, target_name: &str) -> Option<PathBuf> {
        if !dir.is_dir() {
            return None;
        }
        let direct = dir.join(target_name);
        if direct.is_file() {
            return Some(direct);
        }
        let normalized = target_name.to_lowercase().replace(['-', '_', ' '], "");
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_file() {
                    if let Some(file_name) = p.file_name().and_then(|s| s.to_str()) {
                        let file_norm = file_name.to_lowercase().replace(['-', '_', ' '], "");
                        if file_norm == normalized {
                            return Some(p);
                        }
                    }
                }
            }
        }
        None
    }

    pub fn build_from_palette(
        name: String,
        bg_opt: Option<Color>,
        fg_opt: Option<Color>,
        sel_opt: Option<Color>,
        palette: [Option<Color>; 16],
        watched_paths: Vec<PathBuf>,
        last_modified: Option<SystemTime>,
        preferred_name: String,
    ) -> Self {
        let bg = bg_opt.unwrap_or_else(|| palette[0].unwrap_or(Color::Rgb(10, 15, 20)));
        let fg = fg_opt.unwrap_or_else(|| palette[7].or(palette[15]).unwrap_or(Color::Rgb(220, 220, 220)));
        let is_dark = Self::is_color_dark(bg);

        let red = palette[1].unwrap_or(Color::Rgb(237, 91, 90));
        let green = palette[2].unwrap_or(Color::Rgb(146, 165, 147));
        let yellow = palette[3].unwrap_or(Color::Rgb(233, 187, 79));
        let blue = palette[4].unwrap_or(Color::Rgb(125, 130, 217));
        let magenta = palette[5].unwrap_or(Color::Rgb(200, 157, 193));
        let cyan = palette[6].unwrap_or(Color::Rgb(163, 191, 209));

        let bright_red = palette[9].or(palette[1]).unwrap_or(Color::Rgb(250, 170, 169));
        let orange = palette[9].unwrap_or(Color::Rgb(235, 139, 84));
        let bright_green = palette[10].or(palette[2]).unwrap_or(Color::Rgb(196, 207, 196));
        let bright_yellow = palette[11].or(palette[3]).unwrap_or(Color::Rgb(247, 220, 156));
        let bright_blue = palette[12].or(palette[4]).unwrap_or(Color::Rgb(194, 196, 240));
        let bright_magenta = palette[13].or(palette[5]).unwrap_or(Color::Rgb(234, 215, 231));
        let bright_cyan = palette[14].or(palette[6]).unwrap_or(Color::Rgb(223, 234, 240));
        let bright_foreground = palette[15].unwrap_or(fg);

        let accent = green;

        let selection = sel_opt.unwrap_or_else(|| {
            if is_dark {
                if let Some(p8) = palette[8] {
                    if Self::color_contrast(bg, p8) >= 25.0 {
                        return p8;
                    }
                }
                Self::blend_colors(bg, fg, 0.22)
            } else {
                Self::adjust_lightness(bg, 0.88)
            }
        });

        // Ensure muted color has strong, clear contrast (>= 45.0) against background
        let muted = {
            let mut candidate = None;
            if let Some(p8) = palette[8] {
                if Self::color_contrast(bg, p8) >= 45.0 {
                    candidate = Some(p8);
                }
            }
            if candidate.is_none() {
                if let Some(p5) = palette[5] {
                    if Self::color_contrast(bg, p5) >= 45.0 && Self::color_contrast(bg, p5) <= 120.0 {
                        candidate = Some(p5);
                    }
                }
            }
            candidate.unwrap_or_else(|| Self::compute_contrast_muted(bg, fg, is_dark))
        };

        let lighter_background = if is_dark {
            Self::blend_colors(bg, fg, 0.12)
        } else {
            Self::blend_colors(bg, fg, 0.08)
        };

        let (dark_background, darker_background) = if is_dark {
            (
                Self::adjust_lightness(bg, 0.8),
                Self::adjust_lightness(bg, 0.6),
            )
        } else {
            (
                Self::adjust_lightness(bg, 0.96),
                Self::adjust_lightness(bg, 0.92),
            )
        };

        let dark_foreground = muted;
        let light_foreground = Self::blend_colors(fg, bg, 0.20);

        Self {
            name,
            preferred_name,
            is_dark,
            last_checked: SystemTime::now(),
            last_modified,
            watched_paths,

            accent,
            selection,
            muted,

            background: bg,
            dark_background,
            darker_background,
            lighter_background,

            foreground: fg,
            dark_foreground,
            light_foreground,
            bright_foreground,

            red,
            green,
            yellow,
            blue,
            magenta,
            cyan,
            orange,

            bright_red,
            bright_green,
            bright_yellow,
            bright_blue,
            bright_magenta,
            bright_cyan,
        }
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

        // Post-validation: ensure muted has sufficient contrast
        if Self::color_contrast(self.background, self.muted) < 35.0 {
            self.muted = Self::compute_contrast_muted(self.background, self.foreground, self.is_dark);
        }
    }

    pub fn color_contrast(c1: Color, c2: Color) -> f32 {
        let lum = |c: Color| match c {
            Color::Rgb(r, g, b) => 0.299 * (r as f32) + 0.587 * (g as f32) + 0.114 * (b as f32),
            _ => 128.0,
        };
        (lum(c1) - lum(c2)).abs()
    }

    pub fn compute_contrast_muted(bg: Color, fg: Color, is_dark: bool) -> Color {
        if is_dark {
            Self::blend_colors(bg, fg, 0.42)
        } else {
            Self::blend_colors(bg, fg, 0.52)
        }
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

    fn is_color_dark(color: Color) -> bool {
        match color {
            Color::Rgb(r, g, b) => {
                let lum = 0.299 * (r as f32) + 0.587 * (g as f32) + 0.114 * (b as f32);
                lum < 128.0
            }
            _ => true,
        }
    }

    fn adjust_lightness(color: Color, factor: f32) -> Color {
        match color {
            Color::Rgb(r, g, b) => {
                let r_new = ((r as f32) * factor).clamp(0.0, 255.0) as u8;
                let g_new = ((g as f32) * factor).clamp(0.0, 255.0) as u8;
                let b_new = ((b as f32) * factor).clamp(0.0, 255.0) as u8;
                Color::Rgb(r_new, g_new, b_new)
            }
            other => other,
        }
    }

    fn blend_colors(c1: Color, c2: Color, t: f32) -> Color {
        match (c1, c2) {
            (Color::Rgb(r1, g1, b1), Color::Rgb(r2, g2, b2)) => {
                let r = ((r1 as f32) * (1.0 - t) + (r2 as f32) * t).clamp(0.0, 255.0) as u8;
                let g = ((g1 as f32) * (1.0 - t) + (g2 as f32) * t).clamp(0.0, 255.0) as u8;
                let b = ((b1 as f32) * (1.0 - t) + (b2 as f32) * t).clamp(0.0, 255.0) as u8;
                Color::Rgb(r, g, b)
            }
            (c1, _) => c1,
        }
    }

    // =========================================================================
    // Built-in Presets
    // =========================================================================

    pub fn load_preset(name: &str) -> Option<Self> {
        let n = name.to_lowercase().replace(['-', '_', ' '], "");
        match n.as_str() {
            "omarchy" | "omarchydefault" => Some(Self::preset_omarchy_default()),
            "gotham" => Some(Self::preset_gotham()),
            "tokyonight" | "tokyo" => Some(Self::preset_tokyo_night()),
            "catppuccinmocha" | "catppuccin" | "mocha" => Some(Self::preset_catppuccin_mocha()),
            "catppuccinlatte" | "latte" => Some(Self::preset_catppuccin_latte()),
            "gruvbox" | "gruvboxdark" => Some(Self::preset_gruvbox_dark()),
            "nord" => Some(Self::preset_nord()),
            "dracula" => Some(Self::preset_dracula()),
            "rosepine" => Some(Self::preset_rose_pine()),
            "solarizeddark" | "solarized" => Some(Self::preset_solarized_dark()),
            "solarizedlight" => Some(Self::preset_solarized_light()),
            _ => None,
        }
    }

    pub fn preset_omarchy_default() -> Self {
        Self {
            name: "Omarchy Default".to_string(),
            preferred_name: "omarchy".to_string(),
            is_dark: true,
            last_checked: SystemTime::now(),
            last_modified: None,
            watched_paths: Vec::new(),

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

    pub fn preset_gotham() -> Self {
        Self {
            name: "Gotham".to_string(),
            preferred_name: "gotham".to_string(),
            is_dark: true,
            last_checked: SystemTime::now(),
            last_modified: None,
            watched_paths: Vec::new(),

            accent: Color::Rgb(38, 169, 139),           // #26a98b
            selection: Color::Rgb(36, 83, 97),          // #245361
            muted: Color::Rgb(78, 81, 101),             // #4e5165

            background: Color::Rgb(10, 15, 20),         // #0a0f14
            dark_background: Color::Rgb(8, 31, 45),     // #081f2d
            darker_background: Color::Rgb(5, 10, 14),   // #050a0e
            lighter_background: Color::Rgb(16, 21, 27), // #10151b

            foreground: Color::Rgb(152, 209, 206),      // #98d1ce
            dark_foreground: Color::Rgb(78, 81, 101),   // #4e5165
            light_foreground: Color::Rgb(136, 139, 165),// #888ba5
            bright_foreground: Color::Rgb(211, 235, 233),// #d3ebe9

            red: Color::Rgb(195, 48, 39),               // #c33027
            green: Color::Rgb(38, 169, 139),            // #26a98b
            yellow: Color::Rgb(237, 181, 75),           // #edb54b
            orange: Color::Rgb(210, 105, 57),           // #d26939
            blue: Color::Rgb(25, 84, 101),              // #195465
            magenta: Color::Rgb(78, 81, 101),           // #4e5165
            cyan: Color::Rgb(51, 133, 157),             // #33859d

            bright_red: Color::Rgb(210, 105, 57),
            bright_green: Color::Rgb(38, 169, 139),
            bright_yellow: Color::Rgb(237, 181, 75),
            bright_blue: Color::Rgb(89, 156, 170),      // #599caa
            bright_magenta: Color::Rgb(136, 139, 165),
            bright_cyan: Color::Rgb(211, 235, 233),
        }
    }

    pub fn preset_tokyo_night() -> Self {
        Self {
            name: "Tokyo Night".to_string(),
            preferred_name: "tokyonight".to_string(),
            is_dark: true,
            last_checked: SystemTime::now(),
            last_modified: None,
            watched_paths: Vec::new(),

            accent: Color::Rgb(122, 162, 247),          // #7aa2f7
            selection: Color::Rgb(40, 52, 87),          // #283457
            muted: Color::Rgb(86, 95, 137),             // #565f89

            background: Color::Rgb(26, 27, 38),         // #1a1b26
            dark_background: Color::Rgb(22, 22, 30),    // #16161e
            darker_background: Color::Rgb(17, 17, 24),  // #111118
            lighter_background: Color::Rgb(36, 40, 59), // #24283b

            foreground: Color::Rgb(192, 202, 245),      // #c0caf5
            dark_foreground: Color::Rgb(86, 95, 137),   // #565f89
            light_foreground: Color::Rgb(169, 177, 214),// #a9b1d6
            bright_foreground: Color::Rgb(255, 255, 255),

            red: Color::Rgb(247, 118, 142),             // #f7768e
            green: Color::Rgb(158, 206, 106),           // #9ece6a
            yellow: Color::Rgb(224, 175, 104),          // #e0af68
            orange: Color::Rgb(255, 158, 100),          // #ff9e64
            blue: Color::Rgb(122, 162, 247),            // #7aa2f7
            magenta: Color::Rgb(187, 154, 247),         // #bb9af7
            cyan: Color::Rgb(125, 207, 255),            // #7dcfff

            bright_red: Color::Rgb(247, 118, 142),
            bright_green: Color::Rgb(158, 206, 106),
            bright_yellow: Color::Rgb(224, 175, 104),
            bright_blue: Color::Rgb(122, 162, 247),
            bright_magenta: Color::Rgb(187, 154, 247),
            bright_cyan: Color::Rgb(125, 207, 255),
        }
    }

    pub fn preset_catppuccin_mocha() -> Self {
        Self {
            name: "Catppuccin Mocha".to_string(),
            preferred_name: "catppuccin-mocha".to_string(),
            is_dark: true,
            last_checked: SystemTime::now(),
            last_modified: None,
            watched_paths: Vec::new(),

            accent: Color::Rgb(203, 166, 247),          // #cba6f7 (Mauve)
            selection: Color::Rgb(88, 91, 112),         // #585b70
            muted: Color::Rgb(108, 112, 134),           // #6c7086

            background: Color::Rgb(30, 30, 46),         // #1e1e2e
            dark_background: Color::Rgb(24, 24, 37),    // #181825
            darker_background: Color::Rgb(17, 17, 27),  // #11111b
            lighter_background: Color::Rgb(49, 50, 68), // #313244

            foreground: Color::Rgb(205, 214, 244),      // #cdd6f4
            dark_foreground: Color::Rgb(108, 112, 134), // #6c7086
            light_foreground: Color::Rgb(186, 194, 222),// #bac2de
            bright_foreground: Color::Rgb(255, 255, 255),

            red: Color::Rgb(243, 139, 168),             // #f38ba8
            green: Color::Rgb(166, 227, 161),           // #a6e3a1
            yellow: Color::Rgb(249, 226, 175),          // #f9e2af
            orange: Color::Rgb(250, 179, 135),          // #fab387
            blue: Color::Rgb(137, 180, 250),            // #89b4fa
            magenta: Color::Rgb(245, 194, 231),         // #f5c2e7
            cyan: Color::Rgb(148, 226, 213),            // #94e2d5

            bright_red: Color::Rgb(243, 119, 153),
            bright_green: Color::Rgb(137, 216, 139),
            bright_yellow: Color::Rgb(235, 211, 145),
            bright_blue: Color::Rgb(116, 168, 252),
            bright_magenta: Color::Rgb(242, 174, 222),
            bright_cyan: Color::Rgb(107, 215, 202),
        }
    }

    pub fn preset_catppuccin_latte() -> Self {
        Self {
            name: "Catppuccin Latte".to_string(),
            preferred_name: "catppuccin-latte".to_string(),
            is_dark: false,
            last_checked: SystemTime::now(),
            last_modified: None,
            watched_paths: Vec::new(),

            accent: Color::Rgb(136, 57, 239),           // #8839ef (Mauve)
            selection: Color::Rgb(172, 176, 190),       // #acb0be
            muted: Color::Rgb(156, 160, 176),           // #9ca0b0

            background: Color::Rgb(239, 241, 245),      // #eff1f5
            dark_background: Color::Rgb(230, 233, 239), // #e6e9ef
            darker_background: Color::Rgb(220, 224, 232),// #dce0e8
            lighter_background: Color::Rgb(255, 255, 255),

            foreground: Color::Rgb(76, 79, 105),        // #4c4f69
            dark_foreground: Color::Rgb(140, 143, 161), // #8c8fa1
            light_foreground: Color::Rgb(92, 95, 119),  // #5c5f77
            bright_foreground: Color::Rgb(32, 33, 44),

            red: Color::Rgb(210, 15, 57),               // #d20f39
            green: Color::Rgb(64, 160, 43),             // #40a02b
            yellow: Color::Rgb(223, 142, 29),           // #df8e1d
            orange: Color::Rgb(254, 100, 11),           // #fe640b
            blue: Color::Rgb(30, 102, 245),             // #1e66f5
            magenta: Color::Rgb(234, 118, 203),         // #ea76cb
            cyan: Color::Rgb(23, 146, 153),             // #179299

            bright_red: Color::Rgb(210, 15, 57),
            bright_green: Color::Rgb(64, 160, 43),
            bright_yellow: Color::Rgb(223, 142, 29),
            bright_blue: Color::Rgb(30, 102, 245),
            bright_magenta: Color::Rgb(234, 118, 203),
            bright_cyan: Color::Rgb(23, 146, 153),
        }
    }

    pub fn preset_gruvbox_dark() -> Self {
        Self {
            name: "Gruvbox Dark".to_string(),
            preferred_name: "gruvbox-dark".to_string(),
            is_dark: true,
            last_checked: SystemTime::now(),
            last_modified: None,
            watched_paths: Vec::new(),

            accent: Color::Rgb(184, 187, 38),           // #b8bb26 (Gruvbox Green)
            selection: Color::Rgb(80, 73, 69),          // #504945
            muted: Color::Rgb(146, 131, 116),           // #928374

            background: Color::Rgb(40, 40, 40),         // #282828
            dark_background: Color::Rgb(29, 32, 33),    // #1d2021
            darker_background: Color::Rgb(20, 22, 23),  // #141617
            lighter_background: Color::Rgb(60, 56, 54), // #3c3836

            foreground: Color::Rgb(235, 219, 178),      // #ebdbb2
            dark_foreground: Color::Rgb(146, 131, 116), // #928374
            light_foreground: Color::Rgb(213, 196, 161),// #d5c4a1
            bright_foreground: Color::Rgb(251, 241, 199),

            red: Color::Rgb(204, 36, 29),               // #cc241d
            green: Color::Rgb(152, 151, 26),            // #98971a
            yellow: Color::Rgb(215, 153, 33),           // #d79921
            orange: Color::Rgb(214, 93, 14),            // #d65d0e
            blue: Color::Rgb(69, 133, 136),             // #458588
            magenta: Color::Rgb(177, 98, 134),          // #b16286
            cyan: Color::Rgb(104, 157, 106),            // #689d6a

            bright_red: Color::Rgb(251, 73, 52),
            bright_green: Color::Rgb(184, 187, 38),
            bright_yellow: Color::Rgb(250, 189, 47),
            bright_blue: Color::Rgb(131, 165, 152),
            bright_magenta: Color::Rgb(211, 134, 155),
            bright_cyan: Color::Rgb(142, 192, 124),
        }
    }

    pub fn preset_nord() -> Self {
        Self {
            name: "Nord".to_string(),
            preferred_name: "nord".to_string(),
            is_dark: true,
            last_checked: SystemTime::now(),
            last_modified: None,
            watched_paths: Vec::new(),

            accent: Color::Rgb(136, 192, 208),          // #88c0d0 (Frost Cyan)
            selection: Color::Rgb(67, 76, 94),          // #434c5e
            muted: Color::Rgb(97, 110, 136),            // #616e88

            background: Color::Rgb(46, 52, 64),         // #2e3440
            dark_background: Color::Rgb(36, 41, 51),    // #242933
            darker_background: Color::Rgb(27, 31, 39),  // #1b1f27
            lighter_background: Color::Rgb(59, 66, 82), // #3b4252

            foreground: Color::Rgb(236, 239, 244),      // #eceff4
            dark_foreground: Color::Rgb(97, 110, 136),
            light_foreground: Color::Rgb(216, 222, 233),// #d8dee9
            bright_foreground: Color::Rgb(255, 255, 255),

            red: Color::Rgb(191, 97, 106),              // #bf616a
            green: Color::Rgb(163, 190, 140),           // #a3be8c
            yellow: Color::Rgb(235, 203, 139),          // #ebcb8b
            orange: Color::Rgb(208, 135, 112),          // #d08770
            blue: Color::Rgb(129, 161, 193),            // #81a1c1
            magenta: Color::Rgb(180, 142, 173),         // #b48ead
            cyan: Color::Rgb(136, 192, 208),            // #88c0d0

            bright_red: Color::Rgb(191, 97, 106),
            bright_green: Color::Rgb(163, 190, 140),
            bright_yellow: Color::Rgb(235, 203, 139),
            bright_blue: Color::Rgb(129, 161, 193),
            bright_magenta: Color::Rgb(180, 142, 173),
            bright_cyan: Color::Rgb(143, 188, 187),     // #8fbcbb
        }
    }

    pub fn preset_dracula() -> Self {
        Self {
            name: "Dracula".to_string(),
            preferred_name: "dracula".to_string(),
            is_dark: true,
            last_checked: SystemTime::now(),
            last_modified: None,
            watched_paths: Vec::new(),

            accent: Color::Rgb(189, 147, 249),          // #bd93f9 (Purple)
            selection: Color::Rgb(68, 71, 90),          // #44475a
            muted: Color::Rgb(98, 114, 164),            // #6272a4

            background: Color::Rgb(40, 42, 54),         // #282a36
            dark_background: Color::Rgb(33, 34, 44),    // #21222c
            darker_background: Color::Rgb(25, 26, 33),  // #191a21
            lighter_background: Color::Rgb(52, 55, 70), // #343746

            foreground: Color::Rgb(248, 248, 242),      // #f8f8f2
            dark_foreground: Color::Rgb(98, 114, 164),
            light_foreground: Color::Rgb(248, 248, 242),
            bright_foreground: Color::Rgb(255, 255, 255),

            red: Color::Rgb(255, 85, 85),               // #ff5555
            green: Color::Rgb(80, 250, 123),            // #50fa7b
            yellow: Color::Rgb(241, 250, 140),          // #f1fa8c
            orange: Color::Rgb(255, 184, 108),          // #ffb86c
            blue: Color::Rgb(139, 233, 253),            // #8be9fd
            magenta: Color::Rgb(255, 121, 198),         // #ff79c6
            cyan: Color::Rgb(139, 233, 253),            // #8be9fd

            bright_red: Color::Rgb(255, 110, 110),
            bright_green: Color::Rgb(105, 255, 148),
            bright_yellow: Color::Rgb(255, 255, 165),
            bright_blue: Color::Rgb(189, 147, 249),
            bright_magenta: Color::Rgb(255, 146, 213),
            bright_cyan: Color::Rgb(164, 255, 255),
        }
    }

    pub fn preset_rose_pine() -> Self {
        Self {
            name: "Rosé Pine".to_string(),
            preferred_name: "rose-pine".to_string(),
            is_dark: true,
            last_checked: SystemTime::now(),
            last_modified: None,
            watched_paths: Vec::new(),

            accent: Color::Rgb(235, 188, 186),          // #ebbcba (Rose)
            selection: Color::Rgb(64, 61, 82),          // #403d52
            muted: Color::Rgb(110, 106, 134),           // #6e6a86

            background: Color::Rgb(25, 23, 36),         // #191724
            dark_background: Color::Rgb(20, 18, 29),    // #14121d
            darker_background: Color::Rgb(15, 14, 22),  // #0f0e16
            lighter_background: Color::Rgb(38, 35, 53), // #262335

            foreground: Color::Rgb(224, 222, 244),      // #e0def4
            dark_foreground: Color::Rgb(110, 106, 134),
            light_foreground: Color::Rgb(144, 140, 170),// #908caa
            bright_foreground: Color::Rgb(255, 255, 255),

            red: Color::Rgb(235, 111, 146),             // #eb6f92
            green: Color::Rgb(49, 116, 143),            // #31748f
            yellow: Color::Rgb(246, 193, 119),          // #f6c177
            orange: Color::Rgb(235, 188, 186),          // #ebbcba
            blue: Color::Rgb(156, 207, 216),            // #9ccfd8
            magenta: Color::Rgb(196, 167, 231),         // #c4a7e7
            cyan: Color::Rgb(156, 207, 216),            // #9ccfd8

            bright_red: Color::Rgb(235, 111, 146),
            bright_green: Color::Rgb(49, 116, 143),
            bright_yellow: Color::Rgb(246, 193, 119),
            bright_blue: Color::Rgb(156, 207, 216),
            bright_magenta: Color::Rgb(196, 167, 231),
            bright_cyan: Color::Rgb(156, 207, 216),
        }
    }

    pub fn preset_solarized_dark() -> Self {
        Self {
            name: "Solarized Dark".to_string(),
            preferred_name: "solarized-dark".to_string(),
            is_dark: true,
            last_checked: SystemTime::now(),
            last_modified: None,
            watched_paths: Vec::new(),

            accent: Color::Rgb(38, 139, 210),           // #268bd2 (Blue)
            selection: Color::Rgb(7, 54, 66),           // #073642
            muted: Color::Rgb(88, 110, 117),            // #586e75

            background: Color::Rgb(0, 43, 54),          // #002b36
            dark_background: Color::Rgb(0, 33, 41),     // #002129
            darker_background: Color::Rgb(0, 24, 30),   // #00181e
            lighter_background: Color::Rgb(7, 54, 66),  // #073642

            foreground: Color::Rgb(131, 148, 150),      // #839496
            dark_foreground: Color::Rgb(88, 110, 117),  // #586e75
            light_foreground: Color::Rgb(147, 161, 161),// #93a1a1
            bright_foreground: Color::Rgb(253, 246, 227),

            red: Color::Rgb(220, 50, 47),               // #dc322f
            green: Color::Rgb(133, 153, 0),             // #859900
            yellow: Color::Rgb(181, 137, 0),            // #b58900
            orange: Color::Rgb(203, 75, 22),            // #cb4b16
            blue: Color::Rgb(38, 139, 210),             // #268bd2
            magenta: Color::Rgb(211, 54, 130),          // #d33682
            cyan: Color::Rgb(42, 161, 152),             // #2aa198

            bright_red: Color::Rgb(203, 75, 22),
            bright_green: Color::Rgb(88, 110, 117),
            bright_yellow: Color::Rgb(101, 123, 131),
            bright_blue: Color::Rgb(131, 148, 150),
            bright_magenta: Color::Rgb(108, 113, 196),  // #6c71c4
            bright_cyan: Color::Rgb(147, 161, 161),
        }
    }

    pub fn preset_solarized_light() -> Self {
        Self {
            name: "Solarized Light".to_string(),
            preferred_name: "solarized-light".to_string(),
            is_dark: false,
            last_checked: SystemTime::now(),
            last_modified: None,
            watched_paths: Vec::new(),

            accent: Color::Rgb(38, 139, 210),           // #268bd2 (Blue)
            selection: Color::Rgb(238, 232, 213),       // #eee8d5
            muted: Color::Rgb(147, 161, 161),           // #93a1a1

            background: Color::Rgb(253, 246, 227),      // #fdf6e3
            dark_background: Color::Rgb(242, 235, 215), // #f2ebd7
            darker_background: Color::Rgb(230, 223, 203),
            lighter_background: Color::Rgb(255, 255, 255),

            foreground: Color::Rgb(101, 123, 131),      // #657b83
            dark_foreground: Color::Rgb(147, 161, 161),
            light_foreground: Color::Rgb(88, 110, 117), // #586e75
            bright_foreground: Color::Rgb(7, 54, 66),   // #073642

            red: Color::Rgb(220, 50, 47),
            green: Color::Rgb(133, 153, 0),
            yellow: Color::Rgb(181, 137, 0),
            orange: Color::Rgb(203, 75, 22),
            blue: Color::Rgb(38, 139, 210),
            magenta: Color::Rgb(211, 54, 130),
            cyan: Color::Rgb(42, 161, 152),

            bright_red: Color::Rgb(203, 75, 22),
            bright_green: Color::Rgb(88, 110, 117),
            bright_yellow: Color::Rgb(101, 123, 131),
            bright_blue: Color::Rgb(131, 148, 150),
            bright_magenta: Color::Rgb(108, 113, 196),
            bright_cyan: Color::Rgb(147, 161, 161),
        }
    }

    // =========================================================================
    // UI Styling Helpers
    // =========================================================================

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
            .fg(self.background)
            .add_modifier(Modifier::BOLD)
    }

    pub fn unfocused_title_style(&self) -> Style {
        Style::default().fg(self.muted)
    }

    pub fn card_style(&self) -> Style {
        Style::default().bg(self.selection).fg(self.foreground)
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_presets() {
        let gotham = OmarchyTheme::load_preset("gotham").expect("gotham preset should exist");
        assert_eq!(gotham.name, "Gotham");
        assert_eq!(gotham.background, Color::Rgb(10, 15, 20));
        assert_eq!(gotham.foreground, Color::Rgb(152, 209, 206));
        assert!(gotham.is_dark);
        assert!(
            OmarchyTheme::color_contrast(gotham.background, gotham.muted) >= 45.0,
            "Muted contrast too low: {}",
            OmarchyTheme::color_contrast(gotham.background, gotham.muted)
        );

        let tokyo = OmarchyTheme::load_preset("tokyonight").expect("tokyonight preset should exist");
        assert_eq!(tokyo.name, "Tokyo Night");
        assert_eq!(tokyo.background, Color::Rgb(26, 27, 38));
        assert!(OmarchyTheme::color_contrast(tokyo.background, tokyo.muted) >= 45.0);

        let latte = OmarchyTheme::load_preset("catppuccin-latte").expect("catppuccin-latte should exist");
        assert!(!latte.is_dark);
        assert!(OmarchyTheme::color_contrast(latte.background, latte.muted) >= 45.0);
    }

    #[test]
    fn test_next_preset_cycle() {
        let start = "auto";
        let next = OmarchyTheme::next_preset(start);
        assert_eq!(next, "gotham");

        let last = PRESET_THEMES.last().unwrap();
        let loop_back = OmarchyTheme::next_preset(last);
        assert_eq!(loop_back, "auto");
    }

    #[test]
    fn test_parse_hex() {
        assert_eq!(OmarchyTheme::parse_hex("#ffffff"), Some(Color::Rgb(255, 255, 255)));
        assert_eq!(OmarchyTheme::parse_hex("0a0f14"), Some(Color::Rgb(10, 15, 20)));
        assert_eq!(OmarchyTheme::parse_hex("invalid"), None);
    }

    #[test]
    fn test_auto_detection() {
        let theme = OmarchyTheme::load_with_preference(Some("auto"));
        assert!(!theme.name.is_empty());
        assert!(
            OmarchyTheme::color_contrast(theme.background, theme.muted) >= 45.0,
            "Auto-detected theme muted contrast too low: {}",
            OmarchyTheme::color_contrast(theme.background, theme.muted)
        );
        if theme.name.starts_with("Ghostty") {
            assert!(matches!(theme.background, Color::Rgb(_, _, _)));
            assert!(matches!(theme.foreground, Color::Rgb(_, _, _)));
        }
    }
}
