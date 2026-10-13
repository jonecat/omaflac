use crate::ui::theme::OmarchyTheme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Widget};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FolderDialogFocus {
    PathInput,
    FolderList,
}

#[derive(Debug, Clone)]
pub struct FolderEntry {
    pub name: String,
    pub path: PathBuf,
    pub is_parent: bool,
}

#[derive(Debug, Clone)]
pub enum FolderDialogAction {
    FocusPathInput,
    ClearPathInput,
    JumpToPath(PathBuf),
    ToggleHidden,
    ClickEntry(usize),
    OpenEntry(usize),
    OpenSelected,
    ConfirmAdd,
    Cancel,
}

#[derive(Debug, Clone)]
pub struct FolderDialogClickTarget {
    pub rect: Rect,
    pub action: FolderDialogAction,
}

#[derive(Debug, Clone)]
pub struct FolderDialogState {
    pub path_input: String,
    pub current_dir: PathBuf,
    pub entries: Vec<FolderEntry>,
    pub selected_idx: usize,
    pub scroll_offset: usize,
    pub visible_rows: usize,
    pub show_hidden: bool,
    pub focus: FolderDialogFocus,
    pub status_message: Option<String>,
    pub last_list_rect: Rect,
}

impl FolderDialogState {
    pub fn new(initial_dir: PathBuf) -> Self {
        let start_dir = if initial_dir.is_dir() {
            initial_dir
        } else {
            dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"))
        };

        let mut state = Self {
            path_input: start_dir.display().to_string(),
            current_dir: start_dir,
            entries: Vec::new(),
            selected_idx: 0,
            scroll_offset: 0,
            visible_rows: 10,
            show_hidden: false,
            focus: FolderDialogFocus::PathInput,
            status_message: None,
            last_list_rect: Rect::default(),
        };
        state.refresh_entries();
        state
    }

    pub fn resolve_input_path(&self) -> PathBuf {
        let trimmed = self.path_input.trim();
        if trimmed == "~" {
            if let Some(home) = dirs::home_dir() {
                return home;
            }
        } else if let Some(rest) = trimmed.strip_prefix("~/") {
            if let Some(home) = dirs::home_dir() {
                return home.join(rest);
            }
        }
        PathBuf::from(trimmed)
    }

    pub fn refresh_entries(&mut self) {
        let mut entries = Vec::new();

        if let Some(parent) = self.current_dir.parent() {
            entries.push(FolderEntry {
                name: ".. [Parent Directory]".to_string(),
                path: parent.to_path_buf(),
                is_parent: true,
            });
        }

        if let Ok(read_dir) = fs::read_dir(&self.current_dir) {
            let mut subdirs = Vec::new();
            for entry in read_dir.filter_map(|e| e.ok()) {
                let path = entry.path();
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string();

                if name.is_empty() {
                    continue;
                }
                if !self.show_hidden && name.starts_with('.') {
                    continue;
                }
                if path.is_dir() {
                    subdirs.push(FolderEntry {
                        name,
                        path,
                        is_parent: false,
                    });
                }
            }

            subdirs.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
            entries.extend(subdirs);
        }

        self.entries = entries;
        if self.selected_idx >= self.entries.len() {
            self.selected_idx = 0;
        }
        self.sync_scroll();
    }

    pub fn enter_directory(&mut self, dir: PathBuf) {
        if dir.is_dir() {
            self.current_dir = dir;
            self.path_input = self.current_dir.display().to_string();
            self.selected_idx = 0;
            self.scroll_offset = 0;
            self.status_message = None;
            self.refresh_entries();
        }
    }

    pub fn go_parent(&mut self) {
        if let Some(parent) = self.current_dir.parent().map(|p| p.to_path_buf()) {
            self.enter_directory(parent);
        }
    }

    pub fn select_index(&mut self, idx: usize) {
        if self.entries.is_empty() {
            return;
        }
        self.selected_idx = idx.min(self.entries.len() - 1);
        self.sync_scroll();
        self.status_message = None;

        if let Some(entry) = self.entries.get(self.selected_idx) {
            if entry.is_parent {
                self.path_input = self.current_dir.display().to_string();
            } else {
                self.path_input = entry.path.display().to_string();
            }
        }
    }

