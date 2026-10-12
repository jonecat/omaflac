use crate::audio::AudioEngine;
use crate::config::{AppConfig, RepeatMode};
use crate::library::playlist::PlaylistsManager;
use crate::library::scanner::Scanner;
use crate::library::track::Track;
use crate::library::Library;
use crate::ui::artwork::ArtworkCache;
use crate::ui::components::header::HeaderClickTarget;
use crate::ui::components::player_bar::{PlayerAction, PlayerClickTarget};
use crate::ui::components::playlist_dialog::{PlaylistDialogClickTarget, PlaylistDialogMode};
use crate::ui::components::sidebar::{SidebarClickTarget, SidebarItem, ViewMode};
use crate::ui::mouse::{is_inside, relative_x_ratio};
use crate::ui::views::albums_view::AlbumsViewClickTarget;
use crate::ui::views::artists_view::{ArtistColumn, ArtistsViewClickTarget};
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
}

pub struct App {
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

    // Modals
    pub show_help: bool,
    pub playlist_dialog: Option<(PlaylistDialogMode, Track)>,

    // Songs view state
    pub songs_selected_idx: usize,
    pub songs_scroll_offset: usize,

    // Artists view state
    pub artists_col: ArtistColumn,
    pub artists_selected_idx: usize,
    pub artists_album_idx: usize,
    pub artists_song_idx: usize,

    // Albums view state
    pub albums_selected_idx: usize,
    pub albums_scroll_offset: usize,

    // Playlists view state
    pub active_playlist_id: String,
    pub playlists_track_idx: usize,

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

    // Mouse Hit Targets recorded from previous render frame
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
}

pub struct FileBrowserItemTarget {
    pub rect: Rect,
    pub item_index: usize,
}

impl App {
    pub fn new() -> anyhow::Result<Self> {
        let config = AppConfig::load();
        let mut library = Library::load();
        let playlists_manager = PlaylistsManager::load();
        let audio = AudioEngine::new()?;
        let artwork_cache = ArtworkCache::new();

        // If library is empty but config has folders, perform initial scan
        if library.tracks.is_empty() && !config.library_folders.is_empty() {
            library.scan(&config.library_folders);
        }

        let initial_dir = config
            .library_folders
            .first()
            .cloned()
            .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")));

        let browser_items = FileBrowserView::list_directory(&initial_dir);

        let app = Self {
            config,
            library,
            playlists_manager,
            audio,
            artwork_cache,
            view_mode: ViewMode::Songs,
            focus: FocusArea::MainView,
            sidebar_index: 0,
            should_quit: false,
            playback_queue: Vec::new(),
            queue_index: 0,
            is_muted: false,
            pre_mute_volume: 0.8,
            search_query: String::new(),
            is_searching: false,
            show_help: false,
            playlist_dialog: None,
            songs_selected_idx: 0,
            songs_scroll_offset: 0,
            artists_col: ArtistColumn::Artists,
            artists_selected_idx: 0,
            artists_album_idx: 0,
            artists_song_idx: 0,
            albums_selected_idx: 0,
            albums_scroll_offset: 0,
            active_playlist_id: "__favorites__".to_string(),
            playlists_track_idx: 0,
            browser_current_dir: initial_dir,
            browser_items,
            browser_selected_idx: 0,
            browser_scroll_offset: 0,
            settings_selected_folder: 0,
            new_folder_input: String::new(),
            is_entering_folder: false,
            is_scanning: false,
            scan_status: String::new(),
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
        };

        app.audio.set_volume(app.config.volume);
        Ok(app)
    }

    pub fn on_tick(&mut self) {
        // Check for audio events
        while let Some(event) = self.audio.try_recv_event() {
            match event {
                crate::audio::PlayerEvent::TrackFinished => {
                    self.handle_track_finished();
                }
                crate::audio::PlayerEvent::Error(err) => {
                    self.scan_status = format!("Audio Error: {}", err);
                }
            }
        }
    }

    pub fn play_track_now(&mut self, track: Track) {
        self.playlists_manager.add_recently_played(&track.id);
        self.audio.play_track(track);
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
        if let Some(track) = self.audio.current_track() {
            self.playlists_manager.toggle_favorite(&track.id);
        } else if let Some(track) = self.get_selected_track() {
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
            _ => None,
        }
    }

