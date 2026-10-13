use crate::audio::visualizer::VisualizerBridge;
use crate::ui::theme::OmarchyTheme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::Widget;

const BLOCK_CHARS: &[&str] = &[" ", "▁", "▂", "▃", "▄", "▅", "▆", "▇", "█"];

pub struct SpectrumWidget<'a> {
    visualizer: &'a VisualizerBridge,
    theme: &'a OmarchyTheme,
    show_peaks: bool,
}

impl<'a> SpectrumWidget<'a> {
    pub fn new(visualizer: &'a VisualizerBridge, theme: &'a OmarchyTheme) -> Self {
        Self {
            visualizer,
            theme,
            show_peaks: true,
        }
    }
}

impl<'a> Widget for SpectrumWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 4 || area.height < 2 {
            return;
        }

        let (bands, peaks) = self.visualizer.get_bands();
        let num_bands = bands.len();
        if num_bands == 0 {
            return;
        }

        let width = area.width as usize;
        let height = area.height as usize;

        // Scale bar width with viewport width while spanning the full width of the viewport
        let bar_width = if width >= 180 {
            3
        } else if width >= 96 {
            2
        } else {
            1
        };
        let spacing = if width >= 48 { 1 } else { 0 };
        let total_per_bar = bar_width + spacing;
        let displayed_bars = ((width + spacing) / total_per_bar).max(1);

        let total_visualizer_width = displayed_bars * total_per_bar - spacing;
        let start_x = area.left() + ((width.saturating_sub(total_visualizer_width)) / 2) as u16;

        for b in 0..displayed_bars {
            let t = if displayed_bars > 1 {
                b as f32 / (displayed_bars - 1) as f32
            } else {
                0.0
            };
            let pos = t * (num_bands - 1) as f32;
            let idx0 = pos.floor() as usize;
            let idx1 = (idx0 + 1).min(num_bands - 1);
            let frac = pos - idx0 as f32;

            let val = (bands[idx0] * (1.0 - frac) + bands[idx1] * frac).clamp(0.0, 1.0);
            let peak_val = (peaks[idx0] * (1.0 - frac) + peaks[idx1] * frac).clamp(0.0, 1.0);

            let total_steps = (height * 8) as f32;
            let filled_steps = (val * total_steps).round() as usize;
            let peak_step = (peak_val * total_steps).round() as usize;

            let bar_x_start = start_x + (b * total_per_bar) as u16;

            for row in 0..height {
                let y_from_bottom = row;
                let term_y = area.bottom() - 1 - row as u16;

                let row_step_start = y_from_bottom * 8;
                let row_step_end = row_step_start + 8;

                // Color gradient using Omarchy theme palette
                let row_pct = y_from_bottom as f32 / height as f32;
                let bar_color = if row_pct < 0.45 {
                    self.theme.accent
                } else if row_pct < 0.75 {
                    self.theme.cyan
                } else if row_pct < 0.9 {
                    self.theme.yellow
                } else {
                    self.theme.red
                };

                let (symbol, style) = if filled_steps >= row_step_end {
                    (BLOCK_CHARS[8], Style::default().fg(bar_color))
                } else if filled_steps > row_step_start {
                    let sub_step = filled_steps - row_step_start;
                    (BLOCK_CHARS[sub_step], Style::default().fg(bar_color))
                } else if self.show_peaks && peak_step > 0 && peak_step >= row_step_start && peak_step < row_step_end {
                    ("▔", Style::default().fg(self.theme.bright_foreground))
                } else if y_from_bottom == 0 {
                    ("▁", Style::default().fg(self.theme.muted))
                } else {
                    (" ", Style::default().fg(bar_color))
                };

                for w in 0..bar_width {
                    let x = bar_x_start + w as u16;
                    if x < area.right() && term_y < area.bottom() {
                        let cell = &mut buf[(x, term_y)];
                        cell.set_symbol(symbol);
                        cell.set_style(style);
                    }
                }
            }
        }
    }
}
