use crate::library::playlist::PlaylistsManager;
use crate::library::track::Track;
use crate::ui::theme::OmarchyTheme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, BorderType, Borders, Widget};

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
    pub theme: &'a OmarchyTheme,
}

impl<'a> SongsView<'a> {
    pub fn render_with_targets(
        &self,
        area: Rect,
        buf: &mut Buffer,
    ) -> Vec<SongsViewClickTarget> {
        let mut targets = Vec::new();

        let (border_style, title_style, title_text) = if self.is_focused {
            (
                self.theme.block_focus_style(),
                self.theme.focused_title_style(),
                format!(" ● Songs ({}) ", self.tracks.len()),
            )
        } else {
            (
                self.theme.block_style(),
                self.theme.unfocused_title_style(),
                format!(" Songs ({}) ", self.tracks.len()),
            )
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(border_style)
            .style(Style::default().bg(self.theme.background))
            .title(title_text)
            .title_style(title_style);

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.width < 10 || inner.height < 2 {
            return targets;
        }

        // Fill background
        for y in inner.top()..inner.bottom() {
            for x in inner.left()..inner.right() {
                buf[(x, y)].set_bg(self.theme.background);
            }
        }

        // Header row (with 1-row top margin below section border)
        let y_hdr = inner.top() + 1;
        let header_rect = Rect::new(inner.left(), y_hdr, inner.width, 1);
        targets.push(SongsViewClickTarget {
            rect: header_rect,
            track_index: 0,
            is_header: true,
        });

        let col_fav_w = 2u16;
        let col_stat_w = 2u16;
        let col_time_w = 7u16;
        let col_fmt_w = 7u16;

        let remaining = inner.width.saturating_sub(col_fav_w + col_stat_w + col_time_w + col_fmt_w + 5);
        let col_title_w = (remaining as f32 * 0.45) as u16;
        let col_artist_w = (remaining as f32 * 0.28) as u16;
        let col_album_w = remaining.saturating_sub(col_title_w + col_artist_w);

        let col_fav_x = inner.left() + 1;
        let col_stat_x = col_fav_x + col_fav_w;
        let col_title_x = col_stat_x + col_stat_w;
        let col_artist_x = col_title_x + col_title_w + 1;
        let col_album_x = col_artist_x + col_artist_w + 1;
        let col_time_x = col_album_x + col_album_w + 1;
        let col_fmt_x = col_time_x + col_time_w + 1;

        let hdr_style = Style::default().fg(self.theme.muted).add_modifier(Modifier::BOLD);
        buf.set_string(col_fav_x, y_hdr, "★ ", hdr_style);
        buf.set_string(col_stat_x, y_hdr, "  ", hdr_style);
        buf.set_string(col_title_x, y_hdr, "TITLE", hdr_style);
        buf.set_string(col_artist_x, y_hdr, "ARTIST", hdr_style);
        buf.set_string(col_album_x, y_hdr, "ALBUM", hdr_style);
        buf.set_string(col_time_x, y_hdr, "TIME", hdr_style);
        buf.set_string(col_fmt_x, y_hdr, "FORMAT", hdr_style);

        let rows_available = inner.height.saturating_sub(2) as usize;
        let num_tracks = self.tracks.len();

        let start_idx = self.scroll_offset.min(num_tracks.saturating_sub(rows_available));
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

            let base_style = if is_selected {
                self.theme.selected_row()
            } else if is_playing {
                Style::default().fg(self.theme.accent).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(self.theme.foreground)
            };

            if is_selected {
                for x in row_rect.left()..row_rect.right() {
                    buf[(x, row_y)].set_bg(self.theme.selection);
                }
            }

            if is_fav {
                let fav_style = if is_selected {
                    base_style
                } else {
                    Style::default().fg(self.theme.accent)
                };
                buf.set_string(col_fav_x, row_y, "★ ", fav_style);
            } else {
                buf.set_string(col_fav_x, row_y, "  ", base_style);
            }

            if is_playing {
                buf.set_string(col_stat_x, row_y, "▶ ", Style::default().fg(self.theme.accent).add_modifier(Modifier::BOLD));
            } else {
                buf.set_string(col_stat_x, row_y, "  ", base_style);
            }

            let title: String = track.display_title().chars().take(col_title_w as usize).collect();
            buf.set_string(col_title_x, row_y, &title, base_style);

            let artist_style = if is_selected { base_style } else { Style::default().fg(self.theme.light_foreground) };
            let artist: String = track.display_artist().chars().take(col_artist_w as usize).collect();
            buf.set_string(col_artist_x, row_y, &artist, artist_style);

            let album_style = if is_selected { base_style } else { Style::default().fg(self.theme.light_foreground) };
            let album: String = track.display_album().chars().take(col_album_w as usize).collect();
            buf.set_string(col_album_x, row_y, &album, album_style);

            let time_str = track.formatted_duration();
            buf.set_string(col_time_x, row_y, &time_str, Style::default().fg(self.theme.muted));

            let fmt_style = if is_selected {
                base_style
            } else if track.format == "FLAC" {
                Style::default().fg(self.theme.accent)
            } else {
                Style::default().fg(self.theme.cyan)
            };
            let fmt: String = track.format.chars().take(col_fmt_w as usize).collect();
            buf.set_string(col_fmt_x, row_y, &fmt, fmt_style);
        }

        self.theme.render_scrollbar(
            buf,
            area,
            y_hdr + 1,
            inner.height.saturating_sub(2),
            start_idx,
            rows_available,
            num_tracks,
        );

        targets
    }
}
