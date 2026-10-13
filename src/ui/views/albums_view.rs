use crate::library::playlist::PlaylistsManager;
use crate::library::track::Track;
use crate::library::{AlbumSummary, Library};
use crate::ui::theme::OmarchyTheme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::{Block, BorderType, Borders, Widget};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlbumColumn {
    Albums,
    Songs,
}

pub struct AlbumsViewClickTarget {
    pub rect: Rect,
    pub column: AlbumColumn,
    pub item_index: usize,
}

pub struct AlbumsView<'a> {
    pub library: &'a Library,
    pub albums: &'a [AlbumSummary],
    pub active_column: AlbumColumn,
    pub selected_album_idx: usize,
    pub selected_song_idx: usize,
    pub scroll_album_offset: usize,
    pub scroll_song_offset: usize,
    pub playing_track_id: Option<&'a str>,
    pub playlists_manager: &'a PlaylistsManager,
    pub is_focused: bool,
    pub theme: &'a OmarchyTheme,
}

impl<'a> AlbumsView<'a> {
    pub fn render_with_targets(
        &self,
        area: Rect,
        buf: &mut Buffer,
    ) -> Vec<AlbumsViewClickTarget> {
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

        // Split area into 2 panes: Albums (45%) and Songs (55%)
        let total_w = area.width;
        let col1_w = (total_w as f32 * 0.45) as u16;
        let col2_w = total_w.saturating_sub(col1_w);

        let col1_rect = Rect::new(area.left(), area.top(), col1_w, area.height);
        let col2_rect = Rect::new(area.left() + col1_w, area.top(), col2_w, area.height);

        let current_album = self.albums.get(self.selected_album_idx);
        let songs: Vec<&Track> = if let Some(alb) = current_album {
            self.library.tracks_by_album(&alb.artist, &alb.album)
        } else {
            Vec::new()
        };

        // Render Pane 1: Albums List
        targets.extend(self.render_albums_pane(col1_rect, buf));

        // Render Pane 2: Songs within the selected album
        targets.extend(self.render_songs_pane(col2_rect, &songs, current_album, buf));

        targets
    }

