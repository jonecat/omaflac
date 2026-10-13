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
    pub scan_badge: Option<&'a str>,
    pub update_badge: Option<&'a str>,
    pub theme: &'a OmarchyTheme,
}

pub struct HeaderClickTarget {
    pub logo_rect: Rect,
    pub search_rect: Rect,
    pub help_rect: Rect,
    pub scan_rect: Rect,
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
            .style(Style::default().bg(self.theme.background));

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.width == 0 || inner.height == 0 {
            return HeaderClickTarget {
                logo_rect: Rect::default(),
                search_rect: Rect::default(),
                help_rect: Rect::default(),
                scan_rect: Rect::default(),
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
        let x_after_breadcrumb = x + breadcrumb_w + 2;

        // 1. Help Button (Rightmost element: "[ ? Help ]")
        let help_w = 10u16;
        let help_x = inner.right().saturating_sub(help_w);
        let mut help_rect = Rect::default();

        if help_x > x_after_breadcrumb {
            help_rect = Rect::new(help_x, y, help_w, 1);
            for col in 0..help_w {
                buf[(help_x + col, y)].set_bg(self.theme.dark_background);
            }
            let help_cap_style = Style::default()
                .fg(self.theme.muted)
                .bg(self.theme.dark_background);
            buf.set_string(help_x, y, "[", help_cap_style);
            buf.set_string(help_x + help_w - 1, y, "]", help_cap_style);

            buf.set_string(
                help_x + 2,
                y,
                "?",
                Style::default()
                    .fg(self.theme.cyan)
                    .bg(self.theme.dark_background)
                    .add_modifier(Modifier::BOLD),
            );
            buf.set_string(
                help_x + 4,
                y,
                "Help",
                Style::default()
                    .fg(self.theme.foreground)
                    .bg(self.theme.dark_background),
            );
        }

        // 2. Search Field (To the left of Help button)
        let max_search_avail = help_x.saturating_sub(x_after_breadcrumb + 2);
        let ideal_search_w = if inner.width >= 90 {
            28u16
        } else if inner.width >= 75 {
            24u16
        } else {
            20u16
        };
        let search_box_width = ideal_search_w.min(max_search_avail);
        let mut search_rect = Rect::default();

        let search_x = help_x.saturating_sub(search_box_width + 2);

        if search_box_width >= 16 && search_x > x_after_breadcrumb {
            search_rect = Rect::new(search_x, y, search_box_width, 1);

            let search_bg = if self.is_searching {
                self.theme.selection
            } else {
                self.theme.dark_background
            };

            for col in 0..search_box_width {
                buf[(search_x + col, y)].set_bg(search_bg);
            }

            let search_cap_style = if self.is_searching {
                self.theme.block_focus_style().bg(search_bg)
            } else if !self.search_query.is_empty() {
                Style::default().fg(self.theme.cyan).bg(search_bg)
            } else {
                self.theme.block_style().bg(search_bg)
            };

            buf.set_string(search_x, y, "[", search_cap_style);
            buf.set_string(search_x + search_box_width - 1, y, "]", search_cap_style);

            // Right key hint: "Esc" when searching, "/" when idle
            let (hint_text, hint_style) = if self.is_searching {
                (
                    "Esc",
                    Style::default()
                        .fg(self.theme.accent)
                        .bg(search_bg)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                (
                    "/",
                    Style::default()
                        .fg(self.theme.muted)
                        .bg(search_bg),
                )
            };

            let hint_len = hint_text.chars().count() as u16;
            let hint_x = (search_x + search_box_width - 1).saturating_sub(hint_len + 1);
            buf.set_string(hint_x, y, hint_text, hint_style);

            // Left icon
            let icon_style = Style::default()
                .fg(self.theme.accent)
                .bg(search_bg)
                .add_modifier(Modifier::BOLD);
            buf.set_string(search_x + 2, y, "⌕", icon_style);

            // Text between icon and hint
            let text_start_x = search_x + 4;
            let text_end_x = hint_x.saturating_sub(1);
            let text_avail = (text_end_x.saturating_sub(text_start_x)) as usize;

            if text_avail > 0 {
                if self.is_searching {
                    let query = self.search_query;
                    let query_chars: Vec<char> = query.chars().collect();
                    let max_query_len = text_avail.saturating_sub(1); // reserve 1 for cursor
                    let (display_query, cursor_offset) = if query_chars.len() <= max_query_len {
                        (query.to_string(), query_chars.len() as u16)
                    } else {
                        let start = query_chars.len() - max_query_len;
                        let s: String = query_chars[start..].iter().collect();
                        let l = s.chars().count() as u16;
                        (s, l)
                    };

                    buf.set_string(
                        text_start_x,
                        y,
                        &display_query,
                        Style::default()
                            .fg(self.theme.bright_foreground)
                            .bg(search_bg)
                            .add_modifier(Modifier::BOLD),
                    );
                    buf.set_string(
                        text_start_x + cursor_offset,
                        y,
                        "█",
                        Style::default()
                            .fg(self.theme.accent)
                            .bg(search_bg),
                    );
                } else if !self.search_query.is_empty() {
                    let disp: String = self.search_query.chars().take(text_avail).collect();
                    buf.set_string(
                        text_start_x,
                        y,
                        &disp,
                        Style::default()
                            .fg(self.theme.foreground)
                            .bg(search_bg)
                            .add_modifier(Modifier::BOLD),
                    );
                } else {
                    let placeholder = "Search";
                    let disp: String = placeholder.chars().take(text_avail).collect();
                    buf.set_string(
                        text_start_x,
                        y,
                        &disp,
                        Style::default()
                            .fg(self.theme.muted)
                            .bg(search_bg),
                    );
                }
            }
        }

        // 3. Optional Update Notification Badge to the left of the search box
        let mut right_anchor = if search_rect.width > 0 { search_x } else { help_x };
        let mut update_rect = Rect::default();
        if let Some(badge_raw) = self.update_badge {
            let badge_text = badge_raw.trim_matches(|c| c == '[' || c == ']').trim();
            let badge_w = (badge_text.chars().count() as u16) + 4; // "[ " + text + " ]"
            let badge_x = right_anchor.saturating_sub(badge_w + 2);
            if badge_x > x_after_breadcrumb {
                update_rect = Rect::new(badge_x, y, badge_w, 1);
                for col in 0..badge_w {
                    buf[(badge_x + col, y)].set_bg(self.theme.dark_background);
                }
                let cap_style = self.theme.block_focus_style().bg(self.theme.dark_background);
                buf.set_string(badge_x, y, "[", cap_style);
                buf.set_string(badge_x + badge_w - 1, y, "]", cap_style);
                buf.set_string(
                    badge_x + 2,
                    y,
                    badge_text,
                    Style::default()
                        .fg(self.theme.accent)
                        .bg(self.theme.dark_background)
                        .add_modifier(Modifier::BOLD),
                );
                right_anchor = badge_x;
            }
        }

        // 4. Optional Background Library Scan/Sync Badge to the left of the update area
        let mut scan_rect = Rect::default();
        if let Some(scan_raw) = self.scan_badge {
            let scan_text = scan_raw.trim_matches(|c| c == '[' || c == ']').trim();
            let scan_w = (scan_text.chars().count() as u16) + 4;
            let scan_x = right_anchor.saturating_sub(scan_w + 2);
            if scan_x > x_after_breadcrumb {
                scan_rect = Rect::new(scan_x, y, scan_w, 1);
                for col in 0..scan_w {
                    buf[(scan_x + col, y)].set_bg(self.theme.dark_background);
                }
                let cap_style = self.theme.block_style().bg(self.theme.dark_background);
                buf.set_string(scan_x, y, "[", cap_style);
                buf.set_string(scan_x + scan_w - 1, y, "]", cap_style);
                buf.set_string(
                    scan_x + 2,
                    y,
                    scan_text,
                    Style::default()
                        .fg(self.theme.yellow)
                        .bg(self.theme.dark_background)
                        .add_modifier(Modifier::BOLD),
                );
            }
        }

        HeaderClickTarget {
            logo_rect,
            search_rect,
            help_rect,
            scan_rect,
            update_rect,
        }
    }
}
