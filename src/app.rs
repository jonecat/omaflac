use crate::audio::AudioEngine;
use crate::config::{AppConfig, RepeatMode};
use crate::library::playlist::PlaylistsManager;
use crate::library::scanner::{ScanUpdate, Scanner};
use crate::library::track::Track;
use crate::library::Library;
use crate::ui::artwork::ArtworkCache;
use crate::ui::components::about_modal::AboutModalClickTarget;
use crate::ui::components::folder_dialog::{
    FolderDialogAction, FolderDialogClickTarget, FolderDialogFocus, FolderDialogState,
};
use crate::ui::components::header::HeaderClickTarget;
use crate::ui::components::player_bar::{PlayerAction, PlayerClickTarget};
use crate::ui::components::playlist_dialog::{PlaylistDialogClickTarget, PlaylistDialogMode};
use crate::ui::components::sidebar::{SidebarClickTarget, SidebarComponent, SidebarItem, ViewMode};
use crate::ui::mouse::{is_inside, relative_x_ratio};
use crate::ui::views::albums_view::{AlbumColumn, AlbumsViewClickTarget};
use crate::ui::views::artists_view::{ArtistColumn, ArtistsViewClickTarget};
use crate::ui::views::search_results_view::{SearchResultsClickTarget, SearchSection};
use crate::ui::theme::OmarchyTheme;
use crate::ui::views::file_browser::{FileBrowserItem, FileBrowserView};
use crate::ui::views::now_playing::NowPlayingClickTarget;
use crate::ui::views::playlists_view::PlaylistsViewClickTarget;
use crate::ui::views::settings_view::{SettingsAction, SettingsClickTarget};
use crate::ui::views::songs_view::SongsViewClickTarget;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use rand::seq::SliceRandom;
use ratatui::layout::Rect;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusArea {
    Sidebar,
    MainView,
    PlayerBar,
}

pub struct App {
    pub theme: OmarchyTheme,
    pub config: AppConfig,
    pub library: Library,
    pub playlists_manager: PlaylistsManager,
    pub audio: AudioEngine,
    pub artwork_cache: ArtworkCache,

    // Navigation & focus
    pub view_mode: ViewMode,
    pub focus: FocusArea,
    pub sidebar_index: usize,
    pub should_quit: bool,

    // Queue & playback
    pub playback_queue: Vec<Track>,
    pub queue_index: usize,
    pub is_muted: bool,
    pub pre_mute_volume: f32,

    // Global Search
    pub search_query: String,
    pub is_searching: bool,
    pub previous_view_mode: ViewMode,
    pub search_section: SearchSection,
    pub search_artist_idx: usize,
    pub search_album_idx: usize,
    pub search_song_idx: usize,
    pub search_artist_scroll: usize,
    pub search_album_scroll: usize,
    pub search_song_scroll: usize,

    // Modals
    pub show_help: bool,
    pub show_about: bool,
    pub playlist_dialog: Option<(PlaylistDialogMode, Option<Track>)>,
    pub folder_dialog: Option<FolderDialogState>,

    // Songs view state
    pub songs_selected_idx: usize,
    pub songs_scroll_offset: usize,

    // Artists view state
    pub artists_col: ArtistColumn,
    pub artists_selected_idx: usize,
    pub artists_album_idx: usize,
    pub artists_song_idx: usize,
    pub artists_scroll_offset: usize,
    pub artists_album_scroll_offset: usize,
    pub artists_song_scroll_offset: usize,

    // Albums view state
    pub albums_col: AlbumColumn,
    pub albums_selected_idx: usize,
    pub albums_song_idx: usize,
    pub albums_scroll_offset: usize,
    pub albums_song_scroll_offset: usize,

    // Playlists view state
    pub active_playlist_id: String,
    pub playlists_track_idx: usize,
    pub playlists_list_scroll_offset: usize,
    pub playlists_scroll_offset: usize,

    // File Browser view state
    pub browser_current_dir: PathBuf,
    pub browser_items: Vec<FileBrowserItem>,
    pub browser_selected_idx: usize,
    pub browser_scroll_offset: usize,

    // Settings view state
    pub settings_selected_folder: usize,
    pub new_folder_input: String,
    pub is_entering_folder: bool,
    pub is_scanning: bool,
    pub scan_status: String,
    pub scan_found_count: usize,
    pub initial_loading: bool,
    pub scan_receiver: Option<crossbeam_channel::Receiver<ScanUpdate>>,

    // Mouse Hit Targets recorded from previous render frame
    pub last_main_area: Rect,
    pub last_header_targets: Option<HeaderClickTarget>,
    pub last_sidebar_targets: Vec<SidebarClickTarget>,
    pub last_player_targets: Vec<PlayerClickTarget>,
    pub last_songs_targets: Vec<SongsViewClickTarget>,
    pub last_artists_targets: Vec<ArtistsViewClickTarget>,
    pub last_albums_targets: Vec<AlbumsViewClickTarget>,
    pub last_now_playing_target: Option<NowPlayingClickTarget>,
    pub last_playlists_targets: Vec<PlaylistsViewClickTarget>,
    pub last_browser_targets: Vec<FileBrowserItemTarget>,
    pub last_settings_targets: Vec<SettingsClickTarget>,
    pub last_dialog_targets: Vec<PlaylistDialogClickTarget>,
    pub last_folder_dialog_targets: Vec<FolderDialogClickTarget>,
    pub last_about_target: Option<AboutModalClickTarget>,
    pub last_search_targets: Vec<SearchResultsClickTarget>,
}

pub struct FileBrowserItemTarget {
    pub rect: Rect,
    pub item_index: usize,
}

impl App {
    pub fn new() -> anyhow::Result<Self> {
        let config = AppConfig::load();
        let library = Library::load();
        let playlists_manager = PlaylistsManager::load();
        let audio = AudioEngine::new()?;
        let artwork_cache = ArtworkCache::new();

        // If library is empty but config has folders, trigger initial scan asynchronously
        let needs_initial_scan = library.tracks.is_empty() && !config.library_folders.is_empty();

        let initial_dir = config
            .library_folders
            .first()
            .cloned()
            .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")));

        let browser_items = FileBrowserView::list_directory(&initial_dir);

        let theme = OmarchyTheme::load();

        let mut app = Self {
            theme,
            config,
            library,
            playlists_manager,
            audio,
            artwork_cache,
            view_mode: ViewMode::Artists,
            focus: FocusArea::MainView,
            sidebar_index: 0,
            should_quit: false,
            playback_queue: Vec::new(),
            queue_index: 0,
            is_muted: false,
            pre_mute_volume: 0.8,
            search_query: String::new(),
            is_searching: false,
            previous_view_mode: ViewMode::Artists,
            search_section: SearchSection::Songs,
            search_artist_idx: 0,
            search_album_idx: 0,
            search_song_idx: 0,
            search_artist_scroll: 0,
            search_album_scroll: 0,
            search_song_scroll: 0,
            show_help: false,
            show_about: false,
            playlist_dialog: None,
            folder_dialog: None,
            songs_selected_idx: 0,
            songs_scroll_offset: 0,
            artists_col: ArtistColumn::Artists,
            artists_selected_idx: 0,
            artists_album_idx: 0,
            artists_song_idx: 0,
            artists_scroll_offset: 0,
            artists_album_scroll_offset: 0,
            artists_song_scroll_offset: 0,
            albums_col: AlbumColumn::Albums,
            albums_selected_idx: 0,
            albums_song_idx: 0,
            albums_scroll_offset: 0,
            albums_song_scroll_offset: 0,
            active_playlist_id: "__favorites__".to_string(),
            playlists_track_idx: 0,
            playlists_list_scroll_offset: 0,
            playlists_scroll_offset: 0,
            browser_current_dir: initial_dir,
            browser_items,
            browser_selected_idx: 0,
            browser_scroll_offset: 0,
            settings_selected_folder: 0,
            new_folder_input: String::new(),
            is_entering_folder: bool::default(),
            is_scanning: false,
            scan_status: String::new(),
            scan_found_count: 0,
            initial_loading: needs_initial_scan,
            scan_receiver: None,
            last_main_area: Rect::default(),
            last_header_targets: None,
            last_sidebar_targets: Vec::new(),
            last_player_targets: Vec::new(),
            last_songs_targets: Vec::new(),
            last_artists_targets: Vec::new(),
            last_albums_targets: Vec::new(),
            last_now_playing_target: None,
            last_playlists_targets: Vec::new(),
            last_browser_targets: Vec::new(),
            last_settings_targets: Vec::new(),
            last_dialog_targets: Vec::new(),
            last_folder_dialog_targets: Vec::new(),
            last_about_target: None,
            last_search_targets: Vec::new(),
        };

        app.audio.set_volume(app.config.volume);

        if needs_initial_scan {
            app.trigger_scan();
        }

        Ok(app)
    }

    pub fn on_tick(&mut self) {
        // Hot-reload theme if Omarchy desktop colors changed
        self.theme.refresh_if_changed();

        // Check for library scanner events
        if let Some(rx) = &self.scan_receiver {
            while let Ok(update) = rx.try_recv() {
                match update {
                    ScanUpdate::Progress { current_file, found_count } => {
                        self.scan_found_count = found_count;
                        self.scan_status = format!("Scanning: {}", current_file);
                    }
                    ScanUpdate::Completed { tracks } => {
                        self.scan_found_count = tracks.len();
                        self.library.tracks = tracks;
                        self.library.scanned_at = Some(std::time::SystemTime::now());
                        self.library.save().ok();
                        self.is_scanning = false;
                        self.initial_loading = false;
                        self.scan_status = format!("Scan complete: {} audio tracks indexed", self.library.tracks.len());
                    }
                }
            }
        }

        // Check for audio events
        while let Some(event) = self.audio.try_recv_event() {
            match event {
                crate::audio::PlayerEvent::TrackFinished => {
                    self.handle_track_finished();
                }
                crate::audio::PlayerEvent::TrackTransitioned(track) => {
                    self.handle_track_transitioned(track);
                }
                crate::audio::PlayerEvent::Error(err) => {
                    self.scan_status = format!("Audio Error: {}", err);
                }
            }
        }
    }