    fn render_albums_pane(
        &self,
        area: Rect,
        buf: &mut Buffer,
    ) -> Vec<AlbumsViewClickTarget> {
        let mut targets = Vec::new();
        let is_col_focused = self.is_focused && self.active_column == AlbumColumn::Albums;

        let (b_style, t_style, title_text) = if is_col_focused {
            (
                self.theme.block_focus_style(),
                self.theme.focused_title_style(),
                format!(" ● Albums ({}) ", self.albums.len()),
            )
        } else {
            (
                self.theme.block_style(),
                self.theme.unfocused_title_style(),
                format!(" Albums ({}) ", self.albums.len()),
            )
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(b_style)
            .style(Style::default().bg(self.theme.background))
            .title(title_text)
            .title_style(t_style);

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.width < 4 || inner.height < 1 {
            return targets;
        }

        if self.albums.is_empty() {
            buf.set_string(
                inner.left() + 1,
                inner.top() + 1,
                "No albums found in library",
                self.theme.muted_text(),
            );
            return targets;
        }

        let top_pad = 1u16.min(inner.height);
        let content_top = inner.top() + top_pad;
        let avail_h = inner.height.saturating_sub(top_pad);
        let max_rows = avail_h as usize;
        let start = self.scroll_album_offset.min(self.albums.len().saturating_sub(max_rows));
        let visible_albums = &self.albums[start..];

        for (i, album) in visible_albums.iter().enumerate().take(max_rows) {
            let actual_idx = start + i;
            let y = content_top + i as u16;
            let is_selected = actual_idx == self.selected_album_idx;

            let row_rect = Rect::new(inner.left(), y, inner.width, 1);
            targets.push(AlbumsViewClickTarget {
                rect: row_rect,
                column: AlbumColumn::Albums,
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

            let yr_str = album.year.map(|y| format!("{} • ", y)).unwrap_or_default();
            let meta = format!(" — {} ({}{} trk)", album.artist, yr_str, album.track_count);
            let avail_w = inner.width.saturating_sub(1) as usize;
            let meta_chars = meta.chars().count();
            let title_avail = avail_w.saturating_sub(meta_chars);
            let trunc_title: String = album.album.chars().take(title_avail.max(6)).collect();

            let full_line = format!(" {}{}", trunc_title, meta);
            let trunc_line: String = full_line.chars().take(inner.width as usize).collect();
            buf.set_string(inner.left(), y, &trunc_line, style);
        }
        self.theme.render_scrollbar(
            buf,
            area,
            content_top,
            avail_h,
            start,
            max_rows,
            self.albums.len(),
        );

        targets
    }

    fn render_songs_pane(
        &self,
        area: Rect,
        songs: &[&Track],
        current_album: Option<&AlbumSummary>,
        buf: &mut Buffer,
    ) -> Vec<AlbumsViewClickTarget> {
        let mut targets = Vec::new();
        let is_col_focused = self.is_focused && self.active_column == AlbumColumn::Songs;

        let album_name = current_album
            .map(|a| a.album.as_str())
            .unwrap_or("No Album Selected");

        let (b_style, t_style, title_text) = if is_col_focused {
            (
                self.theme.block_focus_style(),
                self.theme.focused_title_style(),
                format!(" ● {} — Songs ({}) ", album_name, songs.len()),
            )
        } else {
            (
                self.theme.block_style(),
                self.theme.unfocused_title_style(),
                format!(" {} — Songs ({}) ", album_name, songs.len()),
            )
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(b_style)
            .style(Style::default().bg(self.theme.background))
            .title(title_text)
            .title_style(t_style);

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.width < 4 || inner.height < 1 {
            return targets;
        }

        if songs.is_empty() {
            buf.set_string(
                inner.left() + 1,
                inner.top() + 1,
                "No songs in selected album",
                self.theme.muted_text(),
            );
            return targets;
        }

        let top_pad = 1u16.min(inner.height);
        let content_top = inner.top() + top_pad;
        let avail_h = inner.height.saturating_sub(top_pad);
        let max_rows = avail_h as usize;
        let start = self.scroll_song_offset.min(songs.len().saturating_sub(max_rows));
        let visible_songs = &songs[start..];

        for (i, song) in visible_songs.iter().enumerate().take(max_rows) {
            let actual_idx = start + i;
            let y = content_top + i as u16;
            let is_sel = actual_idx == self.selected_song_idx;
            let is_playing = self.playing_track_id == Some(&song.id);
            let is_fav = self.playlists_manager.is_favorite(&song.id);

            let row_rect = Rect::new(inner.left(), y, inner.width, 1);
            targets.push(AlbumsViewClickTarget {
                rect: row_rect,
                column: AlbumColumn::Songs,
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
            let trk_num = song
                .track_number
                .map(|n| format!("{:02}. ", n))
                .unwrap_or_else(|| format!("{:02}. ", actual_idx + 1));
            let duration_str = song.formatted_duration();

            let prefix = format!(" {} {}{}", play_prefix, fav_prefix, trk_num);
            let prefix_chars = prefix.chars().count() as u16;
            let title_avail = inner
                .width
                .saturating_sub(prefix_chars + duration_str.len() as u16 + 3)
                as usize;
            let display_title = if let Some(alb) = current_album {
                if !song.display_artist().eq_ignore_ascii_case(&alb.artist) {
                    format!("{} — {}", song.display_title(), song.display_artist())
                } else {
                    song.display_title().to_string()
                }
            } else {
                song.display_title().to_string()
            };
            let title: String = display_title.chars().take(title_avail).collect();

            let line = format!("{}{}", prefix, title);
            buf.set_string(inner.left(), y, &line, style);

            let time_x = inner.right().saturating_sub(duration_str.len() as u16 + 1);
            buf.set_string(time_x, y, &duration_str, self.theme.muted_text());
        }
        self.theme.render_scrollbar(
            buf,
            area,
            content_top,
            avail_h,
            start,
            max_rows,
            songs.len(),
        );

        targets
    }
}

