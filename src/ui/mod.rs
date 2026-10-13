pub mod artwork;
pub mod components;
pub mod mouse;
pub mod spectrum;
pub mod theme;
pub mod views;
pub mod waveform_widget;

use crate::app::{App, FileBrowserItemTarget, FocusArea};
use crate::ui::components::about_modal::AboutModal;
use crate::ui::components::folder_dialog::FolderDialog;
use crate::ui::components::header::HeaderComponent;
use crate::ui::components::help_modal::HelpModal;
use crate::ui::components::player_bar::PlayerBar;
use crate::ui::components::playlist_dialog::PlaylistDialog;
use crate::ui::components::sidebar::{SidebarComponent, ViewMode};
use crate::ui::views::albums_view::AlbumsView;
use crate::ui::views::artists_view::ArtistsView;
use crate::ui::views::file_browser::FileBrowserView;
use crate::ui::views::now_playing::NowPlayingView;
use crate::ui::views::playlists_view::PlaylistsView;
use crate::ui::views::search_results_view::SearchResultsView;
use crate::ui::views::settings_view::SettingsView;
use crate::ui::views::songs_view::SongsView;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, BorderType, Borders, Widget};
use ratatui::Frame;

pub fn render_app(frame: &mut Frame, app: &mut App) {
    let area = frame.area();
    if area.width < 30 || area.height < 10 {
        return;
    }

    // If initial loading scan is in progress, render splash loader overlay
    if app.initial_loading {
        render_initial_loading(frame, app);
        return;
    }

    // Fill root terminal area with Omarchy background color
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            let cell = &mut frame.buffer_mut()[(x, y)];
            cell.set_bg(app.theme.background);
            cell.set_fg(app.theme.foreground);
        }
    }

    let top_margin = 1u16;
    let header_height = 3u16; // 2 rounded border lines + 1 inner content line
    let player_height = 8u16; // 2 border lines + 1 top padding line + 5 inner content lines (1 info + 3 controls + 1 seekbar)
    let center_height = area
        .height
        .saturating_sub(top_margin + header_height + player_height);

    let header_area = Rect::new(area.left(), area.top() + top_margin, area.width, header_height);
    let center_area = Rect::new(
        area.left(),
        area.top() + top_margin + header_height,
        area.width,
        center_height,
    );
    let player_area = Rect::new(area.left(), area.bottom().saturating_sub(player_height), area.width, player_height);

    // 1. Render Header
    let view_title = match app.view_mode {
        ViewMode::Songs => "Library > Songs",
        ViewMode::Artists => "Library > Artists",
        ViewMode::Albums => "Library > Albums",
        ViewMode::NowPlaying => "Visualizer",
        ViewMode::Playlists => "Playlists",
        ViewMode::FileBrowser => "File Browser",
        ViewMode::Settings => "Settings",
        ViewMode::SearchResults => "Search Results",
    };

    let header_comp = HeaderComponent {
        view_title,
        total_tracks: app.library.tracks.len(),
        search_query: &app.search_query,
        is_searching: app.is_searching,
        theme: &app.theme,
    };
    app.last_header_targets = Some(header_comp.render_with_targets(header_area, frame.buffer_mut()));

    // 2. Render Center Area: Sidebar (left) + Main View (right)
    let sidebar_width = 24u16.min(center_area.width / 3).max(18);
    let main_width = center_area.width.saturating_sub(sidebar_width);

    let sidebar_area = Rect::new(center_area.left(), center_area.top(), sidebar_width, center_area.height);
    let main_area = Rect::new(center_area.left() + sidebar_width, center_area.top(), main_width, center_area.height);
    app.last_main_area = main_area;

    // Sidebar
    let sidebar_comp = SidebarComponent {
        current_view: app.view_mode,
        current_playlist_id: Some(&app.active_playlist_id),
        playlists_manager: &app.playlists_manager,
        is_focused: app.focus == FocusArea::Sidebar,
        selected_index: app.sidebar_index,
        theme: &app.theme,
    };
    app.last_sidebar_targets = sidebar_comp.render_with_targets(sidebar_area, frame.buffer_mut());

    // Main View
    let is_main_focused = app.focus == FocusArea::MainView;
    let playing_track = app.audio.current_track();
    let playing_id = playing_track.as_ref().map(|t| t.id.as_str());

    // Clear previous targets for current view
    app.last_songs_targets.clear();
    app.last_artists_targets.clear();
    app.last_albums_targets.clear();
    app.last_now_playing_target = None;
    app.last_playlists_targets.clear();
    app.last_browser_targets.clear();
    app.last_settings_targets.clear();
    app.last_search_targets.clear();

    match app.view_mode {
        ViewMode::Songs => {
            let searched_tracks = app.library.search(&app.search_query);
            let songs_view = SongsView {
                tracks: &searched_tracks,
                selected_index: app.songs_selected_idx,
                scroll_offset: app.songs_scroll_offset,
                playing_track_id: playing_id,
                playlists_manager: &app.playlists_manager,
                is_focused: is_main_focused,
                theme: &app.theme,
            };
            app.last_songs_targets = songs_view.render_with_targets(main_area, frame.buffer_mut());
        }
        ViewMode::Artists => {
            let artists_view = ArtistsView {
                library: &app.library,
                active_column: app.artists_col,
                selected_artist_idx: app.artists_selected_idx,
                selected_album_idx: app.artists_album_idx,
                selected_song_idx: app.artists_song_idx,
                scroll_artist_offset: app.artists_scroll_offset,
                scroll_album_offset: app.artists_album_scroll_offset,
                scroll_song_offset: app.artists_song_scroll_offset,
                playing_track_id: playing_id,
                playlists_manager: &app.playlists_manager,
                is_focused: is_main_focused,
                theme: &app.theme,
            };
            app.last_artists_targets = artists_view.render_with_targets(main_area, frame.buffer_mut());
        }
        ViewMode::Albums => {
            let albums = app.library.all_albums();
            let albums_view = AlbumsView {
                library: &app.library,
                albums: &albums,
                active_column: app.albums_col,
                selected_album_idx: app.albums_selected_idx,
                selected_song_idx: app.albums_song_idx,
                scroll_album_offset: app.albums_scroll_offset,
                scroll_song_offset: app.albums_song_scroll_offset,
                playing_track_id: playing_id,
                playlists_manager: &app.playlists_manager,
                is_focused: is_main_focused,
                theme: &app.theme,
            };
            app.last_albums_targets = albums_view.render_with_targets(main_area, frame.buffer_mut());
        }
        ViewMode::NowPlaying => {
            let waveform = app.audio.current_waveform();
            let np_view = NowPlayingView {
                current_track: playing_track.as_ref(),
                elapsed: app.audio.elapsed(),
                total_duration: app.audio.total_duration(),
                artwork_cache: &app.artwork_cache,
                visualizer: app.audio.visualizer(),
                waveform: waveform.as_ref(),
                is_focused: is_main_focused,
                theme: &app.theme,
            };
            app.last_now_playing_target = np_view.render_with_targets(main_area, frame.buffer_mut());
        }
        ViewMode::Playlists => {
            let playlists_view = PlaylistsView {
                library: &app.library,
                playlists_manager: &app.playlists_manager,
                selected_playlist_id: &app.active_playlist_id,
                selected_track_idx: app.playlists_track_idx,
                scroll_playlist_offset: app.playlists_list_scroll_offset,
                scroll_track_offset: app.playlists_scroll_offset,
                playing_track_id: playing_id,
                is_focused: is_main_focused,
                theme: &app.theme,
            };
            app.last_playlists_targets = playlists_view.render_with_targets(main_area, frame.buffer_mut());
        }
        ViewMode::FileBrowser => {
            let browser_view = FileBrowserView {
                current_dir: &app.browser_current_dir,
                items: &app.browser_items,
                selected_index: app.browser_selected_idx,
                scroll_offset: app.browser_scroll_offset,
                is_focused: is_main_focused,
                theme: &app.theme,
            };
            let raw_targets = browser_view.render_with_targets(main_area, frame.buffer_mut());
            app.last_browser_targets = raw_targets
                .into_iter()
                .map(|t| FileBrowserItemTarget {
                    rect: t.rect,
                    item_index: t.item_index,
                })
                .collect();
        }
        ViewMode::Settings => {
            let settings_view = SettingsView {
                config: &app.config,
                selected_folder_idx: app.settings_selected_folder,
                new_folder_input: &app.new_folder_input,
                is_entering_folder: app.is_entering_folder,
                is_scanning: app.is_scanning,
                scan_status: &app.scan_status,
                is_focused: is_main_focused,
                theme: &app.theme,
            };
            app.last_settings_targets = settings_view.render_with_targets(main_area, frame.buffer_mut());
        }
        ViewMode::SearchResults => {
            let matching_artists = app.library.search_artists(&app.search_query);
            let matching_albums = app.library.search_albums(&app.search_query);
            let matching_tracks = app.library.search_tracks(&app.search_query);
            let search_view = SearchResultsView {
                search_query: &app.search_query,
                matching_artists: &matching_artists,
                matching_albums: &matching_albums,
                matching_tracks: &matching_tracks,
                active_section: app.search_section,
                selected_artist_idx: app.search_artist_idx,
                selected_album_idx: app.search_album_idx,
                selected_song_idx: app.search_song_idx,
                scroll_artist_offset: app.search_artist_scroll,
                scroll_album_offset: app.search_album_scroll,
                scroll_song_offset: app.search_song_scroll,
                playing_track_id: playing_id,
                playlists_manager: &app.playlists_manager,
                is_focused: is_main_focused,
                theme: &app.theme,
            };
            app.last_search_targets = search_view.render_with_targets(main_area, frame.buffer_mut());
        }
    }

    // 3. Render Bottom Player Bar
    let current_waveform = app.audio.current_waveform();
    let player_bar = PlayerBar {
        current_track: playing_track.as_ref(),
        playback_state: app.audio.state(),
        elapsed: app.audio.elapsed(),
        total_duration: app.audio.total_duration(),
        volume: app.audio.volume(),
        is_muted: app.is_muted,
        repeat_mode: app.config.repeat_mode,
        shuffle: app.config.shuffle,
        waveform: current_waveform.as_ref(),
        artwork_cache: &app.artwork_cache,
        is_focused: app.focus == FocusArea::PlayerBar,
        theme: &app.theme,
    };
    app.last_player_targets = player_bar.render_with_targets(player_area, frame.buffer_mut());

    // 4. Overlays & Modals
    app.last_dialog_targets.clear();
    if let Some((mode, track)) = &app.playlist_dialog {
        let dialog = PlaylistDialog {
            mode,
            track: track.as_ref(),
            playlists_manager: &app.playlists_manager,
            theme: &app.theme,
        };
        app.last_dialog_targets = dialog.render_with_targets(area, frame.buffer_mut());
    }

    app.last_folder_dialog_targets.clear();
    if let Some(folder_state) = &mut app.folder_dialog {
        let mut dialog = FolderDialog {
            state: folder_state,
            theme: &app.theme,
        };
        app.last_folder_dialog_targets = dialog.render_with_targets(area, frame.buffer_mut());
    }

    if app.show_help {
        HelpModal::render(area, frame.buffer_mut(), &app.theme);
    }

    app.last_about_target = None;
    if app.show_about {
        app.last_about_target = Some(AboutModal::render_with_targets(
            area,
            frame.buffer_mut(),
            &app.theme,
        ));
    }
}

