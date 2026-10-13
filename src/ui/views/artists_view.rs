use crate::library::playlist::PlaylistsManager;
use crate::library::track::Track;
use crate::library::Library;
use crate::ui::theme::OmarchyTheme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::{Block, BorderType, Borders, Widget};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtistColumn {
    Artists,
    Albums,
    Songs,
}

pub struct ArtistsViewClickTarget {
    pub rect: Rect,
    pub column: ArtistColumn,
    pub item_index: usize,
}

pub struct ArtistsView<'a> {
    pub library: &'a Library,
    pub active_column: ArtistColumn,
    pub selected_artist_idx: usize,
    pub selected_album_idx: usize,
    pub selected_song_idx: usize,
    pub scroll_artist_offset: usize,
    pub scroll_album_offset: usize,
    pub scroll_song_offset: usize,
    pub playing_track_id: Option<&'a str>,
    pub playlists_manager: &'a PlaylistsManager,
    pub is_focused: bool,
    pub theme: &'a OmarchyTheme,
}

impl<'a> ArtistsView<'a> {
    pub fn render_with_targets(
        &self,
        area: Rect,
        buf: &mut Buffer,
    ) -> Vec<ArtistsViewClickTarget> {
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

        // Split area into 3 horizontal columns: Artists (28%), Albums (32%), Songs (40%)
        let total_w = area.width;
        let col1_w = (total_w as f32 * 0.28) as u16;
        let col2_w = (total_w as f32 * 0.32) as u16;
        let col3_w = total_w.saturating_sub(col1_w + col2_w);

        let col1_rect = Rect::new(area.left(), area.top(), col1_w, area.height);
        let col2_rect = Rect::new(area.left() + col1_w, area.top(), col2_w, area.height);
        let col3_rect = Rect::new(area.left() + col1_w + col2_w, area.top(), col3_w, area.height);

        let artists = self.library.artists();
        let current_artist = artists.get(self.selected_artist_idx).cloned();

        let albums = if let Some(artist) = &current_artist {
            self.library.albums_by_artist(artist)
        } else {
            Vec::new()
        };
        let current_album = albums.get(self.selected_album_idx).cloned();

        let songs: Vec<&Track> = match (&current_artist, &current_album) {
            (Some(art), Some(alb)) => self.library.tracks_by_artist_and_album(art, alb),
            (Some(art), None) => self.library.tracks_by_artist(art),
            _ => Vec::new(),
        };

        // Render Column 1: Artists
        let is_col1_focused = self.is_focused && self.active_column == ArtistColumn::Artists;
        let (b_style1, t_style1, t_text1) = if is_col1_focused {
            (
                self.theme.block_focus_style(),
                self.theme.focused_title_style(),
                format!(" ● Artists ({}) ", artists.len()),
            )
        } else {
            (
                self.theme.block_style(),
                self.theme.unfocused_title_style(),
                format!(" Artists ({}) ", artists.len()),
            )
        };
        let block1 = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(b_style1)
            .style(Style::default().bg(self.theme.background))
            .title(t_text1)
            .title_style(t_style1);
        let inner1 = block1.inner(col1_rect);
        block1.render(col1_rect, buf);

        let top_pad = 1u16.min(inner1.height);
        let content_top1 = inner1.top() + top_pad;
        let avail_h1 = inner1.height.saturating_sub(top_pad);
        let rows1 = avail_h1 as usize;
        let offset1 = self.scroll_artist_offset.min(artists.len().saturating_sub(rows1));

        for (i, artist) in artists.iter().skip(offset1).take(rows1).enumerate() {
            let actual_idx = offset1 + i;
            let y = content_top1 + i as u16;
            let is_sel = actual_idx == self.selected_artist_idx;

            let row_rect = Rect::new(inner1.left(), y, inner1.width, 1);
            targets.push(ArtistsViewClickTarget {
                rect: row_rect,
                column: ArtistColumn::Artists,
                item_index: actual_idx,
            });

            let style = if is_sel {
                self.theme.selected_row()
            } else {
                self.theme.primary_text()
            };

            if is_sel {
                for x in row_rect.left()..row_rect.right() {
                    buf[(x, y)].set_bg(self.theme.selection);
                }
            }

            let text: String = format!(" {}", artist).chars().take(inner1.width as usize).collect();
            buf.set_string(inner1.left(), y, &text, style);
        }
        self.theme.render_scrollbar(
            buf,
            col1_rect,
            content_top1,
            avail_h1,
            offset1,
            rows1,
            artists.len(),
        );

        // Render Column 2: Albums
        let is_col2_focused = self.is_focused && self.active_column == ArtistColumn::Albums;
        let (b_style2, t_style2, t_text2) = if is_col2_focused {
            (
                self.theme.block_focus_style(),
                self.theme.focused_title_style(),
                format!(" ● Albums ({}) ", albums.len()),
            )
        } else {
            (
                self.theme.block_style(),
                self.theme.unfocused_title_style(),
                format!(" Albums ({}) ", albums.len()),
            )
        };
        let block2 = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(b_style2)
            .style(Style::default().bg(self.theme.background))
            .title(t_text2)
            .title_style(t_style2);
        let inner2 = block2.inner(col2_rect);
        block2.render(col2_rect, buf);

        let top_pad2 = 1u16.min(inner2.height);
        let content_top2 = inner2.top() + top_pad2;
        let avail_h2 = inner2.height.saturating_sub(top_pad2);
        let rows2 = avail_h2 as usize;
        let offset2 = self.scroll_album_offset.min(albums.len().saturating_sub(rows2));

        for (i, album) in albums.iter().skip(offset2).take(rows2).enumerate() {
            let actual_idx = offset2 + i;
            let y = content_top2 + i as u16;
            let is_sel = actual_idx == self.selected_album_idx;

            let row_rect = Rect::new(inner2.left(), y, inner2.width, 1);
            targets.push(ArtistsViewClickTarget {
                rect: row_rect,
                column: ArtistColumn::Albums,
                item_index: actual_idx,
            });

            let style = if is_sel {
                self.theme.selected_row()
            } else {
                self.theme.primary_text()
            };

            if is_sel {
                for x in row_rect.left()..row_rect.right() {
                    buf[(x, y)].set_bg(self.theme.selection);
                }
            }

            let text: String = format!(" {}", album).chars().take(inner2.width as usize).collect();
            buf.set_string(inner2.left(), y, &text, style);
        }
        self.theme.render_scrollbar(
            buf,
            col2_rect,
            content_top2,
            avail_h2,
            offset2,
            rows2,
            albums.len(),
        );

        // Render Column 3: Songs
        let is_col3_focused = self.is_focused && self.active_column == ArtistColumn::Songs;
        let (b_style3, t_style3, t_text3) = if is_col3_focused {
            (
                self.theme.block_focus_style(),
                self.theme.focused_title_style(),
                format!(" ● Songs ({}) ", songs.len()),
            )
        } else {
            (
                self.theme.block_style(),
                self.theme.unfocused_title_style(),
                format!(" Songs ({}) ", songs.len()),
            )
        };
        let block3 = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(b_style3)
            .style(Style::default().bg(self.theme.background))
            .title(t_text3)
            .title_style(t_style3);
        let inner3 = block3.inner(col3_rect);
        block3.render(col3_rect, buf);

        let top_pad3 = 1u16.min(inner3.height);
        let content_top3 = inner3.top() + top_pad3;
        let avail_h3 = inner3.height.saturating_sub(top_pad3);
        let rows3 = avail_h3 as usize;
        let offset3 = self.scroll_song_offset.min(songs.len().saturating_sub(rows3));

        for (i, song) in songs.iter().skip(offset3).take(rows3).enumerate() {
            let actual_idx = offset3 + i;
            let y = content_top3 + i as u16;
            let is_sel = actual_idx == self.selected_song_idx;
            let is_playing = self.playing_track_id == Some(&song.id);
            let is_fav = self.playlists_manager.is_favorite(&song.id);

            let row_rect = Rect::new(inner3.left(), y, inner3.width, 1);
            targets.push(ArtistsViewClickTarget {
                rect: row_rect,
                column: ArtistColumn::Songs,
                item_index: actual_idx,
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

            let play_prefix = if is_playing { "▶" } else { " " };
            let fav_prefix = if is_fav { "★ " } else { "  " };
            let trk_num = song.track_number.map(|n| format!("{:02}. ", n)).unwrap_or_default();
            let duration_str = song.formatted_duration();

            let prefix = format!(" {} {}{}", play_prefix, fav_prefix, trk_num);
            let prefix_chars = prefix.chars().count() as u16;
            let title_avail = inner3
                .width
                .saturating_sub(prefix_chars + duration_str.len() as u16 + 3)
                as usize;
            let title: String = song.display_title().chars().take(title_avail).collect();

            let line = format!("{}{}", prefix, title);
            buf.set_string(inner3.left(), y, &line, style);

            let time_x = inner3.right().saturating_sub(duration_str.len() as u16 + 1);
            buf.set_string(time_x, y, &duration_str, self.theme.muted_text());
        }
        self.theme.render_scrollbar(
            buf,
            col3_rect,
            content_top3,
            avail_h3,
            offset3,
            rows3,
            songs.len(),
        );

        targets
    }
}