    pub fn peek_next_track(&self) -> Option<Track> {
        if self.playback_queue.is_empty() {
            return None;
        }
        if self.config.shuffle {
            if self.playback_queue.len() > 1 {
                let other_indices: Vec<usize> = (0..self.playback_queue.len())
                    .filter(|&i| i != self.queue_index)
                    .collect();
                let mut rng = rand::thread_rng();
                other_indices.choose(&mut rng).and_then(|&idx| self.playback_queue.get(idx).cloned())
            } else {
                self.playback_queue.get(0).cloned()
            }
        } else {
            match self.config.repeat_mode {
                RepeatMode::Track => self.playback_queue.get(self.queue_index).cloned(),
                RepeatMode::All => {
                    let next_idx = (self.queue_index + 1) % self.playback_queue.len();
                    self.playback_queue.get(next_idx).cloned()
                }
                RepeatMode::Off => {
                    if self.queue_index + 1 < self.playback_queue.len() {
                        self.playback_queue.get(self.queue_index + 1).cloned()
                    } else {
                        None
                    }
                }
            }
        }
    }

    pub fn update_next_track(&mut self) {
        let next = self.peek_next_track();
        self.audio.set_next_track(next);
    }

    pub fn handle_track_transitioned(&mut self, track: Track) {
        self.playlists_manager.add_recently_played(&track.id);
        self.audio.on_track_transitioned(&track);

        // Find position of transitioned track in playback queue
        if let Some(pos) = self.playback_queue.iter().position(|t| t.id == track.id) {
            self.queue_index = pos;
        }

        // Keep table selection in sync if viewing Songs
        if self.view_mode == ViewMode::Songs {
            self.songs_selected_idx = self.queue_index;
        }

        // Start background preloading for subsequent track immediately
        self.update_next_track();
    }

    pub fn play_track_now(&mut self, track: Track) {
        self.playlists_manager.add_recently_played(&track.id);
        self.audio.play_track(track);
        self.update_next_track();
    }

    pub fn play_queue_at(&mut self, queue: Vec<Track>, start_idx: usize) {
        if queue.is_empty() {
            return;
        }
        self.playback_queue = queue;
        self.queue_index = start_idx.min(self.playback_queue.len() - 1);
        if let Some(track) = self.playback_queue.get(self.queue_index).cloned() {
            self.play_track_now(track);
        }
    }

    pub fn handle_track_finished(&mut self) {
        match self.config.repeat_mode {
            RepeatMode::Track => {
                if let Some(track) = self.audio.current_track() {
                    self.play_track_now(track);
                }
            }
            RepeatMode::All => {
                if !self.playback_queue.is_empty() {
                    self.queue_index = (self.queue_index + 1) % self.playback_queue.len();
                    if let Some(track) = self.playback_queue.get(self.queue_index).cloned() {
                        self.play_track_now(track);
                    }
                }
            }
            RepeatMode::Off => {
                if !self.playback_queue.is_empty() && self.queue_index + 1 < self.playback_queue.len() {
                    self.queue_index += 1;
                    if let Some(track) = self.playback_queue.get(self.queue_index).cloned() {
                        self.play_track_now(track);
                    }
                }
            }
        }
    }

    pub fn next_track(&mut self) {
        if self.playback_queue.is_empty() {
            return;
        }
        if self.config.shuffle {
            let mut rng = rand::thread_rng();
            self.queue_index = (0..self.playback_queue.len())
                .collect::<Vec<_>>()
                .choose(&mut rng)
                .copied()
                .unwrap_or(0);
        } else {
            self.queue_index = (self.queue_index + 1) % self.playback_queue.len();
        }
        if let Some(track) = self.playback_queue.get(self.queue_index).cloned() {
            self.play_track_now(track);
        }
    }

    pub fn prev_track(&mut self) {
        if self.playback_queue.is_empty() {
            return;
        }
        if self.audio.elapsed().as_secs() > 3 {
            // Restart current track
            self.audio.seek_to(Duration::from_secs(0));
            return;
        }
        if self.queue_index > 0 {
            self.queue_index -= 1;
        } else {
            self.queue_index = self.playback_queue.len().saturating_sub(1);
        }
        if let Some(track) = self.playback_queue.get(self.queue_index).cloned() {
            self.play_track_now(track);
        }
    }

    pub fn toggle_favorite_current(&mut self) {
        if let Some(track) = self.get_selected_track().or_else(|| self.audio.current_track()) {
            self.playlists_manager.toggle_favorite(&track.id);
        }
    }

    pub fn get_selected_track(&self) -> Option<Track> {
        match self.view_mode {
            ViewMode::Songs => {
                let tracks = self.library.search(&self.search_query);
                tracks.get(self.songs_selected_idx).copied().cloned()
            }
            ViewMode::Artists => {
                let artists = self.library.artists();
                let artist = artists.get(self.artists_selected_idx)?;
                let albums = self.library.albums_by_artist(artist);
                let album = albums.get(self.artists_album_idx)?;
                let songs = self.library.tracks_by_artist_and_album(artist, album);
                songs.get(self.artists_song_idx).copied().cloned()
            }
            ViewMode::Albums => {
                let albums = self.library.all_albums();
                let album = albums.get(self.albums_selected_idx)?;
                let songs = self.library.tracks_by_artist_and_album(&album.artist, &album.album);
                songs.get(self.albums_song_idx).copied().cloned()
            }
            ViewMode::NowPlaying => self.audio.current_track(),
            ViewMode::Playlists => {
                let track_ids: &[String] = if self.active_playlist_id == "__favorites__" {
                    &self.playlists_manager.favorites
                } else if self.active_playlist_id == "__recent__" {
                    &self.playlists_manager.recently_played
                } else if let Some(p) = self.playlists_manager.get_playlist(&self.active_playlist_id) {
                    &p.track_ids
                } else {
                    &[]
                };
                let tracks = self.library.tracks_for_playlist(track_ids);
                tracks.get(self.playlists_track_idx).copied().cloned()
            }
            ViewMode::SearchResults => {
                let songs = self.library.search_tracks(&self.search_query);
                songs.get(self.search_song_idx).copied().cloned()
            }
            _ => None,
        }
    }

    pub fn prompt_add_to_playlist(&mut self) {
        if let Some(track) = self.get_selected_track().or_else(|| self.audio.current_track()) {
            self.playlist_dialog = Some((
                PlaylistDialogMode::SelectPlaylist { selected_idx: 0 },
                Some(track),
            ));
        }
    }

    pub fn prompt_rename_playlist(&mut self, id: &str) {
        if id == "__favorites__" || id == "__recent__" {
            return;
        }
        if let Some(p) = self.playlists_manager.get_playlist(id) {
            let name = p.name.clone();
            self.playlist_dialog = Some((
                PlaylistDialogMode::RenamePlaylist {
                    playlist_id: id.to_string(),
                    name_input: name,
                },
                None,
            ));
        }
    }

    pub fn prompt_delete_playlist(&mut self, id: &str) {
        if id == "__favorites__" || id == "__recent__" {
            return;
        }
        if let Some(p) = self.playlists_manager.get_playlist(id) {
            let name = p.name.clone();
            self.playlist_dialog = Some((
                PlaylistDialogMode::ConfirmDelete {
                    playlist_id: id.to_string(),
                    playlist_name: name,
                },
                None,
            ));
        }
    }

    pub fn open_folder_dialog(&mut self, initial_focus: FolderDialogFocus) {
        let start_dir = if self.is_entering_folder && !self.new_folder_input.trim().is_empty() {
            let candidate = PathBuf::from(self.new_folder_input.trim());
            if candidate.is_dir() {
                candidate
            } else if let Some(parent) = candidate.parent().filter(|p| p.is_dir()) {
                parent.to_path_buf()
            } else {
                dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"))
            }
        } else {
            self.config
                .library_folders
                .get(self.settings_selected_folder)
                .or_else(|| self.config.library_folders.last())
                .filter(|p| p.is_dir())
                .cloned()
                .or_else(|| dirs::audio_dir().filter(|p| p.is_dir()))
                .or_else(dirs::home_dir)
                .unwrap_or_else(|| PathBuf::from("/"))
        };

        let mut state = FolderDialogState::new(start_dir);
        state.focus = initial_focus;
        self.folder_dialog = Some(state);
        self.is_entering_folder = false;
        self.new_folder_input.clear();
    }

    pub fn confirm_folder_dialog(&mut self) {
        let Some(mut state) = self.folder_dialog.take() else {
            return;
        };
        let resolved = state.resolve_input_path();
        if resolved.is_dir() {
            let clean_path = std::fs::canonicalize(&resolved).unwrap_or(resolved);
            if !self.config.library_folders.contains(&clean_path) {
                self.config.library_folders.push(clean_path);
                self.config.save().ok();
                self.trigger_scan();
            } else {
                self.scan_status = format!("Folder already in library: {}", clean_path.display());
            }
            self.folder_dialog = None;
        } else {
            state.status_message = Some(
                "Directory does not exist. Please enter or select a valid folder.".to_string(),
            );
            self.folder_dialog = Some(state);
        }
    }

