use crate::ui::theme::OmarchyTheme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Widget};

pub struct HelpModal;

impl HelpModal {
    pub fn render(area: Rect, buf: &mut Buffer, theme: &OmarchyTheme) {
        let width = 76u16.min(area.width.saturating_sub(2));
        let height = 24u16.min(area.height.saturating_sub(2));

        let x = area.left() + (area.width.saturating_sub(width)) / 2;
        let y = area.top() + (area.height.saturating_sub(height)) / 2;
        let modal_rect = Rect::new(x, y, width, height);

        Clear.render(modal_rect, buf);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(theme.block_focus_style())
            .style(Style::default().bg(theme.lighter_background))
            .title(" OmaFLAC Controls ")
            .title_style(theme.accent_text());

        let inner = block.inner(modal_rect);

        for my in modal_rect.top()..modal_rect.bottom() {
            for mx in modal_rect.left()..modal_rect.right() {
                buf[(mx, my)].set_bg(theme.lighter_background);
            }
        }
        block.render(modal_rect, buf);

        let key_style = Style::default().fg(theme.cyan).add_modifier(Modifier::BOLD);
        let desc_style = theme.primary_text();
        let section_style = theme.accent_text();

        let lines = [
            ("NAVIGATION", ""),
            ("  Tab / Shift-Tab", "Cycle focus: Sidebar ↔ Main ↔ Player"),
            ("  j / k / ↑ / ↓  ", "Navigate items or adjust volume"),
            ("  h / l / ← / →  ", "Switch columns or Prev/Next track"),
            ("  1 .. 6         ", "Directly switch views (Artists, Albums...)"),
            ("", ""),
            ("PLAYBACK", ""),
            ("  Space / Enter  ", "Play / Pause playback"),
            ("  n / p          ", "Next / Previous track"),
            ("  + / -          ", "Adjust volume (m: Mute)"),
            ("  r / z          ", "Repeat mode / Toggle shuffle"),
            ("", ""),
            ("LIBRARY & PLAYLISTS", ""),
            ("  /              ", "Global search across library"),
            ("  a              ", "Add selected track to playlist"),
            ("  f              ", "Toggle favorite status"),
            ("  r / D          ", "Rename / Delete playlist (in Playlists)"),
            ("  d              ", "Remove track from playlist"),
            ("", ""),
            ("MOUSE & SHORTCUTS", ""),
            ("  Click anywhere ", "Activate view, play, seek, volume"),
            ("  Scroll wheel   ", "Scroll lists or adjust volume"),
            ("  ? / Esc        ", "Close help overlay"),
        ];

        let mut cur_y = inner.top() + 1;
        for (k, d) in lines {
            if cur_y >= inner.bottom() {
                break;
            }
            if d.is_empty() {
                let max_w = inner.right().saturating_sub(inner.left()) as usize;
                let trunc_k: String = k.chars().take(max_w).collect();
                buf.set_string(inner.left(), cur_y, &trunc_k, section_style);
            } else {
                let key_col_w = 19u16;
                let key_w = key_col_w.min(inner.width) as usize;
                let trunc_k: String = k.chars().take(key_w).collect();
                buf.set_string(inner.left(), cur_y, &trunc_k, key_style);

                let desc_x = (inner.left() + key_col_w).min(inner.right());
                let max_desc_w = inner.right().saturating_sub(desc_x) as usize;
                let trunc_d: String = d.chars().take(max_desc_w).collect();
                buf.set_string(desc_x, cur_y, &trunc_d, desc_style);
            }
            cur_y += 1;
        }
    }
}
