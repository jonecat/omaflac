use crate::audio::visualizer::VisualizerBridge;
use crate::audio::waveform::WaveformData;
use crate::library::track::Track;
use crate::lyrics::LyricsStatus;
use crate::ui::artwork::{ArtworkCache, ArtworkWidget};
use crate::ui::spectrum::SpectrumWidget;
use crate::ui::theme::OmarchyTheme;
use crate::ui::waveform_widget::WaveformWidget;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, BorderType, Borders, Widget};
use std::time::Duration;

pub struct NowPlayingClickTarget {
    pub waveform_rect: Rect,
    pub lyrics_rect: Option<Rect>,
}

pub struct NowPlayingView<'a> {
    pub current_track: Option<&'a Track>,
    pub elapsed: Duration,
    pub total_duration: Duration,
    pub artwork_cache: &'a ArtworkCache,
    pub visualizer: &'a VisualizerBridge,
    pub waveform: Option<&'a WaveformData>,
    pub lyrics_status: Option<&'a LyricsStatus>,
    pub show_lyrics: bool,
    pub is_focused: bool,
    pub theme: &'a OmarchyTheme,
}

impl<'a> NowPlayingView<'a> {
    fn format_lrc_time(d: Duration) -> String {
        let total_secs = d.as_secs();
        let mins = total_secs / 60;
        let secs = total_secs % 60;
        format!("{:02}:{:02}", mins, secs)
    }

