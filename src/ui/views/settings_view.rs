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
}

pub struct SettingsView<'a> {
    pub config: &'a AppConfig,
    pub selected_folder_idx: usize,
    pub new_folder_input: &'a str,
    pub is_entering_folder: bool,
    pub is_scanning: bool,
    pub scan_status: &'a str,
    pub update_status: &'a UpdateStatus,
    pub is_focused: bool,
    pub theme: &'a OmarchyTheme,
}

impl<'a> SettingsView<'a> {
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
            let trunc: String = label.chars().take(inner.width.saturating_sub(12) as usize).collect();
            buf.set_string(inner.left(), y, &trunc, style);

            // Remove button [X]
            let remove_btn_x = inner.right().saturating_sub(10);
            let remove_rect = Rect::new(remove_btn_x, y, 9, 1);
            buf.set_string(remove_btn_x, y, "[Remove]", Style::default().fg(self.theme.red));
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
        // Add Folder input & button
        if self.is_entering_folder {
            buf.set_string(inner.left() + 1, y, "Enter directory path: ", self.theme.primary_text());
            y += 1;
            let input_rect = Rect::new(inner.left() + 1, y, inner.width.saturating_sub(17), 1);
            for x in input_rect.left()..input_rect.right() {
                buf[(x, y)].set_bg(self.theme.lighter_background);
            }
            let input_text = format!(" {}█", self.new_folder_input);
            buf.set_string(inner.left() + 1, y, &input_text, self.theme.accent_text());

            let add_btn_x = input_rect.right() + 2;
            let add_rect = Rect::new(add_btn_x, y, 10, 1);
            buf.set_string(add_btn_x, y, "[Confirm]", self.theme.accent_text());
            targets.push(SettingsClickTarget {
                rect: add_rect,
                action: SettingsAction::AddFolder,
            });
            y += 2;
        } else {
            let add_rect = Rect::new(inner.left(), y, 18, 1);
            buf.set_string(
                inner.left(),
                y,
                " + [Add Folder...]",
                Style::default()
                    .fg(self.theme.cyan)
                    .add_modifier(Modifier::BOLD),
            );
            targets.push(SettingsClickTarget {
                rect: add_rect,
                action: SettingsAction::AddFolder,
            });
            y += 2;
        }

        // Sync & Full Rescan buttons + status
        let sync_btn_label = if self.is_scanning {
            " ⟳ [Syncing Library...]"
        } else {
            " ⟳ [Sync Library (s)]"
        };
        let sync_w = sync_btn_label.chars().count() as u16;
        let sync_rect = Rect::new(inner.left(), y, sync_w, 1);
        buf.set_string(inner.left(), y, sync_btn_label, self.theme.accent_text());
        targets.push(SettingsClickTarget {
            rect: sync_rect,
            action: SettingsAction::ScanLibrary,
        });

        let full_btn_x = inner.left() + sync_w + 3;
        let full_btn_label = "[Full Rescan (S)]";
        let full_w = full_btn_label.chars().count() as u16;
        let full_rect = Rect::new(full_btn_x, y, full_w, 1);
        buf.set_string(
            full_btn_x,
            y,
            full_btn_label,
            if self.is_scanning {
                self.theme.muted_text()
            } else {
                self.theme.secondary_text()
            },
        );
        targets.push(SettingsClickTarget {
            rect: full_rect,
            action: SettingsAction::FullRescanLibrary,
        });

        if !self.scan_status.is_empty() {
            y += 1;
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
        }

        y += 2;

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
                            "• Status: ↑ Update available: v{}{} — click [Update OmaFLAC Now] or press U",
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
                        "• Status: ✓ Updated to {}! Click [Restart OmaFLAC] or press R to launch new version.",
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
                " ⟳ [Checking...]"
            } else {
                " ⟳ [Check for Updates (u)]"
            };
            let check_w = check_label.chars().count() as u16;
            let check_rect = Rect::new(inner.left(), y, check_w, 1);
            buf.set_string(
                inner.left(),
                y,
                check_label,
                if is_busy {
                    self.theme.muted_text()
                } else {
                    Style::default()
                        .fg(self.theme.cyan)
                        .add_modifier(Modifier::BOLD)
                },
            );
            targets.push(SettingsClickTarget {
                rect: check_rect,
                action: SettingsAction::CheckUpdate,
            });

            let btn2_x = inner.left() + check_w + 3;
            if matches!(
                self.update_status,
                UpdateStatus::UpdatedReadyToRestart { .. }
            ) {
                let restart_label = "[ ↻ Restart OmaFLAC Now (R) ]";
                let restart_w = restart_label.chars().count() as u16;
                let restart_rect = Rect::new(btn2_x, y, restart_w, 1);
                buf.set_string(
                    btn2_x,
                    y,
                    restart_label,
                    Style::default()
                        .fg(self.theme.accent)
                        .add_modifier(Modifier::BOLD),
                );
                targets.push(SettingsClickTarget {
                    rect: restart_rect,
                    action: SettingsAction::RestartApp,
                });
            } else {
                let update_label = if matches!(self.update_status, UpdateStatus::Updating { .. }) {
                    "[ ↑ Updating OmaFLAC... ]"
                } else {
                    "[ ↑ Update OmaFLAC Now (U) ]"
                };
                let update_w = update_label.chars().count() as u16;
                let update_rect = Rect::new(btn2_x, y, update_w, 1);
                let update_style = if is_busy {
                    self.theme.muted_text()
                } else if matches!(
                    self.update_status,
                    UpdateStatus::UpdateAvailable { .. }
                ) {
                    Style::default()
                        .fg(self.theme.accent)
                        .add_modifier(Modifier::BOLD)
                } else {
                    self.theme.secondary_text()
                };
                buf.set_string(btn2_x, y, update_label, update_style);
                targets.push(SettingsClickTarget {
                    rect: update_rect,
                    action: SettingsAction::RunUpdate,
                });
            }

            y += 3;
        }

        // Section: System & Audio Info
        if y < inner.bottom().saturating_sub(3) {
            buf.set_string(
                inner.left() + 1,
                y,
                "SYSTEM & AUDIO SUBSYSTEM",
                self.theme.accent_text(),
            );
            y += 1;
            buf.set_string(
                inner.left() + 1,
                y,
                "• Audio Engine: Symphonia (Bit-perfect multi-format decoder) + CPAL",
                self.theme.secondary_text(),
            );
            y += 1;
            buf.set_string(
                inner.left() + 1,
                y,
                "• Codecs Supported: FLAC, MP3, OGG Vorbis/Opus, WAV, AIFF, M4A/AAC, ALAC",
                self.theme.secondary_text(),
            );
        }

        targets
    }
}
