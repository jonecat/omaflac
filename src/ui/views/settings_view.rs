use crate::config::AppConfig;
use crate::ui::theme::Theme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, Widget};

pub struct SettingsClickTarget {
    pub rect: Rect,
    pub action: SettingsAction,
}

pub enum SettingsAction {
    SelectFolder(usize),
    AddFolder,
    RemoveFolder(usize),
    ScanLibrary,
}

pub struct SettingsView<'a> {
    pub config: &'a AppConfig,
    pub selected_folder_idx: usize,
    pub new_folder_input: &'a str,
    pub is_entering_folder: bool,
    pub is_scanning: bool,
    pub scan_status: &'a str,
    pub is_focused: bool,
}

impl<'a> SettingsView<'a> {
    pub fn render_with_targets(
        &self,
        area: Rect,
        buf: &mut Buffer,
    ) -> Vec<SettingsClickTarget> {
        let mut targets = Vec::new();

        let border_style = if self.is_focused {
            Theme::block_focus_style()
        } else {
            Theme::block_style()
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(" Settings & Library Configuration ")
            .title_style(Style::default().fg(Theme::OMARCHY_GREEN).add_modifier(Modifier::BOLD));

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.width < 15 || inner.height < 6 {
            return targets;
        }

        let mut y = inner.top();

        // Section: Library Folders
        buf.set_string(
            inner.left(),
            y,
            "LIBRARY FOLDERS",
            Style::default().fg(Theme::OMARCHY_GREEN).add_modifier(Modifier::BOLD),
        );
        y += 1;
        buf.set_string(
            inner.left(),
            y,
            "omaflac scans these directories recursively for FLAC, MP3, OGG, WAV, M4A audio files.",
            Style::default().fg(Theme::TEXT_SECONDARY),
        );
        y += 2;

        for (i, folder) in self.config.library_folders.iter().enumerate() {
            if y >= inner.bottom().saturating_sub(8) {
                break;
            }
            let is_sel = i == self.selected_folder_idx;
            let style = if is_sel {
                Theme::selected_row()
            } else {
                Style::default().fg(Theme::TEXT_PRIMARY)
            };

            let row_rect = Rect::new(inner.left(), y, inner.width, 1);
            if is_sel {
                for x in row_rect.left()..row_rect.right() {
                    buf[(x, y)].set_bg(Theme::BG_HIGHLIGHT);
                }
            }

            let path_str = folder.display().to_string();
            let label = format!(" 📁 {}", path_str);
            let trunc: String = label.chars().take(inner.width.saturating_sub(12) as usize).collect();
            buf.set_string(inner.left(), y, &trunc, style);

            // Remove button [X]
            let remove_btn_x = inner.right().saturating_sub(10);
            let remove_rect = Rect::new(remove_btn_x, y, 9, 1);
            buf.set_string(remove_btn_x, y, "[Remove]", Style::default().fg(Theme::ACCENT_RED));
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
            buf.set_string(inner.left(), y, "Enter directory path: ", Style::default().fg(Theme::TEXT_PRIMARY));
            y += 1;
            let input_rect = Rect::new(inner.left(), y, inner.width.saturating_sub(16), 1);
            for x in input_rect.left()..input_rect.right() {
                buf[(x, y)].set_bg(Theme::BG_CARD);
            }
            let input_text = format!(" {}█", self.new_folder_input);
            buf.set_string(inner.left(), y, &input_text, Style::default().fg(Theme::OMARCHY_GREEN).add_modifier(Modifier::BOLD));

            let add_btn_x = input_rect.right() + 2;
            let add_rect = Rect::new(add_btn_x, y, 10, 1);
            buf.set_string(add_btn_x, y, "[Confirm]", Style::default().fg(Theme::OMARCHY_GREEN).add_modifier(Modifier::BOLD));
            targets.push(SettingsClickTarget {
                rect: add_rect,
                action: SettingsAction::AddFolder,
            });
            y += 2;
        } else {
            let add_rect = Rect::new(inner.left(), y, 18, 1);
            buf.set_string(inner.left(), y, " ➕ [Add Folder...]", Style::default().fg(Theme::ACCENT_CYAN).add_modifier(Modifier::BOLD));
            targets.push(SettingsClickTarget {
                rect: add_rect,
                action: SettingsAction::AddFolder,
            });
            y += 2;
        }

        // Scan button & status
        let scan_btn_label = if self.is_scanning {
            " ⏳ [Scanning Library...]"
        } else {
            " 🔄 [Scan Library Now]"
        };
        let scan_rect = Rect::new(inner.left(), y, scan_btn_label.len() as u16, 1);
        buf.set_string(inner.left(), y, scan_btn_label, Style::default().fg(Theme::OMARCHY_GREEN).add_modifier(Modifier::BOLD));
        targets.push(SettingsClickTarget {
            rect: scan_rect,
            action: SettingsAction::ScanLibrary,
        });

        if !self.scan_status.is_empty() {
            buf.set_string(inner.left() + scan_btn_label.len() as u16 + 2, y, self.scan_status, Style::default().fg(Theme::TEXT_SECONDARY));
        }

        y += 3;
        // Section: System & Audio Info
        if y < inner.bottom().saturating_sub(4) {
            buf.set_string(
                inner.left(),
                y,
                "SYSTEM & AUDIO SUBSYSTEM",
                Style::default().fg(Theme::OMARCHY_GREEN).add_modifier(Modifier::BOLD),
            );
            y += 1;
            buf.set_string(
                inner.left(),
                y,
                "• Operating System: Omarchy Linux 4.0.4 (Arch-based rolling)",
                Style::default().fg(Theme::TEXT_SECONDARY),
            );
            y += 1;
            buf.set_string(
                inner.left(),
                y,
                "• Audio Engine: Symphonia (Bit-perfect multi-format decoder) + CPAL (ALSA / PipeWire)",
                Style::default().fg(Theme::TEXT_SECONDARY),
            );
            y += 1;
            buf.set_string(
                inner.left(),
                y,
                "• Codecs Supported: FLAC, MP3, OGG Vorbis/Opus, WAV, AIFF, M4A/AAC, ALAC",
                Style::default().fg(Theme::TEXT_SECONDARY),
            );
        }

        targets
    }
}
