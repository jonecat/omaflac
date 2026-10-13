use crate::audio::waveform::WaveformData;
use crate::ui::theme::OmarchyTheme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::Widget;
use std::time::Duration;

pub struct WaveformWidget<'a> {
    waveform: Option<&'a WaveformData>,
    elapsed: Duration,
    total_duration: Duration,
    theme: &'a OmarchyTheme,
}

impl<'a> WaveformWidget<'a> {
    pub fn new(
        waveform: Option<&'a WaveformData>,
        elapsed: Duration,
        total_duration: Duration,
        theme: &'a OmarchyTheme,
    ) -> Self {
        Self {
            waveform,
            elapsed,
            total_duration,
            theme,
        }
    }

    fn sample_peak(peaks: &[f32], t: f32) -> f32 {
        if peaks.is_empty() {
            return 0.2;
        }
        let pos = t.clamp(0.0, 1.0) * (peaks.len() - 1) as f32;
        let idx0 = pos.floor() as usize;
        let idx1 = (idx0 + 1).min(peaks.len() - 1);
        let frac = pos - idx0 as f32;
        peaks[idx0] * (1.0 - frac) + peaks[idx1] * frac
    }
}

impl<'a> Widget for WaveformWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 4 || area.height == 0 {
            return;
        }

        let total_secs = self.total_duration.as_secs_f32().max(0.001);
        let elapsed_secs = self.elapsed.as_secs_f32();
        let progress = (elapsed_secs / total_secs).clamp(0.0, 1.0);

        let width = area.width as usize;
        let height = area.height as usize;
        let head_col = ((progress * area.width as f32).round() as u16).min(area.width.saturating_sub(1));

        let peaks_ref: Vec<f32>;
        let peaks: &[f32] = if let Some(wf) = self.waveform {
            &wf.peaks
        } else {
            peaks_ref = vec![0.3; width * 2];
            &peaks_ref
        };

        if height == 1 {
            // Normal line with playhead for single-row progress bar
            for col in 0..area.width {
                let x = area.left() + col;
                let is_played = col < head_col;
                let is_head = col == head_col;

                let (symbol, style) = if total_secs <= 0.001 {
                    ("─", Style::default().fg(self.theme.muted))
                } else if is_head {
                    ("●", Style::default().fg(self.theme.accent).add_modifier(Modifier::BOLD))
                } else if is_played {
                    ("━", Style::default().fg(self.theme.accent))
                } else {
                    ("─", Style::default().fg(self.theme.muted))
                };

                let cell = &mut buf[(x, area.top())];
                cell.set_symbol(symbol);
                cell.set_style(style);
            }
            return;
        }

        // High-fidelity btop-style 2x4 sub-pixel Braille canvas rasterizer:
        // Every time slice (px) renders a solid, consistent vertical column of Braille dots
        // from y_top to y_bot so dot density never varies or clumps.
        let canvas_w = width * 2;
        let canvas_h = height * 4;
        let center_top = (canvas_h / 2).saturating_sub(1) as isize;
        let center_bot = (canvas_h / 2) as isize;
        let max_half = center_top.max(1) as f32;

        // Boolean sub-pixel raster grid
        let mut grid = vec![false; canvas_w * canvas_h];

        for px in 0..canvas_w {
            let t0 = px as f32 / (canvas_w as f32);
            let t1 = (px + 1) as f32 / (canvas_w as f32);
            let t_mid = (t0 + t1) * 0.5;
            let amp = Self::sample_peak(peaks, t0)
                .max(Self::sample_peak(peaks, t_mid))
                .max(Self::sample_peak(peaks, t1))
                .clamp(0.0, 1.0);

            let half = (amp * max_half).round() as isize;
            let y_top = (center_top - half).clamp(0, (canvas_h - 1) as isize) as usize;
            let y_bot = (center_bot + half).clamp(0, (canvas_h - 1) as isize) as usize;

            for y in y_top..=y_bot {
                grid[y * canvas_w + px] = true;
            }
        }

        // Render Braille character cells
        for row in 0..height {
            let y = area.top() + row as u16;
            for col in 0..width {
                let x = area.left() + col as u16;

                let mut mask = 0u8;
                for dx in 0..2 {
                    let px = col * 2 + dx;
                    for dy in 0..4 {
                        let py = row * 4 + dy;
                        if grid[py * canvas_w + px] {
                            let bit = match (dx, dy) {
                                (0, 0) => 0x01,
                                (0, 1) => 0x02,
                                (0, 2) => 0x04,
                                (0, 3) => 0x40,
                                (1, 0) => 0x08,
                                (1, 1) => 0x10,
                                (1, 2) => 0x20,
                                (1, 3) => 0x80,
                                _ => 0,
                            };
                            mask |= bit;
                        }
                    }
                }

                let is_played = (col as u16) < head_col;
                let is_head = (col as u16) == head_col;

                let style = if is_head {
                    Style::default().fg(self.theme.bright_yellow).add_modifier(Modifier::BOLD)
                } else if is_played {
                    Style::default().fg(self.theme.accent)
                } else {
                    Style::default().fg(self.theme.muted)
                };

                let cell = &mut buf[(x, y)];
                if mask != 0 {
                    if let Some(ch) = char::from_u32(0x2800 + mask as u32) {
                        let mut buf_str = [0u8; 4];
                        let s = ch.encode_utf8(&mut buf_str);
                        cell.set_symbol(s);
                        cell.set_style(style);
                    }
                } else {
                    cell.set_symbol(" ");
                }
            }
        }
    }
}
