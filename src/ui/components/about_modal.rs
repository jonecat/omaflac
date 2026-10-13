use crate::ui::theme::OmarchyTheme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Widget};

pub struct AboutModalClickTarget {
    pub modal_rect: Rect,
    pub url_rect: Rect,
    pub close_rect: Rect,
}

pub struct AboutModal;

const LOGO_OMA: [&str; 6] = [
    " ██████╗ ███╗   ███╗ █████╗ ",
    "██╔═══██╗████╗ ████║██╔══██╗",
    "██║   ██║██╔████╔██║███████║",
    "██║   ██║██║╚██╔╝██║██╔══██║",
    "╚██████╔╝██║ ╚═╝ ██║██║  ██║",
    " ╚═════╝ ╚═╝     ╚═╝╚═╝  ╚═╝",
];

const LOGO_FLAC: [&str; 6] = [
    "███████╗██╗      █████╗  ██████╗",
    "██╔════╝██║     ██╔══██╗██╔════╝",
    "█████╗  ██║     ███████║██║     ",
    "██╔══╝  ██║     ██╔══██║██║     ",
    "██║     ███████╗██║  ██║╚██████╗",
    "╚═╝     ╚══════╝╚═╝  ╚═╝ ╚═════╝",
];

impl AboutModal {
    pub fn render_with_targets(
        area: Rect,
        buf: &mut Buffer,
        theme: &OmarchyTheme,
    ) -> AboutModalClickTarget {
        let width = 72u16.min(area.width.saturating_sub(4));
        let height = 23u16.min(area.height.saturating_sub(2));

        let x = area.left() + (area.width.saturating_sub(width)) / 2;
        let y = area.top() + (area.height.saturating_sub(height)) / 2;
        let modal_rect = Rect::new(x, y, width, height);

        Clear.render(modal_rect, buf);

        for my in modal_rect.top()..modal_rect.bottom() {
            for mx in modal_rect.left()..modal_rect.right() {
                buf[(mx, my)].set_bg(theme.background);
            }
        }

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(theme.block_focus_style())
            .style(Style::default().bg(theme.background))
            .title(" ● About OmaFLAC ")
            .title_style(theme.focused_title_style());

        let inner = block.inner(modal_rect);
        block.render(modal_rect, buf);

        let mut url_rect = Rect::default();
        let mut close_rect = Rect::default();

        if inner.width < 20 || inner.height < 8 {
            return AboutModalClickTarget {
                modal_rect,
                url_rect,
                close_rect,
            };
        }

        let mut cur_y = inner.top() + 1;

        // 1. Stylized two-tone ASCII art "OmaFLAC"
        let logo_w = 60u16;
        if inner.width >= logo_w && cur_y + 6 < inner.bottom() {
            let logo_x = inner.left() + (inner.width.saturating_sub(logo_w)) / 2;
            let oma_style = Style::default()
                .fg(theme.accent)
                .bg(theme.background)
                .add_modifier(Modifier::BOLD);
            let flac_style = Style::default()
                .fg(theme.cyan)
                .bg(theme.background)
                .add_modifier(Modifier::BOLD);

            for i in 0..6 {
                let oma_part = LOGO_OMA[i];
                let flac_part = LOGO_FLAC[i];
                let oma_chars = oma_part.chars().count() as u16;
                buf.set_string(logo_x, cur_y + i as u16, oma_part, oma_style);
                buf.set_string(logo_x + oma_chars, cur_y + i as u16, flac_part, flac_style);
            }
            cur_y += 7;
        } else {
            let fallback = "♬  OmaFLAC  ♬";
            let fx = inner.left() + (inner.width.saturating_sub(fallback.chars().count() as u16)) / 2;
            buf.set_string(fx, cur_y, fallback, theme.accent_text());
            cur_y += 2;
        }

        // 2. Subtitle & Version
        if cur_y < inner.bottom() {
            let subtitle = format!(
                "Audiophile Terminal Audio Player  •  {}",
                crate::updater::Updater::display_version()
            );
            let sub_w = subtitle.chars().count() as u16;
            let sub_x = inner.left() + (inner.width.saturating_sub(sub_w)) / 2;
            buf.set_string(sub_x, cur_y, &subtitle, theme.secondary_text());
            cur_y += 1;
        }

        // 3. Supported Codecs
        if cur_y < inner.bottom() {
            let prefix = if inner.width >= 65 {
                "Supported Codecs: "
            } else {
                "Codecs: "
            };
            let codecs = "FLAC • MP3 • OGG • WAV • AIFF • M4A/AAC • ALAC";
            let total_w = (prefix.chars().count() + codecs.chars().count()) as u16;
            let start_x = inner.left() + (inner.width.saturating_sub(total_w)) / 2;
            buf.set_string(start_x, cur_y, prefix, theme.secondary_text());
            buf.set_string(
                start_x + prefix.chars().count() as u16,
                cur_y,
                codecs,
                Style::default().fg(theme.cyan).add_modifier(Modifier::BOLD),
            );
            cur_y += 2;
        }

        // 4. Divider
        if cur_y < inner.bottom() {
            let div_w = inner.width.saturating_sub(10).min(56);
            let div_x = inner.left() + (inner.width.saturating_sub(div_w)) / 2;
            let div_str: String = "─".repeat(div_w as usize);
            buf.set_string(div_x, cur_y, &div_str, theme.block_style());
            cur_y += 2;
        }

        // 5. Created by Jon Lawrence
        if cur_y < inner.bottom() {
            let author = "Created by Jon Lawrence";
            let auth_w = author.chars().count() as u16;
            let auth_x = inner.left() + (inner.width.saturating_sub(auth_w)) / 2;
            buf.set_string(
                auth_x,
                cur_y,
                author,
                Style::default()
                    .fg(theme.bright_foreground)
                    .add_modifier(Modifier::BOLD),
            );
            cur_y += 1;
        }

        // 6. GitHub URL
        if cur_y < inner.bottom() {
            let url = "https://github.com/jonecat/omaflac";
            let url_w = url.chars().count() as u16;
            let url_x = inner.left() + (inner.width.saturating_sub(url_w)) / 2;
            url_rect = Rect::new(url_x, cur_y, url_w, 1);
            buf.set_string(
                url_x,
                cur_y,
                url,
                Style::default()
                    .fg(theme.cyan)
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            );
            cur_y += 2;
        }

        // 7. Close Button Cap (Outlined with rounded borders, no filled background)
        if cur_y + 3 <= inner.bottom() {
            let btn_label = " Close ";
            let btn_w = btn_label.chars().count() as u16 + 4;
            let btn_x = inner.left() + (inner.width.saturating_sub(btn_w)) / 2;
            close_rect = Rect::new(btn_x, cur_y, btn_w, 3);
            for by in cur_y..cur_y + 3 {
                for bx in btn_x..btn_x + btn_w {
                    buf[(bx, by)].set_bg(theme.background);
                }
            }
            let btn_block = Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(theme.block_focus_style())
                .style(Style::default().bg(theme.background));
            btn_block.render(close_rect, buf);
            buf.set_string(
                btn_x + 2,
                cur_y + 1,
                btn_label,
                Style::default()
                    .fg(theme.accent)
                    .bg(theme.background)
                    .add_modifier(Modifier::BOLD),
            );
        } else if cur_y < inner.bottom() {
            let close_str = "[ Close (Esc) ]";
            let cw = close_str.len() as u16;
            let cx = inner.left() + (inner.width.saturating_sub(cw)) / 2;
            close_rect = Rect::new(cx, cur_y, cw, 1);
            buf.set_string(cx, cur_y, close_str, theme.accent_text());
        }

        AboutModalClickTarget {
            modal_rect,
            url_rect,
            close_rect,
        }
    }
}
