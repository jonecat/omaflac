use crate::config::AppConfig;
use crate::ui::theme::OmarchyTheme;
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
}

pub struct SettingsView<'a> {
    pub config: &'a AppConfig,
    pub selected_folder_idx: usize,
    pub new_folder_input: &'a str,
    pub is_entering_folder: bool,
    pub is_scanning: bool,
    pub scan_status: &'a str,
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
            inner.left(),
            y,
            "LIBRARY FOLDERS",
            self.theme.accent_text(),
        );
        y += 1;
        buf.set_string(
            inner.left(),
            y,
            "OmaFLAC scans these directories recursively for FLAC, MP3, OGG, WAV, M4A audio files.",
            self.theme.secondary_text(),
        );
        y += 2;

        for (i, folder) in self.config.library_folders.iter().enumerate() {
            if y >= inner.bottom().saturating_sub(8) {
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
            buf.set_string(inner.left(), y, "Enter directory path: ", self.theme.primary_text());
            y += 1;
            let input_rect = Rect::new(inner.left(), y, inner.width.saturating_sub(16), 1);
            for x in input_rect.left()..input_rect.right() {
                buf[(x, y)].set_bg(self.theme.lighter_background);
            }
            let input_text = format!(" {}█", self.new_folder_input);
            buf.set_string(inner.left(), y, &input_text, self.theme.accent_text());

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

        // Scan button & status
        let scan_btn_label = if self.is_scanning {
            " ⟳ [Scanning Library...]"
        } else {
            " ⟳ [Scan Library Now]"
        };
        let scan_rect = Rect::new(inner.left(), y, scan_btn_label.len() as u16, 1);
        buf.set_string(inner.left(), y, scan_btn_label, self.theme.accent_text());
        targets.push(SettingsClickTarget {
            rect: scan_rect,
            action: SettingsAction::ScanLibrary,
        });

        if !self.scan_status.is_empty() {
            buf.set_string(inner.left() + scan_btn_label.len() as u16 + 2, y, self.scan_status, self.theme.secondary_text());
        }

        y += 3;
        // Section: System & Audio Info
        if y < inner.bottom().saturating_sub(3) {
            buf.set_string(
                inner.left(),
                y,
                "SYSTEM & AUDIO SUBSYSTEM",
                self.theme.accent_text(),
            );
            y += 1;
            buf.set_string(
                inner.left(),
                y,
                "• Audio Engine: Symphonia (Bit-perfect multi-format decoder) + CPAL",
                self.theme.secondary_text(),
            );
            y += 1;
            buf.set_string(
                inner.left(),
                y,
                "• Codecs Supported: FLAC, MP3, OGG Vorbis/Opus, WAV, AIFF, M4A/AAC, ALAC",
                self.theme.secondary_text(),
            );
        }

        targets
    }
}
