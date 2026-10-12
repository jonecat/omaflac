use ratatui::style::{Color, Modifier, Style};

pub struct Theme;

impl Theme {
    // Omarchy signature colors
    pub const OMARCHY_GREEN: Color = Color::Rgb(158, 206, 106); // #9ec66a
    pub const ACCENT_CYAN: Color = Color::Rgb(125, 207, 255);   // #7dcfff
    pub const ACCENT_BLUE: Color = Color::Rgb(122, 162, 247);   // #7aa2f7
    pub const ACCENT_MAGENTA: Color = Color::Rgb(187, 154, 247); // #bb9af7
    pub const ACCENT_YELLOW: Color = Color::Rgb(224, 175, 104); // #e0af68
    pub const ACCENT_RED: Color = Color::Rgb(247, 118, 142);    // #f7768e

    pub const BG_MAIN: Color = Color::Rgb(26, 27, 38);
    pub const BG_SIDEBAR: Color = Color::Rgb(22, 22, 30);
    pub const BG_CARD: Color = Color::Rgb(36, 40, 59);
    pub const BG_HIGHLIGHT: Color = Color::Rgb(47, 53, 77);
    pub const BORDER: Color = Color::Rgb(65, 72, 104);
    pub const BORDER_FOCUS: Color = Self::OMARCHY_GREEN;

    pub const TEXT_PRIMARY: Color = Color::Rgb(192, 202, 245);
    pub const TEXT_SECONDARY: Color = Color::Rgb(120, 130, 160);
    pub const TEXT_MUTED: Color = Color::Rgb(86, 95, 137);

    pub fn primary_text() -> Style {
        Style::default().fg(Self::TEXT_PRIMARY)
    }

    pub fn secondary_text() -> Style {
        Style::default().fg(Self::TEXT_SECONDARY)
    }

    pub fn muted_text() -> Style {
        Style::default().fg(Self::TEXT_MUTED)
    }

    pub fn accent_text() -> Style {
        Style::default().fg(Self::OMARCHY_GREEN).add_modifier(Modifier::BOLD)
    }

    pub fn block_style() -> Style {
        Style::default().fg(Self::BORDER)
    }

    pub fn block_focus_style() -> Style {
        Style::default().fg(Self::BORDER_FOCUS)
    }

    pub fn selected_row() -> Style {
        Style::default()
            .bg(Self::BG_HIGHLIGHT)
            .fg(Self::OMARCHY_GREEN)
            .add_modifier(Modifier::BOLD)
    }

    pub fn active_tab() -> Style {
        Style::default()
            .fg(Self::OMARCHY_GREEN)
            .add_modifier(Modifier::BOLD)
    }

    pub fn inactive_tab() -> Style {
        Style::default().fg(Self::TEXT_SECONDARY)
    }
}
