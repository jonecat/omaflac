use crate::library::playlist::PlaylistsManager;
use crate::library::track::Track;
use crate::ui::theme::Theme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, Widget};

pub struct SongsViewClickTarget {
    pub rect: Rect,
    pub track_index: usize,
    pub is_header: bool,
}

pub struct SongsView<'a> {
    pub tracks: &'a [&'a Track],
    pub selected_index: usize,
    pub scroll_offset: usize,
    pub playing_track_id: Option<&'a str>,
    pub playlists_manager: &'a PlaylistsManager,
    pub is_focused: bool,
}

impl<'a> SongsView<'a> {
    pub fn render_with_targets(
        &self,
        area: Rect,
        buf: &mut Buffer,
    ) -> Vec<SongsViewClickTarget> {
        let mut targets = Vec::new();

        let border_style = if self.is_focused {
            Theme::block_focus_style()
        } else {
            Theme::block_style()
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(format!(" Songs ({}) ", self.tracks.len()))
            .title_style(Style::default().fg(Theme::OMARCHY_GREEN).add_modifier(Modifier::BOLD));

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.width < 10 || inner.height < 2 {
            return targets;
        }

        // Header row
        let y_hdr = inner.top();
        let header_rect = Rect::new(inner.left(), y_hdr, inner.width, 1);
        targets.push(SongsViewClickTarget {
            rect: header_rect,
            track_index: 0,
            is_header: true,
        });

        // Column widths
        // [Fav 2] [Status 2] [Title 35%] [Artist 25%] [Album 25%] [Time 6] [Format 5]
        let col_fav_w = 2u16;
        let col_stat_w = 2u16;
        let col_time_w = 7u16;
        let col_fmt_w = 7u16;

        let remaining = inner.width.saturating_sub(col_fav_w + col_stat_w + col_time_w + col_fmt_w + 4);
        let col_title_w = (remaining as f32 * 0.45) as u16;
        let col_artist_w = (remaining as f32 * 0.28) as u16;
        let col_album_w = remaining.saturating_sub(col_title_w + col_artist_w);

        let col_fav_x = inner.left();
        let col_stat_x = col_fav_x + col_fav_w;
        let col_title_x = col_stat_x + col_stat_w;
        let col_artist_x = col_title_x + col_title_w + 1;
        let col_album_x = col_artist_x + col_artist_w + 1;
        let col_time_x = col_album_x + col_album_w + 1;
        let col_fmt_x = col_time_x + col_time_w + 1;

        // Render header strings
        let hdr_style = Style::default().fg(Theme::TEXT_MUTED).add_modifier(Modifier::BOLD);
        buf.set_string(col_fav_x, y_hdr, "⭐", hdr_style);
        buf.set_string(col_stat_x, y_hdr, "  ", hdr_style);
        buf.set_string(col_title_x, y_hdr, "TITLE", hdr_style);
        buf.set_string(col_artist_x, y_hdr, "ARTIST", hdr_style);
        buf.set_string(col_album_x, y_hdr, "ALBUM", hdr_style);
        buf.set_string(col_time_x, y_hdr, "TIME", hdr_style);
        buf.set_string(col_fmt_x, y_hdr, "FORMAT", hdr_style);

        let rows_available = inner.height.saturating_sub(1) as usize;
        let num_tracks = self.tracks.len();

        let start_idx = self.scroll_offset.min(num_tracks);
        let end_idx = (start_idx + rows_available).min(num_tracks);

        for (i, track) in self.tracks[start_idx..end_idx].iter().enumerate() {
            let actual_idx = start_idx + i;
            let row_y = y_hdr + 1 + i as u16;

            let is_selected = actual_idx == self.selected_index;
            let is_playing = self.playing_track_id == Some(&track.id);
            let is_fav = self.playlists_manager.is_favorite(&track.id);

            let row_rect = Rect::new(inner.left(), row_y, inner.width, 1);
            targets.push(SongsViewClickTarget {
                rect: row_rect,
                track_index: actual_idx,
                is_header: false,
            });

            // Row styling
            let base_style = if is_selected {
                Theme::selected_row()
            } else if is_playing {
                Style::default().fg(Theme::OMARCHY_GREEN).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Theme::TEXT_PRIMARY)
            };

            // Background fill
            if is_selected {
                for x in row_rect.left()..row_rect.right() {
                    buf[(x, row_y)].set_bg(Theme::BG_HIGHLIGHT);
                }
            }

            // Favorite star
            if is_fav {
                buf.set_string(col_fav_x, row_y, "★ ", Style::default().fg(Theme::ACCENT_YELLOW));
            } else {
                buf.set_string(col_fav_x, row_y, "  ", base_style);
            }

            // Status icon (Playing: ▶)
            if is_playing {
                buf.set_string(col_stat_x, row_y, "▶ ", Style::default().fg(Theme::OMARCHY_GREEN).add_modifier(Modifier::BOLD));
            } else {
                buf.set_string(col_stat_x, row_y, "  ", base_style);
            }

            // Title
            let title: String = track.display_title().chars().take(col_title_w as usize).collect();
            buf.set_string(col_title_x, row_y, &title, base_style);

            // Artist
            let artist_style = if is_selected { base_style } else { Style::default().fg(Theme::TEXT_SECONDARY) };
            let artist: String = track.display_artist().chars().take(col_artist_w as usize).collect();
            buf.set_string(col_artist_x, row_y, &artist, artist_style);

            // Album
            let album_style = if is_selected { base_style } else { Style::default().fg(Theme::TEXT_SECONDARY) };
            let album: String = track.display_album().chars().take(col_album_w as usize).collect();
            buf.set_string(col_album_x, row_y, &album, album_style);

            // Duration
            let time_str = track.formatted_duration();
            buf.set_string(col_time_x, row_y, &time_str, Style::default().fg(Theme::TEXT_MUTED));

            // Format badge
            let fmt_style = if is_selected {
                base_style
            } else if track.format == "FLAC" {
                Style::default().fg(Theme::OMARCHY_GREEN)
            } else {
                Style::default().fg(Theme::ACCENT_CYAN)
            };
            let fmt: String = track.format.chars().take(col_fmt_w as usize).collect();
            buf.set_string(col_fmt_x, row_y, &fmt, fmt_style);
        }

        targets
    }
}
