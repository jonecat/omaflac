use crate::library::playlist::PlaylistsManager;
use crate::library::track::Track;
use crate::ui::theme::OmarchyTheme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Widget};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlaylistDialogMode {
    SelectPlaylist { selected_idx: usize },
    CreateNew { name_input: String },
    RenamePlaylist { playlist_id: String, name_input: String },
    ConfirmDelete { playlist_id: String, playlist_name: String },
}

pub struct PlaylistDialog<'a> {
    pub mode: &'a PlaylistDialogMode,
    pub track: Option<&'a Track>,
    pub playlists_manager: &'a PlaylistsManager,
    pub theme: &'a OmarchyTheme,
}

pub struct PlaylistDialogClickTarget {
    pub rect: Rect,
    pub index: Option<usize>,
    pub is_confirm_button: bool,
    pub is_cancel_button: bool,
}

impl<'a> PlaylistDialog<'a> {
    pub fn render_with_targets(
        &self,
        area: Rect,
        buf: &mut Buffer,
    ) -> Vec<PlaylistDialogClickTarget> {
        let mut targets = Vec::new();

        let (modal_width, modal_height, title) = match self.mode {
            PlaylistDialogMode::SelectPlaylist { .. } => (56u16, 17u16, " Add to Playlist "),
            PlaylistDialogMode::CreateNew { .. } => (52u16, 11u16, " Create New Playlist "),
            PlaylistDialogMode::RenamePlaylist { .. } => (52u16, 11u16, " ✎ Alter Playlist Name "),
            PlaylistDialogMode::ConfirmDelete { .. } => (56u16, 11u16, " ✕ Delete Playlist "),
        };

        let width = modal_width.min(area.width.saturating_sub(4));
        let height = modal_height.min(area.height.saturating_sub(4));

        let x = area.left() + (area.width.saturating_sub(width)) / 2;
        let y = area.top() + (area.height.saturating_sub(height)) / 2;
        let modal_rect = Rect::new(x, y, width, height);

        Clear.render(modal_rect, buf);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(self.theme.block_focus_style())
            .style(Style::default().bg(self.theme.lighter_background))
            .title(title)
            .title_style(self.theme.accent_text());

        let inner = block.inner(modal_rect);
        block.render(modal_rect, buf);

        // Fill background
        for my in inner.top()..inner.bottom() {
            for mx in inner.left()..inner.right() {
                buf[(mx, my)].set_bg(self.theme.lighter_background);
            }
        }

        match self.mode {
            PlaylistDialogMode::SelectPlaylist { selected_idx } => {
                let mut line_y = inner.top();
                if let Some(track) = self.track {
                    let track_label = format!("Track: {} — {}", track.display_title(), track.display_artist());
                    let trunc_track: String = track_label.chars().take(inner.width as usize).collect();
                    buf.set_string(inner.left(), line_y, &trunc_track, self.theme.primary_text().add_modifier(Modifier::BOLD));
                    line_y += 2;
                }

                // [New Playlist] option at index 0
                let is_new_sel = *selected_idx == 0;
                let new_style = if is_new_sel {
                    self.theme.selected_row()
                } else {
                    Style::default().fg(self.theme.cyan).add_modifier(Modifier::BOLD)
                };
                let new_rect = Rect::new(inner.left(), line_y, inner.width, 1);
                buf.set_string(inner.left(), line_y, " + [Create New Playlist...]", new_style);
                targets.push(PlaylistDialogClickTarget {
                    rect: new_rect,
                    index: Some(0),
                    is_confirm_button: true,
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
                        self.theme.selected_row()
                    } else {
                        self.theme.primary_text()
                    };
                    let rect = Rect::new(inner.left(), line_y, inner.width, 1);
                    let label = format!(" ≡ {} ({} tracks)", p.name, p.track_ids.len());
                    let trunc: String = label.chars().take(inner.width as usize).collect();
                    buf.set_string(inner.left(), line_y, &trunc, style);
                    targets.push(PlaylistDialogClickTarget {
                        rect,
                        index: Some(item_idx),
                        is_confirm_button: false,
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
                    self.theme.muted_text(),
                );
            }
            PlaylistDialogMode::CreateNew { name_input } => {
                let line_y = inner.top() + 1;
                buf.set_string(inner.left(), line_y, "Playlist Name:", self.theme.primary_text());

                let input_y = line_y + 2;
                let input_rect = Rect::new(inner.left(), input_y, inner.width, 1);
                for mx in input_rect.left()..input_rect.right() {
                    buf[(mx, input_y)].set_bg(self.theme.dark_background);
                }
                let input_str = format!(" {}█", name_input);
                buf.set_string(inner.left(), input_y, &input_str, self.theme.accent_text());

                let btn_y = input_y + 2;
                let ok_rect = Rect::new(inner.left() + 2, btn_y, 12, 1);
                buf.set_string(ok_rect.left(), btn_y, "[ Create ]", self.theme.accent_text());
                targets.push(PlaylistDialogClickTarget {
                    rect: ok_rect,
                    index: None,
                    is_confirm_button: true,
                    is_cancel_button: false,
                });

                let cancel_rect = Rect::new(inner.left() + 18, btn_y, 12, 1);
                buf.set_string(cancel_rect.left(), btn_y, "[ Cancel ]", self.theme.secondary_text());
                targets.push(PlaylistDialogClickTarget {
                    rect: cancel_rect,
                    index: None,
                    is_confirm_button: false,
                    is_cancel_button: true,
                });
            }
            PlaylistDialogMode::RenamePlaylist { playlist_id: _, name_input } => {
                let line_y = inner.top() + 1;
                buf.set_string(inner.left(), line_y, "Enter New Playlist Name:", self.theme.primary_text().add_modifier(Modifier::BOLD));

                let input_y = line_y + 2;
                let input_rect = Rect::new(inner.left(), input_y, inner.width, 1);
                for mx in input_rect.left()..input_rect.right() {
                    buf[(mx, input_y)].set_bg(self.theme.dark_background);
                }
                let input_str = format!(" {}█", name_input);
                buf.set_string(inner.left(), input_y, &input_str, self.theme.accent_text());

                let btn_y = input_y + 2;
                let ok_rect = Rect::new(inner.left() + 2, btn_y, 12, 1);
                buf.set_string(ok_rect.left(), btn_y, "[ Save ]", self.theme.accent_text());
                targets.push(PlaylistDialogClickTarget {
                    rect: ok_rect,
                    index: None,
                    is_confirm_button: true,
                    is_cancel_button: false,
                });

                let cancel_rect = Rect::new(inner.left() + 18, btn_y, 12, 1);
                buf.set_string(cancel_rect.left(), btn_y, "[ Cancel ]", self.theme.secondary_text());
                targets.push(PlaylistDialogClickTarget {
                    rect: cancel_rect,
                    index: None,
                    is_confirm_button: false,
                    is_cancel_button: true,
                });
            }
            PlaylistDialogMode::ConfirmDelete { playlist_id: _, playlist_name } => {
                let line_y = inner.top() + 1;
                let prompt = format!("Delete playlist \"{}\"?", playlist_name);
                let trunc_prompt: String = prompt.chars().take(inner.width as usize).collect();
                buf.set_string(inner.left(), line_y, &trunc_prompt, self.theme.accent_text().add_modifier(Modifier::BOLD));

                let warn_y = line_y + 2;
                let warn_text = "This will remove the playlist. Audio files will not be deleted.";
                let trunc_warn: String = warn_text.chars().take(inner.width as usize).collect();
                buf.set_string(inner.left(), warn_y, &trunc_warn, self.theme.muted_text());

                let btn_y = warn_y + 2;
                let del_rect = Rect::new(inner.left() + 2, btn_y, 20, 1);
                buf.set_string(del_rect.left(), btn_y, "[ Delete Playlist ]", Style::default().fg(self.theme.accent).add_modifier(Modifier::BOLD));
                targets.push(PlaylistDialogClickTarget {
                    rect: del_rect,
                    index: None,
                    is_confirm_button: true,
                    is_cancel_button: false,
                });

                let cancel_rect = Rect::new(inner.left() + 26, btn_y, 12, 1);
                buf.set_string(cancel_rect.left(), btn_y, "[ Cancel ]", self.theme.secondary_text());
                targets.push(PlaylistDialogClickTarget {
                    rect: cancel_rect,
                    index: None,
                    is_confirm_button: false,
                    is_cancel_button: true,
                });
            }
        }

        targets
    }
}
