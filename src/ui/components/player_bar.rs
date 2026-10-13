use crate::audio::player::PlaybackState;
use crate::audio::waveform::WaveformData;
use crate::config::RepeatMode;
use crate::library::track::Track;
use crate::ui::artwork::{ArtSize, ArtworkCache, ArtworkWidget};
use crate::ui::theme::OmarchyTheme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, BorderType, Borders, Widget};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlayerAction {
    Previous,
    TogglePlayPause,
    Next,
    ToggleRepeat,
    ToggleShuffle,
    Seek(f32),
    SetVolume(f32),
    ToggleMute,
    OpenNowPlaying,
}

pub struct PlayerClickTarget {
    pub rect: Rect,
    pub action: PlayerAction,
}

pub struct PlayerBar<'a> {
    pub current_track: Option<&'a Track>,
    pub playback_state: PlaybackState,
    pub elapsed: Duration,
    pub total_duration: Duration,
    pub volume: f32,
    pub is_muted: bool,
    pub repeat_mode: RepeatMode,
    pub shuffle: bool,
    #[allow(dead_code)]
    pub waveform: Option<&'a WaveformData>,
    pub artwork_cache: &'a ArtworkCache,
    pub is_focused: bool,
    pub theme: &'a OmarchyTheme,
}

impl<'a> PlayerBar<'a> {
    pub fn render_with_targets(
        &self,
        area: Rect,
        buf: &mut Buffer,
    ) -> Vec<PlayerClickTarget> {
        let mut targets = Vec::new();

        if area.width < 10 || area.height < 2 {
            return targets;
        }

        let (border_style, title_style, title_text) = if self.is_focused {
            (
                self.theme.block_focus_style(),
                self.theme.focused_title_style(),
                " ● Now Playing ",
            )
        } else {
            (
                self.theme.block_style(),
                self.theme.unfocused_title_style(),
                " Now Playing ",
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

        if inner.height == 0 {
            return targets;
        }

        // Fill background of inner area
        for y in inner.top()..inner.bottom() {
            for x in inner.left()..inner.right() {
                buf[(x, y)].set_bg(self.theme.background);
            }
        }

        let top_pad = if inner.height >= 4 { 1 } else { 0 };
        let content_top = inner.top() + top_pad;
        let avail_h = inner.height.saturating_sub(top_pad);

        // Render small album artwork thumbnail on the left if space permits
        let content_rect = if inner.width >= 40 && avail_h >= 2 {
            let thumb_h = avail_h.min(5);
            let thumb_w = self.artwork_cache.thumb_width_for_height(thumb_h);
            let left_pad = 1;
            let thumb_rect = Rect::new(inner.left() + left_pad, content_top, thumb_w, thumb_h);

            let art_widget = ArtworkWidget::new(self.current_track, self.artwork_cache, Some(self.theme))
                .with_size(ArtSize::Thumb);
            art_widget.render(thumb_rect, buf);

            // Register click target on the album artwork to switch to Now Playing view
            targets.push(PlayerClickTarget {
                rect: thumb_rect,
                action: PlayerAction::OpenNowPlaying,
            });

            let content_x = inner.left() + left_pad + thumb_w + 1;
            let content_w = inner.width.saturating_sub(left_pad + thumb_w + 1);
            Rect::new(content_x, content_top, content_w, avail_h)
        } else {
            Rect::new(inner.left(), content_top, inner.width, avail_h)
        };

        // Layout rows depending on avail_h:
        // avail_h >= 5:
        //   Row 0: Track info & audio format/quality badge
        //   Rows 1..=3: 3-row tall rounded-corner transport buttons & volume
        //   Row 4: Elapsed time, progress seekbar, total time
        if avail_h >= 5 {
            let y_info = content_top;
            let y_ctrl = content_top + 1;
            let y_wave = content_top + 4;

            self.render_row_info(content_rect, y_info, buf);
            self.render_row_controls(content_rect, y_ctrl, 3, buf, &mut targets);
            self.render_row_waveform(content_rect, y_wave, buf, &mut targets);
        } else if avail_h >= 3 {
            let y_info = content_top;
            let y_ctrl = content_top + 1;
            let y_wave = content_top + 2;

            self.render_row_info(content_rect, y_info, buf);
            self.render_row_controls(content_rect, y_ctrl, 1, buf, &mut targets);
            self.render_row_waveform(content_rect, y_wave, buf, &mut targets);
        } else if avail_h == 2 {
            let y_ctrl = content_top;
            let y_wave = content_top + 1;

            self.render_row_controls(content_rect, y_ctrl, 1, buf, &mut targets);
            self.render_row_waveform(content_rect, y_wave, buf, &mut targets);
        } else if avail_h >= 1 {
            let y_ctrl = content_top;
            self.render_row_controls(content_rect, y_ctrl, 1, buf, &mut targets);
        }

        targets
    }

    /// Row 0: Track Title, Artist, Album, and Hi-Res / Quality badge
    fn render_row_info(&self, inner: Rect, y: u16, buf: &mut Buffer) {
        let x_start = inner.left() + 1;
        let right_edge = inner.right().saturating_sub(1);

        if right_edge <= x_start {
            return;
        }

        let total_avail = (right_edge - x_start) as usize;

        if let Some(track) = self.current_track {
            let quality = track.quality_info();
            let badge = format!("[{}]", quality);
            let badge_width = badge.chars().count() as u16;

            let badge_x = right_edge.saturating_sub(badge_width);
            if badge_x > x_start + 10 {
                buf.set_string(
                    badge_x,
                    y,
                    &badge,
                    Style::default()
                        .fg(self.theme.cyan)
                        .add_modifier(Modifier::BOLD),
                );
            }

            let max_title_w = if badge_x > x_start + 10 {
                (badge_x.saturating_sub(x_start + 2)) as usize
            } else {
                total_avail
            };

            let title = track.display_title();
            let artist = track.display_artist();
            let album = track.display_album();

            let full_info = format!("♫  {} — {}  •  {}", title, artist, album);
            let trunc_info: String = full_info.chars().take(max_title_w).collect();

            buf.set_string(
                x_start,
                y,
                &trunc_info,
                Style::default()
                    .fg(self.theme.bright_foreground)
                    .add_modifier(Modifier::BOLD),
            );
        } else {
            buf.set_string(
                x_start,
                y,
                "♫  No track selected",
                Style::default().fg(self.theme.muted),
            );
        }
    }

    fn render_button_cap(
        &self,
        buf: &mut Buffer,
        x: u16,
        y: u16,
        btn_h: u16,
        label: &str,
        fg_style: Style,
        border_style: Style,
    ) -> Rect {
        let label_chars = label.chars().count() as u16;
        let width = label_chars + 4; // "│ " + label + " │"
        let bg_color = self.theme.background;

        if btn_h >= 3 {
            let rect = Rect::new(x, y, width, 3);
            for row in y..y + 3 {
                for col in x..x + width {
                    buf[(col, row)].set_bg(bg_color);
                }
            }
            let block = Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(border_style.bg(bg_color))
                .style(Style::default().bg(bg_color));
            block.render(rect, buf);
            buf.set_string(x + 2, y + 1, label, fg_style.bg(bg_color));
            rect
        } else {
            let rect = Rect::new(x, y, width, 1);
            buf.set_string(x, y, "[ ", border_style);
            buf.set_string(x + 2, y, label, fg_style);
            buf.set_string(x + 2 + label_chars, y, " ]", border_style);
            rect
        }
    }

    /// Transport & mode controls (3-row tall rounded button caps when btn_h == 3)
    fn render_row_controls(
        &self,
        inner: Rect,
        y: u16,
        btn_h: u16,
        buf: &mut Buffer,
        targets: &mut Vec<PlayerClickTarget>,
    ) {
        let mut x = inner.left() + 1;

        let subtle_border = self.theme.block_style();
        let active_border = self.theme.block_focus_style();
        let active_ctrl_style = Style::default()
            .fg(self.theme.accent)
            .add_modifier(Modifier::BOLD);
        let normal_ctrl_style = Style::default()
            .fg(self.theme.bright_foreground)
            .add_modifier(Modifier::BOLD);
        let muted_ctrl_style = Style::default().fg(self.theme.muted);

        // 1. Previous: ╭────╮ / │ |◀ │ / ╰────╯
        let prev_rect = self.render_button_cap(
            buf,
            x,
            y,
            btn_h,
            "|◀",
            normal_ctrl_style,
            subtle_border,
        );
        targets.push(PlayerClickTarget {
            rect: prev_rect,
            action: PlayerAction::Previous,
        });
        x += prev_rect.width + 1;

        // 2. Play / Pause: ╭─────╮ / │  ▶  │ / ╰─────╯
        let is_playing = self.playback_state == PlaybackState::Playing;
        let play_sym = if is_playing { " ‖ " } else { " ▶ " };
        let play_rect = self.render_button_cap(
            buf,
            x,
            y,
            btn_h,
            play_sym,
            active_ctrl_style,
            if is_playing { active_border } else { subtle_border },
        );
        targets.push(PlayerClickTarget {
            rect: play_rect,
            action: PlayerAction::TogglePlayPause,
        });
        x += play_rect.width + 1;

        // 3. Next: ╭────╮ / │ ▶| │ / ╰────╯
        let next_rect = self.render_button_cap(
            buf,
            x,
            y,
            btn_h,
            "▶|",
            normal_ctrl_style,
            subtle_border,
        );
        targets.push(PlayerClickTarget {
            rect: next_rect,
            action: PlayerAction::Next,
        });
        x += next_rect.width + 2;

        // 5. Shuffle: ╭─────╮ / │  ⇄  │ / ╰─────╯
        let shuf_rect = self.render_button_cap(
            buf,
            x,
            y,
            btn_h,
            " ⇄ ",
            if self.shuffle { active_ctrl_style } else { muted_ctrl_style },
            if self.shuffle { active_border } else { subtle_border },
        );
        targets.push(PlayerClickTarget {
            rect: shuf_rect,
            action: PlayerAction::ToggleShuffle,
        });
        x += shuf_rect.width + 1;

        // 6. Repeat: ╭─────╮ / │  ↻  │ / ╰─────╯
        let rep_inner = match self.repeat_mode {
            RepeatMode::Off => " ↻ ",
            RepeatMode::All => " ↻ All ",
            RepeatMode::Track => " ↻ 1 ",
        };
        let rep_active = self.repeat_mode != RepeatMode::Off;
        let rep_rect = self.render_button_cap(
            buf,
            x,
            y,
            btn_h,
            rep_inner,
            if rep_active { active_ctrl_style } else { muted_ctrl_style },
            if rep_active { active_border } else { subtle_border },
        );
        targets.push(PlayerClickTarget {
            rect: rep_rect,
            action: PlayerAction::ToggleRepeat,
        });

        // Right side: Volume controls
        let right_margin = inner.right().saturating_sub(1);
        let vol_slider_len = 10u16;
        let pct_width = 5u16;
        let btn_width = 5u16; // ╭───╮ / │ ♪ │ / ╰───╯
        let total_vol_width = btn_width + 1 + vol_slider_len + 1 + pct_width;

        if right_margin > x + rep_rect.width + total_vol_width + 2 {
            let vol_start_x = right_margin.saturating_sub(total_vol_width);
            let is_muted = self.is_muted || self.volume <= 0.01;
            let (v_icon, v_style, v_border) = if is_muted {
                (
                    "✕",
                    Style::default().fg(self.theme.accent).add_modifier(Modifier::BOLD),
                    active_border,
                )
            } else {
                ("♪", normal_ctrl_style, subtle_border)
            };

            let vol_btn_rect = self.render_button_cap(
                buf,
                vol_start_x,
                y,
                btn_h,
                v_icon,
                v_style,
                v_border,
            );
            targets.push(PlayerClickTarget {
                rect: vol_btn_rect,
                action: PlayerAction::ToggleMute,
            });

            // Slider (vertically centered with the button cap)
            let slider_y = y + btn_h / 2;
            let slider_x = vol_start_x + btn_width + 1;
            let effective_vol = if self.is_muted { 0.0 } else { self.volume };
            let vol_head = ((effective_vol.min(1.0) * vol_slider_len as f32).round() as u16).min(vol_slider_len);

            for i in 0..vol_slider_len {
                let sym = if i == vol_head {
                    "●"
                } else if i < vol_head {
                    "━"
                } else {
                    "─"
                };
                let style = if i <= vol_head {
                    Style::default().fg(self.theme.accent)
                } else {
                    Style::default().fg(self.theme.muted)
                };
                buf.set_string(slider_x + i, slider_y, sym, style);
            }

            targets.push(PlayerClickTarget {
                rect: Rect::new(slider_x, y, vol_slider_len, btn_h),
                action: PlayerAction::SetVolume(self.volume),
            });

            // Percentage label
            let pct_text = format!("{:>3}%", (effective_vol * 100.0).round() as u32);
            buf.set_string(
                slider_x + vol_slider_len + 1,
                slider_y,
                &pct_text,
                Style::default().fg(self.theme.light_foreground),
            );
        }
    }

    /// Row 2: Elapsed time, Track progress line with playhead, and Total duration
    fn render_row_waveform(
        &self,
        inner: Rect,
        y: u16,
        buf: &mut Buffer,
        targets: &mut Vec<PlayerClickTarget>,
    ) {
        let elapsed_secs = self.elapsed.as_secs();
        let total_secs = self.total_duration.as_secs();
        let elapsed_str = format!("{:02}:{:02}", elapsed_secs / 60, elapsed_secs % 60);
        let total_str = format!("{:02}:{:02}", total_secs / 60, total_secs % 60);

        let left_x = inner.left() + 1;
        buf.set_string(
            left_x,
            y,
            &elapsed_str,
            Style::default().fg(self.theme.light_foreground),
        );

        let time_w = elapsed_str.len() as u16 + 2;
        let right_time_x = inner.right().saturating_sub(total_str.len() as u16 + 1);
        buf.set_string(
            right_time_x,
            y,
            &total_str,
            Style::default().fg(self.theme.light_foreground),
        );

        let bar_x = left_x + time_w;
        let bar_end = right_time_x.saturating_sub(1);

        if bar_end > bar_x + 5 {
            let bar_w = bar_end - bar_x;
            let bar_rect = Rect::new(bar_x, y, bar_w, 1);

            let total_f = self.total_duration.as_secs_f32();
            let elapsed_f = self.elapsed.as_secs_f32();
            let progress = if total_f > 0.0 {
                (elapsed_f / total_f).clamp(0.0, 1.0)
            } else {
                0.0
            };

            let head_idx = if total_f > 0.0 {
                let pos = (progress * (bar_w.saturating_sub(1)) as f32).round() as u16;
                pos.min(bar_w.saturating_sub(1))
            } else {
                0
            };

            for i in 0..bar_w {
                let (sym, style) = if total_f <= 0.0 {
                    ("─", Style::default().fg(self.theme.muted))
                } else if i == head_idx {
                    ("●", Style::default().fg(self.theme.accent).add_modifier(Modifier::BOLD))
                } else if i < head_idx {
                    ("━", Style::default().fg(self.theme.accent))
                } else {
                    ("─", Style::default().fg(self.theme.muted))
                };
                buf.set_string(bar_x + i, y, sym, style);
            }

            targets.push(PlayerClickTarget {
                rect: bar_rect,
                action: PlayerAction::Seek(0.0),
            });
        }
    }
}
