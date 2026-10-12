use crate::library::track::Track;
use crate::library::Library;
use crate::ui::theme::Theme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, Widget};

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
    pub playing_track_id: Option<&'a str>,
    pub is_focused: bool,
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

        // Split area into 3 horizontal columns: Artists (30%), Albums (30%), Songs (40%)
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
        let col1_border = if self.is_focused && self.active_column == ArtistColumn::Artists {
            Theme::block_focus_style()
        } else {
            Theme::block_style()
        };
        let block1 = Block::default()
            .borders(Borders::ALL)
            .border_style(col1_border)
            .title(format!(" Artists ({}) ", artists.len()))
            .title_style(Style::default().fg(Theme::OMARCHY_GREEN).add_modifier(Modifier::BOLD));
        let inner1 = block1.inner(col1_rect);
        block1.render(col1_rect, buf);

        let rows1 = inner1.height as usize;
        let offset1 = if self.selected_artist_idx >= rows1 {
            self.selected_artist_idx.saturating_sub(rows1 / 2)
        } else {
            0
        };

        for (i, artist) in artists.iter().skip(offset1).take(rows1).enumerate() {
            let actual_idx = offset1 + i;
            let y = inner1.top() + i as u16;
            let is_sel = actual_idx == self.selected_artist_idx;

            let row_rect = Rect::new(inner1.left(), y, inner1.width, 1);
            targets.push(ArtistsViewClickTarget {
                rect: row_rect,
                column: ArtistColumn::Artists,
                item_index: actual_idx,
            });

            let style = if is_sel {
                Theme::selected_row()
            } else {
                Style::default().fg(Theme::TEXT_PRIMARY)
            };

            if is_sel {
                for x in row_rect.left()..row_rect.right() {
                    buf[(x, y)].set_bg(Theme::BG_HIGHLIGHT);
                }
            }

            let text: String = artist.chars().take(inner1.width as usize).collect();
            buf.set_string(inner1.left(), y, &text, style);
        }

        // Render Column 2: Albums
        let col2_border = if self.is_focused && self.active_column == ArtistColumn::Albums {
            Theme::block_focus_style()
        } else {
            Theme::block_style()
        };
        let block2 = Block::default()
            .borders(Borders::ALL)
            .border_style(col2_border)
            .title(format!(" Albums ({}) ", albums.len()))
            .title_style(Style::default().fg(Theme::OMARCHY_GREEN).add_modifier(Modifier::BOLD));
        let inner2 = block2.inner(col2_rect);
        block2.render(col2_rect, buf);

        let rows2 = inner2.height as usize;
        let offset2 = if self.selected_album_idx >= rows2 {
            self.selected_album_idx.saturating_sub(rows2 / 2)
        } else {
            0
        };

        for (i, album) in albums.iter().skip(offset2).take(rows2).enumerate() {
            let actual_idx = offset2 + i;
            let y = inner2.top() + i as u16;
            let is_sel = actual_idx == self.selected_album_idx;

            let row_rect = Rect::new(inner2.left(), y, inner2.width, 1);
            targets.push(ArtistsViewClickTarget {
                rect: row_rect,
                column: ArtistColumn::Albums,
                item_index: actual_idx,
            });

            let style = if is_sel {
                Theme::selected_row()
            } else {
                Style::default().fg(Theme::TEXT_PRIMARY)
            };

            if is_sel {
                for x in row_rect.left()..row_rect.right() {
                    buf[(x, y)].set_bg(Theme::BG_HIGHLIGHT);
                }
            }

            let text: String = album.chars().take(inner2.width as usize).collect();
            buf.set_string(inner2.left(), y, &text, style);
        }

        // Render Column 3: Songs
        let col3_border = if self.is_focused && self.active_column == ArtistColumn::Songs {
            Theme::block_focus_style()
        } else {
            Theme::block_style()
        };
        let block3 = Block::default()
            .borders(Borders::ALL)
            .border_style(col3_border)
            .title(format!(" Songs ({}) ", songs.len()))
            .title_style(Style::default().fg(Theme::OMARCHY_GREEN).add_modifier(Modifier::BOLD));
        let inner3 = block3.inner(col3_rect);
        block3.render(col3_rect, buf);

        let rows3 = inner3.height as usize;
        let offset3 = if self.selected_song_idx >= rows3 {
            self.selected_song_idx.saturating_sub(rows3 / 2)
        } else {
            0
        };

        for (i, song) in songs.iter().skip(offset3).take(rows3).enumerate() {
            let actual_idx = offset3 + i;
            let y = inner3.top() + i as u16;
            let is_sel = actual_idx == self.selected_song_idx;
            let is_playing = self.playing_track_id == Some(&song.id);

            let row_rect = Rect::new(inner3.left(), y, inner3.width, 1);
            targets.push(ArtistsViewClickTarget {
                rect: row_rect,
                column: ArtistColumn::Songs,
                item_index: actual_idx,
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

            let prefix = if is_playing { "▶ " } else { "  " };
            let trk_num = song.track_number.map(|n| format!("{:02}. ", n)).unwrap_or_default();
            let duration_str = song.formatted_duration();

            let title_avail = inner3.width.saturating_sub(prefix.len() as u16 + trk_num.len() as u16 + duration_str.len() as u16 + 2) as usize;
            let title: String = song.display_title().chars().take(title_avail).collect();

            let line = format!("{}{}{}", prefix, trk_num, title);
            buf.set_string(inner3.left(), y, &line, style);

            let time_x = inner3.right().saturating_sub(duration_str.len() as u16);
            buf.set_string(time_x, y, &duration_str, Style::default().fg(Theme::TEXT_MUTED));
        }

        targets
    }
}
