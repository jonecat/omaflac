use crate::audio::player::PlaybackState;
use crate::audio::waveform::WaveformData;
use crate::config::RepeatMode;
use crate::library::track::Track;
use crate::ui::theme::Theme;
use crate::ui::waveform_widget::WaveformWidget;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, Widget};
use std::time::Duration;

pub enum PlayerAction {
    Previous,
    TogglePlayPause,
    Stop,
    Next,
    ToggleRepeat,
    ToggleShuffle,
    Seek(f32), // 0.0 to 1.0 percentage
    SetVolume(f32), // 0.0 to 1.5
    ToggleMute,
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
    pub waveform: Option<&'a WaveformData>,
}

impl<'a> PlayerBar<'a> {
    pub fn render_with_targets(
        &self,
        area: Rect,
        buf: &mut Buffer,
    ) -> Vec<PlayerClickTarget> {
        let mut targets = Vec::new();

        if area.width < 10 || area.height < 3 {
            return targets;
        }

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Theme::block_style())
            .title(" Now Playing ")
            .title_style(Style::default().fg(Theme::OMARCHY_GREEN).add_modifier(Modifier::BOLD));

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height < 2 {
            return targets;
        }

        let y_top = inner.top();
        let y_bottom = inner.top() + 1;

        // Line 1: Controls (left), Track Info (middle), Volume & Modes (right)
        let mut x = inner.left();

        // [⏮]
        let prev_rect = Rect::new(x, y_top, 3, 1);
        buf.set_string(x, y_top, "⏮ ", Style::default().fg(Theme::TEXT_PRIMARY));
        targets.push(PlayerClickTarget { rect: prev_rect, action: PlayerAction::Previous });
        x += 3;

        // [▶/⏸]
        let play_symbol = match self.playback_state {
            PlaybackState::Playing => "⏸ ",
            PlaybackState::Paused | PlaybackState::Stopped => "▶ ",
        };
        let play_rect = Rect::new(x, y_top, 3, 1);
        buf.set_string(x, y_top, play_symbol, Style::default().fg(Theme::OMARCHY_GREEN).add_modifier(Modifier::BOLD));
        targets.push(PlayerClickTarget { rect: play_rect, action: PlayerAction::TogglePlayPause });
        x += 3;

        // [⏹]
        let stop_rect = Rect::new(x, y_top, 3, 1);
        buf.set_string(x, y_top, "⏹ ", Style::default().fg(Theme::TEXT_PRIMARY));
        targets.push(PlayerClickTarget { rect: stop_rect, action: PlayerAction::Stop });
        x += 3;

        // [⏭]
        let next_rect = Rect::new(x, y_top, 3, 1);
        buf.set_string(x, y_top, "⏭ ", Style::default().fg(Theme::TEXT_PRIMARY));
        targets.push(PlayerClickTarget { rect: next_rect, action: PlayerAction::Next });
        x += 4;

        // Right side: Volume & Repeat/Shuffle
        let right_margin = inner.right();
        let vol_width = 16u16;
        let mode_width = 8u16;

        let vol_x = right_margin.saturating_sub(vol_width + 1);
        let mode_x = vol_x.saturating_sub(mode_width + 2);

        // Repeat & Shuffle buttons
        if mode_x > x + 15 {
            // Repeat
            let rep_label = match self.repeat_mode {
                RepeatMode::Off => "🔁",
                RepeatMode::All => "🔁all",
                RepeatMode::Track => "🔁1",
            };
            let rep_style = if self.repeat_mode != RepeatMode::Off {
                Style::default().fg(Theme::OMARCHY_GREEN).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Theme::TEXT_MUTED)
            };
            buf.set_string(mode_x, y_top, rep_label, rep_style);
            targets.push(PlayerClickTarget {
                rect: Rect::new(mode_x, y_top, 4, 1),
                action: PlayerAction::ToggleRepeat,
            });

