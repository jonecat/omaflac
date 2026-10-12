use crate::ui::theme::Theme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, Widget};

pub struct HeaderComponent<'a> {
    pub view_title: &'a str,
    pub total_tracks: usize,
    pub search_query: &'a str,
    pub is_searching: bool,
}

pub struct HeaderClickTarget {
    pub search_rect: Rect,
    pub help_rect: Rect,
}

impl<'a> HeaderComponent<'a> {
    pub fn render_with_targets(
        &self,
        area: Rect,
        buf: &mut Buffer,
    ) -> HeaderClickTarget {
        let block = Block::default()
            .borders(Borders::BOTTOM)
            .border_style(Theme::block_style());

        block.render(area, buf);

        let y = area.top();

        // Logo
        let logo = " ♬ omaflac";
        buf.set_string(
            area.left(),
            y,
            logo,
            Style::default().fg(Theme::OMARCHY_GREEN).add_modifier(Modifier::BOLD),
        );

        let x = area.left() + logo.len() as u16 + 2;

        // View breadcrumb
        let breadcrumb = format!("│  {}", self.view_title);
        buf.set_string(
            x,
            y,
            &breadcrumb,
            Style::default().fg(Theme::TEXT_SECONDARY),
        );

        // Right side: Help button & Search bar
        let help_text = "[? Help] ";
        let help_x = area.right().saturating_sub(help_text.len() as u16);
        let help_rect = Rect::new(help_x, y, help_text.len() as u16, 1);
        buf.set_string(
            help_x,
            y,
            help_text,
            Style::default().fg(Theme::ACCENT_CYAN),
        );

        // Search box
        let search_box_width = 24u16;
        let search_x = help_x.saturating_sub(search_box_width + 2);
        let search_rect = Rect::new(search_x, y, search_box_width, 1);

        if search_x > x + breadcrumb.len() as u16 + 2 {
            let (search_str, search_style) = if self.is_searching {
                (
                    format!(" 🔍 {}█", self.search_query),
                    Style::default().bg(Theme::BG_CARD).fg(Theme::OMARCHY_GREEN).add_modifier(Modifier::BOLD),
                )
            } else if !self.search_query.is_empty() {
                (
                    format!(" 🔍 {} [/]", self.search_query),
                    Style::default().bg(Theme::BG_CARD).fg(Theme::TEXT_PRIMARY),
                )
            } else {
                (
                    " 🔍 Search [/]...".to_string(),
                    Style::default().bg(Theme::BG_CARD).fg(Theme::TEXT_MUTED),
                )
            };

            for col in 0..search_box_width {
                buf[(search_x + col, y)].set_bg(Theme::BG_CARD);
            }
            let truncated: String = search_str.chars().take(search_box_width as usize).collect();
            buf.set_string(search_x, y, &truncated, search_style);
        }

        HeaderClickTarget {
            search_rect,
            help_rect,
        }
    }
}
