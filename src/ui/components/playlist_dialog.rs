use crate::library::playlist::PlaylistsManager;
use crate::library::track::Track;
use crate::ui::theme::Theme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, Clear, Widget};

pub enum PlaylistDialogMode {
    SelectPlaylist { selected_idx: usize },
    CreateNew { name_input: String },
}

pub struct PlaylistDialog<'a> {
    pub mode: &'a PlaylistDialogMode,
    pub track: &'a Track,
    pub playlists_manager: &'a PlaylistsManager,
}

pub struct PlaylistDialogClickTarget {
    pub rect: Rect,
    pub index: Option<usize>,
    pub is_create_button: bool,
    pub is_cancel_button: bool,
}

impl<'a> PlaylistDialog<'a> {
    pub fn render_with_targets(
        &self,
        area: Rect,
        buf: &mut Buffer,
    ) -> Vec<PlaylistDialogClickTarget> {
        let mut targets = Vec::new();

        // Center modal
        let modal_width = 54u16.min(area.width.saturating_sub(4));
        let modal_height = 16u16.min(area.height.saturating_sub(4));

        let x = area.left() + (area.width.saturating_sub(modal_width)) / 2;
        let y = area.top() + (area.height.saturating_sub(modal_height)) / 2;
        let modal_rect = Rect::new(x, y, modal_width, modal_height);

        Clear.render(modal_rect, buf);

        let title = match self.mode {
            PlaylistDialogMode::SelectPlaylist { .. } => " Add to Playlist ",
            PlaylistDialogMode::CreateNew { .. } => " Create New Playlist ",
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Theme::block_focus_style())
            .title(title)
            .title_style(Style::default().fg(Theme::OMARCHY_GREEN).add_modifier(Modifier::BOLD));

        let inner = block.inner(modal_rect);
        block.render(modal_rect, buf);

        // Fill background
        for my in inner.top()..inner.bottom() {
            for mx in inner.left()..inner.right() {
                buf[(mx, my)].set_bg(Theme::BG_CARD);
            }
        }

        match self.mode {
            PlaylistDialogMode::SelectPlaylist { selected_idx } => {
                let mut line_y = inner.top();
                let track_label = format!("Track: {} — {}", self.track.display_title(), self.track.display_artist());
                let trunc_track: String = track_label.chars().take(inner.width as usize).collect();
                buf.set_string(inner.left(), line_y, &trunc_track, Style::default().fg(Theme::TEXT_PRIMARY).add_modifier(Modifier::BOLD));
                line_y += 2;

                // [New Playlist] option at index 0
                let is_new_sel = *selected_idx == 0;
                let new_style = if is_new_sel {
                    Theme::selected_row()
                } else {
                    Style::default().fg(Theme::ACCENT_CYAN)
                };
                let new_rect = Rect::new(inner.left(), line_y, inner.width, 1);
                buf.set_string(inner.left(), line_y, " ➕ [Create New Playlist...]", new_style);
                targets.push(PlaylistDialogClickTarget {
                    rect: new_rect,
                    index: Some(0),
                    is_create_button: true,
                    is_cancel_button: false,
                });
                line_y += 1;

                // List existing playlists
                for (i, p) in self.playlists_manager.playlists.iter().enumerate() {
                    if line_y >= inner.bottom().saturating_sub(2) {
                        break;
                    }
                    let item_idx = i + 1;
                    let is_sel = *selected_idx == item_idx;
                    let style = if is_sel {
                        Theme::selected_row()
                    } else {
                        Style::default().fg(Theme::TEXT_PRIMARY)
                    };
                    let rect = Rect::new(inner.left(), line_y, inner.width, 1);
                    let label = format!(" 📑 {} ({} tracks)", p.name, p.track_ids.len());
                    let trunc: String = label.chars().take(inner.width as usize).collect();
                    buf.set_string(inner.left(), line_y, &trunc, style);
                    targets.push(PlaylistDialogClickTarget {
                        rect,
                        index: Some(item_idx),
                        is_create_button: false,
                        is_cancel_button: false,
                    });
                    line_y += 1;
                }

                // Bottom hints
                let hint_y = inner.bottom() - 1;
                buf.set_string(
                    inner.left(),
                    hint_y,
                    "[Enter] Select   [Esc] Cancel",
                    Style::default().fg(Theme::TEXT_MUTED),
                );
            }
            PlaylistDialogMode::CreateNew { name_input } => {
                let line_y = inner.top() + 1;
                buf.set_string(inner.left(), line_y, "Playlist Name:", Style::default().fg(Theme::TEXT_PRIMARY));

                let input_y = line_y + 2;
                let input_rect = Rect::new(inner.left(), input_y, inner.width, 1);
                for mx in input_rect.left()..input_rect.right() {
                    buf[(mx, input_y)].set_bg(Theme::BG_MAIN);
                }
                let input_str = format!(" {}█", name_input);
                buf.set_string(inner.left(), input_y, &input_str, Style::default().fg(Theme::OMARCHY_GREEN).add_modifier(Modifier::BOLD));

                let btn_y = input_y + 3;
                let ok_rect = Rect::new(inner.left() + 2, btn_y, 10, 1);
                buf.set_string(ok_rect.left(), btn_y, "[ Create ]", Style::default().fg(Theme::OMARCHY_GREEN).add_modifier(Modifier::BOLD));
                targets.push(PlaylistDialogClickTarget {
                    rect: ok_rect,
                    index: None,
                    is_create_button: true,
                    is_cancel_button: false,
                });

                let cancel_rect = Rect::new(inner.left() + 16, btn_y, 10, 1);
                buf.set_string(cancel_rect.left(), btn_y, "[ Cancel ]", Style::default().fg(Theme::TEXT_SECONDARY));
                targets.push(PlaylistDialogClickTarget {
                    rect: cancel_rect,
                    index: None,
                    is_create_button: false,
                    is_cancel_button: true,
                });
            }
        }

        targets
    }
}