            // Shuffle
            let shuf_style = if self.shuffle {
                Style::default().fg(Theme::OMARCHY_GREEN).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Theme::TEXT_MUTED)
            };
            buf.set_string(mode_x + 5, y_top, "🔀", shuf_style);
            targets.push(PlayerClickTarget {
                rect: Rect::new(mode_x + 5, y_top, 3, 1),
                action: PlayerAction::ToggleShuffle,
            });
        }

        // Volume slider
        if vol_x > x + 10 {
            let mute_icon = if self.is_muted || self.volume <= 0.01 {
                "🔇"
            } else if self.volume < 0.4 {
                "🔈"
            } else if self.volume < 0.8 {
                "🔉"
            } else {
                "🔊"
            };
            buf.set_string(vol_x, y_top, mute_icon, Style::default().fg(Theme::TEXT_PRIMARY));
            targets.push(PlayerClickTarget {
                rect: Rect::new(vol_x, y_top, 2, 1),
                action: PlayerAction::ToggleMute,
            });

            // Volume bar
            let bar_start_x = vol_x + 3;
            let bar_len = 8u16;
            let effective_vol = if self.is_muted { 0.0 } else { self.volume };
            let vol_head = ((effective_vol.min(1.0) * bar_len as f32).round() as u16).min(bar_len);

            for i in 0..bar_len {
                let sym = if i == vol_head {
                    "●"
                } else if i < vol_head {
                    "━"
                } else {
                    "─"
                };
                let style = if i <= vol_head {
                    Style::default().fg(Theme::OMARCHY_GREEN)
                } else {
                    Style::default().fg(Theme::TEXT_MUTED)
                };
                buf.set_string(bar_start_x + i, y_top, sym, style);
            }

            targets.push(PlayerClickTarget {
                rect: Rect::new(bar_start_x, y_top, bar_len, 1),
                action: PlayerAction::SetVolume(self.volume),
            });

            let pct_text = format!("{:>3}%", (effective_vol * 100.0).round() as u32);
            buf.set_string(bar_start_x + bar_len + 1, y_top, &pct_text, Style::default().fg(Theme::TEXT_SECONDARY));
        }

        // Middle: Track Title, Artist, Album, Quality
        let max_info_x = mode_x.saturating_sub(2);
        if max_info_x > x + 4 {
            let available_width = (max_info_x - x) as usize;
            if let Some(track) = self.current_track {
                let title = track.display_title();
                let artist = track.display_artist();
                let quality = track.quality_info();

                let info_line = format!("{} — {}   [{}]", title, artist, quality);
                let truncated: String = info_line.chars().take(available_width).collect();
                buf.set_string(x, y_top, &truncated, Style::default().fg(Theme::TEXT_PRIMARY).add_modifier(Modifier::BOLD));
            } else {
                buf.set_string(x, y_top, "No track selected", Style::default().fg(Theme::TEXT_MUTED));
            }
        }

        // Line 2: Elapsed Time (left), Waveform Seekbar (middle), Duration (right)
        let elapsed_secs = self.elapsed.as_secs();
        let total_secs = self.total_duration.as_secs();
        let elapsed_str = format!("{:02}:{:02}", elapsed_secs / 60, elapsed_secs % 60);
        let total_str = format!("{:02}:{:02}", total_secs / 60, total_secs % 60);

        buf.set_string(inner.left(), y_bottom, &elapsed_str, Style::default().fg(Theme::TEXT_SECONDARY));

        let time_width = elapsed_str.len() as u16 + 1;
        let right_time_x = inner.right().saturating_sub(total_str.len() as u16);
        buf.set_string(right_time_x, y_bottom, &total_str, Style::default().fg(Theme::TEXT_SECONDARY));

        let wave_x = inner.left() + time_width;
        let wave_end = right_time_x.saturating_sub(1);

        if wave_end > wave_x + 5 {
            let wave_width = wave_end - wave_x;
            let wave_rect = Rect::new(wave_x, y_bottom, wave_width, 1);

            let wave_widget = WaveformWidget::new(self.waveform, self.elapsed, self.total_duration);
            wave_widget.render(wave_rect, buf);

            targets.push(PlayerClickTarget {
                rect: wave_rect,
                action: PlayerAction::Seek(0.0),
            });
        }

        targets
    }
}