    fn handle_folder_dialog_key(&mut self, key: KeyEvent) {
        if key.code == KeyCode::Esc {
            self.folder_dialog = None;
            return;
        }

        let Some(state) = &mut self.folder_dialog else {
            return;
        };

        if key.code == KeyCode::Tab || key.code == KeyCode::BackTab {
            state.focus = match state.focus {
                FolderDialogFocus::PathInput => FolderDialogFocus::FolderList,
                FolderDialogFocus::FolderList => FolderDialogFocus::PathInput,
            };
            return;
        }

        match state.focus {
            FolderDialogFocus::PathInput => match key.code {
                KeyCode::Enter => {
                    self.confirm_folder_dialog();
                }
                KeyCode::Down => {
                    state.focus = FolderDialogFocus::FolderList;
                }
                KeyCode::Up => {
                    state.focus = FolderDialogFocus::FolderList;
                }
                KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    state.path_input.clear();
                    state.status_message = None;
                }
                KeyCode::Backspace => {
                    state.path_input.pop();
                    state.on_path_input_edited();
                }
                KeyCode::Char(c) => {
                    state.path_input.push(c);
                    state.on_path_input_edited();
                }
                _ => {}
            },
            FolderDialogFocus::FolderList => match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    let next = state.selected_idx.saturating_sub(1);
                    state.select_index(next);
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    let next = state.selected_idx + 1;
                    state.select_index(next);
                }
                KeyCode::PageUp => {
                    let step = state.visible_rows.max(1);
                    let next = state.selected_idx.saturating_sub(step);
                    state.select_index(next);
                }
                KeyCode::PageDown => {
                    let step = state.visible_rows.max(1);
                    let next = state.selected_idx + step;
                    state.select_index(next);
                }
                KeyCode::Enter | KeyCode::Right | KeyCode::Char('l') => {
                    state.open_selected_entry();
                }
                KeyCode::Left | KeyCode::Backspace | KeyCode::Char('h') => {
                    state.go_parent();
                }
                KeyCode::Char('.') => {
                    state.show_hidden = !state.show_hidden;
                    state.refresh_entries();
                }
                KeyCode::Char('/') => {
                    state.focus = FolderDialogFocus::PathInput;
                }
                KeyCode::Char(' ') | KeyCode::Char('a') => {
                    self.confirm_folder_dialog();
                }
                _ => {}
            },
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        // 1. Help or About Modal active
        if self.show_help {
            if key.code == KeyCode::Esc || key.code == KeyCode::Char('?') || key.code == KeyCode::Char('q') {
                self.show_help = false;
            }
            return;
        }
        if self.show_about {
            if key.code == KeyCode::Esc || key.code == KeyCode::Enter || key.code == KeyCode::Char(' ') || key.code == KeyCode::Char('q') {
                self.show_about = false;
            }
            return;
        }

        // 2. Folder Dialog active
        if self.folder_dialog.is_some() {
            self.handle_folder_dialog_key(key);
            return;
        }

        // 3. Playlist Dialog active
        if let Some((mode, track)) = self.playlist_dialog.take() {
            match mode {
                PlaylistDialogMode::SelectPlaylist { selected_idx } => match key.code {
                    KeyCode::Esc => {
                        self.playlist_dialog = None;
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        let new_idx = selected_idx.saturating_sub(1);
                        self.playlist_dialog = Some((PlaylistDialogMode::SelectPlaylist { selected_idx: new_idx }, track));
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        let max_idx = self.playlists_manager.playlists.len();
                        let new_idx = (selected_idx + 1).min(max_idx);
                        self.playlist_dialog = Some((PlaylistDialogMode::SelectPlaylist { selected_idx: new_idx }, track));
                    }
                    KeyCode::Enter => {
                        if selected_idx == 0 {
                            // Prompt for new playlist name
                            self.playlist_dialog = Some((PlaylistDialogMode::CreateNew { name_input: String::new() }, track));
                        } else {
                            let p_idx = selected_idx - 1;
                            if let Some(p) = self.playlists_manager.playlists.get(p_idx).cloned() {
                                if let Some(ref tr) = track {
                                    self.playlists_manager.add_track(&p.id, &tr.id);
                                }
                            }
                            self.playlist_dialog = None;
                        }
                    }
                    _ => {
                        self.playlist_dialog = Some((PlaylistDialogMode::SelectPlaylist { selected_idx }, track));
                    }
                },
                PlaylistDialogMode::CreateNew { mut name_input } => match key.code {
                    KeyCode::Esc => {
                        self.playlist_dialog = None;
                    }
                    KeyCode::Char(c) => {
                        name_input.push(c);
                        self.playlist_dialog = Some((PlaylistDialogMode::CreateNew { name_input }, track));
                    }
                    KeyCode::Backspace => {
                        name_input.pop();
                        self.playlist_dialog = Some((PlaylistDialogMode::CreateNew { name_input }, track));
                    }
                    KeyCode::Enter => {
                        let trimmed = name_input.trim();
                        if !trimmed.is_empty() {
                            let new_id = self.playlists_manager.create_playlist(trimmed);
                            if let Some(ref tr) = track {
                                self.playlists_manager.add_track(&new_id, &tr.id);
                            }
                        }
                        self.playlist_dialog = None;
                    }
                    _ => {
                        self.playlist_dialog = Some((PlaylistDialogMode::CreateNew { name_input }, track));
                    }
                },
                PlaylistDialogMode::RenamePlaylist { playlist_id, mut name_input } => match key.code {
                    KeyCode::Esc => {
                        self.playlist_dialog = None;
                    }
                    KeyCode::Char(c) => {
                        name_input.push(c);
                        self.playlist_dialog = Some((PlaylistDialogMode::RenamePlaylist { playlist_id, name_input }, track));
                    }
                    KeyCode::Backspace => {
                        name_input.pop();
                        self.playlist_dialog = Some((PlaylistDialogMode::RenamePlaylist { playlist_id, name_input }, track));
                    }
                    KeyCode::Enter => {
                        let trimmed = name_input.trim();
                        if !trimmed.is_empty() {
                            self.playlists_manager.rename_playlist(&playlist_id, trimmed);
                        }
                        self.playlist_dialog = None;
                    }
                    _ => {
                        self.playlist_dialog = Some((PlaylistDialogMode::RenamePlaylist { playlist_id, name_input }, track));
                    }
                },
                PlaylistDialogMode::ConfirmDelete { playlist_id, playlist_name } => match key.code {
                    KeyCode::Esc => {
                        self.playlist_dialog = None;
                    }
                    KeyCode::Enter => {
                        self.playlists_manager.delete_playlist(&playlist_id);
                        if self.active_playlist_id == playlist_id {
                            self.active_playlist_id = "__favorites__".to_string();
                            self.playlists_track_idx = 0;
                        }
                        self.playlist_dialog = None;
                    }
                    _ => {
                        self.playlist_dialog = Some((PlaylistDialogMode::ConfirmDelete { playlist_id, playlist_name }, track));
                    }
                },
            }
            return;
        }

        // 4. Search Mode active
        if self.is_searching {
            if key.modifiers.contains(KeyModifiers::CONTROL)
                && (key.code == KeyCode::Char('c') || key.code == KeyCode::Char('q'))
            {
                self.should_quit = true;
                return;
            }
            match key.code {
                KeyCode::Esc => {
                    self.is_searching = false;
                    if self.search_query.is_empty() && self.view_mode == ViewMode::SearchResults {
                        self.set_view(self.previous_view_mode);
                    }
                }
                KeyCode::Enter | KeyCode::Tab | KeyCode::BackTab => {
                    self.is_searching = false;
                    self.focus = FocusArea::MainView;
                }
                KeyCode::Down | KeyCode::Up => {
                    self.is_searching = false;
                    self.focus = FocusArea::MainView;
                    self.handle_main_view_key(key);
                }
                KeyCode::Backspace => {
                    self.search_query.pop();
                    self.search_artist_idx = 0;
                    self.search_album_idx = 0;
                    self.search_song_idx = 0;
                    self.search_artist_scroll = 0;
                    self.search_album_scroll = 0;
                    self.search_song_scroll = 0;
                }
                KeyCode::Char(c) => {
                    self.search_query.push(c);
                    self.search_artist_idx = 0;
                    self.search_album_idx = 0;
                    self.search_song_idx = 0;
                    self.search_artist_scroll = 0;
                    self.search_album_scroll = 0;
                    self.search_song_scroll = 0;
                    self.view_mode = ViewMode::SearchResults;
                }
                _ => {}
            }
            return;
        }

        // 5. Folder input mode in Settings
        if self.is_entering_folder {
            match key.code {
                KeyCode::Esc => {
                    self.is_entering_folder = false;
                    self.new_folder_input.clear();
                }
                KeyCode::Enter => {
                    let path = PathBuf::from(self.new_folder_input.trim());
                    if path.exists() {
                        if !self.config.library_folders.contains(&path) {
                            self.config.library_folders.push(path);
                            self.config.save().ok();
                            self.trigger_scan();
                        }
                    }
                    self.is_entering_folder = false;
                    self.new_folder_input.clear();
                }
                KeyCode::Backspace => {
                    self.new_folder_input.pop();
                }
                KeyCode::Char(c) => {
                    self.new_folder_input.push(c);
                }
                _ => {}
            }
            return;
        }

        // 6. Global Hotkeys
        match key.code {
            KeyCode::Char('q') => {
                self.should_quit = true;
                return;
            }
            KeyCode::Char('?') => {
                self.show_help = true;
                return;
            }
            KeyCode::Char('/') => {
                if self.view_mode != ViewMode::SearchResults {
                    self.previous_view_mode = self.view_mode;
                }
                self.is_searching = true;
                self.view_mode = ViewMode::SearchResults;
                self.focus = FocusArea::MainView;
                return;
            }
            KeyCode::Tab => {
                self.focus = match self.focus {
                    FocusArea::Sidebar => FocusArea::MainView,
                    FocusArea::MainView => FocusArea::PlayerBar,
                    FocusArea::PlayerBar => FocusArea::Sidebar,
                };
                return;
            }
            KeyCode::BackTab => {
                self.focus = match self.focus {
                    FocusArea::Sidebar => FocusArea::PlayerBar,
                    FocusArea::MainView => FocusArea::Sidebar,
                    FocusArea::PlayerBar => FocusArea::MainView,
                };
                return;
            }
            // Number keys 1..6 switch views directly
            KeyCode::Char('1') => { self.set_view(ViewMode::Artists); return; }
            KeyCode::Char('2') => { self.set_view(ViewMode::Albums); return; }
            KeyCode::Char('3') => { self.set_view(ViewMode::Songs); return; }
            KeyCode::Char('4') => { self.set_view(ViewMode::NowPlaying); return; }
            KeyCode::Char('5') => { self.set_view(ViewMode::Playlists); return; }
            KeyCode::Char('6') => { self.set_view(ViewMode::Settings); return; }

            // Transport controls
            KeyCode::Char(' ') => {
                self.audio.toggle_pause();
                return;
            }
            KeyCode::Char('s') => {
                self.audio.stop();
                return;
            }
            KeyCode::Char('n') => {
                self.next_track();
                return;
            }
            KeyCode::Char('p') => {
                self.prev_track();
                return;
            }
            KeyCode::Left if key.modifiers.contains(KeyModifiers::SHIFT) => {
                if self.focus == FocusArea::PlayerBar {
                    self.prev_track();
                } else {
                    self.audio.seek_relative(-30);
                }
                return;
            }
            KeyCode::Right if key.modifiers.contains(KeyModifiers::SHIFT) => {
                if self.focus == FocusArea::PlayerBar {
                    self.next_track();
                } else {
                    self.audio.seek_relative(30);
                }
                return;
            }
            KeyCode::Char('[') => {
                self.audio.seek_relative(-30);
                return;
            }
            KeyCode::Char(']') => {
                self.audio.seek_relative(30);
                return;
            }
            KeyCode::Char('+') | KeyCode::Char('=') => {
                self.adjust_volume(0.05);
                return;
            }
            KeyCode::Char('-') => {
                self.adjust_volume(-0.05);
                return;
            }
            KeyCode::Char('m') => {
                self.toggle_mute();
                return;
            }
            KeyCode::F(2) => {
                if self.focus == FocusArea::Sidebar {
                    self.handle_sidebar_key(key);
                    return;
                }
                if self.view_mode == ViewMode::Playlists && self.active_playlist_id != "__favorites__" && self.active_playlist_id != "__recent__" {
                    self.prompt_rename_playlist(&self.active_playlist_id.clone());
                    return;
                }
            }
            KeyCode::Char('r') => {
                if self.focus == FocusArea::Sidebar {
                    self.handle_sidebar_key(key);
                    return;
                }
                if self.view_mode == ViewMode::Playlists && self.active_playlist_id != "__favorites__" && self.active_playlist_id != "__recent__" {
                    self.prompt_rename_playlist(&self.active_playlist_id.clone());
                    return;
                }
                self.config.repeat_mode = self.config.repeat_mode.next();
                self.config.save().ok();
                self.update_next_track();
                return;
            }
            KeyCode::Char('D') => {
                if self.focus == FocusArea::Sidebar {
                    self.handle_sidebar_key(key);
                    return;
                }
                if self.view_mode == ViewMode::Playlists && self.active_playlist_id != "__favorites__" && self.active_playlist_id != "__recent__" {
                    self.prompt_delete_playlist(&self.active_playlist_id.clone());
                    return;
                }
            }
            KeyCode::Char('z') => {
                self.config.shuffle = !self.config.shuffle;
                self.config.save().ok();
                self.update_next_track();
                return;
            }
            KeyCode::Char('a') => {
                if self.view_mode == ViewMode::Settings {
                    self.open_folder_dialog(FolderDialogFocus::PathInput);
                } else {
                    self.prompt_add_to_playlist();
                }
                return;
            }
            KeyCode::Char('b') if self.view_mode == ViewMode::Settings => {
                self.open_folder_dialog(FolderDialogFocus::FolderList);
                return;
            }
            KeyCode::Char('f') => {
                self.toggle_favorite_current();
                return;
            }
            _ => {}
        }

        // 6. Navigation based on FocusArea
        match self.focus {
            FocusArea::Sidebar => {
                self.handle_sidebar_key(key);
            }
            FocusArea::MainView => {
                self.handle_main_view_key(key);
            }
            FocusArea::PlayerBar => {
                self.handle_player_bar_key(key);
            }
        }
    }

    fn handle_player_bar_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Left | KeyCode::Char('h') => {
                self.prev_track();
            }
            KeyCode::Right | KeyCode::Char('l') => {
                self.next_track();
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.adjust_volume(0.05);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.adjust_volume(-0.05);
            }
            KeyCode::Enter => {
                self.audio.toggle_pause();
            }
            _ => {}
        }
    }

    fn handle_sidebar_key(&mut self, key: KeyEvent) {
        let max_items = SidebarComponent {
            current_view: self.view_mode,
            current_playlist_id: Some(&self.active_playlist_id),
            playlists_manager: &self.playlists_manager,
            is_focused: false,
            selected_index: 0,
            theme: &self.theme,
        }.build_items().len();

        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.sidebar_index = self.sidebar_index.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.sidebar_index = (self.sidebar_index + 1).min(max_items.saturating_sub(1));
            }
            KeyCode::Enter => {
                // Activate selected sidebar item
                self.activate_sidebar_index(self.sidebar_index);
            }
            KeyCode::Char('r') | KeyCode::F(2) => {
                let items = SidebarComponent {
                    current_view: self.view_mode,
                    current_playlist_id: Some(&self.active_playlist_id),
                    playlists_manager: &self.playlists_manager,
                    is_focused: false,
                    selected_index: 0,
                    theme: &self.theme,
                }.build_items();
                if let Some((SidebarItem::CustomPlaylist(id), _, _)) = items.get(self.sidebar_index) {
                    self.prompt_rename_playlist(&id.clone());
                }
            }
            KeyCode::Char('D') | KeyCode::Delete => {
                let items = SidebarComponent {
                    current_view: self.view_mode,
                    current_playlist_id: Some(&self.active_playlist_id),
                    playlists_manager: &self.playlists_manager,
                    is_focused: false,
                    selected_index: 0,
                    theme: &self.theme,
                }.build_items();
                if let Some((SidebarItem::CustomPlaylist(id), _, _)) = items.get(self.sidebar_index) {
                    self.prompt_delete_playlist(&id.clone());
                }
            }
            _ => {}
        }
    }

    fn activate_sidebar_index(&mut self, index: usize) {
        let items = SidebarComponent {
            current_view: self.view_mode,
            current_playlist_id: Some(&self.active_playlist_id),
            playlists_manager: &self.playlists_manager,
            is_focused: false,
            selected_index: 0,
            theme: &self.theme,
        }.build_items();

        if let Some((item, _, _)) = items.get(index) {
            match item {
                SidebarItem::View(vm) => self.set_view(*vm),
                SidebarItem::Favorites => {
                    self.active_playlist_id = "__favorites__".to_string();
                    self.set_view(ViewMode::Playlists);
                }
                SidebarItem::RecentlyPlayed => {
                    self.active_playlist_id = "__recent__".to_string();
                    self.set_view(ViewMode::Playlists);
                }
                SidebarItem::CustomPlaylist(id) => {
                    self.active_playlist_id = id.clone();
                    self.set_view(ViewMode::Playlists);
                }
                SidebarItem::NewPlaylist => {
                    let track = self.get_selected_track().or_else(|| self.audio.current_track());
                    self.playlist_dialog = Some((PlaylistDialogMode::CreateNew { name_input: String::new() }, track));
                }
            }
        }
        self.focus = FocusArea::MainView;
    }

    fn visible_main_rows(&self, header_lines: u16) -> usize {
        let h = if self.last_main_area.height > 3 + header_lines {
            self.last_main_area.height - 3 - header_lines
        } else {
            15
        };
        (h as usize).max(1)
    }

    fn sync_scroll(selected: usize, scroll: &mut usize, visible_rows: usize) {
        if selected < *scroll {
            *scroll = selected;
        } else if visible_rows > 0 && selected >= *scroll + visible_rows {
            *scroll = selected + 1 - visible_rows;
        }
    }

    fn scroll_pane(is_up: bool, scroll: &mut usize, total_items: usize, visible_rows: usize) {
        let max_scroll = total_items.saturating_sub(visible_rows);
        if is_up {
            *scroll = (*scroll).min(max_scroll).saturating_sub(1);
        } else {
            *scroll = ((*scroll) + 1).min(max_scroll);
        }
    }

    fn handle_main_view_key(&mut self, key: KeyEvent) {
        let vis_0 = self.visible_main_rows(0);
        let vis_2 = self.visible_main_rows(1);

        match self.view_mode {
            ViewMode::Songs => match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    self.songs_selected_idx = self.songs_selected_idx.saturating_sub(1);
                    Self::sync_scroll(self.songs_selected_idx, &mut self.songs_scroll_offset, vis_2);
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    let tracks = self.library.search(&self.search_query);
                    if !tracks.is_empty() {
                        self.songs_selected_idx = (self.songs_selected_idx + 1).min(tracks.len() - 1);
                        Self::sync_scroll(self.songs_selected_idx, &mut self.songs_scroll_offset, vis_2);
                    }
                }
                KeyCode::Left => {
                    self.audio.seek_relative(-5);
                }
                KeyCode::Right => {
                    self.audio.seek_relative(5);
                }
                KeyCode::Enter => {
                    let tracks: Vec<Track> = self.library.search(&self.search_query).into_iter().cloned().collect();
                    self.play_queue_at(tracks, self.songs_selected_idx);
                }
                _ => {}
            },
            ViewMode::Artists => match key.code {
                KeyCode::Left | KeyCode::Char('h') => {
                    self.artists_col = match self.artists_col {
                        ArtistColumn::Artists => ArtistColumn::Artists,
                        ArtistColumn::Albums => ArtistColumn::Artists,
                        ArtistColumn::Songs => ArtistColumn::Albums,
                    };
                }
                KeyCode::Right | KeyCode::Char('l') => {
                    self.artists_col = match self.artists_col {
                        ArtistColumn::Artists => ArtistColumn::Albums,
                        ArtistColumn::Albums => ArtistColumn::Songs,
                        ArtistColumn::Songs => ArtistColumn::Songs,
                    };
                }
                KeyCode::Up | KeyCode::Char('k') => match self.artists_col {
                    ArtistColumn::Artists => {
                        self.artists_selected_idx = self.artists_selected_idx.saturating_sub(1);
                        self.artists_album_idx = 0;
                        self.artists_song_idx = 0;
                        self.artists_album_scroll_offset = 0;
                        self.artists_song_scroll_offset = 0;
                        Self::sync_scroll(self.artists_selected_idx, &mut self.artists_scroll_offset, vis_0);
                    }
                    ArtistColumn::Albums => {
                        self.artists_album_idx = self.artists_album_idx.saturating_sub(1);
                        self.artists_song_idx = 0;
                        self.artists_song_scroll_offset = 0;
                        Self::sync_scroll(self.artists_album_idx, &mut self.artists_album_scroll_offset, vis_0);
                    }
                    ArtistColumn::Songs => {
                        self.artists_song_idx = self.artists_song_idx.saturating_sub(1);
                        Self::sync_scroll(self.artists_song_idx, &mut self.artists_song_scroll_offset, vis_0);
                    }
                },
                KeyCode::Down | KeyCode::Char('j') => match self.artists_col {
                    ArtistColumn::Artists => {
                        let artists = self.library.artists();
                        if !artists.is_empty() {
                            self.artists_selected_idx = (self.artists_selected_idx + 1).min(artists.len() - 1);
                            self.artists_album_idx = 0;
                            self.artists_song_idx = 0;
                            self.artists_album_scroll_offset = 0;
                            self.artists_song_scroll_offset = 0;
                            Self::sync_scroll(self.artists_selected_idx, &mut self.artists_scroll_offset, vis_0);
                        }
                    }
                    ArtistColumn::Albums => {
                        let artists = self.library.artists();
                        if let Some(art) = artists.get(self.artists_selected_idx) {
                            let albums = self.library.albums_by_artist(art);
                            if !albums.is_empty() {
                                self.artists_album_idx = (self.artists_album_idx + 1).min(albums.len() - 1);
                                self.artists_song_idx = 0;
                                self.artists_song_scroll_offset = 0;
                                Self::sync_scroll(self.artists_album_idx, &mut self.artists_album_scroll_offset, vis_0);
                            }
                        }
                    }
                    ArtistColumn::Songs => {
                        let artists = self.library.artists();
                        if let Some(art) = artists.get(self.artists_selected_idx) {
                            let albums = self.library.albums_by_artist(art);
                            if let Some(alb) = albums.get(self.artists_album_idx) {
                                let songs = self.library.tracks_by_artist_and_album(art, alb);
                                if !songs.is_empty() {
                                    self.artists_song_idx = (self.artists_song_idx + 1).min(songs.len() - 1);
                                    Self::sync_scroll(self.artists_song_idx, &mut self.artists_song_scroll_offset, vis_0);
                                }
                            }
                        }
                    }
                },
                KeyCode::Enter => {
                    let artists = self.library.artists();
                    if let Some(art) = artists.get(self.artists_selected_idx) {
                        let albums = self.library.albums_by_artist(art);
                        if let Some(alb) = albums.get(self.artists_album_idx) {
                            let songs: Vec<Track> = self
                                .library
                                .tracks_by_artist_and_album(art, alb)
                                .into_iter()
                                .cloned()
                                .collect();
                            self.play_queue_at(songs, self.artists_song_idx);
                        }
                    }
                }
                _ => {}
            },
            ViewMode::Albums => match key.code {
                KeyCode::Tab | KeyCode::BackTab => {
                    self.albums_col = match self.albums_col {
                        AlbumColumn::Albums => AlbumColumn::Songs,
                        AlbumColumn::Songs => AlbumColumn::Albums,
                    };
                }
                KeyCode::Left | KeyCode::Char('h') => {
                    self.albums_col = AlbumColumn::Albums;
                }
                KeyCode::Right | KeyCode::Char('l') => {
                    self.albums_col = AlbumColumn::Songs;
                }
                KeyCode::Up | KeyCode::Char('k') => match self.albums_col {
                    AlbumColumn::Albums => {
                        self.albums_selected_idx = self.albums_selected_idx.saturating_sub(1);
                        self.albums_song_idx = 0;
                        self.albums_song_scroll_offset = 0;
                        Self::sync_scroll(self.albums_selected_idx, &mut self.albums_scroll_offset, vis_0);
                    }
                    AlbumColumn::Songs => {
                        self.albums_song_idx = self.albums_song_idx.saturating_sub(1);
                        Self::sync_scroll(self.albums_song_idx, &mut self.albums_song_scroll_offset, vis_0);
                    }
                },
                KeyCode::Down | KeyCode::Char('j') => match self.albums_col {
                    AlbumColumn::Albums => {
                        let albums = self.library.all_albums();
                        if !albums.is_empty() {
                            self.albums_selected_idx = (self.albums_selected_idx + 1).min(albums.len() - 1);
                            self.albums_song_idx = 0;
                            self.albums_song_scroll_offset = 0;
                            Self::sync_scroll(self.albums_selected_idx, &mut self.albums_scroll_offset, vis_0);
                        }
                    }
                    AlbumColumn::Songs => {
                        let albums = self.library.all_albums();
                        if let Some(alb) = albums.get(self.albums_selected_idx) {
                            let songs = self.library.tracks_by_artist_and_album(&alb.artist, &alb.album);
                            if !songs.is_empty() {
                                self.albums_song_idx = (self.albums_song_idx + 1).min(songs.len() - 1);
                                Self::sync_scroll(self.albums_song_idx, &mut self.albums_song_scroll_offset, vis_0);
                            }
                        }
                    }
                },
                KeyCode::Enter => {
                    let albums = self.library.all_albums();
                    if let Some(alb) = albums.get(self.albums_selected_idx) {
                        let songs: Vec<Track> = self
                            .library
                            .tracks_by_artist_and_album(&alb.artist, &alb.album)
                            .into_iter()
                            .cloned()
                            .collect();
                        let start_idx = match self.albums_col {
                            AlbumColumn::Albums => 0,
                            AlbumColumn::Songs => self.albums_song_idx,
                        };
                        self.play_queue_at(songs, start_idx);
                    }
                }
                _ => {}
            },
            ViewMode::NowPlaying => match key.code {
                KeyCode::Left => {
                    self.audio.seek_relative(-5);
                }
                KeyCode::Right => {
                    self.audio.seek_relative(5);
                }
                _ => {}
            },
            ViewMode::Playlists => match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    self.playlists_track_idx = self.playlists_track_idx.saturating_sub(1);
                    Self::sync_scroll(self.playlists_track_idx, &mut self.playlists_scroll_offset, vis_2);
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    let track_ids: &[String] = if self.active_playlist_id == "__favorites__" {
                        &self.playlists_manager.favorites
                    } else if self.active_playlist_id == "__recent__" {
                        &self.playlists_manager.recently_played
                    } else if let Some(p) = self.playlists_manager.get_playlist(&self.active_playlist_id) {
                        &p.track_ids
                    } else {
                        &[]
                    };
                    if !track_ids.is_empty() {
                        self.playlists_track_idx = (self.playlists_track_idx + 1).min(track_ids.len() - 1);
                        Self::sync_scroll(self.playlists_track_idx, &mut self.playlists_scroll_offset, vis_2);
                    }
                }
                KeyCode::Enter => {
                    let track_ids: &[String] = if self.active_playlist_id == "__favorites__" {
                        &self.playlists_manager.favorites
                    } else if self.active_playlist_id == "__recent__" {
                        &self.playlists_manager.recently_played
                    } else if let Some(p) = self.playlists_manager.get_playlist(&self.active_playlist_id) {
                        &p.track_ids
                    } else {
                        &[]
                    };
                    let songs: Vec<Track> = self
                        .library
                        .tracks_for_playlist(track_ids)
                        .into_iter()
                        .cloned()
                        .collect();
                    self.play_queue_at(songs, self.playlists_track_idx);
                }
                KeyCode::Char('r') | KeyCode::F(2) => {
                    if self.active_playlist_id != "__favorites__" && self.active_playlist_id != "__recent__" {
                        self.prompt_rename_playlist(&self.active_playlist_id.clone());
                    }
                }
                KeyCode::Char('D') => {
                    if self.active_playlist_id != "__favorites__" && self.active_playlist_id != "__recent__" {
                        self.prompt_delete_playlist(&self.active_playlist_id.clone());
                    }
                }
                KeyCode::Delete | KeyCode::Char('d') => {
                    if self.active_playlist_id != "__favorites__" && self.active_playlist_id != "__recent__" {
                        self.playlists_manager
                            .remove_track(&self.active_playlist_id.clone(), self.playlists_track_idx);
                    }
                }
                _ => {}
            },
            ViewMode::FileBrowser => match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    self.browser_selected_idx = self.browser_selected_idx.saturating_sub(1);
                    Self::sync_scroll(self.browser_selected_idx, &mut self.browser_scroll_offset, vis_0);
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if !self.browser_items.is_empty() {
                        self.browser_selected_idx = (self.browser_selected_idx + 1).min(self.browser_items.len() - 1);
                        Self::sync_scroll(self.browser_selected_idx, &mut self.browser_scroll_offset, vis_0);
                    }
                }
                KeyCode::Enter => {
                    if let Some(item) = self.browser_items.get(self.browser_selected_idx).cloned() {
                        if item.is_dir {
                            self.browser_current_dir = item.path;
                            self.browser_items = FileBrowserView::list_directory(&self.browser_current_dir);
                            self.browser_selected_idx = 0;
                            self.browser_scroll_offset = 0;
                        } else if let Some(track) = Scanner::read_track(&item.path) {
                            self.play_track_now(track);
                        }
                    }
                }
                _ => {}
            },
            ViewMode::Settings => match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    self.settings_selected_folder = self.settings_selected_folder.saturating_sub(1);
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if !self.config.library_folders.is_empty() {
                        self.settings_selected_folder =
                            (self.settings_selected_folder + 1).min(self.config.library_folders.len() - 1);
                    }
                }
                KeyCode::Char('a') => {
                    self.open_folder_dialog(FolderDialogFocus::PathInput);
                }
                KeyCode::Char('b') => {
                    self.open_folder_dialog(FolderDialogFocus::FolderList);
                }
                KeyCode::Char('s') => {
                    self.trigger_scan();
                }
                _ => {}
            },
            ViewMode::SearchResults => match key.code {
                KeyCode::Tab | KeyCode::Right | KeyCode::Char('l') => {
                    self.search_section = self.search_section.next();
                }
                KeyCode::BackTab | KeyCode::Left | KeyCode::Char('h') => {
                    self.search_section = self.search_section.prev();
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    match self.search_section {
                        SearchSection::Artists => {
                            self.search_artist_idx = self.search_artist_idx.saturating_sub(1);
                            Self::sync_scroll(self.search_artist_idx, &mut self.search_artist_scroll, vis_0);
                        }
                        SearchSection::Albums => {
                            self.search_album_idx = self.search_album_idx.saturating_sub(1);
                            Self::sync_scroll(self.search_album_idx, &mut self.search_album_scroll, vis_0);
                        }
                        SearchSection::Songs => {
                            self.search_song_idx = self.search_song_idx.saturating_sub(1);
                            Self::sync_scroll(self.search_song_idx, &mut self.search_song_scroll, vis_0);
                        }
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    match self.search_section {
                        SearchSection::Artists => {
                            let count = self.library.search_artists(&self.search_query).len();
                            if count > 0 {
                                self.search_artist_idx = (self.search_artist_idx + 1).min(count - 1);
                                Self::sync_scroll(self.search_artist_idx, &mut self.search_artist_scroll, vis_0);
                            }
                        }
                        SearchSection::Albums => {
                            let count = self.library.search_albums(&self.search_query).len();
                            if count > 0 {
                                self.search_album_idx = (self.search_album_idx + 1).min(count - 1);
                                Self::sync_scroll(self.search_album_idx, &mut self.search_album_scroll, vis_0);
                            }
                        }
                        SearchSection::Songs => {
                            let count = self.library.search_tracks(&self.search_query).len();
                            if count > 0 {
                                self.search_song_idx = (self.search_song_idx + 1).min(count - 1);
                                Self::sync_scroll(self.search_song_idx, &mut self.search_song_scroll, vis_0);
                            }
                        }
                    }
                }
                KeyCode::Enter => {
                    match self.search_section {
                        SearchSection::Artists => {
                            let artists = self.library.search_artists(&self.search_query);
                            if let Some(art) = artists.get(self.search_artist_idx) {
                                let songs: Vec<Track> = self.library.tracks_by_artist(&art.name).into_iter().cloned().collect();
                                self.play_queue_at(songs, 0);
                            }
                        }
                        SearchSection::Albums => {
                            let albums = self.library.search_albums(&self.search_query);
                            if let Some(alb) = albums.get(self.search_album_idx) {
                                let songs: Vec<Track> = self.library.tracks_by_artist_and_album(&alb.artist, &alb.album).into_iter().cloned().collect();
                                self.play_queue_at(songs, 0);
                            }
                        }
                        SearchSection::Songs => {
                            let songs: Vec<Track> = self.library.search_tracks(&self.search_query).into_iter().cloned().collect();
                            if self.search_song_idx < songs.len() {
                                self.play_queue_at(songs, self.search_song_idx);
                            }
                        }
                    }
                }
                KeyCode::Char('/') => {
                    self.is_searching = true;
                }
                KeyCode::Esc => {
                    self.search_query.clear();
                    self.set_view(self.previous_view_mode);
                }
                _ => {}
            },
        }
    }

    pub fn handle_mouse(&mut self, mouse: MouseEvent) {
        let (x, y) = (mouse.column, mouse.row);

        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                self.handle_mouse_click(x, y);
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                self.handle_mouse_drag(x, y);
            }
            MouseEventKind::ScrollUp => {
                self.handle_mouse_scroll(true, x, y);
            }
            MouseEventKind::ScrollDown => {
                self.handle_mouse_scroll(false, x, y);
            }
            _ => {}
        }
    }

    fn handle_mouse_click(&mut self, x: u16, y: u16) {
        // Uncapture search mode whenever the user clicks outside the search box
        if self.is_searching {
            let clicked_search = self
                .last_header_targets
                .as_ref()
                .map(|hdr| is_inside(hdr.search_rect, x, y))
                .unwrap_or(false);
            if !clicked_search {
                self.is_searching = false;
            }
        }

        // 1. Help or About modal click
        if self.show_help {
            self.show_help = false;
            return;
        }
        if self.show_about {
            if let Some(target) = &self.last_about_target {
                if is_inside(target.url_rect, x, y) {
                    let cmd = if cfg!(target_os = "macos") {
                        "open"
                    } else {
                        "xdg-open"
                    };
                    std::process::Command::new(cmd)
                        .arg("https://github.com/jonecat/omaflac")
                        .stdin(std::process::Stdio::null())
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null())
                        .spawn()
                        .ok();
                    return;
                }
                if is_inside(target.close_rect, x, y) || !is_inside(target.modal_rect, x, y) {
                    self.show_about = false;
                    return;
                }
            } else {
                self.show_about = false;
            }
            return;
        }

        // 2. Folder Dialog click
        if self.folder_dialog.is_some() {
            let hit_action = self
                .last_folder_dialog_targets
                .iter()
                .find(|t| is_inside(t.rect, x, y))
                .map(|t| t.action.clone());

            if let Some(action) = hit_action {
                match action {
                    FolderDialogAction::FocusPathInput => {
                        if let Some(state) = &mut self.folder_dialog {
                            state.focus = FolderDialogFocus::PathInput;
                        }
                    }
                    FolderDialogAction::ClearPathInput => {
                        if let Some(state) = &mut self.folder_dialog {
                            state.focus = FolderDialogFocus::PathInput;
                            state.path_input.clear();
                            state.status_message = None;
                        }
                    }
                    FolderDialogAction::JumpToPath(path) => {
                        if let Some(state) = &mut self.folder_dialog {
                            state.focus = FolderDialogFocus::FolderList;
                            state.enter_directory(path);
                        }
                    }
                    FolderDialogAction::ToggleHidden => {
                        if let Some(state) = &mut self.folder_dialog {
                            state.show_hidden = !state.show_hidden;
                            state.refresh_entries();
                        }
                    }
                    FolderDialogAction::OpenEntry(idx) => {
                        if let Some(state) = &mut self.folder_dialog {
                            state.focus = FolderDialogFocus::FolderList;
                            state.selected_idx = idx;
                            state.open_selected_entry();
                        }
                    }
                    FolderDialogAction::ClickEntry(idx) => {
                        if let Some(state) = &mut self.folder_dialog {
                            let is_parent = state
                                .entries
                                .get(idx)
                                .map(|e| e.is_parent)
                                .unwrap_or(false);
                            if is_parent
                                || (state.focus == FolderDialogFocus::FolderList
                                    && state.selected_idx == idx)
                            {
                                state.focus = FolderDialogFocus::FolderList;
                                state.selected_idx = idx;
                                state.open_selected_entry();
                            } else {
                                state.focus = FolderDialogFocus::FolderList;
                                state.select_index(idx);
                            }
                        }
                    }
                    FolderDialogAction::OpenSelected => {
                        if let Some(state) = &mut self.folder_dialog {
                            state.focus = FolderDialogFocus::FolderList;
                            state.open_selected_entry();
                        }
                    }
                    FolderDialogAction::ConfirmAdd => {
                        self.confirm_folder_dialog();
                    }
                    FolderDialogAction::Cancel => {
                        self.folder_dialog = None;
                    }
                }
            }
            return;
        }

        // 3. Playlist Dialog click
        for t in &self.last_dialog_targets {
            if is_inside(t.rect, x, y) {
                if t.is_cancel_button {
                    self.playlist_dialog = None;
                    return;
                }
                if let Some((mode, track)) = self.playlist_dialog.take() {
                    match mode {
                        PlaylistDialogMode::SelectPlaylist { .. } => {
                            if t.is_confirm_button || t.index == Some(0) {
                                self.playlist_dialog = Some((PlaylistDialogMode::CreateNew { name_input: String::new() }, track));
                            } else if let Some(idx) = t.index {
                                if idx > 0 {
                                    let p_idx = idx - 1;
                                    if let Some(p) = self.playlists_manager.playlists.get(p_idx).cloned() {
                                        if let Some(ref tr) = track {
                                            self.playlists_manager.add_track(&p.id, &tr.id);
                                        }
                                    }
                                }
                                self.playlist_dialog = None;
                            }
                        }
                        PlaylistDialogMode::CreateNew { name_input } => {
                            if t.is_confirm_button {
                                let trimmed = name_input.trim();
                                if !trimmed.is_empty() {
                                    let new_id = self.playlists_manager.create_playlist(trimmed);
                                    if let Some(ref tr) = track {
                                        self.playlists_manager.add_track(&new_id, &tr.id);
                                    }
                                }
                                self.playlist_dialog = None;
                            } else {
                                self.playlist_dialog = Some((PlaylistDialogMode::CreateNew { name_input }, track));
                            }
                        }
                        PlaylistDialogMode::RenamePlaylist { playlist_id, name_input } => {
                            if t.is_confirm_button {
                                let trimmed = name_input.trim();
                                if !trimmed.is_empty() {
                                    self.playlists_manager.rename_playlist(&playlist_id, trimmed);
                                }
                                self.playlist_dialog = None;
                            } else {
                                self.playlist_dialog = Some((PlaylistDialogMode::RenamePlaylist { playlist_id, name_input }, track));
                            }
                        }
                        PlaylistDialogMode::ConfirmDelete { playlist_id, playlist_name } => {
                            if t.is_confirm_button {
                                self.playlists_manager.delete_playlist(&playlist_id);
                                if self.active_playlist_id == playlist_id {
                                    self.active_playlist_id = "__favorites__".to_string();
                                    self.playlists_track_idx = 0;
                                }
                                self.playlist_dialog = None;
                            } else {
                                self.playlist_dialog = Some((PlaylistDialogMode::ConfirmDelete { playlist_id, playlist_name }, track));
                            }
                        }
                    }
                }
                return;
            }
        }

        // 4. Header clicks
        if let Some(hdr) = &self.last_header_targets {
            if is_inside(hdr.logo_rect, x, y) {
                self.show_about = true;
                return;
            }
            if is_inside(hdr.help_rect, x, y) {
                self.show_help = true;
                return;
            }
            if is_inside(hdr.search_rect, x, y) {
                if self.view_mode != ViewMode::SearchResults {
                    self.previous_view_mode = self.view_mode;
                }
                self.is_searching = true;
                self.view_mode = ViewMode::SearchResults;
                self.focus = FocusArea::MainView;
                return;
            }
        }

        // 5. Player bar controls click
        for t in &self.last_player_targets {
            if is_inside(t.rect, x, y) {
                self.focus = FocusArea::PlayerBar;
                match t.action {
                    PlayerAction::Previous => self.prev_track(),
                    PlayerAction::TogglePlayPause => self.audio.toggle_pause(),
                    PlayerAction::Stop => self.audio.stop(),
                    PlayerAction::Next => self.next_track(),
                    PlayerAction::ToggleRepeat => {
                        self.config.repeat_mode = self.config.repeat_mode.next();
                        self.config.save().ok();
                        self.update_next_track();
                    }
                    PlayerAction::ToggleShuffle => {
                        self.config.shuffle = !self.config.shuffle;
                        self.config.save().ok();
                        self.update_next_track();
                    }
                    PlayerAction::ToggleMute => self.toggle_mute(),
                    PlayerAction::Seek(_) => {
                        let ratio = relative_x_ratio(t.rect, x);
                        let total = self.audio.total_duration().as_secs_f32();
                        let target_secs = ratio * total;
                        self.audio.seek_to(Duration::from_secs_f32(target_secs));
                    }
                    PlayerAction::SetVolume(_) => {
                        let ratio = relative_x_ratio(t.rect, x);
                        self.set_volume(ratio);
                    }
                    PlayerAction::OpenNowPlaying => {
                        self.set_view(ViewMode::NowPlaying);
                    }
                }
                return;
            }
        }

        // 6. Sidebar clicks
        for t in &self.last_sidebar_targets {
            if is_inside(t.rect, x, y) {
                match &t.item {
                    SidebarItem::View(vm) => self.set_view(*vm),
                    SidebarItem::Favorites => {
                        self.active_playlist_id = "__favorites__".to_string();
                        self.set_view(ViewMode::Playlists);
                    }
                    SidebarItem::RecentlyPlayed => {
                        self.active_playlist_id = "__recent__".to_string();
                        self.set_view(ViewMode::Playlists);
                    }
                    SidebarItem::CustomPlaylist(id) => {
                        self.active_playlist_id = id.clone();
                        self.set_view(ViewMode::Playlists);
                    }
                    SidebarItem::NewPlaylist => {
                        let track = self.get_selected_track().or_else(|| self.audio.current_track());
                        self.playlist_dialog = Some((PlaylistDialogMode::CreateNew { name_input: String::new() }, track));
                    }
                }
                self.focus = FocusArea::MainView;
                return;
            }
        }

        // 7. Main View Clicks
        match self.view_mode {
            ViewMode::Songs => {
                for t in &self.last_songs_targets {
                    if is_inside(t.rect, x, y) && !t.is_header {
                        if self.songs_selected_idx == t.track_index {
                            // Clicked already selected row: play!
                            let tracks: Vec<Track> = self.library.search(&self.search_query).into_iter().cloned().collect();
                            self.play_queue_at(tracks, t.track_index);
                        } else {
                            self.songs_selected_idx = t.track_index;
                        }
                        self.focus = FocusArea::MainView;
                        return;
                    }
                }
            }
            ViewMode::Artists => {
                for t in &self.last_artists_targets {
                    if is_inside(t.rect, x, y) {
                        let prev_col = self.artists_col;
                        self.artists_col = t.column;
                        match t.column {
                            ArtistColumn::Artists => {
                                self.artists_selected_idx = t.item_index;
                                self.artists_album_idx = 0;
                                self.artists_song_idx = 0;
                                self.artists_album_scroll_offset = 0;
                                self.artists_song_scroll_offset = 0;
                            }
                            ArtistColumn::Albums => {
                                self.artists_album_idx = t.item_index;
                                self.artists_song_idx = 0;
                                self.artists_song_scroll_offset = 0;
                            }
                            ArtistColumn::Songs => {
                                if prev_col == ArtistColumn::Songs && self.artists_song_idx == t.item_index {
                                    // Play clicked song on double-click / re-click!
                                    let artists = self.library.artists();
                                    if let Some(art) = artists.get(self.artists_selected_idx) {
                                        let albums = self.library.albums_by_artist(art);
                                        if let Some(alb) = albums.get(self.artists_album_idx) {
                                            let songs: Vec<Track> = self
                                                .library
                                                .tracks_by_artist_and_album(art, alb)
                                                .into_iter()
                                                .cloned()
                                                .collect();
                                            self.play_queue_at(songs, t.item_index);
                                        }
                                    }
                                } else {
                                    self.artists_song_idx = t.item_index;
                                }
                            }
                        }
                        self.focus = FocusArea::MainView;
                        return;
                    }
                }
            }
            ViewMode::Albums => {
                for t in &self.last_albums_targets {
                    if is_inside(t.rect, x, y) {
                        let prev_col = self.albums_col;
                        self.albums_col = t.column;
                        match t.column {
                            AlbumColumn::Albums => {
                                if prev_col == AlbumColumn::Albums && self.albums_selected_idx == t.item_index {
                                    // Play album on double-click
                                    let albums = self.library.all_albums();
                                    if let Some(alb) = albums.get(t.item_index) {
                                        let songs: Vec<Track> = self
                                            .library
                                            .tracks_by_artist_and_album(&alb.artist, &alb.album)
                                            .into_iter()
                                            .cloned()
                                            .collect();
                                        self.play_queue_at(songs, 0);
                                    }
                                } else {
                                    self.albums_selected_idx = t.item_index;
                                    self.albums_song_idx = 0;
                                    self.albums_song_scroll_offset = 0;
                                }
                            }
                            AlbumColumn::Songs => {
                                if prev_col == AlbumColumn::Songs && self.albums_song_idx == t.item_index {
                                    let albums = self.library.all_albums();
                                    if let Some(alb) = albums.get(self.albums_selected_idx) {
                                        let songs: Vec<Track> = self
                                            .library
                                            .tracks_by_artist_and_album(&alb.artist, &alb.album)
                                            .into_iter()
                                            .cloned()
                                            .collect();
                                        self.play_queue_at(songs, t.item_index);
                                    }
                                } else {
                                    self.albums_song_idx = t.item_index;
                                }
                            }
                        }
                        self.focus = FocusArea::MainView;
                        return;
                    }
                }
            }
            ViewMode::NowPlaying => {
                if let Some(target) = &self.last_now_playing_target {
                    if is_inside(target.waveform_rect, x, y) {
                        let ratio = relative_x_ratio(target.waveform_rect, x);
                        let total = self.audio.total_duration().as_secs_f32();
                        self.audio.seek_to(Duration::from_secs_f32(ratio * total));
                    }
                }
            }
            ViewMode::Playlists => {
                let hit = self.last_playlists_targets.iter().find(|t| is_inside(t.rect, x, y)).cloned();
                if let Some(t) = hit {
                    if t.is_rename_button {
                        if let Some(id) = t.playlist_id {
                            self.prompt_rename_playlist(&id);
                        }
                    } else if t.is_delete_button {
                        if let Some(id) = t.playlist_id {
                            self.prompt_delete_playlist(&id);
                        }
                    } else if t.is_playlist_selector {
                        if let Some(id) = t.playlist_id {
                            self.active_playlist_id = id;
                            self.playlists_track_idx = 0;
                            self.playlists_scroll_offset = 0;
                        }
                    } else if let Some(idx) = t.track_index {
                        if self.playlists_track_idx == idx {
                            // Play!
                            let track_ids: &[String] = if self.active_playlist_id == "__favorites__" {
                                &self.playlists_manager.favorites
                            } else if self.active_playlist_id == "__recent__" {
                                &self.playlists_manager.recently_played
                            } else if let Some(p) = self.playlists_manager.get_playlist(&self.active_playlist_id) {
                                &p.track_ids
                            } else {
                                &[]
                            };
                            let songs: Vec<Track> = self.library.tracks_for_playlist(track_ids).into_iter().cloned().collect();
                            self.play_queue_at(songs, idx);
                        } else {
                            self.playlists_track_idx = idx;
                        }
                    }
                    self.focus = FocusArea::MainView;
                    return;
                }
            }
            ViewMode::FileBrowser => {
                for t in &self.last_browser_targets {
                    if is_inside(t.rect, x, y) {
                        if self.browser_selected_idx == t.item_index {
                            // Open dir or play file
                            if let Some(item) = self.browser_items.get(t.item_index).cloned() {
                                if item.is_dir {
                                    self.browser_current_dir = item.path;
                                    self.browser_items = FileBrowserView::list_directory(&self.browser_current_dir);
                                    self.browser_selected_idx = 0;
                                    self.browser_scroll_offset = 0;
                                } else if let Some(track) = Scanner::read_track(&item.path) {
                                    self.play_track_now(track);
                                }
                            }
                        } else {
                            self.browser_selected_idx = t.item_index;
                        }
                        self.focus = FocusArea::MainView;
                        return;
                    }
                }
            }
            ViewMode::Settings => {
                for t in &self.last_settings_targets {
                    if is_inside(t.rect, x, y) {
                        match t.action {
                            SettingsAction::SelectFolder(idx) => {
                                self.settings_selected_folder = idx;
                            }
                            SettingsAction::AddFolder => {
                                if self.is_entering_folder {
                                    let path = PathBuf::from(self.new_folder_input.trim());
                                    if path.exists() && !self.config.library_folders.contains(&path) {
                                        self.config.library_folders.push(path);
                                        self.config.save().ok();
                                        self.trigger_scan();
                                    }
                                    self.is_entering_folder = false;
                                    self.new_folder_input.clear();
                                } else {
                                    self.open_folder_dialog(FolderDialogFocus::PathInput);
                                }
                            }
                            SettingsAction::RemoveFolder(idx) => {
                                if idx < self.config.library_folders.len() {
                                    self.config.library_folders.remove(idx);
                                    self.config.save().ok();
                                    self.trigger_scan();
                                }
                            }
                            SettingsAction::ScanLibrary => {
                                self.trigger_scan();
                            }
                        }
                        self.focus = FocusArea::MainView;
                        return;
                    }
                }
            }
            ViewMode::SearchResults => {
                for t in &self.last_search_targets {
                    if is_inside(t.rect, x, y) {
                        let prev_sec = self.search_section;
                        self.search_section = t.section;
                        match t.section {
                            SearchSection::Artists => {
                                if prev_sec == SearchSection::Artists && self.search_artist_idx == t.item_index {
                                    let artists = self.library.search_artists(&self.search_query);
                                    if let Some(art) = artists.get(t.item_index) {
                                        let songs: Vec<Track> = self.library.tracks_by_artist(&art.name).into_iter().cloned().collect();
                                        self.play_queue_at(songs, 0);
                                    }
                                } else {
                                    self.search_artist_idx = t.item_index;
                                }
                            }
                            SearchSection::Albums => {
                                if prev_sec == SearchSection::Albums && self.search_album_idx == t.item_index {
                                    let albums = self.library.search_albums(&self.search_query);
                                    if let Some(alb) = albums.get(t.item_index) {
                                        let songs: Vec<Track> = self.library.tracks_by_artist_and_album(&alb.artist, &alb.album).into_iter().cloned().collect();
                                        self.play_queue_at(songs, 0);
                                    }
                                } else {
                                    self.search_album_idx = t.item_index;
                                }
                            }
                            SearchSection::Songs => {
                                if prev_sec == SearchSection::Songs && self.search_song_idx == t.item_index {
                                    let songs: Vec<Track> = self.library.search_tracks(&self.search_query).into_iter().cloned().collect();
                                    self.play_queue_at(songs, t.item_index);
                                } else {
                                    self.search_song_idx = t.item_index;
                                }
                            }
                        }
                        self.focus = FocusArea::MainView;
                        return;
                    }
                }
            }
        }
    }

    fn handle_mouse_drag(&mut self, x: u16, y: u16) {
        let mut seek_ratio = None;
        let mut vol_ratio = None;

        for t in &self.last_player_targets {
            if is_inside(t.rect, x, y) {
                match t.action {
                    PlayerAction::Seek(_) => {
                        seek_ratio = Some(relative_x_ratio(t.rect, x));
                    }
                    PlayerAction::SetVolume(_) => {
                        vol_ratio = Some(relative_x_ratio(t.rect, x));
                    }
                    _ => {}
                }
            }
        }

        if let Some(ratio) = seek_ratio {
            let total = self.audio.total_duration().as_secs_f32();
            self.audio.seek_to(Duration::from_secs_f32(ratio * total));
        }
        if let Some(ratio) = vol_ratio {
            self.set_volume(ratio);
        }
    }

    fn handle_mouse_scroll(&mut self, is_up: bool, x: u16, y: u16) {
        if let Some(state) = &mut self.folder_dialog {
            state.scroll_by(is_up);
            return;
        }

        // Check if mouse is hovering over player volume
        for t in &self.last_player_targets {
            if is_inside(t.rect, x, y) {
                if let PlayerAction::SetVolume(_) = t.action {
                    let delta = if is_up { 0.05 } else { -0.05 };
                    self.adjust_volume(delta);
                    return;
                }
            }
        }

        let vis_0 = self.visible_main_rows(0);
        let vis_2 = self.visible_main_rows(1);
        let ma = self.last_main_area;

        // Scroll only the scroll depth (scroll_offset) of the hovered pane without modifying selection
        match self.view_mode {
            ViewMode::Songs => {
                let count = self.library.search(&self.search_query).len();
                Self::scroll_pane(is_up, &mut self.songs_scroll_offset, count, vis_2);
            }
            ViewMode::Artists => {
                let col = if ma.width > 0 && x >= ma.left() {
                    let col1_w = (ma.width * 28) / 100;
                    let col2_w = (ma.width * 32) / 100;
                    if x < ma.left() + col1_w {
                        ArtistColumn::Artists
                    } else if x < ma.left() + col1_w + col2_w {
                        ArtistColumn::Albums
                    } else {
                        ArtistColumn::Songs
                    }
                } else {
                    self.artists_col
                };
                match col {
                    ArtistColumn::Artists => {
                        let count = self.library.artists().len();
                        Self::scroll_pane(is_up, &mut self.artists_scroll_offset, count, vis_0);
                    }
                    ArtistColumn::Albums => {
                        let artists = self.library.artists();
                        let count = artists
                            .get(self.artists_selected_idx)
                            .map(|art| self.library.albums_by_artist(art).len())
                            .unwrap_or(0);
                        Self::scroll_pane(is_up, &mut self.artists_album_scroll_offset, count, vis_0);
                    }
                    ArtistColumn::Songs => {
                        let artists = self.library.artists();
                        let count = artists
                            .get(self.artists_selected_idx)
                            .and_then(|art| {
                                let albums = self.library.albums_by_artist(art);
                                albums
                                    .get(self.artists_album_idx)
                                    .map(|alb| self.library.tracks_by_artist_and_album(art, alb).len())
                            })
                            .unwrap_or(0);
                        Self::scroll_pane(is_up, &mut self.artists_song_scroll_offset, count, vis_0);
                    }
                }
            }
            ViewMode::Albums => {
                let col = if ma.width > 0 && x >= ma.left() {
                    let col1_w = (ma.width * 42) / 100;
                    if x < ma.left() + col1_w {
                        AlbumColumn::Albums
                    } else {
                        AlbumColumn::Songs
                    }
                } else {
                    self.albums_col
                };
                match col {
                    AlbumColumn::Albums => {
                        let count = self.library.all_albums().len();
                        Self::scroll_pane(is_up, &mut self.albums_scroll_offset, count, vis_0);
                    }
                    AlbumColumn::Songs => {
                        let albums = self.library.all_albums();
                        let count = albums
                            .get(self.albums_selected_idx)
                            .map(|alb| self.library.tracks_by_artist_and_album(&alb.artist, &alb.album).len())
                            .unwrap_or(0);
                        Self::scroll_pane(is_up, &mut self.albums_song_scroll_offset, count, vis_0);
                    }
                }
            }
            ViewMode::Playlists => {
                let left_w = 28u16.min(ma.width / 3);
                if ma.width > 0 && x >= ma.left() && x < ma.left() + left_w {
                    let count = 2 + self.playlists_manager.playlists.len();
                    Self::scroll_pane(is_up, &mut self.playlists_list_scroll_offset, count, vis_0);
                } else {
                    let track_ids: &[String] = if self.active_playlist_id == "__favorites__" {
                        &self.playlists_manager.favorites
                    } else if self.active_playlist_id == "__recent__" {
                        &self.playlists_manager.recently_played
                    } else if let Some(p) = self.playlists_manager.get_playlist(&self.active_playlist_id) {
                        &p.track_ids
                    } else {
                        &[]
                    };
                    let count = self.library.tracks_for_playlist(track_ids).len();
                    Self::scroll_pane(is_up, &mut self.playlists_scroll_offset, count, vis_2);
                }
            }
            ViewMode::FileBrowser => {
                let count = self.browser_items.len();
                Self::scroll_pane(is_up, &mut self.browser_scroll_offset, count, vis_0);
            }
            ViewMode::SearchResults => {
                let sec = if ma.width > 0 && x >= ma.left() {
                    let col1_w = (ma.width * 25) / 100;
                    let col2_w = (ma.width * 30) / 100;
                    if x < ma.left() + col1_w {
                        SearchSection::Artists
                    } else if x < ma.left() + col1_w + col2_w {
                        SearchSection::Albums
                    } else {
                        SearchSection::Songs
                    }
                } else {
                    self.search_section
                };
                match sec {
                    SearchSection::Artists => {
                        let count = self.library.search_artists(&self.search_query).len();
                        Self::scroll_pane(is_up, &mut self.search_artist_scroll, count, vis_0);
                    }
                    SearchSection::Albums => {
                        let count = self.library.search_albums(&self.search_query).len();
                        Self::scroll_pane(is_up, &mut self.search_album_scroll, count, vis_0);
                    }
                    SearchSection::Songs => {
                        let count = self.library.search_tracks(&self.search_query).len();
                        Self::scroll_pane(is_up, &mut self.search_song_scroll, count, vis_0);
                    }
                }
            }
            _ => {}
        }
    }

    pub fn set_view(&mut self, view: ViewMode) {
        if view != ViewMode::SearchResults {
            self.is_searching = false;
        }
        self.view_mode = view;
        let items = SidebarComponent {
            current_view: view,
            current_playlist_id: Some(&self.active_playlist_id),
            playlists_manager: &self.playlists_manager,
            is_focused: false,
            selected_index: 0,
            theme: &self.theme,
        }.build_items();

        if let Some(pos) = items.iter().position(|(_, _, is_active)| *is_active) {
            self.sidebar_index = pos;
        }
    }

    pub fn adjust_volume(&mut self, delta: f32) {
        let cur = self.audio.volume();
        self.set_volume(cur + delta);
    }

    pub fn set_volume(&mut self, vol: f32) {
        let clamped = vol.clamp(0.0, 1.5);
        self.is_muted = false;
        self.audio.set_volume(clamped);
        self.config.volume = clamped;
        self.config.save().ok();
    }

    pub fn toggle_mute(&mut self) {
        if self.is_muted {
            self.is_muted = false;
            self.audio.set_volume(self.pre_mute_volume);
        } else {
            self.pre_mute_volume = self.audio.volume();
            self.is_muted = true;
            self.audio.set_volume(0.0);
        }
    }

    pub fn trigger_scan(&mut self) {
        self.is_scanning = true;
        self.scan_found_count = 0;
        self.scan_status = "Scanning library folders...".to_string();
        let folders = self.config.library_folders.clone();
        let (tx, rx) = crossbeam_channel::unbounded();
        self.scan_receiver = Some(rx);

        std::thread::Builder::new()
            .name("library-scanner".into())
            .spawn(move || {
                Scanner::scan_directories_with_sender(&folders, tx);
            })
            .ok();
    }
}
