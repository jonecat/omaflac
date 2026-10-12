use crate::audio::waveform::WaveformData;
use crate::ui::theme::Theme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::widgets::Widget;
use std::time::Duration;

pub struct WaveformWidget<'a> {
    waveform: Option<&'a WaveformData>,
    elapsed: Duration,
    total_duration: Duration,
}

impl<'a> WaveformWidget<'a> {
    pub fn new(
        waveform: Option<&'a WaveformData>,
        elapsed: Duration,
        total_duration: Duration,
    ) -> Self {
        Self {
            waveform,
            elapsed,
            total_duration,
        }
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

        // Get or generate peak slices
        let peaks_ref: Vec<f32>;
        let peaks: &[f32] = if let Some(wf) = self.waveform {
            &wf.peaks
        } else {
            // Placeholder flat bars
            peaks_ref = vec![0.3; width];
            &peaks_ref
        };

        let num_peaks = peaks.len().max(1);

        for col in 0..area.width {
            let x = area.left() + col;
            let peak_idx = (col as usize * num_peaks) / width;
            let amp = peaks.get(peak_idx).copied().unwrap_or(0.3).clamp(0.05, 1.0);

            let is_played = col < head_col;
            let is_head = col == head_col;

            let color = if is_head {
                Color::Rgb(255, 255, 255)
            } else if is_played {
                Theme::OMARCHY_GREEN
            } else {
                Color::Rgb(65, 72, 104)
            };

            if height == 1 {
                // Single line waveform
                let block_chars = [" ", " ", "▂", "▃", "▄", "▅", "▆", "▇", "█"];
                let char_idx = (amp * 8.0).round() as usize;
                let symbol = if is_head {
                    "█"
                } else {
                    block_chars[char_idx.clamp(1, 8)]
                };
                let cell = &mut buf[(x, area.top())];
                cell.set_symbol(symbol);
                cell.set_style(Style::default().fg(color));
            } else {
                // Multi-line symmetrical vertical waveform
                let center_y = height as f32 / 2.0;
                let half_height = (amp * (height as f32 / 2.0)).max(0.6);

                for row in 0..height {
                    let y = area.top() + row as u16;
                    let dist = ((row as f32 + 0.5) - center_y).abs();

                    let symbol = if is_head {
                        "█"
                    } else if dist <= half_height {
                        "█"
                    } else if dist <= half_height + 0.4 {
                        if (row as f32) < center_y { "▄" } else { "▀" }
                    } else {
                        " "
                    };

                    if symbol != " " {
                        let cell = &mut buf[(x, y)];
                        cell.set_symbol(symbol);
                        cell.set_style(Style::default().fg(color));
                    }
                }
            }
        }
    }
}