    fn render_lyrics_card(&self, area: Rect, buf: &mut Buffer) {
        if area.width < 10 || area.height < 1 {
            return;
        }

        // Fill background
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                buf[(x, y)].set_bg(self.theme.background);
            }
        }

        let inner = if area.height >= 4 {
            let lyrics_block = Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(self.theme.block_style())
                .style(Style::default().bg(self.theme.background))
                .title(" ♫ Lyrics ")
                .title_style(self.theme.accent_text());

            let inn = lyrics_block.inner(area);
            lyrics_block.render(area, buf);
            inn
        } else {
            area
        };

        if inner.width < 6 || inner.height < 1 {
            return;
        }

        match self.lyrics_status {
            Some(LyricsStatus::Loading) => {
                let ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis();
                let spinner = crate::ui::artwork::SPINNER_CHARS
                    [(ms / 90) as usize % crate::ui::artwork::SPINNER_CHARS.len()];
                let msg = format!("{} Fetching lyrics from LRCLIB...", spinner);
                let trunc: String = msg.chars().take(inner.width as usize).collect();
                let x = inner.left() + (inner.width.saturating_sub(trunc.chars().count() as u16)) / 2;
                let y = inner.top() + inner.height / 2;
                buf.set_string(
                    x,
                    y,
                    &trunc,
                    Style::default().fg(self.theme.cyan).add_modifier(Modifier::BOLD),
                );
            }
            Some(LyricsStatus::Instrumental) => {
                let msg = "♪   Instrumental Track   ♪";
                let trunc: String = msg.chars().take(inner.width as usize).collect();
                let x = inner.left() + (inner.width.saturating_sub(trunc.chars().count() as u16)) / 2;
                let y = inner.top() + inner.height / 2;
                buf.set_string(
                    x,
                    y,
                    &trunc,
                    Style::default().fg(self.theme.magenta).add_modifier(Modifier::BOLD),
                );
            }
            Some(LyricsStatus::NotFound) => {
                let msg = "○  No lyrics available  ○";
                let trunc: String = msg.chars().take(inner.width as usize).collect();
                let x = inner.left() + (inner.width.saturating_sub(trunc.chars().count() as u16)) / 2;
                let y = inner.top() + inner.height / 2;
                buf.set_string(x, y, &trunc, self.theme.muted_text());
            }
            Some(LyricsStatus::Found(synced)) => {
                let window = synced.get_window(self.elapsed);

                // Row 0: Previous line (if height >= 3)
                if inner.height >= 3 {
                    if let Some(prev) = window.prev {
                        let prev_str = format!("[{}]  {}", Self::format_lrc_time(prev.timestamp), prev.text);
                        let trunc: String = prev_str.chars().take(inner.width as usize).collect();
                        let x = inner.left() + (inner.width.saturating_sub(trunc.chars().count() as u16)) / 2;
                        buf.set_string(x, inner.top(), &trunc, self.theme.muted_text());
                    }
                }

                // Row 1: Current line (middle row)
                let curr_y = if inner.height >= 3 {
                    inner.top() + 1
                } else {
                    inner.top()
                };

                if window.is_instrumental_gap {
                    let curr_str = "♪   Instrumental Break   ♪";
                    let trunc: String = curr_str.chars().take(inner.width as usize).collect();
                    let x = inner.left() + (inner.width.saturating_sub(trunc.chars().count() as u16)) / 2;
                    buf.set_string(
                        x,
                        curr_y,
                        &trunc,
                        Style::default().fg(self.theme.cyan).add_modifier(Modifier::BOLD),
                    );
                } else if let Some(curr) = window.curr {
                    let curr_str = format!("♫  [{}]  {}  ♫", Self::format_lrc_time(curr.timestamp), curr.text);
                    let trunc: String = curr_str.chars().take(inner.width as usize).collect();
                    let x = inner.left() + (inner.width.saturating_sub(trunc.chars().count() as u16)) / 2;
                    buf.set_string(
                        x,
                        curr_y,
                        &trunc,
                        self.theme.accent_text().add_modifier(Modifier::BOLD),
                    );
                } else {
                    let curr_str = "♫   ...   ♫";
                    let x = inner.left() + (inner.width.saturating_sub(curr_str.chars().count() as u16)) / 2;
                    buf.set_string(x, curr_y, curr_str, self.theme.muted_text());
                }

                // Row 2: Next line (if height >= 3)
                if inner.height >= 3 {
                    if let Some(next) = window.next {
                        let next_str = format!("[{}]  {}", Self::format_lrc_time(next.timestamp), next.text);
                        let trunc: String = next_str.chars().take(inner.width as usize).collect();
                        let x = inner.left() + (inner.width.saturating_sub(trunc.chars().count() as u16)) / 2;
                        buf.set_string(x, inner.top() + 2, &trunc, self.theme.secondary_text());
                    }
                }
            }
            _ => {
                let msg = "○  No track playing  ○";
                let trunc: String = msg.chars().take(inner.width as usize).collect();
                let x = inner.left() + (inner.width.saturating_sub(trunc.chars().count() as u16)) / 2;
                let y = inner.top() + inner.height / 2;
                buf.set_string(x, y, &trunc, self.theme.muted_text());
            }
        }
    }

    pub fn render_with_targets(
        &self,
        area: Rect,
        buf: &mut Buffer,
    ) -> Option<NowPlayingClickTarget> {
        // Fill background
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                buf[(x, y)].set_bg(self.theme.background);
            }
        }

        let lyrics_badge = if self.show_lyrics { " • Lyrics [L] " } else { " [L: Lyrics] " };
        let (border_style, title_style, title_text) = if self.is_focused {
            (
                self.theme.block_focus_style(),
                self.theme.focused_title_style(),
                format!(" ● Visualizer{} ", lyrics_badge),
            )
        } else {
            (
                self.theme.block_style(),
                self.theme.unfocused_title_style(),
                format!(" Visualizer{} ", lyrics_badge),
            )
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(border_style)
            .style(Style::default().bg(self.theme.background))
            .title(title_text)
            .title_style(title_style);

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.width < 20 || inner.height < 10 {
            return None;
        }

        // Layout:
        // Top section: Artwork + Track Details (with top margin)
        // Bottom section: Synchronized Lyrics + Real-time Spectrum visualizer + Waveform
        let top_margin = 1u16.min(inner.height);
        let top_height = (inner.height * 40 / 100).max(6).min(14);
        let bottom_height = inner.height.saturating_sub(top_height);

        let top_area = Rect::new(
            inner.left(),
            inner.top() + top_margin,
            inner.width,
            top_height.saturating_sub(top_margin),
        );
        let bottom_area = Rect::new(inner.left(), inner.top() + top_height, inner.width, bottom_height);

        // Fixed artwork display size:
        let target_art_h = 14u16.min(top_area.height.saturating_sub(1));
        let max_art_w = top_area.width.saturating_sub(24);
        let art_h = (target_art_h).min(max_art_w / 2).max(3);
        let art_w = art_h * 2; // Strictly 2:1 character cell aspect ratio (1:1 pixel square)

        let art_y = top_area.top() + (top_area.height.saturating_sub(art_h)) / 2;
        let art_x = top_area.left() + 2;
        let art_rect = Rect::new(art_x, art_y, art_w, art_h);

        // Render Artwork
        let art_widget = ArtworkWidget::new(self.current_track, self.artwork_cache, Some(self.theme));
        art_widget.render(art_rect, buf);

        // Render Track Details to the right of artwork
        let details_x = art_rect.right() + 3;
        if details_x < top_area.right() {
            let details_width = top_area.right() - details_x;
            let mut dy = art_y;

            if let Some(track) = self.current_track {
                // Title
                let title_style = self.theme.accent_text();
                let title_str = format!("♫ {}", track.display_title());
                let trunc_title: String = title_str.chars().take(details_width as usize).collect();
                buf.set_string(details_x, dy, &trunc_title, title_style);
                dy += 2;

                // Artist
                let art_str = format!("Artist: {}", track.display_artist());
                let trunc_art: String = art_str.chars().take(details_width as usize).collect();
                buf.set_string(details_x, dy, &trunc_art, self.theme.primary_text().add_modifier(Modifier::BOLD));
                dy += 1;

                // Album
                let alb_str = format!("Album:  {}", track.display_album());
                let trunc_alb: String = alb_str.chars().take(details_width as usize).collect();
                buf.set_string(details_x, dy, &trunc_alb, self.theme.secondary_text());
                dy += 1;

                // Original Release Date & Genre
                let mut meta_parts = Vec::new();
                if let Some(d) = track.release_date.as_deref().filter(|s| !s.is_empty()) {
                    meta_parts.push(format!("Original Release: {}", d));
                } else if let Some(y) = track.year {
                    meta_parts.push(format!("Original Release: {}", y));
                }
                if let Some(g) = &track.genre {
                    meta_parts.push(format!("Genre: {}", g));
                }
                if !meta_parts.is_empty() {
                    buf.set_string(details_x, dy, &meta_parts.join("  •  "), self.theme.muted_text());
                    dy += 1;
                }

                // Audio Quality & Format info
                dy += 1;
                let quality = track.quality_info();
                buf.set_string(
                    details_x,
                    dy,
                    &quality,
                    Style::default()
                        .fg(self.theme.cyan)
                        .add_modifier(Modifier::BOLD),
                );
            } else {
                buf.set_string(details_x, dy, "No Track Playing", self.theme.muted_text());
            }
        }

        // Bottom section:
        // Waveform height remains consistent and anchored at the bottom,
        // and does NOT shrink or jump when lyrics are opened/closed.
        let wave_h = (bottom_area.height * 25 / 100).max(3).min(5);
        let wave_rect = Rect::new(
            bottom_area.left(),
            bottom_area.bottom().saturating_sub(wave_h),
            bottom_area.width,
            wave_h,
        );

        let (lyrics_rect, spec_rect) = if self.show_lyrics && bottom_area.height >= 2 {
            let lyrics_h = if bottom_area.height >= wave_h + 5 {
                5u16
            } else {
                bottom_area.height.saturating_sub(wave_h)
            };

            // Calculate space between sound graph and waveform:
            // Ensure there is clear visual space (1-2 lines) between the spectrum bars and waveform
            let available_for_graph_and_gap = bottom_area.height.saturating_sub(lyrics_h + wave_h);
            let (_gap, spec_h) = if available_for_graph_and_gap >= 5 {
                (2u16, available_for_graph_and_gap - 2)
            } else if available_for_graph_and_gap >= 2 {
                (1u16, available_for_graph_and_gap - 1)
            } else {
                (available_for_graph_and_gap, 0u16)
            };

            let l_rect = Rect::new(bottom_area.left(), bottom_area.top(), bottom_area.width, lyrics_h);
            let s_rect = Rect::new(bottom_area.left(), bottom_area.top() + lyrics_h, bottom_area.width, spec_h);

            (Some(l_rect), s_rect)
        } else {
            let gap = 1u16;
            let spec_h = bottom_area.height.saturating_sub(wave_h + gap);
            let s_rect = Rect::new(bottom_area.left(), bottom_area.top(), bottom_area.width, spec_h);
            (None, s_rect)
        };

        // Render lyrics card if active
        if let Some(l_rect) = lyrics_rect {
            self.render_lyrics_card(l_rect, buf);
        }

        // Spectrum Visualizer
        if spec_rect.height >= 2 {
            let spec_widget = SpectrumWidget::new(self.visualizer, self.theme);
            spec_widget.render(spec_rect, buf);
        }

        // Full Waveform
        if wave_rect.height >= 1 {
            let wave_widget = WaveformWidget::new(self.waveform, self.elapsed, self.total_duration, self.theme);
            wave_widget.render(wave_rect, buf);

            let is_loading_waveform = self
                .waveform
                .map(|w| !w.is_complete)
                .unwrap_or(self.current_track.is_some());

            if is_loading_waveform && wave_rect.width >= 35 {
                let ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis();
                let spinner = crate::ui::artwork::SPINNER_CHARS
                    [(ms / 90) as usize % crate::ui::artwork::SPINNER_CHARS.len()];
                let badge = format!(" {} Computing high-fidelity waveform... ", spinner);
                let badge_x = wave_rect.right().saturating_sub(badge.chars().count() as u16 + 1);
                buf.set_string(
                    badge_x,
                    wave_rect.top(),
                    &badge,
                    Style::default()
                        .bg(self.theme.background)
                        .fg(self.theme.cyan)
                        .add_modifier(Modifier::BOLD),
                );
            }
        }

        Some(NowPlayingClickTarget {
            waveform_rect: wave_rect,
            lyrics_rect,
        })
    }
}