    pub fn prompt_add_to_playlist(&mut self) {
        if let Some(track) = self.get_selected_track().or_else(|| self.audio.current_track()) {
            self.playlist_dialog = Some((
                PlaylistDialogMode::SelectPlaylist { selected_idx: 0 },
                track,
            ));
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        // 1. Help Modal active
        if self.show_help {
            if key.code == KeyCode::Esc || key.code == KeyCode::Char('?') || key.code == KeyCode::Char('q') {
                self.show_help = false;
            }
            return;
        }

        // 2. Playlist Dialog active
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
                                self.playlists_manager.add_track(&p.id, &track.id);
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
                            self.playlists_manager.add_track(&new_id, &track.id);
                        }
                        self.playlist_dialog = None;
                    }
                    _ => {
                        self.playlist_dialog = Some((PlaylistDialogMode::CreateNew { name_input }, track));
                    }
                },
            }
            return;
        }

        // 3. Search Mode active
        if self.is_searching {
            match key.code {
                KeyCode::Esc => {
                    self.is_searching = false;
                }
                KeyCode::Enter => {
                    self.is_searching = false;
                    // Play first search match
                    let matches = self.library.search(&self.search_query);
                    if let Some(t) = matches.first() {
                        self.play_track_now((*t).clone());
                    }
                }
                KeyCode::Backspace => {
                    self.search_query.pop();
                    self.songs_selected_idx = 0;
                    self.songs_scroll_offset = 0;
                }
                KeyCode::Char(c) => {
                    self.search_query.push(c);
                    self.songs_selected_idx = 0;
                    self.songs_scroll_offset = 0;
                }
                _ => {}
            }
            return;
        }

        // 4. Folder input mode in Settings
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

        // 5. Global Hotkeys
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
                self.is_searching = true;
                return;
            }
            KeyCode::Tab => {
                self.focus = match self.focus {
                    FocusArea::Sidebar => FocusArea::MainView,
                    FocusArea::MainView => FocusArea::Sidebar,
                };
                return;
            }
            KeyCode::BackTab => {
                self.focus = match self.focus {
                    FocusArea::Sidebar => FocusArea::MainView,
                    FocusArea::MainView => FocusArea::Sidebar,
                };
                return;
            }
            // Number keys 1..7 switch views directly
            KeyCode::Char('1') => { self.set_view(ViewMode::Songs); return; }
            KeyCode::Char('2') => { self.set_view(ViewMode::Artists); return; }
            KeyCode::Char('3') => { self.set_view(ViewMode::Albums); return; }
            KeyCode::Char('4') => { self.set_view(ViewMode::NowPlaying); return; }
            KeyCode::Char('5') => { self.set_view(ViewMode::Playlists); return; }
            KeyCode::Char('6') => { self.set_view(ViewMode::FileBrowser); return; }
            KeyCode::Char('7') => { self.set_view(ViewMode::Settings); return; }

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
                self.audio.seek_relative(-30);
                return;
            }
            KeyCode::Right if key.modifiers.contains(KeyModifiers::SHIFT) => {
                self.audio.seek_relative(30);
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
            KeyCode::Char('r') => {
                self.config.repeat_mode = self.config.repeat_mode.next();
                self.config.save().ok();
                return;
            }
            KeyCode::Char('z') => {
                self.config.shuffle = !self.config.shuffle;
                self.config.save().ok();
                return;
            }
            KeyCode::Char('a') => {
                self.prompt_add_to_playlist();
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
        }
    }

    fn handle_sidebar_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.sidebar_index = self.sidebar_index.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let max_items = 8 + self.playlists_manager.playlists.len();
                self.sidebar_index = (self.sidebar_index + 1).min(max_items.saturating_sub(1));
            }
            KeyCode::Enter => {
                // Activate selected sidebar item
                self.activate_sidebar_index(self.sidebar_index);
            }
            _ => {}
        }
    }

    fn activate_sidebar_index(&mut self, index: usize) {
        // 0: Songs, 1: Artists, 2: Albums, 3: NowPlaying, 4: FileBrowser
        // 5: Favorites, 6: RecentlyPlayed, 7..7+N: Playlists, N+8: NewPlaylist, N+9: Settings
        match index {
            0 => self.set_view(ViewMode::Songs),
            1 => self.set_view(ViewMode::Artists),
            2 => self.set_view(ViewMode::Albums),
            3 => self.set_view(ViewMode::NowPlaying),
            4 => self.set_view(ViewMode::FileBrowser),
            5 => {
                self.active_playlist_id = "__favorites__".to_string();
                self.set_view(ViewMode::Playlists);
            }
            6 => {
                self.active_playlist_id = "__recent__".to_string();
                self.set_view(ViewMode::Playlists);
            }
            idx => {
                let playlist_offset = idx.saturating_sub(7);
                if playlist_offset < self.playlists_manager.playlists.len() {
                    let id = self.playlists_manager.playlists[playlist_offset].id.clone();
                    self.active_playlist_id = id;
                    self.set_view(ViewMode::Playlists);
                } else if playlist_offset == self.playlists_manager.playlists.len() {
                    // New playlist dialog
                    if let Some(track) = self.get_selected_track().or_else(|| self.audio.current_track()) {
                        self.playlist_dialog = Some((PlaylistDialogMode::CreateNew { name_input: String::new() }, track));
                    }
                } else {
                    self.set_view(ViewMode::Settings);
                }
            }
        }
        self.focus = FocusArea::MainView;
    }

    fn handle_main_view_key(&mut self, key: KeyEvent) {
        match self.view_mode {
            ViewMode::Songs => match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    self.songs_selected_idx = self.songs_selected_idx.saturating_sub(1);
                    if self.songs_selected_idx < self.songs_scroll_offset {
                        self.songs_scroll_offset = self.songs_selected_idx;
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    let tracks = self.library.search(&self.search_query);
                    if !tracks.is_empty() {
                        self.songs_selected_idx = (self.songs_selected_idx + 1).min(tracks.len() - 1);
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
                    }
                    ArtistColumn::Albums => {
                        self.artists_album_idx = self.artists_album_idx.saturating_sub(1);
                        self.artists_song_idx = 0;
                    }
                    ArtistColumn::Songs => {
                        self.artists_song_idx = self.artists_song_idx.saturating_sub(1);
                    }
                },
                KeyCode::Down | KeyCode::Char('j') => match self.artists_col {
                    ArtistColumn::Artists => {
                        let artists = self.library.artists();
                        if !artists.is_empty() {
                            self.artists_selected_idx = (self.artists_selected_idx + 1).min(artists.len() - 1);
                            self.artists_album_idx = 0;
                            self.artists_song_idx = 0;
                        }
                    }
                    ArtistColumn::Albums => {
                        let artists = self.library.artists();
                        if let Some(art) = artists.get(self.artists_selected_idx) {
                            let albums = self.library.albums_by_artist(art);
                            if !albums.is_empty() {
                                self.artists_album_idx = (self.artists_album_idx + 1).min(albums.len() - 1);
                                self.artists_song_idx = 0;
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
                KeyCode::Up | KeyCode::Char('k') => {
                    self.albums_selected_idx = self.albums_selected_idx.saturating_sub(1);
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    let albums = self.library.all_albums();
                    if !albums.is_empty() {
                        self.albums_selected_idx = (self.albums_selected_idx + 1).min(albums.len() - 1);
                    }
                }
                KeyCode::Enter => {
                    let albums = self.library.all_albums();
                    if let Some(alb) = albums.get(self.albums_selected_idx) {
                        let songs: Vec<Track> = self
                            .library
                            .tracks_by_artist_and_album(&alb.artist, &alb.album)
                            .into_iter()
                            .cloned()
                            .collect();
                        self.play_queue_at(songs, 0);
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
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if !self.browser_items.is_empty() {
                        self.browser_selected_idx = (self.browser_selected_idx + 1).min(self.browser_items.len() - 1);
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
                    self.is_entering_folder = true;
                    self.new_folder_input.clear();
                }
                KeyCode::Char('s') => {
                    self.trigger_scan();
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
        // 1. Help modal click to close
        if self.show_help {
            self.show_help = false;
            return;
        }

        // 2. Playlist Dialog click
        for t in &self.last_dialog_targets {
            if is_inside(t.rect, x, y) {
                if t.is_cancel_button {
                    self.playlist_dialog = None;
                    return;
                }
                if let Some((mode, track)) = self.playlist_dialog.take() {
                    if t.is_create_button {
                        match mode {
                            PlaylistDialogMode::SelectPlaylist { .. } => {
                                self.playlist_dialog = Some((PlaylistDialogMode::CreateNew { name_input: String::new() }, track));
                            }
                            PlaylistDialogMode::CreateNew { name_input } => {
                                let trimmed = name_input.trim();
                                if !trimmed.is_empty() {
                                    let new_id = self.playlists_manager.create_playlist(trimmed);
                                    self.playlists_manager.add_track(&new_id, &track.id);
                                }
                                self.playlist_dialog = None;
                            }
                        }
                    } else if let Some(idx) = t.index {
                        if idx > 0 {
                            let p_idx = idx - 1;
                            if let Some(p) = self.playlists_manager.playlists.get(p_idx).cloned() {
                                self.playlists_manager.add_track(&p.id, &track.id);
                            }
                        }
                        self.playlist_dialog = None;
                    }
                }
                return;
            }
        }

        // 3. Header clicks
        if let Some(hdr) = &self.last_header_targets {
            if is_inside(hdr.help_rect, x, y) {
                self.show_help = true;
                return;
            }
            if is_inside(hdr.search_rect, x, y) {
                self.is_searching = true;
                return;
            }
        }

        // 4. Player bar controls click
        for t in &self.last_player_targets {
            if is_inside(t.rect, x, y) {
                match t.action {
                    PlayerAction::Previous => self.prev_track(),
                    PlayerAction::TogglePlayPause => self.audio.toggle_pause(),
                    PlayerAction::Stop => self.audio.stop(),
                    PlayerAction::Next => self.next_track(),
                    PlayerAction::ToggleRepeat => {
                        self.config.repeat_mode = self.config.repeat_mode.next();
                        self.config.save().ok();
                    }
                    PlayerAction::ToggleShuffle => {
                        self.config.shuffle = !self.config.shuffle;
                        self.config.save().ok();
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
                }
                return;
            }
        }

        // 5. Sidebar clicks
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
                        if let Some(track) = self.get_selected_track().or_else(|| self.audio.current_track()) {
                            self.playlist_dialog = Some((PlaylistDialogMode::CreateNew { name_input: String::new() }, track));
                        }
                    }
                }
                self.focus = FocusArea::Sidebar;
                return;
            }
        }

        // 6. Main View Clicks
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
                        self.artists_col = t.column;
                        match t.column {
                            ArtistColumn::Artists => {
                                self.artists_selected_idx = t.item_index;
                                self.artists_album_idx = 0;
                                self.artists_song_idx = 0;
                            }
                            ArtistColumn::Albums => {
                                self.artists_album_idx = t.item_index;
                                self.artists_song_idx = 0;
                            }
                            ArtistColumn::Songs => {
                                self.artists_song_idx = t.item_index;
                                // Play clicked song!
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
                        if self.albums_selected_idx == t.album_index {
                            let albums = self.library.all_albums();
                            if let Some(alb) = albums.get(t.album_index) {
                                let songs: Vec<Track> = self
                                    .library
                                    .tracks_by_artist_and_album(&alb.artist, &alb.album)
                                    .into_iter()
                                    .cloned()
                                    .collect();
                                self.play_queue_at(songs, 0);
                            }
                        } else {
                            self.albums_selected_idx = t.album_index;
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
                for t in &self.last_playlists_targets {
                    if is_inside(t.rect, x, y) {
                        if t.is_playlist_selector {
                            if let Some(id) = &t.playlist_id {
                                self.active_playlist_id = id.clone();
                                self.playlists_track_idx = 0;
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
                                    self.is_entering_folder = true;
                                    self.new_folder_input.clear();
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

        // Otherwise scroll current view
        match self.view_mode {
            ViewMode::Songs => {
                if is_up {
                    self.songs_selected_idx = self.songs_selected_idx.saturating_sub(1);
                } else {
                    let count = self.library.search(&self.search_query).len();
                    if count > 0 {
                        self.songs_selected_idx = (self.songs_selected_idx + 1).min(count - 1);
                    }
                }
            }
            ViewMode::Artists => {
                if is_up {
                    self.artists_selected_idx = self.artists_selected_idx.saturating_sub(1);
                } else {
                    let count = self.library.artists().len();
                    if count > 0 {
                        self.artists_selected_idx = (self.artists_selected_idx + 1).min(count - 1);
                    }
                }
            }
            ViewMode::Albums => {
                if is_up {
                    self.albums_selected_idx = self.albums_selected_idx.saturating_sub(1);
                } else {
                    let count = self.library.all_albums().len();
                    if count > 0 {
                        self.albums_selected_idx = (self.albums_selected_idx + 1).min(count - 1);
                    }
                }
            }
            ViewMode::Playlists => {
                if is_up {
                    self.playlists_track_idx = self.playlists_track_idx.saturating_sub(1);
                } else {
                    self.playlists_track_idx += 1;
                }
            }
            ViewMode::FileBrowser => {
                if is_up {
                    self.browser_selected_idx = self.browser_selected_idx.saturating_sub(1);
                } else if !self.browser_items.is_empty() {
                    self.browser_selected_idx = (self.browser_selected_idx + 1).min(self.browser_items.len() - 1);
                }
            }
            _ => {}
        }
    }

    pub fn set_view(&mut self, view: ViewMode) {
        self.view_mode = view;
        self.sidebar_index = match view {
            ViewMode::Songs => 0,
            ViewMode::Artists => 1,
            ViewMode::Albums => 2,
            ViewMode::NowPlaying => 3,
            ViewMode::FileBrowser => 4,
            ViewMode::Playlists => 5,
            ViewMode::Settings => 7 + self.playlists_manager.playlists.len() + 1,
        };
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
        self.scan_status = "Scanning library folders...".to_string();
        let folders = self.config.library_folders.clone();
        self.library.scan(&folders);
        self.is_scanning = false;
        self.scan_status = format!("Scan complete: {} audio tracks indexed", self.library.tracks.len());
    }
}
