use crate::ui::theme::OmarchyTheme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, BorderType, Borders, Widget};

pub struct HeaderComponent<'a> {
    pub view_title: &'a str,
    pub total_tracks: usize,
    pub search_query: &'a str,
    pub is_searching: bool,
    pub update_badge: Option<&'a str>,
    pub theme: &'a OmarchyTheme,
}

pub struct HeaderClickTarget {
    pub logo_rect: Rect,
    pub search_rect: Rect,
    pub help_rect: Rect,
    pub update_rect: Rect,
}

impl<'a> HeaderComponent<'a> {
    pub fn render_with_targets(
        &self,
        area: Rect,
        buf: &mut Buffer,
    ) -> HeaderClickTarget {
        let border_style = if self.is_searching {
            self.theme.block_focus_style()
        } else {
            self.theme.block_style()
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(border_style)
            .style(Style::default().bg(self.theme.dark_background));

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.width == 0 || inner.height == 0 {
            return HeaderClickTarget {
                logo_rect: Rect::default(),
                search_rect: Rect::default(),
                help_rect: Rect::default(),
                update_rect: Rect::default(),
            };
        }

        let y = inner.top();

        // Logo (clickable to open About modal)
        let logo = " ♬ OmaFLAC";
        let logo_w = logo.chars().count() as u16;
        let logo_rect = Rect::new(inner.left(), area.top(), logo_w + 2, area.height);
        buf.set_string(
            inner.left(),
            y,
            logo,
            Style::default().fg(self.theme.accent).add_modifier(Modifier::BOLD),
        );

        let x = inner.left() + logo_w + 2;

        // View breadcrumb
        let breadcrumb = format!("│  {}", self.view_title);
        let breadcrumb_w = breadcrumb.chars().count() as u16;
        if x + breadcrumb_w <= inner.right() {
            buf.set_string(
                x,
                y,
                &breadcrumb,
                Style::default().fg(self.theme.light_foreground),
            );
        }

        // Right side: Help button & Search bar
        let help_text = "[? Help] ";
        let help_w = help_text.chars().count() as u16;
        let help_x = inner.right().saturating_sub(help_w);
        let help_rect = Rect::new(help_x, y, help_w, 1);
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

        if search_x > x + breadcrumb_w + 2 {
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

        // Optional Update Notification Badge to the left of the search box
        let mut update_rect = Rect::default();
        if let Some(badge_text) = self.update_badge {
            let badge_w = badge_text.chars().count() as u16;
            let badge_x = search_x.saturating_sub(badge_w + 2);
            if badge_x > x + breadcrumb_w + 2 {
                update_rect = Rect::new(badge_x, y, badge_w, 1);
                buf.set_string(
                    badge_x,
                    y,
                    badge_text,
                    Style::default()
                        .fg(self.theme.accent)
                        .add_modifier(Modifier::BOLD),
                );
            }
        }

        HeaderClickTarget {
            logo_rect,
            search_rect,
            help_rect,
            update_rect,
        }
    }
}
