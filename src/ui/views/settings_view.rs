use crate::config::AppConfig;
use crate::ui::theme::OmarchyTheme;
use crate::updater::{UpdateStatus, Updater};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, BorderType, Borders, Widget};

pub struct SettingsClickTarget {
    pub rect: Rect,
    pub action: SettingsAction,
}

pub enum SettingsAction {
    SelectFolder(usize),
    AddFolder,
    RemoveFolder(usize),
    ScanLibrary,
    FullRescanLibrary,
    CheckUpdate,
    RunUpdate,
    RestartApp,
    StartEnteringFps,
    ConfirmFps,
    CancelFps,
}

pub struct SettingsView<'a> {
    pub config: &'a AppConfig,
    pub selected_folder_idx: usize,
    pub new_folder_input: &'a str,
    pub is_entering_folder: bool,
    pub is_entering_fps: bool,
    pub fps_input: &'a str,
    pub is_scanning: bool,
    pub scan_status: &'a str,
    pub update_status: &'a UpdateStatus,
    pub is_focused: bool,
    pub theme: &'a OmarchyTheme,
}

impl<'a> SettingsView<'a> {
    fn render_button_cap(
        &self,
        buf: &mut Buffer,
        x: u16,
        y: u16,
        label: &str,
        fg_style: Style,
        border_style: Style,
    ) -> Rect {
        let label_chars = label.chars().count() as u16;
        let width = label_chars + 4;
        let bg_color = self.theme.background;
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
    }

