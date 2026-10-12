use crate::audio::visualizer::VisualizerBridge;
use crate::audio::waveform::WaveformData;
use crate::library::track::Track;
use crate::ui::artwork::{ArtworkCache, ArtworkWidget};
use crate::ui::spectrum::SpectrumWidget;
use crate::ui::theme::Theme;
use crate::ui::waveform_widget::WaveformWidget;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, Widget};
use std::time::Duration;

pub struct NowPlayingClickTarget {
    pub waveform_rect: Rect,
}

pub struct NowPlayingView<'a> {
    pub current_track: Option<&'a Track>,
    pub elapsed: Duration,
    pub total_duration: Duration,
    pub artwork_cache: &'a ArtworkCache,
    pub visualizer: &'a VisualizerBridge,
    pub waveform: Option<&'a WaveformData>,
    pub is_focused: bool,
}

impl<'a> NowPlayingView<'a> {
    pub fn render_with_targets(
        &self,
        area: Rect,
        buf: &mut Buffer,
    ) -> Option<NowPlayingClickTarget> {
        let border_style = if self.is_focused {
            Theme::block_focus_style()
        } else {
            Theme::block_style()
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(" Now Playing & Visualizer ")
            .title_style(Style::default().fg(Theme::OMARCHY_GREEN).add_modifier(Modifier::BOLD));

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.width < 20 || inner.height < 10 {
            return None;
        }

        // Layout:
        // Top section (55% of height): Artwork (square on left) + Track Details (on right)
        // Bottom section (45% of height): Spectrum visualizer + Waveform seekbar

        let top_height = (inner.height as f32 * 0.52) as u16;
        let bottom_height = inner.height.saturating_sub(top_height);

        let top_area = Rect::new(inner.left(), inner.top(), inner.width, top_height);
        let bottom_area = Rect::new(inner.left(), inner.top() + top_height, inner.width, bottom_height);

        // Artwork width: in terminals, character cells have ~1:2 aspect ratio.
        // With halfblocks (2 pixels per character vertically), a square image has width = height.
        // Let's allocate width = (top_height * 2).min(38) to keep it square and prominent!
        let art_width = (top_height * 2).min(inner.width / 2).max(12);
        let art_rect = Rect::new(top_area.left(), top_area.top(), art_width, top_area.height);

        // Render Artwork
        let art_widget = ArtworkWidget::new(self.current_track, self.artwork_cache);
        art_widget.render(art_rect, buf);

        // Render Track Details to the right of artwork
        let details_x = art_rect.right() + 2;
        if details_x < top_area.right() {
            let details_width = top_area.right() - details_x;
            let mut dy = top_area.top() + 1;

            if let Some(track) = self.current_track {
                // Title
                let title_style = Style::default().fg(Theme::OMARCHY_GREEN).add_modifier(Modifier::BOLD);
                let title_str = format!("♫ {}", track.display_title());
                let trunc_title: String = title_str.chars().take(details_width as usize).collect();
                buf.set_string(details_x, dy, &trunc_title, title_style);
                dy += 2;

                // Artist
                let art_str = format!("Artist: {}", track.display_artist());
                let trunc_art: String = art_str.chars().take(details_width as usize).collect();
                buf.set_string(details_x, dy, &trunc_art, Style::default().fg(Theme::TEXT_PRIMARY).add_modifier(Modifier::BOLD));
                dy += 1;

                // Album
                let alb_str = format!("Album:  {}", track.display_album());
                let trunc_alb: String = alb_str.chars().take(details_width as usize).collect();
                buf.set_string(details_x, dy, &trunc_alb, Style::default().fg(Theme::TEXT_SECONDARY));
                dy += 1;

                // Year & Genre
                let mut meta_parts = Vec::new();
                if let Some(y) = track.year {
                    meta_parts.push(format!("Year: {}", y));
                }
                if let Some(g) = &track.genre {
                    meta_parts.push(format!("Genre: {}", g));
                }
                if !meta_parts.is_empty() {
                    buf.set_string(details_x, dy, &meta_parts.join("  •  "), Style::default().fg(Theme::TEXT_MUTED));
                    dy += 1;
                }

                // Audio Quality & Format badge
                dy += 1;
                let quality = track.quality_info();
                let badge = format!(" Hi-Res Audio  │  {} ", quality);
                buf.set_string(details_x, dy, &badge, Style::default().bg(Theme::BG_CARD).fg(Theme::ACCENT_CYAN).add_modifier(Modifier::BOLD));
                dy += 2;

                // File Path
                let path_str = track.path.to_string_lossy();
                let path_disp = format!("Path: {}", path_str);
                let trunc_path: String = path_disp.chars().take(details_width as usize).collect();
                buf.set_string(details_x, dy, &trunc_path, Style::default().fg(Theme::TEXT_MUTED));
            } else {
                buf.set_string(details_x, dy, "No Track Playing", Style::default().fg(Theme::TEXT_MUTED));
            }
        }

        // Bottom section: Split into Real-time Spectrum Analyzer & Waveform
        let wave_h = 3u16.min(bottom_area.height / 3);
        let spec_h = bottom_area.height.saturating_sub(wave_h + 1);

        let spec_rect = Rect::new(bottom_area.left(), bottom_area.top(), bottom_area.width, spec_h);
        let wave_rect = Rect::new(bottom_area.left(), bottom_area.top() + spec_h + 1, bottom_area.width, wave_h);

        // Spectrum Visualizer
        if spec_h >= 2 {
            let spec_widget = SpectrumWidget::new(self.visualizer);
            spec_widget.render(spec_rect, buf);
        }

        // Full Waveform
        if wave_h >= 1 {
            let wave_widget = WaveformWidget::new(self.waveform, self.elapsed, self.total_duration);
            wave_widget.render(wave_rect, buf);
        }

        Some(NowPlayingClickTarget {
            waveform_rect: wave_rect,
        })
    }
}
