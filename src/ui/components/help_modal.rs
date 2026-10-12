use crate::ui::theme::Theme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, Clear, Widget};

pub struct HelpModal;

impl HelpModal {
    pub fn render(area: Rect, buf: &mut Buffer) {
        let width = 64u16.min(area.width.saturating_sub(4));
        let height = 22u16.min(area.height.saturating_sub(2));

        let x = area.left() + (area.width.saturating_sub(width)) / 2;
        let y = area.top() + (area.height.saturating_sub(height)) / 2;
        let modal_rect = Rect::new(x, y, width, height);

        Clear.render(modal_rect, buf);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Theme::block_focus_style())
            .title(" omaflac Keyboard & Mouse Controls ")
            .title_style(Style::default().fg(Theme::OMARCHY_GREEN).add_modifier(Modifier::BOLD));

        let inner = block.inner(modal_rect);
        block.render(modal_rect, buf);

        for my in inner.top()..inner.bottom() {
            for mx in inner.left()..inner.right() {
                buf[(mx, my)].set_bg(Theme::BG_CARD);
            }
        }

        let key_style = Style::default().fg(Theme::ACCENT_CYAN).add_modifier(Modifier::BOLD);
        let desc_style = Style::default().fg(Theme::TEXT_PRIMARY);
        let section_style = Style::default().fg(Theme::OMARCHY_GREEN).add_modifier(Modifier::BOLD);

        let lines = [
            ("NAVIGATION", ""),
            ("  Tab / Shift-Tab", "Switch focus between Sidebar & Main view"),
            ("  j / k / ↑ / ↓  ", "Navigate items or rows"),
            ("  h / l / ← / →  ", "Switch columns in Artists view or seek"),
            ("  1 .. 7         ", "Directly switch views (Songs, Artists, Albums...)"),
            ("", ""),
            ("PLAYBACK", ""),
            ("  Space          ", "Play / Pause playback"),
            ("  Enter          ", "Play currently selected track / album"),
            ("  n / p          ", "Next / Previous track"),
            ("  ← / →          ", "Seek backward / forward 5 seconds"),
            ("  + / -          ", "Increase / decrease volume"),
            ("  m              ", "Mute / unmute audio"),
            ("  r / z          ", "Cycle repeat mode / toggle shuffle"),
            ("", ""),
            ("LIBRARY & PLAYLISTS", ""),
            ("  /              ", "Global search across tracks, artists, albums"),
            ("  a              ", "Add selected track to playlist"),
            ("  f              ", "Toggle favorite status for track"),
            ("", ""),
            ("MOUSE & TOUCH", ""),
            ("  Click anywhere ", "Activate view, play track, seek waveform, set volume"),
            ("  Mouse wheel    ", "Scroll lists, adjust volume when hovering volume bar"),
            ("", ""),
            ("  ? / Esc        ", "Close this help overlay"),
        ];

        let mut cur_y = inner.top();
        for (k, d) in lines {
            if cur_y >= inner.bottom() {
                break;
            }
            if d.is_empty() {
                buf.set_string(inner.left(), cur_y, k, section_style);
            } else {
                buf.set_string(inner.left(), cur_y, k, key_style);
                buf.set_string(inner.left() + 20, cur_y, d, desc_style);
            }
            cur_y += 1;
        }
    }
}