    pub fn render_with_targets(
        &self,
        area: Rect,
        buf: &mut Buffer,
    ) -> Vec<SettingsClickTarget> {
        let mut targets = Vec::new();

        // Fill background
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                buf[(x, y)].set_bg(self.theme.background);
            }
        }

        let (border_style, title_style, title_text) = if self.is_focused {
            (
                self.theme.block_focus_style(),
                self.theme.focused_title_style(),
                " ● Settings ",
            )
        } else {
            (
                self.theme.block_style(),
                self.theme.unfocused_title_style(),
                " Settings ",
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

        if inner.width < 15 || inner.height < 6 {
            return targets;
        }

        let mut y = inner.top() + 1;

        // Section: Library Folders
        buf.set_string(
            inner.left() + 1,
            y,
            "LIBRARY FOLDERS",
            self.theme.accent_text(),
        );
        y += 1;
        buf.set_string(
            inner.left() + 1,
            y,
            "OmaFLAC scans these directories recursively for FLAC, MP3, OGG, WAV, M4A audio files.",
            self.theme.secondary_text(),
        );
        y += 2;

        for (i, folder) in self.config.library_folders.iter().enumerate() {
            if y >= inner.bottom().saturating_sub(12) {
                break;
            }
            let is_sel = i == self.selected_folder_idx;
            let style = if is_sel {
                self.theme.selected_row()
            } else {
                self.theme.primary_text()
            };

            let row_rect = Rect::new(inner.left(), y, inner.width, 1);
            if is_sel {
                for x in row_rect.left()..row_rect.right() {
                    buf[(x, y)].set_bg(self.theme.selection);
                }
            }

            let path_str = folder.display().to_string();
            let label = format!(" ▪ {}", path_str);
            let trunc: String = label.chars().take(inner.width.saturating_sub(16) as usize).collect();
            buf.set_string(inner.left(), y, &trunc, style);

            // Remove button [X]
            let remove_label = "[ ✕ Remove ]";
            let remove_w = remove_label.chars().count() as u16;
            let remove_btn_x = inner.right().saturating_sub(remove_w + 1);
            let remove_rect = Rect::new(remove_btn_x, y, remove_w, 1);
            buf.set_string(remove_btn_x, y, remove_label, Style::default().fg(self.theme.red));
            targets.push(SettingsClickTarget {
                rect: remove_rect,
                action: SettingsAction::RemoveFolder(i),
            });

            targets.push(SettingsClickTarget {
                rect: row_rect,
                action: SettingsAction::SelectFolder(i),
            });

            y += 1;
        }

        y += 1;
        // Add Folder input & buttons
        if self.is_entering_folder {
            buf.set_string(inner.left() + 1, y, "Enter directory path: ", self.theme.primary_text());
            y += 1;
            let input_width = inner.width.saturating_sub(18).max(10);
            let input_rect = Rect::new(inner.left() + 1, y + 1, input_width, 1);
            for x in input_rect.left()..input_rect.right() {
                buf[(x, y + 1)].set_bg(self.theme.selection);
            }
            let input_text = format!(" {}█", self.new_folder_input);
            buf.set_string(inner.left() + 1, y + 1, &input_text, self.theme.accent_text());

            let confirm_rect = self.render_button_cap(
                buf,
                input_rect.right() + 2,
                y,
                "Confirm",
                Style::default().fg(self.theme.accent).add_modifier(Modifier::BOLD),
                self.theme.block_focus_style(),
            );
            targets.push(SettingsClickTarget {
                rect: confirm_rect,
                action: SettingsAction::AddFolder,
            });
            y += 4;
        } else {
            let add_label = "+ Add Folder";
            let add_fg = Style::default().fg(self.theme.cyan).add_modifier(Modifier::BOLD);
            let add_border = self.theme.block_style();

            let (sync_label, sync_fg, sync_border) = if self.is_scanning {
                ("⟳ Syncing...", Style::default().fg(self.theme.yellow).add_modifier(Modifier::BOLD), self.theme.block_focus_style())
            } else {
                ("⟳ Sync Library (s)", Style::default().fg(self.theme.accent).add_modifier(Modifier::BOLD), self.theme.block_style())
            };

            let (full_label, full_fg, full_border) = if self.is_scanning {
                ("Full Rescan (S)", self.theme.muted_text(), self.theme.block_style())
            } else {
                ("Full Rescan (S)", self.theme.secondary_text(), self.theme.block_style())
            };

            let add_w = (add_label.chars().count() as u16) + 4;
            let sync_w = (sync_label.chars().count() as u16) + 4;
            let full_w = (full_label.chars().count() as u16) + 4;

            if inner.width >= add_w + sync_w + full_w + 6 {
                let mut btn_x = inner.left() + 1;
                let add_rect = self.render_button_cap(buf, btn_x, y, add_label, add_fg, add_border);
                targets.push(SettingsClickTarget {
                    rect: add_rect,
                    action: SettingsAction::AddFolder,
                });
                btn_x += add_rect.width + 2;

                let sync_rect = self.render_button_cap(buf, btn_x, y, sync_label, sync_fg, sync_border);
                targets.push(SettingsClickTarget {
                    rect: sync_rect,
                    action: SettingsAction::ScanLibrary,
                });
                btn_x += sync_rect.width + 2;

                let full_rect = self.render_button_cap(buf, btn_x, y, full_label, full_fg, full_border);
                targets.push(SettingsClickTarget {
                    rect: full_rect,
                    action: SettingsAction::FullRescanLibrary,
                });
                y += 4;
            } else {
                let add_rect = self.render_button_cap(buf, inner.left() + 1, y, add_label, add_fg, add_border);
                targets.push(SettingsClickTarget {
                    rect: add_rect,
                    action: SettingsAction::AddFolder,
                });
                y += 4;

                let mut btn_x = inner.left() + 1;
                let sync_rect = self.render_button_cap(buf, btn_x, y, sync_label, sync_fg, sync_border);
                targets.push(SettingsClickTarget {
                    rect: sync_rect,
                    action: SettingsAction::ScanLibrary,
                });
                btn_x += sync_rect.width + 2;

                let full_rect = self.render_button_cap(buf, btn_x, y, full_label, full_fg, full_border);
                targets.push(SettingsClickTarget {
                    rect: full_rect,
                    action: SettingsAction::FullRescanLibrary,
                });
                y += 4;
            }
        }

        if !self.scan_status.is_empty() {
            let status_trunc: String = self
                .scan_status
                .chars()
                .take(inner.width.saturating_sub(2) as usize)
                .collect();
            buf.set_string(
                inner.left() + 1,
                y,
                &status_trunc,
                Style::default().fg(self.theme.yellow),
            );
            y += 2;
        } else {
            y += 1;
        }



        // Section: Visualizer & Refresh Rate
        if y < inner.bottom().saturating_sub(6) {
            buf.set_string(
                inner.left() + 1,
                y,
                "VISUALIZER & REFRESH RATE",
                self.theme.accent_text(),
            );
            y += 1;

            let fps_val = self.config.visualizer_fps.clamp(10, 240);
            let fps_label = format!("• Target Refresh Rate: {} FPS  (Range: 10 – 240 FPS)", fps_val);
            buf.set_string(inner.left() + 1, y, &fps_label, self.theme.primary_text());
            y += 2;

            if self.is_entering_fps {
                buf.set_string(
                    inner.left() + 1,
                    y,
                    "Enter target frame rate (10 – 240 FPS):",
                    self.theme.primary_text(),
                );
                y += 1;

                let input_width = 12u16.min(inner.width.saturating_sub(22).max(6));
                let input_rect = Rect::new(inner.left() + 1, y + 1, input_width, 1);
                for x in input_rect.left()..input_rect.right() {
                    buf[(x, y + 1)].set_bg(self.theme.selection);
                }
                let input_text = format!(" {}█", self.fps_input);
                buf.set_string(inner.left() + 1, y + 1, &input_text, self.theme.accent_text());

                let mut btn_x = input_rect.right() + 2;
                let confirm_rect = self.render_button_cap(
                    buf,
                    btn_x,
                    y,
                    "Confirm",
                    Style::default().fg(self.theme.accent).add_modifier(Modifier::BOLD),
                    self.theme.block_focus_style(),
                );
                targets.push(SettingsClickTarget {
                    rect: confirm_rect,
                    action: SettingsAction::ConfirmFps,
                });
                btn_x += confirm_rect.width + 2;

                let cancel_rect = self.render_button_cap(
                    buf,
                    btn_x,
                    y,
                    "Cancel (Esc)",
                    self.theme.muted_text(),
                    self.theme.block_style(),
                );
                targets.push(SettingsClickTarget {
                    rect: cancel_rect,
                    action: SettingsAction::CancelFps,
                });

                y += 4;
            } else {
                let change_rect = self.render_button_cap(
                    buf,
                    inner.left() + 1,
                    y,
                    "Change FPS (f)",
                    Style::default()
                        .fg(self.theme.cyan)
                        .add_modifier(Modifier::BOLD),
                    self.theme.block_style(),
                );
                targets.push(SettingsClickTarget {
                    rect: change_rect,
                    action: SettingsAction::StartEnteringFps,
                });
                y += 4;
            }
        }

        // Section: Application Version & Updates
        if y < inner.bottom().saturating_sub(5) {
            buf.set_string(
                inner.left() + 1,
                y,
                "APPLICATION & UPDATES",
                self.theme.accent_text(),
            );
            y += 1;

            let ver_line = format!(
                "• Installed Version: OmaFLAC {}   (CLI: omaflac update)",
                Updater::display_version()
            );
            buf.set_string(inner.left() + 1, y, &ver_line, self.theme.primary_text());
            y += 1;

            let ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis();
            let spinner = crate::ui::artwork::SPINNER_CHARS
                [(ms / 90) as usize % crate::ui::artwork::SPINNER_CHARS.len()];

            let (status_text, status_style) = match self.update_status {
                UpdateStatus::Idle => (
                    "• Status: Ready to check GitHub for updates.".to_string(),
                    self.theme.secondary_text(),
                ),
                UpdateStatus::Checking => (
                    format!("• Status: {} Checking GitHub for latest version...", spinner),
                    Style::default()
                        .fg(self.theme.cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                UpdateStatus::UpToDate { version, commit } => {
                    let c = commit.as_ref().map(|s| format!(" ({})", s)).unwrap_or_default();
                    (
                        format!("• Status: ✓ Up to date (latest: v{}{})", version, c),
                        Style::default()
                            .fg(self.theme.accent)
                            .add_modifier(Modifier::BOLD),
                    )
                }
                UpdateStatus::UpdateAvailable {
                    latest_version,
                    latest_commit,
                } => {
                    let c = latest_commit
                        .as_ref()
                        .map(|s| format!(" ({})", s))
                        .unwrap_or_default();
                    (
                        format!(
                            "• Status: ↑ Update available: v{}{} — click [Update OmaFLAC (U)] or press U",
                            latest_version, c
                        ),
                        Style::default()
                            .fg(self.theme.cyan)
                            .add_modifier(Modifier::BOLD),
                    )
                }
                UpdateStatus::Updating { step } => (
                    format!("• Status: {} {}", spinner, step),
                    Style::default()
                        .fg(self.theme.cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                UpdateStatus::UpdatedReadyToRestart { new_version } => (
                    format!(
                        "• Status: ✓ Updated to {}! Click [Restart OmaFLAC (R)] or press R to launch new version.",
                        new_version
                    ),
                    Style::default()
                        .fg(self.theme.accent)
                        .add_modifier(Modifier::BOLD),
                ),
                UpdateStatus::Error(err) => (
                    format!("• Status: ✕ {}", err),
                    Style::default().fg(self.theme.red),
                ),
            };

            let trunc_status: String = status_text
                .chars()
                .take(inner.width.saturating_sub(2) as usize)
                .collect();
            buf.set_string(inner.left() + 1, y, &trunc_status, status_style);
            y += 2;

            // Update Action Buttons
            let is_busy = matches!(
                self.update_status,
                UpdateStatus::Checking | UpdateStatus::Updating { .. }
            );

            let check_label = if matches!(self.update_status, UpdateStatus::Checking) {
                "⟳ Checking..."
            } else {
                "⟳ Check for Updates (u)"
            };
            let (check_fg, check_border) = if is_busy {
                (self.theme.muted_text(), self.theme.block_style())
            } else {
                (
                    Style::default()
                        .fg(self.theme.cyan)
                        .add_modifier(Modifier::BOLD),
                    self.theme.block_style(),
                )
            };

            let mut btn_x = inner.left() + 1;
            let check_rect = self.render_button_cap(
                buf,
                btn_x,
                y,
                check_label,
                check_fg,
                check_border,
            );
            targets.push(SettingsClickTarget {
                rect: check_rect,
                action: SettingsAction::CheckUpdate,
            });
            btn_x += check_rect.width + 2;

            if matches!(
                self.update_status,
                UpdateStatus::UpdatedReadyToRestart { .. }
            ) {
                let restart_label = "↻ Restart OmaFLAC (R)";
                let restart_rect = self.render_button_cap(
                    buf,
                    btn_x,
                    y,
                    restart_label,
                    Style::default()
                        .fg(self.theme.accent)
                        .add_modifier(Modifier::BOLD),
                    self.theme.block_focus_style(),
                );
                targets.push(SettingsClickTarget {
                    rect: restart_rect,
                    action: SettingsAction::RestartApp,
                });
            } else {
                let (update_label, update_fg, update_border) = if matches!(self.update_status, UpdateStatus::Updating { .. }) {
                    ("↑ Updating OmaFLAC...", self.theme.muted_text(), self.theme.block_style())
                } else if matches!(self.update_status, UpdateStatus::UpdateAvailable { .. }) {
                    (
                        "↑ Update OmaFLAC (U)",
                        Style::default()
                            .fg(self.theme.accent)
                            .add_modifier(Modifier::BOLD),
                        self.theme.block_focus_style(),
                    )
                } else {
                    (
                        "↑ Update OmaFLAC (U)",
                        self.theme.secondary_text(),
                        self.theme.block_style(),
                    )
                };

                let update_rect = self.render_button_cap(
                    buf,
                    btn_x,
                    y,
                    update_label,
                    update_fg,
                    update_border,
                );
                targets.push(SettingsClickTarget {
                    rect: update_rect,
                    action: SettingsAction::RunUpdate,
                });
            }
        }

        targets
    }
}
