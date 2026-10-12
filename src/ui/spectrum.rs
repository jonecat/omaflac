use crate::audio::visualizer::VisualizerBridge;
use crate::ui::theme::Theme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::Widget;

const BLOCK_CHARS: &[&str] = &[" ", " ", "▂", "▃", "▄", "▅", "▆", "▇", "█"];

pub struct SpectrumWidget<'a> {
    visualizer: &'a VisualizerBridge,
    show_peaks: bool,
}

impl<'a> SpectrumWidget<'a> {
    pub fn new(visualizer: &'a VisualizerBridge) -> Self {
        Self {
            visualizer,
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

        // Determine bar spacing: 1 char per bar or 2 chars per bar
        let bar_width = if width >= num_bands * 2 { 2 } else { 1 };
        let spacing = if width >= num_bands * (bar_width + 1) { 1 } else { 0 };
        let total_per_bar = bar_width + spacing;
        let displayed_bars = (width / total_per_bar).min(num_bands);

        // Center visualizer within area
        let total_visualizer_width = displayed_bars * total_per_bar;
        let start_x = area.left() + ((width.saturating_sub(total_visualizer_width)) / 2) as u16;

        for b in 0..displayed_bars {
            // Map b to bands slice
            let band_idx = (b * num_bands) / displayed_bars;
            let val = bands[band_idx].clamp(0.0, 1.0);
            let peak_val = peaks[band_idx].clamp(0.0, 1.0);

            let total_steps = (height * 8) as f32;
            let filled_steps = (val * total_steps).round() as usize;
            let peak_step = (peak_val * total_steps).round() as usize;

            let bar_x_start = start_x + (b * total_per_bar) as u16;

            for row in 0..height {
                // y from bottom (0 = bottom, height-1 = top)
                let y_from_bottom = row;
                let term_y = area.bottom() - 1 - row as u16;

                let row_step_start = y_from_bottom * 8;
                let row_step_end = row_step_start + 8;

                // Color calculation for row
                let row_pct = y_from_bottom as f32 / height as f32;
                let bar_color = if row_pct < 0.4 {
                    Theme::OMARCHY_GREEN
                } else if row_pct < 0.75 {
                    Theme::ACCENT_CYAN
                } else if row_pct < 0.9 {
                    Theme::ACCENT_YELLOW
                } else {
                    Theme::ACCENT_RED
                };

                let symbol = if filled_steps >= row_step_end {
                    BLOCK_CHARS[8] // Full block
                } else if filled_steps > row_step_start {
                    let sub_step = filled_steps - row_step_start;
                    BLOCK_CHARS[sub_step]
                } else if self.show_peaks && peak_step >= row_step_start && peak_step < row_step_end {
                    "▔" // Peak marker
                } else {
                    " "
                };

                let style = if symbol == "▔" {
                    Style::default().fg(Theme::TEXT_PRIMARY)
                } else {
                    Style::default().fg(bar_color)
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
