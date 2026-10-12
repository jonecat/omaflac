use crate::library::AlbumSummary;
use crate::ui::theme::Theme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, Widget};

pub struct AlbumsViewClickTarget {
    pub rect: Rect,
    pub album_index: usize,
}

pub struct AlbumsView<'a> {
    pub albums: &'a [AlbumSummary],
    pub selected_index: usize,
    pub scroll_offset: usize,
    pub is_focused: bool,
}

impl<'a> AlbumsView<'a> {
    pub fn render_with_targets(
        &self,
        area: Rect,
        buf: &mut Buffer,
    ) -> Vec<AlbumsViewClickTarget> {
        let mut targets = Vec::new();

        let border_style = if self.is_focused {
            Theme::block_focus_style()
        } else {
            Theme::block_style()
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(format!(" Albums ({}) ", self.albums.len()))
            .title_style(Style::default().fg(Theme::OMARCHY_GREEN).add_modifier(Modifier::BOLD));

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.width < 10 || inner.height < 2 {
            return targets;
        }

        let y_hdr = inner.top();
        let hdr_style = Style::default().fg(Theme::TEXT_MUTED).add_modifier(Modifier::BOLD);

        let col_art_w = 4u16;
        let col_yr_w = 6u16;
        let col_cnt_w = 10u16;
        let col_dur_w = 8u16;

        let remaining = inner.width.saturating_sub(col_art_w + col_yr_w + col_cnt_w + col_dur_w + 3);
        let col_alb_w = (remaining as f32 * 0.55) as u16;
        let col_artist_w = remaining.saturating_sub(col_alb_w);

        let col_art_x = inner.left();
        let col_alb_x = col_art_x + col_art_w;
        let col_artist_x = col_alb_x + col_alb_w + 1;
        let col_yr_x = col_artist_x + col_artist_w + 1;
        let col_cnt_x = col_yr_x + col_yr_w + 1;
        let col_dur_x = col_cnt_x + col_cnt_w + 1;

        buf.set_string(col_art_x, y_hdr, "ART", hdr_style);
        buf.set_string(col_alb_x, y_hdr, "ALBUM", hdr_style);
        buf.set_string(col_artist_x, y_hdr, "ARTIST", hdr_style);
        buf.set_string(col_yr_x, y_hdr, "YEAR", hdr_style);
        buf.set_string(col_cnt_x, y_hdr, "TRACKS", hdr_style);
        buf.set_string(col_dur_x, y_hdr, "DURATION", hdr_style);

        let rows_avail = inner.height.saturating_sub(1) as usize;
        let start_idx = self.scroll_offset.min(self.albums.len());
        let end_idx = (start_idx + rows_avail).min(self.albums.len());

        for (i, album) in self.albums[start_idx..end_idx].iter().enumerate() {
            let actual_idx = start_idx + i;
            let y = y_hdr + 1 + i as u16;
            let is_sel = actual_idx == self.selected_index;

            let row_rect = Rect::new(inner.left(), y, inner.width, 1);
            targets.push(AlbumsViewClickTarget {
                rect: row_rect,
                album_index: actual_idx,
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

            let art_badge = if album.has_cover_art { "🖼 " } else { "💿 " };
            buf.set_string(col_art_x, y, art_badge, style);

            let alb_str: String = album.album.chars().take(col_alb_w as usize).collect();
            buf.set_string(col_alb_x, y, &alb_str, style);

            let artist_style = if is_sel { style } else { Style::default().fg(Theme::TEXT_SECONDARY) };
            let artist_str: String = album.artist.chars().take(col_artist_w as usize).collect();
            buf.set_string(col_artist_x, y, &artist_str, artist_style);

            let yr_str = album.year.map(|y| y.to_string()).unwrap_or_else(|| "—".into());
            buf.set_string(col_yr_x, y, &yr_str, Style::default().fg(Theme::TEXT_MUTED));

            let cnt_str = format!("{} songs", album.track_count);
            buf.set_string(col_cnt_x, y, &cnt_str, Style::default().fg(Theme::TEXT_SECONDARY));

            let dur_str = album.formatted_duration();
            buf.set_string(col_dur_x, y, &dur_str, Style::default().fg(Theme::TEXT_MUTED));
        }

        targets
    }
}
