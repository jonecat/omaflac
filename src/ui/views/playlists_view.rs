use crate::library::playlist::PlaylistsManager;
use crate::library::Library;
use crate::ui::theme::Theme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, Widget};

pub struct PlaylistsViewClickTarget {
    pub rect: Rect,
    pub is_playlist_selector: bool,
    pub playlist_id: Option<String>,
    pub track_index: Option<usize>,
}

pub struct PlaylistsView<'a> {
    pub library: &'a Library,
    pub playlists_manager: &'a PlaylistsManager,
    pub selected_playlist_id: &'a str,
    pub selected_track_idx: usize,
    pub playing_track_id: Option<&'a str>,
    pub is_focused: bool,
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

        // Left 28%: playlist selector, Right 72%: playlist tracks
        let list_w = (area.width as f32 * 0.28).max(18.0) as u16;
        let track_w = area.width.saturating_sub(list_w);

        let left_rect = Rect::new(area.left(), area.top(), list_w, area.height);
        let right_rect = Rect::new(area.left() + list_w, area.top(), track_w, area.height);

        // Left block
        let block_left = Block::default()
            .borders(Borders::ALL)
            .border_style(Theme::block_style())
            .title(" Playlists ")
            .title_style(Style::default().fg(Theme::OMARCHY_GREEN).add_modifier(Modifier::BOLD));
        let inner_l = block_left.inner(left_rect);
        block_left.render(left_rect, buf);

        // Right block
        let playlist_name = if self.selected_playlist_id == "__favorites__" {
            "⭐ Favorites".to_string()
        } else if self.selected_playlist_id == "__recent__" {
            "🕒 Recently Played".to_string()
        } else if let Some(p) = self.playlists_manager.get_playlist(self.selected_playlist_id) {
            format!("📑 {}", p.name)
        } else {
            "Playlist".to_string()
        };

        let block_right = Block::default()
            .borders(Borders::ALL)
            .border_style(if self.is_focused { Theme::block_focus_style() } else { Theme::block_style() })
            .title(format!(" {} ", playlist_name))
            .title_style(Style::default().fg(Theme::OMARCHY_GREEN).add_modifier(Modifier::BOLD));
        let inner_r = block_right.inner(right_rect);
        block_right.render(right_rect, buf);

        // Render Left items
        let mut y_l = inner_l.top();

        // Favorites item
        let fav_rect = Rect::new(inner_l.left(), y_l, inner_l.width, 1);
        let is_fav_sel = self.selected_playlist_id == "__favorites__";
        let fav_style = if is_fav_sel { Theme::selected_row() } else { Style::default().fg(Theme::TEXT_PRIMARY) };
        if is_fav_sel {
            for x in fav_rect.left()..fav_rect.right() { buf[(x, y_l)].set_bg(Theme::BG_HIGHLIGHT); }
        }
        let fav_label = format!(" ⭐ Favorites ({})", self.playlists_manager.favorites.len());
        buf.set_string(inner_l.left(), y_l, &fav_label, fav_style);
        targets.push(PlaylistsViewClickTarget {
            rect: fav_rect,
            is_playlist_selector: true,
            playlist_id: Some("__favorites__".into()),
            track_index: None,
        });
        y_l += 1;

        // Recently Played item
        let recent_rect = Rect::new(inner_l.left(), y_l, inner_l.width, 1);
        let is_recent_sel = self.selected_playlist_id == "__recent__";
        let recent_style = if is_recent_sel { Theme::selected_row() } else { Style::default().fg(Theme::TEXT_PRIMARY) };
        if is_recent_sel {
            for x in recent_rect.left()..recent_rect.right() { buf[(x, y_l)].set_bg(Theme::BG_HIGHLIGHT); }
        }
        let recent_label = format!(" 🕒 Recent ({})", self.playlists_manager.recently_played.len());
        buf.set_string(inner_l.left(), y_l, &recent_label, recent_style);
        targets.push(PlaylistsViewClickTarget {
            rect: recent_rect,
            is_playlist_selector: true,
            playlist_id: Some("__recent__".into()),
            track_index: None,
        });
        y_l += 2;

        // Custom Playlists
        for p in &self.playlists_manager.playlists {
            if y_l >= inner_l.bottom().saturating_sub(2) {
                break;
            }
            let is_sel = self.selected_playlist_id == p.id;
            let style = if is_sel { Theme::selected_row() } else { Style::default().fg(Theme::TEXT_PRIMARY) };
            let p_rect = Rect::new(inner_l.left(), y_l, inner_l.width, 1);
            if is_sel {
                for x in p_rect.left()..p_rect.right() { buf[(x, y_l)].set_bg(Theme::BG_HIGHLIGHT); }
            }
            let label = format!(" 📑 {} ({})", p.name, p.track_ids.len());
            let trunc: String = label.chars().take(inner_l.width as usize).collect();
            buf.set_string(inner_l.left(), y_l, &trunc, style);
            targets.push(PlaylistsViewClickTarget {
                rect: p_rect,
                is_playlist_selector: true,
                playlist_id: Some(p.id.clone()),
                track_index: None,
            });
            y_l += 1;
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

        let y_hdr = inner_r.top();
        let hdr_style = Style::default().fg(Theme::TEXT_MUTED).add_modifier(Modifier::BOLD);
        buf.set_string(inner_r.left(), y_hdr, "#  TITLE", hdr_style);
        let artist_x = inner_r.left() + (inner_r.width as f32 * 0.45) as u16;
        buf.set_string(artist_x, y_hdr, "ARTIST", hdr_style);
        let time_x = inner_r.right().saturating_sub(8);
        buf.set_string(time_x, y_hdr, "TIME", hdr_style);

        let rows_avail = inner_r.height.saturating_sub(1) as usize;
        for (i, track) in tracks.iter().take(rows_avail).enumerate() {
            let y = y_hdr + 1 + i as u16;
            let is_sel = i == self.selected_track_idx;
            let is_playing = self.playing_track_id == Some(&track.id);

            let row_rect = Rect::new(inner_r.left(), y, inner_r.width, 1);
            targets.push(PlaylistsViewClickTarget {
                rect: row_rect,
                is_playlist_selector: false,
                playlist_id: None,
                track_index: Some(i),
            });

            let style = if is_sel {
                Theme::selected_row()
            } else if is_playing {
                Style::default().fg(Theme::OMARCHY_GREEN).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Theme::TEXT_PRIMARY)
            };

            if is_sel {
                for x in row_rect.left()..row_rect.right() {
                    buf[(x, y)].set_bg(Theme::BG_HIGHLIGHT);
                }
            }

            let num_str = format!("{:>2}. ", i + 1);
            let prefix = if is_playing { "▶" } else { " " };
            let line_prefix = format!("{}{}", prefix, num_str);
            buf.set_string(inner_r.left(), y, &line_prefix, style);

            let title_avail = artist_x.saturating_sub(inner_r.left() + line_prefix.len() as u16 + 1) as usize;
            let title_str: String = track.display_title().chars().take(title_avail).collect();
            buf.set_string(inner_r.left() + line_prefix.len() as u16, y, &title_str, style);

            let artist_avail = time_x.saturating_sub(artist_x + 1) as usize;
            let artist_str: String = track.display_artist().chars().take(artist_avail).collect();
            let artist_style = if is_sel { style } else { Style::default().fg(Theme::TEXT_SECONDARY) };
            buf.set_string(artist_x, y, &artist_str, artist_style);

            let time_str = track.formatted_duration();
            buf.set_string(time_x, y, &time_str, Style::default().fg(Theme::TEXT_MUTED));
        }

        targets
    }
}
