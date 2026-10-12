use crate::library::playlist::PlaylistsManager;
use crate::ui::theme::Theme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, Widget};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    Songs,
    Artists,
    Albums,
    NowPlaying,
    Playlists,
    FileBrowser,
    Settings,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SidebarItem {
    View(ViewMode),
    Favorites,
    RecentlyPlayed,
    CustomPlaylist(String),
    NewPlaylist,
}

pub struct SidebarClickTarget {
    pub rect: Rect,
    pub item: SidebarItem,
}

pub struct SidebarComponent<'a> {
    pub current_view: ViewMode,
    pub current_playlist_id: Option<&'a str>,
    pub playlists_manager: &'a PlaylistsManager,
    pub is_focused: bool,
    pub selected_index: usize,
}

impl<'a> SidebarComponent<'a> {
    pub fn build_items(&self) -> Vec<(SidebarItem, String, bool)> {
        let mut items = Vec::new();

        // LIBRARY
        items.push((
            SidebarItem::View(ViewMode::Songs),
            " 🎵 Songs".to_string(),
            self.current_view == ViewMode::Songs,
        ));
        items.push((
            SidebarItem::View(ViewMode::Artists),
            " 🧑‍🎤 Artists".to_string(),
            self.current_view == ViewMode::Artists,
        ));
        items.push((
            SidebarItem::View(ViewMode::Albums),
            " 💿 Albums".to_string(),
            self.current_view == ViewMode::Albums,
        ));
        items.push((
            SidebarItem::View(ViewMode::NowPlaying),
            " 📊 Now Playing".to_string(),
            self.current_view == ViewMode::NowPlaying,
        ));
        items.push((
            SidebarItem::View(ViewMode::FileBrowser),
            " 📂 File Browser".to_string(),
            self.current_view == ViewMode::FileBrowser,
        ));

        // PLAYLISTS
        let is_fav_active = self.current_view == ViewMode::Playlists && self.current_playlist_id == Some("__favorites__");
        items.push((
            SidebarItem::Favorites,
            format!(" ⭐ Favorites ({})", self.playlists_manager.favorites.len()),
            is_fav_active,
        ));

        let is_recent_active = self.current_view == ViewMode::Playlists && self.current_playlist_id == Some("__recent__");
        items.push((
            SidebarItem::RecentlyPlayed,
            " 🕒 Recently Played".to_string(),
            is_recent_active,
        ));

        for p in &self.playlists_manager.playlists {
            let is_active = self.current_view == ViewMode::Playlists && self.current_playlist_id == Some(&p.id);
            items.push((
                SidebarItem::CustomPlaylist(p.id.clone()),
                format!(" 📑 {}", p.name),
                is_active,
            ));
        }

        items.push((
            SidebarItem::NewPlaylist,
            " ➕ New Playlist...".to_string(),
            false,
        ));

        // SYSTEM
        items.push((
            SidebarItem::View(ViewMode::Settings),
            " ⚙️ Settings / Folders".to_string(),
            self.current_view == ViewMode::Settings,
        ));

        items
    }

    pub fn render_with_targets(
        &self,
        area: Rect,
        buf: &mut Buffer,
    ) -> Vec<SidebarClickTarget> {
        let mut click_targets = Vec::new();

        let border_style = if self.is_focused {
            Theme::block_focus_style()
        } else {
            Theme::block_style()
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(" Library ")
            .title_style(Style::default().fg(Theme::OMARCHY_GREEN).add_modifier(Modifier::BOLD));

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.width < 4 || inner.height < 4 {
            return click_targets;
        }

        let mut y = inner.top();
        let items = self.build_items();

        let mut item_idx = 0;

        // Section: LIBRARY
        if y < inner.bottom() {
            buf.set_string(
                inner.left(),
                y,
                "LIBRARY",
                Style::default().fg(Theme::TEXT_MUTED).add_modifier(Modifier::BOLD),
            );
            y += 1;
        }

        for (item, label, is_active) in &items[0..5] {
            if y >= inner.bottom() {
                break;
            }
            let is_cursor = self.is_focused && self.selected_index == item_idx;
            let style = if *is_active {
                Style::default().bg(Theme::BG_HIGHLIGHT).fg(Theme::OMARCHY_GREEN).add_modifier(Modifier::BOLD)
            } else if is_cursor {
                Style::default().bg(Theme::BG_CARD).fg(Theme::TEXT_PRIMARY).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Theme::TEXT_PRIMARY)
            };

            let row_rect = Rect::new(inner.left(), y, inner.width, 1);
            // Fill background
            for x in row_rect.left()..row_rect.right() {
                buf[(x, y)].set_style(style);
            }
            buf.set_string(inner.left(), y, label, style);

            click_targets.push(SidebarClickTarget {
                rect: row_rect,
                item: item.clone(),
            });

            y += 1;
            item_idx += 1;
        }

        // Section: PLAYLISTS
        y += 1;
        if y < inner.bottom() {
            buf.set_string(
                inner.left(),
                y,
                "PLAYLISTS",
                Style::default().fg(Theme::TEXT_MUTED).add_modifier(Modifier::BOLD),
            );
            y += 1;
        }

        let playlist_items = &items[5..items.len() - 1];
        for (item, label, is_active) in playlist_items {
            if y >= inner.bottom() {
                break;
            }
            let is_cursor = self.is_focused && self.selected_index == item_idx;
            let style = if *is_active {
                Style::default().bg(Theme::BG_HIGHLIGHT).fg(Theme::OMARCHY_GREEN).add_modifier(Modifier::BOLD)
            } else if is_cursor {
                Style::default().bg(Theme::BG_CARD).fg(Theme::TEXT_PRIMARY).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Theme::TEXT_PRIMARY)
            };

            let row_rect = Rect::new(inner.left(), y, inner.width, 1);
            for x in row_rect.left()..row_rect.right() {
                buf[(x, y)].set_style(style);
            }
            buf.set_string(inner.left(), y, label, style);

            click_targets.push(SidebarClickTarget {
                rect: row_rect,
                item: item.clone(),
            });

            y += 1;
            item_idx += 1;
        }

        // Section: SYSTEM
        y += 1;
        if y < inner.bottom() {
            buf.set_string(
                inner.left(),
                y,
                "SYSTEM",
                Style::default().fg(Theme::TEXT_MUTED).add_modifier(Modifier::BOLD),
            );
            y += 1;
        }

        if let Some((item, label, is_active)) = items.last() {
            if y < inner.bottom() {
                let is_cursor = self.is_focused && self.selected_index == item_idx;
                let style = if *is_active {
                    Style::default().bg(Theme::BG_HIGHLIGHT).fg(Theme::OMARCHY_GREEN).add_modifier(Modifier::BOLD)
                } else if is_cursor {
                    Style::default().bg(Theme::BG_CARD).fg(Theme::TEXT_PRIMARY).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Theme::TEXT_PRIMARY)
                };

                let row_rect = Rect::new(inner.left(), y, inner.width, 1);
                for x in row_rect.left()..row_rect.right() {
                    buf[(x, y)].set_style(style);
                }
                buf.set_string(inner.left(), y, label, style);

                click_targets.push(SidebarClickTarget {
                    rect: row_rect,
                    item: item.clone(),
                });
            }
        }

        click_targets
    }
}
