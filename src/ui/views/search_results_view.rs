use crate::library::playlist::PlaylistsManager;
use crate::library::track::Track;
use crate::library::{AlbumSummary, ArtistSummary};
use crate::ui::theme::OmarchyTheme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::{Block, BorderType, Borders, Widget};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchSection {
    Artists,
    Albums,
    Songs,
}

impl SearchSection {
    pub fn next(&self) -> Self {
        match self {
            SearchSection::Artists => SearchSection::Albums,
            SearchSection::Albums => SearchSection::Songs,
            SearchSection::Songs => SearchSection::Artists,
        }
    }

    pub fn prev(&self) -> Self {
        match self {
            SearchSection::Artists => SearchSection::Songs,
            SearchSection::Albums => SearchSection::Artists,
            SearchSection::Songs => SearchSection::Albums,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SearchResultsClickTarget {
    pub rect: Rect,
    pub section: SearchSection,
    pub item_index: usize,
}

pub struct SearchResultsView<'a> {
    pub search_query: &'a str,
    pub matching_artists: &'a [ArtistSummary],
    pub matching_albums: &'a [AlbumSummary],
    pub matching_tracks: &'a [&'a Track],
    pub active_section: SearchSection,
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

impl<'a> SearchResultsView<'a> {
    pub fn render_with_targets(
        &self,
        area: Rect,
        buf: &mut Buffer,
    ) -> Vec<SearchResultsClickTarget> {
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

        // Layout: 3 horizontal columns: Artists (28%), Albums (32%), Songs (40%)
        let total_w = area.width;
        let col1_w = (total_w as f32 * 0.28) as u16;
        let col2_w = (total_w as f32 * 0.32) as u16;
        let col3_w = total_w.saturating_sub(col1_w + col2_w);

        let col1_rect = Rect::new(area.left(), area.top(), col1_w, area.height);
        let col2_rect = Rect::new(area.left() + col1_w, area.top(), col2_w, area.height);
        let col3_rect = Rect::new(area.left() + col1_w + col2_w, area.top(), col3_w, area.height);

        // Column 1: Matching Artists
        targets.extend(self.render_artists_section(col1_rect, buf));

        // Column 2: Matching Albums
        targets.extend(self.render_albums_section(col2_rect, buf));

        // Column 3: Matching Songs
        targets.extend(self.render_songs_section(col3_rect, buf));

        targets
    }

    fn render_artists_section(
        &self,
        area: Rect,
        buf: &mut Buffer,
    ) -> Vec<SearchResultsClickTarget> {
        let mut targets = Vec::new();
        let is_active = self.is_focused && self.active_section == SearchSection::Artists;

        let (b_style, t_style, title) = if is_active {
            (
                self.theme.block_focus_style(),
                self.theme.focused_title_style(),
                format!(" ● Artists ({}) ", self.matching_artists.len()),
            )
        } else {
            (
                self.theme.block_style(),
                self.theme.unfocused_title_style(),
                format!(" Artists ({}) ", self.matching_artists.len()),
            )
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(b_style)
            .style(Style::default().bg(self.theme.background))
            .title(title)
            .title_style(t_style);

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.width < 4 || inner.height < 1 {
            return targets;
        }

        if self.matching_artists.is_empty() {
            let msg = if self.search_query.trim().is_empty() {
                "Type to search..."
            } else {
                "No artists matched"
            };
            buf.set_string(inner.left() + 1, inner.top() + 1, msg, self.theme.muted_text());
            return targets;
        }

        let max_rows = inner.height as usize;
        let start = self.scroll_artist_offset.min(self.matching_artists.len().saturating_sub(max_rows));
        let visible_items = &self.matching_artists[start..];

        for (i, artist) in visible_items.iter().enumerate().take(max_rows) {
            let actual_idx = start + i;
            let y = inner.top() + i as u16;
            let is_selected = actual_idx == self.selected_artist_idx;

            let row_rect = Rect::new(inner.left(), y, inner.width, 1);
            targets.push(SearchResultsClickTarget {
                rect: row_rect,
                section: SearchSection::Artists,
                item_index: actual_idx,
            });

            let style = if is_selected {
                self.theme.selected_row()
            } else {
                self.theme.primary_text()
            };

            if is_selected {
                for col in row_rect.left()..row_rect.right() {
                    buf[(col, y)].set_bg(self.theme.selection);
                }
            }

            let meta = format!(" ({} alb, {} trk)", artist.album_count, artist.track_count);
            let avail_w = inner.width as usize;
            let name_avail = avail_w.saturating_sub(meta.chars().count());
            let trunc_name: String = artist.name.chars().take(name_avail.max(6)).collect();

            let full_line = format!("{}{}", trunc_name, meta);
            let trunc_line: String = full_line.chars().take(inner.width as usize).collect();
            buf.set_string(inner.left(), y, &trunc_line, style);
        }
        self.theme.render_scrollbar(
            buf,
            area,
            inner.top(),
            inner.height,
            start,
            max_rows,
            self.matching_artists.len(),
        );

        targets
    }

    fn render_albums_section(
        &self,
        area: Rect,
        buf: &mut Buffer,
    ) -> Vec<SearchResultsClickTarget> {
        let mut targets = Vec::new();
        let is_active = self.is_focused && self.active_section == SearchSection::Albums;

        let (b_style, t_style, title) = if is_active {
            (
                self.theme.block_focus_style(),
                self.theme.focused_title_style(),
                format!(" ● Albums ({}) ", self.matching_albums.len()),
            )
        } else {
            (
                self.theme.block_style(),
                self.theme.unfocused_title_style(),
                format!(" Albums ({}) ", self.matching_albums.len()),
            )
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(b_style)
            .style(Style::default().bg(self.theme.background))
            .title(title)
            .title_style(t_style);

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.width < 4 || inner.height < 1 {
            return targets;
        }

        if self.matching_albums.is_empty() {
            let msg = if self.search_query.trim().is_empty() {
                "Type to search..."
            } else {
                "No albums matched"
            };
            buf.set_string(inner.left() + 1, inner.top() + 1, msg, self.theme.muted_text());
            return targets;
        }

        let max_rows = inner.height as usize;
        let start = self.scroll_album_offset.min(self.matching_albums.len().saturating_sub(max_rows));
        let visible_items = &self.matching_albums[start..];

        for (i, album) in visible_items.iter().enumerate().take(max_rows) {
            let actual_idx = start + i;
            let y = inner.top() + i as u16;
            let is_selected = actual_idx == self.selected_album_idx;

            let row_rect = Rect::new(inner.left(), y, inner.width, 1);
            targets.push(SearchResultsClickTarget {
                rect: row_rect,
                section: SearchSection::Albums,
                item_index: actual_idx,
            });

            let style = if is_selected {
                self.theme.selected_row()
            } else {
                self.theme.primary_text()
            };

            if is_selected {
                for col in row_rect.left()..row_rect.right() {
                    buf[(col, y)].set_bg(self.theme.selection);
                }
            }

            let meta = format!(" — {} ({} trk)", album.artist, album.track_count);
            let avail_w = inner.width as usize;
            let title_avail = avail_w.saturating_sub(meta.chars().count());
            let trunc_title: String = album.album.chars().take(title_avail.max(6)).collect();

            let full_line = format!("{}{}", trunc_title, meta);
            let trunc_line: String = full_line.chars().take(inner.width as usize).collect();
            buf.set_string(inner.left(), y, &trunc_line, style);
        }
        self.theme.render_scrollbar(
            buf,
            area,
            inner.top(),
            inner.height,
            start,
            max_rows,
            self.matching_albums.len(),
        );

        targets
    }

    fn render_songs_section(
        &self,
        area: Rect,
        buf: &mut Buffer,
    ) -> Vec<SearchResultsClickTarget> {
        let mut targets = Vec::new();
        let is_active = self.is_focused && self.active_section == SearchSection::Songs;

        let (b_style, t_style, title) = if is_active {
            (
                self.theme.block_focus_style(),
                self.theme.focused_title_style(),
                format!(" ● Songs ({}) ", self.matching_tracks.len()),
            )
        } else {
            (
                self.theme.block_style(),
                self.theme.unfocused_title_style(),
                format!(" Songs ({}) ", self.matching_tracks.len()),
            )
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(b_style)
            .style(Style::default().bg(self.theme.background))
            .title(title)
            .title_style(t_style);

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.width < 4 || inner.height < 1 {
            return targets;
        }

        if self.matching_tracks.is_empty() {
            let msg = if self.search_query.trim().is_empty() {
                "Type to search..."
            } else {
                "No songs matched"
            };
            buf.set_string(inner.left() + 1, inner.top() + 1, msg, self.theme.muted_text());
            return targets;
        }

        let max_rows = inner.height as usize;
        let start = self.scroll_song_offset.min(self.matching_tracks.len().saturating_sub(max_rows));
        let visible_items = &self.matching_tracks[start..];

        for (i, track) in visible_items.iter().enumerate().take(max_rows) {
            let actual_idx = start + i;
            let y = inner.top() + i as u16;
            let is_selected = actual_idx == self.selected_song_idx;
            let is_playing = self.playing_track_id == Some(&track.id);
            let is_fav = self.playlists_manager.is_favorite(&track.id);

            let row_rect = Rect::new(inner.left(), y, inner.width, 1);
            targets.push(SearchResultsClickTarget {
                rect: row_rect,
                section: SearchSection::Songs,
                item_index: actual_idx,
            });

            let style = if is_selected {
                self.theme.selected_row()
            } else if is_playing {
                self.theme.accent_text()
            } else {
                self.theme.primary_text()
            };

            if is_selected {
                for col in row_rect.left()..row_rect.right() {
                    buf[(col, y)].set_bg(self.theme.selection);
                }
            }

            let play_prefix = if is_playing { "▶" } else { " " };
            let fav_prefix = if is_fav { "★ " } else { "  " };
            let duration_str = track.formatted_duration();

            let prefix = format!("{} {}", play_prefix, fav_prefix);
            let prefix_chars = prefix.chars().count() as u16;
            let title_artist = format!("{} — {}", track.display_title(), track.display_artist());
            let max_text_w = inner
                .width
                .saturating_sub(prefix_chars + duration_str.len() as u16 + 2)
                as usize;
            let trunc_ta: String = title_artist.chars().take(max_text_w).collect();

            let line = format!("{}{}", prefix, trunc_ta);
            buf.set_string(inner.left(), y, &line, style);

            let time_x = inner.right().saturating_sub(duration_str.len() as u16);
            buf.set_string(time_x, y, &duration_str, self.theme.muted_text());
        }
        self.theme.render_scrollbar(
            buf,
            area,
            inner.top(),
            inner.height,
            start,
            max_rows,
            self.matching_tracks.len(),
        );

        targets
    }
}

