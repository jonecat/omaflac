use crate::library::playlist::PlaylistsManager;
use crate::library::Library;
use crate::ui::theme::OmarchyTheme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, BorderType, Borders, Widget};

#[derive(Clone, Debug)]
pub struct PlaylistsViewClickTarget {
    pub rect: Rect,
    pub is_playlist_selector: bool,
    pub playlist_id: Option<String>,
    pub track_index: Option<usize>,
    pub is_rename_button: bool,
    pub is_delete_button: bool,
}

pub struct PlaylistsView<'a> {
    pub library: &'a Library,
    pub playlists_manager: &'a PlaylistsManager,
    pub selected_playlist_id: &'a str,
    pub selected_track_idx: usize,
    pub scroll_playlist_offset: usize,
    pub scroll_track_offset: usize,
    pub playing_track_id: Option<&'a str>,
    pub is_focused: bool,
    pub theme: &'a OmarchyTheme,
}

impl<'a> PlaylistsView<'a> {
    pub fn render_with_targets(
        &self,
        area: Rect,
        buf: &mut Buffer,
    ) -> Vec<PlaylistsViewClickTarget> {
        let mut targets = Vec::new();

        if area.width < 15 || area.height < 4 {
            return targets;
        }

        // Fill background
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                buf[(x, y)].set_bg(self.theme.background);
            }
        }

        // Left 28%: playlist selector, Right 72%: playlist tracks
        let list_w = (area.width as f32 * 0.28).max(18.0) as u16;
        let track_w = area.width.saturating_sub(list_w);

        let left_rect = Rect::new(area.left(), area.top(), list_w, area.height);
        let right_rect = Rect::new(area.left() + list_w, area.top(), track_w, area.height);

        // Left block
        let block_left = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(self.theme.block_style())
            .style(Style::default().bg(self.theme.background))
            .title(" Playlists ")
            .title_style(self.theme.unfocused_title_style());
        let inner_l = block_left.inner(left_rect);
        block_left.render(left_rect, buf);

        // Right block
        let playlist_name = if self.selected_playlist_id == "__favorites__" {
            "★ Favorites".to_string()
        } else if self.selected_playlist_id == "__recent__" {
            "◷ Recently Played".to_string()
        } else if let Some(p) = self.playlists_manager.get_playlist(self.selected_playlist_id) {
            format!("≡ {}", p.name)
        } else {
            "Playlist".to_string()
        };

        let title_hint = if self.selected_playlist_id != "__favorites__" && self.selected_playlist_id != "__recent__" {
            "  ([r] Rename • [D] Delete)"
        } else {
            ""
        };

        let (r_bstyle, r_tstyle, r_title) = if self.is_focused {
            (
                self.theme.block_focus_style(),
                self.theme.focused_title_style(),
                format!(" ● {}{} ", playlist_name, title_hint),
            )
        } else {
            (
                self.theme.block_style(),
                self.theme.unfocused_title_style(),
                format!(" {}{} ", playlist_name, title_hint),
            )
        };

        let block_right = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(r_bstyle)
            .style(Style::default().bg(self.theme.background))
            .title(r_title)
            .title_style(r_tstyle);
        let inner_r = block_right.inner(right_rect);
        block_right.render(right_rect, buf);

        // Render Left items
        let mut y_l = inner_l.top() + 1;

        // Favorites item
        let fav_rect = Rect::new(inner_l.left(), y_l, inner_l.width, 1);
        let is_fav_sel = self.selected_playlist_id == "__favorites__";
        let fav_style = if is_fav_sel { self.theme.selected_row() } else { self.theme.primary_text() };
        if is_fav_sel {
            for x in fav_rect.left()..fav_rect.right() { buf[(x, y_l)].set_bg(self.theme.selection); }
        }
        let fav_label = format!(" ★ Favorites ({})", self.playlists_manager.favorites.len());
        buf.set_string(inner_l.left(), y_l, &fav_label, fav_style);
        targets.push(PlaylistsViewClickTarget {
            rect: fav_rect,
            is_playlist_selector: true,
            playlist_id: Some("__favorites__".into()),
            track_index: None,
            is_rename_button: false,
            is_delete_button: false,
        });
        y_l += 1;

        // Recently Played item
        let recent_rect = Rect::new(inner_l.left(), y_l, inner_l.width, 1);
        let is_recent_sel = self.selected_playlist_id == "__recent__";
        let recent_style = if is_recent_sel { self.theme.selected_row() } else { self.theme.primary_text() };
        if is_recent_sel {
            for x in recent_rect.left()..recent_rect.right() { buf[(x, y_l)].set_bg(self.theme.selection); }
        }
        let recent_label = format!(" ◷ Recent ({})", self.playlists_manager.recently_played.len());
        buf.set_string(inner_l.left(), y_l, &recent_label, recent_style);
        targets.push(PlaylistsViewClickTarget {
            rect: recent_rect,
            is_playlist_selector: true,
            playlist_id: Some("__recent__".into()),
            track_index: None,
            is_rename_button: false,
            is_delete_button: false,
        });
        y_l += 2;

        // Custom Playlists
        let custom_top = y_l;
        let custom_bot = inner_l.bottom().saturating_sub(2);
        let custom_avail = custom_bot.saturating_sub(custom_top) as usize;
        let total_custom = self.playlists_manager.playlists.len();
        let start_p = self.scroll_playlist_offset.min(total_custom.saturating_sub(custom_avail));

        for p in self.playlists_manager.playlists.iter().skip(start_p).take(custom_avail) {
            if y_l >= custom_bot {
                break;
            }
            let is_sel = self.selected_playlist_id == p.id;
            let style = if is_sel { self.theme.selected_row() } else { self.theme.primary_text() };
            let p_rect = Rect::new(inner_l.left(), y_l, inner_l.width, 1);
            if is_sel {
                for x in p_rect.left()..p_rect.right() { buf[(x, y_l)].set_bg(self.theme.selection); }
            }
            let label = format!(" ≡ {} ({})", p.name, p.track_ids.len());
            let trunc: String = label.chars().take(inner_l.width as usize).collect();
            buf.set_string(inner_l.left(), y_l, &trunc, style);
            targets.push(PlaylistsViewClickTarget {
                rect: p_rect,
                is_playlist_selector: true,
                playlist_id: Some(p.id.clone()),
                track_index: None,
                is_rename_button: false,
                is_delete_button: false,
            });
            y_l += 1;
        }
        self.theme.render_scrollbar(
            buf,
            left_rect,
            custom_top,
            custom_avail as u16,
            start_p,
            custom_avail,
            total_custom,
        );

        // Action buttons at bottom of left pane for custom playlists
        if self.selected_playlist_id != "__favorites__" && self.selected_playlist_id != "__recent__" {
            let act_y = inner_l.bottom().saturating_sub(1);
            if act_y > inner_l.top() + 5 {
                let ren_w = 12u16.min(inner_l.width / 2);
                let del_w = 12u16.min(inner_l.width.saturating_sub(ren_w + 1));
                let ren_rect = Rect::new(inner_l.left() + 1, act_y, ren_w, 1);
                buf.set_string(ren_rect.left(), act_y, "[ ✎ Rename ]", Style::default().fg(self.theme.cyan));
                targets.push(PlaylistsViewClickTarget {
                    rect: ren_rect,
                    is_playlist_selector: false,
                    playlist_id: Some(self.selected_playlist_id.to_string()),
                    track_index: None,
                    is_rename_button: true,
                    is_delete_button: false,
                });

                let del_rect = Rect::new(ren_rect.right() + 1, act_y, del_w, 1);
                buf.set_string(del_rect.left(), act_y, "[ ✕ Delete ]", Style::default().fg(self.theme.accent));
                targets.push(PlaylistsViewClickTarget {
                    rect: del_rect,
                    is_playlist_selector: false,
                    playlist_id: Some(self.selected_playlist_id.to_string()),
                    track_index: None,
                    is_rename_button: false,
                    is_delete_button: true,
                });
            }
        }

        // Render Right Tracks
        let track_ids: &[String] = if self.selected_playlist_id == "__favorites__" {
            &self.playlists_manager.favorites
        } else if self.selected_playlist_id == "__recent__" {
            &self.playlists_manager.recently_played
        } else if let Some(p) = self.playlists_manager.get_playlist(self.selected_playlist_id) {
            &p.track_ids
        } else {
            &[]
        };

        let tracks = self.library.tracks_for_playlist(track_ids);

        let y_hdr = inner_r.top() + 1;
        let hdr_style = self.theme.muted_text().add_modifier(Modifier::BOLD);
        buf.set_string(inner_r.left(), y_hdr, "#  TITLE", hdr_style);
        let artist_x = inner_r.left() + (inner_r.width as f32 * 0.45) as u16;
        buf.set_string(artist_x, y_hdr, "ARTIST", hdr_style);
        let time_x = inner_r.right().saturating_sub(8);
        buf.set_string(time_x, y_hdr, "TIME", hdr_style);

        let rows_avail = inner_r.height.saturating_sub(2) as usize;
        let start_idx = self.scroll_track_offset.min(tracks.len().saturating_sub(rows_avail));

        for (i, track) in tracks.iter().skip(start_idx).take(rows_avail).enumerate() {
            let actual_idx = start_idx + i;
            let y = y_hdr + 1 + i as u16;
            let is_sel = actual_idx == self.selected_track_idx;
            let is_playing = self.playing_track_id == Some(&track.id);

            let row_rect = Rect::new(inner_r.left(), y, inner_r.width, 1);
            targets.push(PlaylistsViewClickTarget {
                rect: row_rect,
                is_playlist_selector: false,
                playlist_id: None,
                track_index: Some(actual_idx),
                is_rename_button: false,
                is_delete_button: false,
            });

            let style = if is_sel {
                self.theme.selected_row()
            } else if is_playing {
                self.theme.accent_text()
            } else {
                self.theme.primary_text()
            };

            if is_sel {
                for x in row_rect.left()..row_rect.right() {
                    buf[(x, y)].set_bg(self.theme.selection);
                }
            }

            let num_str = format!("{:>2}. ", actual_idx + 1);
            let prefix = if is_playing { "▶" } else { " " };
            let line_prefix = format!("{}{}", prefix, num_str);
            let prefix_chars = line_prefix.chars().count() as u16;
            buf.set_string(inner_r.left(), y, &line_prefix, style);

            let title_avail = artist_x.saturating_sub(inner_r.left() + prefix_chars + 1) as usize;
            let title_str: String = track.display_title().chars().take(title_avail).collect();
            buf.set_string(inner_r.left() + prefix_chars, y, &title_str, style);

            let artist_avail = time_x.saturating_sub(artist_x + 1) as usize;
            let artist_str: String = track.display_artist().chars().take(artist_avail).collect();
            let artist_style = if is_sel { style } else { self.theme.secondary_text() };
            buf.set_string(artist_x, y, &artist_str, artist_style);

            let time_str = track.formatted_duration();
            buf.set_string(time_x, y, &time_str, self.theme.muted_text());
        }

        self.theme.render_scrollbar(
            buf,
            right_rect,
            y_hdr + 1,
            inner_r.height.saturating_sub(2),
            start_idx,
            rows_avail,
            tracks.len(),
        );

        targets
    }
}
