use crate::ui::theme::OmarchyTheme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, Widget};

pub struct HeaderComponent<'a> {
    pub view_title: &'a str,
    pub total_tracks: usize,
    pub search_query: &'a str,
    pub is_searching: bool,
    pub theme: &'a OmarchyTheme,
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
            .border_style(self.theme.block_style())
            .style(Style::default().bg(self.theme.dark_background));

        block.render(area, buf);

        // Fill background
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                buf[(x, y)].set_bg(self.theme.dark_background);
            }
        }

        let y = area.top();

        // Logo
        let logo = " ♬ OmaFLAC";
        buf.set_string(
            area.left(),
            y,
            logo,
            Style::default().fg(self.theme.accent).add_modifier(Modifier::BOLD),
        );

        let x = area.left() + logo.len() as u16 + 2;

        // View breadcrumb
        let breadcrumb = format!("│  {}", self.view_title);
        buf.set_string(
            x,
            y,
            &breadcrumb,
            Style::default().fg(self.theme.light_foreground),
        );

        // Right side: Help button & Search bar
        let help_text = "[? Help] ";
        let help_x = area.right().saturating_sub(help_text.len() as u16);
        let help_rect = Rect::new(help_x, y, help_text.len() as u16, 1);
        buf.set_string(
            help_x,
            y,
            help_text,
            Style::default().fg(self.theme.cyan).add_modifier(Modifier::BOLD),
        );

        // Search box
        let search_box_width = 24u16;
        let search_x = help_x.saturating_sub(search_box_width + 2);
        let search_rect = Rect::new(search_x, y, search_box_width, 1);

        if search_x > x + breadcrumb.len() as u16 + 2 {
            let (search_str, search_style) = if self.is_searching {
                (
                    format!(" ⌕ {}█", self.search_query),
                    Style::default().bg(self.theme.lighter_background).fg(self.theme.accent).add_modifier(Modifier::BOLD),
                )
            } else if !self.search_query.is_empty() {
                (
                    format!(" ⌕ {} [/]", self.search_query),
                    Style::default().bg(self.theme.lighter_background).fg(self.theme.foreground),
                )
            } else {
                (
                    " ⌕ Search [/]...".to_string(),
                    Style::default().bg(self.theme.lighter_background).fg(self.theme.muted),
                )
            };

            for col in 0..search_box_width {
                buf[(search_x + col, y)].set_bg(self.theme.lighter_background);
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
