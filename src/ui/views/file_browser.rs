use crate::library::scanner::Scanner;
use crate::ui::theme::Theme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, Widget};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct FileBrowserItem {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
}

pub struct FileBrowserClickTarget {
    pub rect: Rect,
    pub item_index: usize,
}

pub struct FileBrowserView<'a> {
    pub current_dir: &'a Path,
    pub items: &'a [FileBrowserItem],
    pub selected_index: usize,
    pub scroll_offset: usize,
    pub is_focused: bool,
}

impl<'a> FileBrowserView<'a> {
    pub fn list_directory(dir: &Path) -> Vec<FileBrowserItem> {
        let mut items = Vec::new();

        if let Some(parent) = dir.parent() {
            items.push(FileBrowserItem {
                name: ".. [Parent Directory]".to_string(),
                path: parent.to_path_buf(),
                is_dir: true,
            });
        }

        if let Ok(entries) = fs::read_dir(dir) {
            let mut dirs = Vec::new();
            let mut files = Vec::new();

            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                let file_name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string();

                if file_name.starts_with('.') {
                    continue;
                }

                if path.is_dir() {
                    dirs.push(FileBrowserItem {
                        name: file_name,
                        path,
                        is_dir: true,
                    });
                } else if Scanner::is_audio_file(&path) {
                    files.push(FileBrowserItem {
                        name: file_name,
                        path,
                        is_dir: false,
                    });
                }
            }

            dirs.sort_by(|a, b| a.name.cmp(&b.name));
            files.sort_by(|a, b| a.name.cmp(&b.name));

            items.extend(dirs);
            items.extend(files);
        }

        items
    }

    pub fn render_with_targets(
        &self,
        area: Rect,
        buf: &mut Buffer,
    ) -> Vec<FileBrowserClickTarget> {
        let mut targets = Vec::new();

        let border_style = if self.is_focused {
            Theme::block_focus_style()
        } else {
            Theme::block_style()
        };

        let title = format!(" File Browser: {} ", self.current_dir.display());
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(title)
            .title_style(Style::default().fg(Theme::OMARCHY_GREEN).add_modifier(Modifier::BOLD));

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.width < 10 || inner.height < 2 {
            return targets;
        }

        let rows_avail = inner.height as usize;
        let start_idx = self.scroll_offset.min(self.items.len());
        let end_idx = (start_idx + rows_avail).min(self.items.len());

        for (i, item) in self.items[start_idx..end_idx].iter().enumerate() {
            let actual_idx = start_idx + i;
            let y = inner.top() + i as u16;
            let is_sel = actual_idx == self.selected_index;

            let row_rect = Rect::new(inner.left(), y, inner.width, 1);
            targets.push(FileBrowserClickTarget {
                rect: row_rect,
                item_index: actual_idx,
            });

            let style = if is_sel {
                Theme::selected_row()
            } else if item.is_dir {
                Style::default().fg(Theme::ACCENT_CYAN).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Theme::TEXT_PRIMARY)
            };

            if is_sel {
                for x in row_rect.left()..row_rect.right() {
                    buf[(x, y)].set_bg(Theme::BG_HIGHLIGHT);
                }
            }

            let icon = if item.is_dir { "📁 " } else { "🎵 " };
            let line = format!("{}{}", icon, item.name);
            let trunc: String = line.chars().take(inner.width as usize).collect();
            buf.set_string(inner.left(), y, &trunc, style);
        }

        targets
    }
}