    pub fn open_selected_entry(&mut self) {
        if let Some(entry) = self.entries.get(self.selected_idx).cloned() {
            self.enter_directory(entry.path);
        }
    }

    pub fn on_path_input_edited(&mut self) {
        self.status_message = None;
        let resolved = self.resolve_input_path();
        if resolved.is_dir() {
            if resolved != self.current_dir {
                self.current_dir = resolved;
                self.selected_idx = 0;
                self.scroll_offset = 0;
                self.refresh_entries();
            }
        } else if let Some(parent) = resolved.parent() {
            if parent.is_dir() {
                if parent != self.current_dir {
                    self.current_dir = parent.to_path_buf();
                    self.scroll_offset = 0;
                    self.refresh_entries();
                }
                if let Some(partial) = resolved.file_name().and_then(|n| n.to_str()) {
                    let partial_lower = partial.to_lowercase();
                    if let Some(pos) = self
                        .entries
                        .iter()
                        .position(|e| !e.is_parent && e.name.to_lowercase().starts_with(&partial_lower))
                    {
                        self.selected_idx = pos;
                        self.sync_scroll();
                    }
                }
            }
        }
    }

    pub fn sync_scroll(&mut self) {
        let vis = self.visible_rows.max(1);
        if self.selected_idx < self.scroll_offset {
            self.scroll_offset = self.selected_idx;
        } else if self.selected_idx >= self.scroll_offset + vis {
            self.scroll_offset = self.selected_idx + 1 - vis;
        }
    }

    pub fn scroll_by(&mut self, is_up: bool) {
        let vis = self.visible_rows.max(1);
        let max_scroll = self.entries.len().saturating_sub(vis);
        if is_up {
            self.scroll_offset = self.scroll_offset.min(max_scroll).saturating_sub(1);
        } else {
            self.scroll_offset = (self.scroll_offset + 1).min(max_scroll);
        }
    }
}

pub struct FolderDialog<'a> {
    pub state: &'a mut FolderDialogState,
    pub theme: &'a OmarchyTheme,
}