fn render_initial_loading(frame: &mut Frame, app: &App) {
    let area = frame.area();
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            let cell = &mut frame.buffer_mut()[(x, y)];
            cell.set_bg(app.theme.background);
        }
    }

    let box_w = 72u16.min(area.width.saturating_sub(4));
    let box_h = 13u16.min(area.height.saturating_sub(2));
    let box_x = area.left() + (area.width.saturating_sub(box_w)) / 2;
    let box_y = area.top() + (area.height.saturating_sub(box_h)) / 2;
    let box_rect = Rect::new(box_x, box_y, box_w, box_h);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(app.theme.block_focus_style())
        .style(Style::default().bg(app.theme.dark_background))
        .title(" ● OmaFLAC Audiophile Player ")
        .title_style(app.theme.focused_title_style());

    let inner = block.inner(box_rect);
    block.render(box_rect, frame.buffer_mut());

    if inner.width < 8 || inner.height < 4 {
        return;
    }

    let max_text_w = inner.width.saturating_sub(4) as usize;

    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let spinner = crate::ui::artwork::SPINNER_CHARS
        [(ms / 80) as usize % crate::ui::artwork::SPINNER_CHARS.len()];

    let mut y = inner.top() + 1;
    let buf = frame.buffer_mut();

    let title_line: String = "♪  INDEXING AUDIO LIBRARY  ♪"
        .chars()
        .take(max_text_w)
        .collect();
    let title_x =
        inner.left() + (inner.width.saturating_sub(title_line.chars().count() as u16)) / 2;
    buf.set_string(
        title_x,
        y,
        &title_line,
        Style::default()
            .fg(app.theme.accent)
            .add_modifier(Modifier::BOLD),
    );
    y += 2;

    if y < inner.bottom() {
        let spin_msg: String = format!("{} Scanning audio folders in background...", spinner)
            .chars()
            .take(max_text_w)
            .collect();
        buf.set_string(
            inner.left() + 2,
            y,
            &spin_msg,
            Style::default()
                .fg(app.theme.cyan)
                .add_modifier(Modifier::BOLD),
        );
        y += 2;
    }

    if y < inner.bottom() {
        let status_str = if app.scan_status.is_empty() {
            "Discovering audio files...".to_string()
        } else {
            app.scan_status.clone()
        };
        let trunc_status: String = status_str.chars().take(max_text_w).collect();
        buf.set_string(inner.left() + 2, y, &trunc_status, app.theme.primary_text());
        y += 2;
    }

    if y < inner.bottom() {
        let count = app.scan_found_count.max(app.library.tracks.len());
        let badge: String = format!("Discovered tracks: {} audio files", count)
            .chars()
            .take(max_text_w)
            .collect();
        buf.set_string(inner.left() + 2, y, &badge, app.theme.secondary_text());
        y += 2;
    }

    if y < inner.bottom() {
        let footer: String = "Initial scan runs once. UI launches automatically when ready."
            .chars()
            .take(max_text_w)
            .collect();
        buf.set_string(inner.left() + 2, y, &footer, app.theme.muted_text());
    }
}