impl<'a> FolderDialog<'a> {
    fn render_button_cap(
        &self,
        buf: &mut Buffer,
        x: u16,
        y: u16,
        btn_h: u16,
        label: &str,
        fg_style: Style,
        border_style: Style,
        is_highlighted: bool,
    ) -> Rect {
        let label_chars = label.chars().count() as u16;
        let width = label_chars + 4;
        let bg_color = if is_highlighted {
            self.theme.lighter_background
        } else {
            self.theme.dark_background
        };

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

    pub fn render_with_targets(
        &mut self,
        area: Rect,
        buf: &mut Buffer,
    ) -> Vec<FolderDialogClickTarget> {
        let mut targets = Vec::new();

        let modal_width = 78u16.min(area.width.saturating_sub(4));
        let modal_height = 28u16.min(area.height.saturating_sub(2));
        if modal_width < 30 || modal_height < 14 {
            return targets;
        }

        let x = area.left() + (area.width.saturating_sub(modal_width)) / 2;
        let y = area.top() + (area.height.saturating_sub(modal_height)) / 2;
        let modal_rect = Rect::new(x, y, modal_width, modal_height);

        Clear.render(modal_rect, buf);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(self.theme.block_focus_style())
            .style(Style::default().bg(self.theme.background))
            .title(" ● Add Library Folder ")
            .title_style(self.theme.focused_title_style());

        let inner = block.inner(modal_rect);
        block.render(modal_rect, buf);

        for my in inner.top()..inner.bottom() {
            for mx in inner.left()..inner.right() {
                buf[(mx, my)].set_bg(self.theme.background);
            }
        }

        let mut cur_y = inner.top() + 1;
        let content_x = inner.left() + 1;
        let content_w = inner.width.saturating_sub(2);

        // 1. Header row for Path Input
        let is_input_focused = self.state.focus == FolderDialogFocus::PathInput;
        let path_label = if is_input_focused {
            "● FOLDER PATH (Type path or browse below)"
        } else {
            "FOLDER PATH (Click or Tab to type)"
        };
        buf.set_string(
            content_x,
            cur_y,
            path_label,
            if is_input_focused {
                self.theme.accent_text()
            } else {
                self.theme.secondary_text()
            },
        );

        // Validation badge & [Clear] button on the right of the label row
        let resolved = self.state.resolve_input_path();
        let is_valid_dir = resolved.is_dir();
        let clear_label = "[Clear]";
        let clear_w = clear_label.len() as u16;
        let clear_x = content_x + content_w.saturating_sub(clear_w);
        let clear_rect = Rect::new(clear_x, cur_y, clear_w, 1);
        buf.set_string(
            clear_x,
            cur_y,
            clear_label,
            Style::default()
                .fg(self.theme.cyan)
                .add_modifier(Modifier::BOLD),
        );
        targets.push(FolderDialogClickTarget {
            rect: clear_rect,
            action: FolderDialogAction::ClearPathInput,
        });

        let status_badge = if is_valid_dir {
            ("✓ Valid Folder", Style::default().fg(self.theme.accent))
        } else {
            ("! Path Not Found", Style::default().fg(self.theme.red))
        };
        let badge_w = status_badge.0.chars().count() as u16;
        if clear_x > content_x + 36 + badge_w {
            buf.set_string(clear_x - badge_w - 2, cur_y, status_badge.0, status_badge.1);
        }
        cur_y += 1;

        // 2. Rounded Path Input Box (3 rows)
        let input_box_rect = Rect::new(content_x, cur_y, content_w, 3);
        let input_border_style = if is_input_focused {
            self.theme.block_focus_style()
        } else {
            self.theme.block_style()
        };
        for iy in input_box_rect.top()..input_box_rect.bottom() {
            for ix in input_box_rect.left()..input_box_rect.right() {
                buf[(ix, iy)].set_bg(self.theme.dark_background);
            }
        }
        let input_block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(input_border_style)
            .style(Style::default().bg(self.theme.dark_background));
        let input_inner = input_block.inner(input_box_rect);
        input_block.render(input_box_rect, buf);

        let max_input_chars = input_inner.width.saturating_sub(3) as usize;
        let input_chars: Vec<char> = self.state.path_input.chars().collect();
        let visible_input: String = if input_chars.len() > max_input_chars {
            let start = input_chars.len() - max_input_chars;
            format!("…{}", input_chars[start..].iter().collect::<String>())
        } else {
            self.state.path_input.clone()
        };
        let display_input = if is_input_focused {
            format!(" {}█", visible_input)
        } else {
            format!(" {}", visible_input)
        };
        buf.set_string(
            input_inner.left(),
            input_inner.top(),
            &display_input,
            Style::default()
                .fg(self.theme.bright_foreground)
                .bg(self.theme.dark_background)
                .add_modifier(Modifier::BOLD),
        );
        targets.push(FolderDialogClickTarget {
            rect: input_box_rect,
            action: FolderDialogAction::FocusPathInput,
        });
        cur_y += 3;

        // 3. Quick-Jump Bar + Show Hidden Toggle
        let mut qx = content_x;
        buf.set_string(qx, cur_y, "Quick:", self.theme.muted_text());
        qx += 7;

        let mut quick_dirs: Vec<(&str, PathBuf)> = Vec::new();
        if let Some(home) = dirs::home_dir() {
            quick_dirs.push(("[~ Home]", home.clone()));
            let music = dirs::audio_dir().unwrap_or_else(|| home.join("Music"));
            if music.is_dir() {
                quick_dirs.push(("[♪ Music]", music));
            }
        }
        quick_dirs.push((
            "[/ Root]",
            PathBuf::from(if cfg!(windows) { "C:\\" } else { "/" }),
        ));
        for mount in ["/Volumes", "/mnt", "/media", "/run/media"] {
            let mp = PathBuf::from(mount);
            if mp.is_dir() {
                quick_dirs.push((
                    match mount {
                        "/Volumes" => "[/Volumes]",
                        "/mnt" => "[/mnt]",
                        _ => "[/media]",
                    },
                    mp,
                ));
                break;
            }
        }

        for (label, target_path) in quick_dirs {
            let w = label.chars().count() as u16;
            if qx + w + 18 < content_x + content_w {
                let is_cur = self.state.current_dir == target_path;
                let style = if is_cur {
                    self.theme.accent_text()
                } else {
                    Style::default()
                        .fg(self.theme.cyan)
                        .add_modifier(Modifier::BOLD)
                };
                buf.set_string(qx, cur_y, label, style);
                targets.push(FolderDialogClickTarget {
                    rect: Rect::new(qx, cur_y, w, 1),
                    action: FolderDialogAction::JumpToPath(target_path),
                });
                qx += w + 2;
            }
        }

        let hidden_label = if self.state.show_hidden {
            "[. Hidden: On]"
        } else {
            "[. Hidden: Off]"
        };
        let hw = hidden_label.len() as u16;
        let hx = content_x + content_w.saturating_sub(hw);
        if hx > qx {
            let h_style = if self.state.show_hidden {
                self.theme.accent_text()
            } else {
                self.theme.secondary_text()
            };
            buf.set_string(hx, cur_y, hidden_label, h_style);
            targets.push(FolderDialogClickTarget {
                rect: Rect::new(hx, cur_y, hw, 1),
                action: FolderDialogAction::ToggleHidden,
            });
        }
        cur_y += 1;

        // 4. Folder Explorer Sub-Box
        let bottom_reserved = if inner.height >= 20 { 4u16 } else { 2u16 };
        let explorer_h = inner
            .bottom()
            .saturating_sub(cur_y + bottom_reserved)
            .max(5);
        let explorer_rect = Rect::new(content_x, cur_y, content_w, explorer_h);
        self.state.last_list_rect = explorer_rect;

        let is_list_focused = self.state.focus == FolderDialogFocus::FolderList;
        let (exp_border, exp_title_style, exp_prefix) = if is_list_focused {
            (
                self.theme.block_focus_style(),
                self.theme.focused_title_style(),
                " ● Folders in ",
            )
        } else {
            (
                self.theme.block_style(),
                self.theme.unfocused_title_style(),
                " Folders in ",
            )
        };

        let cur_dir_str = self.state.current_dir.display().to_string();
        let max_title_dir = content_w.saturating_sub(18) as usize;
        let dir_chars: Vec<char> = cur_dir_str.chars().collect();
        let trunc_dir: String = if dir_chars.len() > max_title_dir {
            let start = dir_chars.len() - max_title_dir;
            format!("…{}", dir_chars[start..].iter().collect::<String>())
        } else {
            cur_dir_str
        };
        let exp_title = format!("{}{} ", exp_prefix, trunc_dir);

        let exp_block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(exp_border)
            .style(Style::default().bg(self.theme.dark_background))
            .title(exp_title)
            .title_style(exp_title_style);
        let exp_inner = exp_block.inner(explorer_rect);
        exp_block.render(explorer_rect, buf);

        for ey in exp_inner.top()..exp_inner.bottom() {
            for ex in exp_inner.left()..exp_inner.right() {
                buf[(ex, ey)].set_bg(self.theme.dark_background);
            }
        }

        let top_pad = 1u16.min(exp_inner.height);
        let list_top = exp_inner.top() + top_pad;
        let avail_h = exp_inner.height.saturating_sub(top_pad);
        let rows_avail = avail_h as usize;
        self.state.visible_rows = rows_avail.max(1);
        self.state.sync_scroll();

        let max_scroll = self.state.entries.len().saturating_sub(rows_avail);
        let start_idx = self.state.scroll_offset.min(max_scroll);
        let end_idx = (start_idx + rows_avail).min(self.state.entries.len());

        for (i, entry) in self.state.entries[start_idx..end_idx].iter().enumerate() {
            let actual_idx = start_idx + i;
            let row_y = list_top + i as u16;
            let is_sel = actual_idx == self.state.selected_idx;

            let row_rect = Rect::new(exp_inner.left(), row_y, exp_inner.width, 1);

            let style = if is_sel {
                self.theme.selected_row()
            } else if entry.is_parent {
                Style::default()
                    .fg(self.theme.magenta)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
                    .fg(self.theme.cyan)
                    .add_modifier(Modifier::BOLD)
            };

            if is_sel {
                for rx in row_rect.left()..row_rect.right() {
                    buf[(rx, row_y)].set_bg(self.theme.selection);
                }
            }

            let icon = if entry.is_parent { " ▴ " } else { " ▸ " };
            let line = format!("{}{}", icon, entry.name);
            let max_name_w = exp_inner.width.saturating_sub(12) as usize;
            let trunc: String = line.chars().take(max_name_w).collect();
            buf.set_string(exp_inner.left(), row_y, &trunc, style);

            // Action badge on the right side of the selected row
            if is_sel && exp_inner.width > 20 {
                let open_badge = if entry.is_parent {
                    "[Up ▴]"
                } else {
                    "[Open ▸]"
                };
                let bw = open_badge.chars().count() as u16;
                let bx = exp_inner.right().saturating_sub(bw + 1);
                let open_rect = Rect::new(bx, row_y, bw, 1);
                buf.set_string(
                    bx,
                    row_y,
                    open_badge,
                    Style::default()
                        .fg(self.theme.accent)
                        .bg(self.theme.selection)
                        .add_modifier(Modifier::BOLD),
                );
                targets.push(FolderDialogClickTarget {
                    rect: open_rect,
                    action: FolderDialogAction::OpenEntry(actual_idx),
                });
            }

            targets.push(FolderDialogClickTarget {
                rect: row_rect,
                action: FolderDialogAction::ClickEntry(actual_idx),
            });
        }

        if self.state.entries.len() == 1 && rows_avail > 2 {
            let empty_y = list_top + 1;
            buf.set_string(
                exp_inner.left() + 3,
                empty_y,
                "(No subdirectories in this folder — click '+ Add Folder' below to add it)",
                self.theme.muted_text(),
            );
        }

        self.theme.render_scrollbar(
            buf,
            explorer_rect,
            list_top,
            avail_h,
            start_idx,
            rows_avail,
            self.state.entries.len(),
        );

        cur_y += explorer_h;

        // 5. Bottom Action Buttons (3-row rounded button caps when space allows)
        let btn_h = if inner.bottom().saturating_sub(cur_y) >= 4 {
            3u16
        } else {
            1u16
        };
        let mut bx = content_x;

        let active_ctrl_style = Style::default()
            .fg(self.theme.accent)
            .add_modifier(Modifier::BOLD);
        let normal_ctrl_style = Style::default()
            .fg(self.theme.bright_foreground)
            .add_modifier(Modifier::BOLD);
        let cancel_ctrl_style = Style::default().fg(self.theme.secondary_text().fg.unwrap_or(self.theme.foreground));

        let add_rect = self.render_button_cap(
            buf,
            bx,
            cur_y,
            btn_h,
            "+ Add Folder",
            active_ctrl_style,
            self.theme.block_focus_style(),
            true,
        );
        targets.push(FolderDialogClickTarget {
            rect: add_rect,
            action: FolderDialogAction::ConfirmAdd,
        });
        bx += add_rect.width + 2;

        let open_label = if self
            .state
            .entries
            .get(self.state.selected_idx)
            .map(|e| e.is_parent)
            .unwrap_or(false)
        {
            "Parent Folder ▴"
        } else {
            "Open Subfolder ▸"
        };
        let open_btn_rect = self.render_button_cap(
            buf,
            bx,
            cur_y,
            btn_h,
            open_label,
            normal_ctrl_style,
            self.theme.block_style(),
            false,
        );
        targets.push(FolderDialogClickTarget {
            rect: open_btn_rect,
            action: FolderDialogAction::OpenSelected,
        });
        bx += open_btn_rect.width + 2;

        let cancel_rect = self.render_button_cap(
            buf,
            bx,
            cur_y,
            btn_h,
            "Cancel",
            cancel_ctrl_style,
            self.theme.block_style(),
            false,
        );
        targets.push(FolderDialogClickTarget {
            rect: cancel_rect,
            action: FolderDialogAction::Cancel,
        });

        // 6. Footer Hints / Error Status Row
        let hint_y = inner.bottom().saturating_sub(1);
        if let Some(err) = &self.state.status_message {
            let trunc_err: String = err.chars().take(content_w as usize).collect();
            buf.set_string(
                content_x,
                hint_y,
                &trunc_err,
                Style::default()
                    .fg(self.theme.red)
                    .add_modifier(Modifier::BOLD),
            );
        } else {
            let hint = if is_input_focused {
                "[Enter] Add Folder   [↓ / Tab] Browse Folders   [Ctrl+U] Clear   [Esc] Cancel"
            } else {
                "[Enter / →] Open Dir   [← / Bksp] Parent Dir   [Space] Add Folder   [Tab] Edit Path   [Esc] Cancel"
            };
            let trunc_hint: String = hint.chars().take(content_w as usize).collect();
            buf.set_string(content_x, hint_y, &trunc_hint, self.theme.muted_text());
        }

        targets
    }
}
