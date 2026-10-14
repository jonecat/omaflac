use omaflac::library::track::Track;
use omaflac::library::Library;
use omaflac::ui::artwork::ArtworkCache;
use omaflac::ui::views::search_results_view::SearchSection;
use std::path::PathBuf;

fn create_sample_library() -> Library {
    let t1 = Track {
        id: "1".into(),
        path: PathBuf::from("/music/floyd/time.flac"),
        title: "Time".into(),
        artist: "Pink Floyd".into(),
        album: "The Dark Side of the Moon".into(),
        album_artist: Some("Pink Floyd".into()),
        genre: Some("Progressive Rock".into()),
        year: Some(1973),
        release_date: Some("1973-03-24".into()),
        track_number: Some(4),
        disc_number: Some(1),
        duration_secs: 425,
        format: "FLAC".into(),
        sample_rate: Some(96000),
        bit_depth: Some(24),
        bitrate_kbps: Some(2800),
        has_cover_art: true,
        file_size: Some(45_000_000),
        modified_secs: Some(1_700_000_000),
    };

    let t2 = Track {
        id: "2".into(),
        path: PathBuf::from("/music/floyd/money.flac"),
        title: "Money".into(),
        artist: "Pink Floyd".into(),
        album: "The Dark Side of the Moon".into(),
        album_artist: Some("Pink Floyd".into()),
        genre: Some("Progressive Rock".into()),
        year: Some(1973),
        release_date: Some("1973-03-24".into()),
        track_number: Some(6),
        disc_number: Some(1),
        duration_secs: 382,
        format: "FLAC".into(),
        sample_rate: Some(96000),
        bit_depth: Some(24),
        bitrate_kbps: Some(2850),
        has_cover_art: true,
        file_size: Some(42_000_000),
        modified_secs: Some(1_700_000_000),
    };

    let t3 = Track {
        id: "3".into(),
        path: PathBuf::from("/music/radiohead/karma.flac"),
        title: "Karma Police".into(),
        artist: "Radiohead".into(),
        album: "OK Computer".into(),
        album_artist: Some("Radiohead".into()),
        genre: Some("Alternative Rock".into()),
        year: Some(1997),
        release_date: Some("1997-05-21".into()),
        track_number: Some(6),
        disc_number: Some(1),
        duration_secs: 261,
        format: "FLAC".into(),
        sample_rate: Some(44100),
        bit_depth: Some(16),
        bitrate_kbps: Some(890),
        has_cover_art: true,
        file_size: Some(28_000_000),
        modified_secs: Some(1_700_000_000),
    };

    Library {
        tracks: vec![t1, t2, t3],
        scanned_at: None,
    }
}

#[test]
fn test_search_subsections() {
    let lib = create_sample_library();

    // 1. Search for "floyd"
    let artists = lib.search_artists("floyd");
    assert_eq!(artists.len(), 1);
    assert_eq!(artists[0].name, "Pink Floyd");
    assert_eq!(artists[0].album_count, 1);
    assert_eq!(artists[0].track_count, 2);

    // 2. Search for "dark"
    let albums = lib.search_albums("dark");
    assert_eq!(albums.len(), 1);
    assert_eq!(albums[0].album, "The Dark Side of the Moon");
    assert_eq!(albums[0].artist, "Pink Floyd");
    assert_eq!(albums[0].track_count, 2);

    // 3. Search for "karma"
    let songs = lib.search_tracks("karma");
    assert_eq!(songs.len(), 1);
    assert_eq!(songs[0].title, "Karma Police");
    assert_eq!(songs[0].artist, "Radiohead");

    // 4. Case-insensitive search
    let radio_artists = lib.search_artists("RADIO");
    assert_eq!(radio_artists.len(), 1);
    assert_eq!(radio_artists[0].name, "Radiohead");
}

#[test]
fn test_search_section_navigation() {
    let sec = SearchSection::Artists;
    assert_eq!(sec.next(), SearchSection::Albums);
    assert_eq!(sec.next().next(), SearchSection::Songs);
    assert_eq!(sec.next().next().next(), SearchSection::Artists);

    assert_eq!(sec.prev(), SearchSection::Songs);
    assert_eq!(sec.prev().prev(), SearchSection::Albums);
    assert_eq!(sec.prev().prev().prev(), SearchSection::Artists);
}

#[test]
fn test_artwork_cache_non_blocking_initialization() {
    let cache = ArtworkCache::new();
    let fake_path = PathBuf::from("/nonexistent/song.flac");
    assert!(!cache.is_loading(&fake_path));
}

#[test]
fn test_albums_two_pane_view() {
    use omaflac::library::playlist::PlaylistsManager;
    use omaflac::ui::theme::OmarchyTheme;
    use omaflac::ui::views::albums_view::{AlbumColumn, AlbumsView};
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    let lib = create_sample_library();
    let albums = lib.all_albums();
    assert_eq!(albums.len(), 2);

    let mut pm = PlaylistsManager::default();
    pm.favorites.push("1".to_string()); // Favorite "Time" in Pink Floyd - The Dark Side of the Moon (album index 0)

    let theme = OmarchyTheme::default();
    let view = AlbumsView {
        library: &lib,
        albums: &albums,
        active_column: AlbumColumn::Albums,
        selected_album_idx: 0,
        selected_song_idx: 0,
        scroll_album_offset: 0,
        scroll_song_offset: 0,
        playing_track_id: None,
        playlists_manager: &pm,
        is_focused: true,
        theme: &theme,
    };

    let area = Rect::new(0, 0, 100, 30);
    let mut buf = Buffer::empty(area);
    let targets = view.render_with_targets(area, &mut buf);

    // Targets should contain both AlbumColumn::Albums and AlbumColumn::Songs
    let has_album_targets = targets.iter().any(|t| t.column == AlbumColumn::Albums);
    let has_song_targets = targets.iter().any(|t| t.column == AlbumColumn::Songs);

    assert!(has_album_targets, "Should register click targets for album catalog");
    assert!(has_song_targets, "Should register click targets for songs in selected album");

    // Verify favorite star '★' is rendered in the buffer
    let rendered_text: String = buf.content().iter().map(|c| c.symbol()).collect();
    assert!(rendered_text.contains('★'), "Favorited song in AlbumsView should display ★");
}

#[test]
fn test_incremental_sync_and_offline_mount_protection() {
    use omaflac::library::scanner::{ScanUpdate, Scanner};

    let mut lib = create_sample_library();
    assert_eq!(lib.tracks.len(), 3);

    // 1. Test offline mount protection: "/music" does not exist on disk, so syncing ["/music"]
    // must NOT delete the 3 cached tracks!
    let (tx, rx) = crossbeam_channel::unbounded();
    Scanner::sync_directories_with_sender(
        &[PathBuf::from("/music")],
        lib.tracks.clone(),
        false,
        tx,
    );

    let mut offline_detected = 0;
    while let Ok(update) = rx.recv() {
        match update {
            ScanUpdate::RemoveTracks { removed_paths } => {
                lib.remove_tracks_by_path(&removed_paths);
            }
            ScanUpdate::SyncCompleted {
                offline_folders, ..
            } => {
                offline_detected = offline_folders;
            }
            _ => {}
        }
    }
    assert_eq!(offline_detected, 1, "Should detect offline /music mount");
    assert_eq!(
        lib.tracks.len(),
        3,
        "Cached tracks must be preserved when remote mount is offline"
    );

    // 2. Test incremental upsert of a new track without wiping existing tracks
    let new_track = Track {
        id: "4".into(),
        path: PathBuf::from("/music/daftpunk/one_more_time.flac"),
        title: "One More Time".into(),
        artist: "Daft Punk".into(),
        album: "Discovery".into(),
        album_artist: Some("Daft Punk".into()),
        genre: Some("Electronic".into()),
        year: Some(2001),
        release_date: Some("2001-03-12".into()),
        track_number: Some(1),
        disc_number: Some(1),
        duration_secs: 320,
        format: "FLAC".into(),
        sample_rate: Some(44100),
        bit_depth: Some(16),
        bitrate_kbps: Some(950),
        has_cover_art: true,
        file_size: Some(35_000_000),
        modified_secs: Some(1_700_000_000),
    };
    let (added, updated) = lib.upsert_tracks(vec![new_track]);
    assert_eq!(added, 1);
    assert_eq!(updated, 0);
    assert_eq!(lib.tracks.len(), 4);
    assert_eq!(
        lib.artists()[0],
        "Daft Punk",
        "New artist should be sorted alphabetically into existing library"
    );
}

#[test]
fn test_artist_sorting_ignores_leading_the() {
    let mut lib = create_sample_library(); // Has "Pink Floyd" and "Radiohead"
    let make_track = |id: &str, artist: &str, album: &str, title: &str| Track {
        id: id.into(),
        path: PathBuf::from(format!("/music/{}/{}.flac", id, title)),
        title: title.into(),
        artist: artist.into(),
        album: album.into(),
        album_artist: Some(artist.into()),
        genre: None,
        year: None,
        release_date: None,
        track_number: Some(1),
        disc_number: Some(1),
        duration_secs: 200,
        format: "FLAC".into(),
        sample_rate: Some(44100),
        bit_depth: Some(16),
        bitrate_kbps: None,
        has_cover_art: false,
        file_size: None,
        modified_secs: None,
    };

    lib.upsert_tracks(vec![
        make_track("10", "The Beatles", "Abbey Road", "Come Together"),
        make_track("11", "The White Stripes", "Elephant", "Seven Nation Army"),
        make_track("12", "Beach Boys", "Pet Sounds", "Wouldn't It Be Nice"),
        make_track("13", "Beck", "Odelay", "Where It's At"),
        make_track("14", "ZZ Top", "Eliminator", "Sharp Dressed Man"),
    ]);

    let artists = lib.artists();
    assert_eq!(
        artists,
        vec![
            "Beach Boys",
            "The Beatles",
            "Beck",
            "Pink Floyd",
            "Radiohead",
            "The White Stripes",
            "ZZ Top",
        ]
    );
}

#[test]
fn test_multi_artist_album_unified_in_albums_and_split_in_artists() {
    use omaflac::library::playlist::PlaylistsManager;
    use omaflac::ui::theme::OmarchyTheme;
    use omaflac::ui::views::albums_view::{AlbumColumn, AlbumsView};
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    let make_track = |id: &str, title: &str, artist: &str, album_artist: Option<&str>, trk_num: u32| Track {
        id: id.into(),
        path: PathBuf::from(format!("/music/Queen/Classic Queen/{}.flac", title)),
        title: title.into(),
        artist: artist.into(),
        album: "Classic Queen".into(),
        album_artist: album_artist.map(|s| s.into()),
        genre: Some("Rock".into()),
        year: Some(1992),
        release_date: Some("1992-03-03".into()),
        track_number: Some(trk_num),
        disc_number: Some(1),
        duration_secs: 240,
        format: "FLAC".into(),
        sample_rate: Some(44100),
        bit_depth: Some(16),
        bitrate_kbps: Some(900),
        has_cover_art: true,
        file_size: Some(25_000_000),
        modified_secs: Some(1_700_000_000),
    };

    let mut lib = Library::default();
    lib.upsert_tracks(vec![
        make_track("q1", "A Kind of Magic", "Queen", Some("Queen"), 1),
        make_track("q2", "Bohemian Rhapsody", "Queen", Some("Queen"), 2),
        make_track("q3", "Under Pressure", "Queen & David Bowie", Some("Queen"), 3),
        make_track("q4", "Radio Ga Ga", "Queen", Some("Queen"), 4),
    ]);

    // 1. ALBUMS VIEW VERIFICATION:
    // "Classic Queen" must be ONE album, not two separate albums
    let albums = lib.all_albums();
    assert_eq!(albums.len(), 1, "Classic Queen should be consolidated into a single album");
    assert_eq!(albums[0].album, "Classic Queen");
    assert_eq!(albums[0].artist, "Queen");
    assert_eq!(albums[0].track_count, 4, "All 4 tracks must be counted in Classic Queen");

    // All tracks of the album must be returned in the tracklist in track number order
    let album_tracks = lib.tracks_by_album(&albums[0].artist, &albums[0].album);
    assert_eq!(album_tracks.len(), 4);
    assert_eq!(album_tracks[0].title, "A Kind of Magic");
    assert_eq!(album_tracks[1].title, "Bohemian Rhapsody");
    assert_eq!(album_tracks[2].title, "Under Pressure");
    assert_eq!(album_tracks[2].artist, "Queen & David Bowie");
    assert_eq!(album_tracks[3].title, "Radio Ga Ga");

    // Render AlbumsView and verify Under Pressure displays artist credit "Queen & David Bowie"
    let pm = PlaylistsManager::default();
    let theme = OmarchyTheme::default();
    let view = AlbumsView {
        library: &lib,
        albums: &albums,
        active_column: AlbumColumn::Songs,
        selected_album_idx: 0,
        selected_song_idx: 2,
        scroll_album_offset: 0,
        scroll_song_offset: 0,
        playing_track_id: None,
        playlists_manager: &pm,
        is_focused: true,
        theme: &theme,
    };
    let area = Rect::new(0, 0, 100, 20);
    let mut buf = Buffer::empty(area);
    view.render_with_targets(area, &mut buf);
    let rendered: String = buf.content().iter().map(|c| c.symbol()).collect();
    assert!(
        rendered.contains("Under Pressure — Queen & David Bowie"),
        "Tracklist should render guest/featured credit when song artist != album artist"
    );

    // 2. ARTIST VIEW VERIFICATION:
    // Tracks remain split by artist in the artist view
    let artists = lib.artists();
    assert!(artists.contains(&"Queen".to_string()));
    assert!(artists.contains(&"Queen & David Bowie".to_string()));

    let queen_tracks = lib.tracks_by_artist_and_album("Queen", "Classic Queen");
    assert_eq!(queen_tracks.len(), 3, "Queen solo view should have 3 tracks");
    assert!(!queen_tracks.iter().any(|t| t.title == "Under Pressure"));

    let collab_tracks = lib.tracks_by_artist_and_album("Queen & David Bowie", "Classic Queen");
    assert_eq!(collab_tracks.len(), 1, "Queen & David Bowie view should only have Under Pressure");
    assert_eq!(collab_tracks[0].title, "Under Pressure");

    // 3. HEURISTIC MAJORITY VERIFICATION (album_artist = None):
    let mut lib2 = Library::default();
    lib2.upsert_tracks(vec![
        make_track("h1", "A Kind of Magic", "Queen", None, 1),
        make_track("h2", "Bohemian Rhapsody", "Queen", None, 2),
        make_track("h3", "Under Pressure", "Queen & David Bowie", None, 3),
        make_track("h4", "Radio Ga Ga", "Queen", None, 4),
    ]);
    let albums2 = lib2.all_albums();
    assert_eq!(albums2.len(), 1, "Should unify even without album_artist tag via majority heuristic");
    assert_eq!(albums2[0].artist, "Queen");
    assert_eq!(albums2[0].track_count, 4);
    let tracks2 = lib2.tracks_by_album(&albums2[0].artist, &albums2[0].album);
    assert_eq!(tracks2.len(), 4);
}

#[test]
fn test_header_search_and_help_button_styling() {
    use omaflac::ui::components::header::HeaderComponent;
    use omaflac::ui::theme::OmarchyTheme;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    let theme = OmarchyTheme::default();
    let area = Rect::new(0, 0, 100, 3);
    let mut buf = Buffer::empty(area);

    // 1. Idle state with empty search query
    let header_idle = HeaderComponent {
        view_title: "Library > Songs",
        total_tracks: 42,
        search_query: "",
        is_searching: false,
        scan_badge: None,
        update_badge: None,
        theme: &theme,
    };

    let targets = header_idle.render_with_targets(area, &mut buf);
    assert!(targets.help_rect.width > 0, "Help button target should be registered");
    assert!(targets.search_rect.width > 0, "Search field target should be registered");

    let text_idle: String = buf.content().iter().map(|c| c.symbol()).collect();
    assert!(text_idle.contains("? Help"), "Help button should show '? Help'");
    assert!(!text_idle.contains("F1"), "Help button should not contain F1");
    assert!(text_idle.contains("⌕ Search"), "Search field should show '⌕ Search'");
    assert!(text_idle.contains('/'), "Idle search field should display '/' key hint");

    // 2. Active search state with query "Radiohead"
    let mut buf_active = Buffer::empty(area);
    let header_active = HeaderComponent {
        view_title: "Search Results",
        total_tracks: 42,
        search_query: "Radiohead",
        is_searching: true,
        scan_badge: None,
        update_badge: None,
        theme: &theme,
    };

    header_active.render_with_targets(area, &mut buf_active);
    let text_active: String = buf_active.content().iter().map(|c| c.symbol()).collect();
    assert!(text_active.contains("Radiohead█"), "Active search should render query and cursor");
    assert!(text_active.contains("Esc"), "Active search should display 'Esc' cancel hint");
}

#[test]
fn test_settings_view_locked_terminal_theme() {
    use omaflac::config::AppConfig;
    use omaflac::ui::theme::OmarchyTheme;
    use omaflac::ui::views::settings_view::SettingsView;
    use omaflac::updater::UpdateStatus;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    let config = AppConfig::default();
    let theme = OmarchyTheme::default();
    let update_status = UpdateStatus::Idle;

    let view = SettingsView {
        config: &config,
        selected_folder_idx: 0,
        new_folder_input: "",
        is_entering_folder: false,
        is_entering_fps: false,
        fps_input: "",
        is_scanning: false,
        scan_status: "",
        update_status: &update_status,
        is_focused: true,
        theme: &theme,
    };

    let area = Rect::new(0, 0, 100, 30);
    let mut buf = Buffer::empty(area);
    let targets = view.render_with_targets(area, &mut buf);

    let rendered: String = buf.content().iter().map(|c| c.symbol()).collect();
    assert!(
        !rendered.contains("APPEARANCE & THEME"),
        "Settings should not contain APPEARANCE & THEME section"
    );
    assert!(
        !rendered.contains("Cycle Theme"),
        "Settings should not have a Cycle Theme button / random theme selector"
    );
    assert!(
        rendered.contains("VISUALIZER & REFRESH RATE"),
        "Settings should contain VISUALIZER & REFRESH RATE section"
    );
    assert!(
        rendered.contains("Change FPS"),
        "Settings should contain Change FPS button"
    );

    // Verify no click target exists for cycling theme
    for target in &targets {
        let text_inside: String = (target.rect.left()..target.rect.right())
            .map(|x| buf[(x, target.rect.top() + (target.rect.height / 2))].symbol())
            .collect();
        assert!(
            !text_inside.contains("Cycle"),
            "Click target should not be for cycling theme: {}",
            text_inside
        );
    }

    // Verify FPS input prompt rendering
    let view_entering = SettingsView {
        config: &config,
        selected_folder_idx: 0,
        new_folder_input: "",
        is_entering_folder: false,
        is_entering_fps: true,
        fps_input: "120",
        is_scanning: false,
        scan_status: "",
        update_status: &update_status,
        is_focused: true,
        theme: &theme,
    };
    let mut buf2 = Buffer::empty(area);
    let targets2 = view_entering.render_with_targets(area, &mut buf2);
    let rendered2: String = buf2.content().iter().map(|c| c.symbol()).collect();
    assert!(rendered2.contains("120█"));
    assert!(rendered2.contains("Confirm"));
    assert!(rendered2.contains("Cancel"));
    assert!(targets2.iter().any(|t| matches!(t.action, omaflac::ui::views::settings_view::SettingsAction::ConfirmFps)));
    assert!(targets2.iter().any(|t| matches!(t.action, omaflac::ui::views::settings_view::SettingsAction::CancelFps)));
}

#[test]
fn test_fps_setting_adjustment_and_clamping() {
    use omaflac::audio::visualizer::VisualizerBridge;

    let viz = VisualizerBridge::new(44100);
    // Test set_fps clamping
    viz.set_fps(5); // below min 10
    let (b1, _) = viz.get_bands();
    assert_eq!(b1.len(), 64);

    viz.set_fps(144); // higher refresh rate
    let (b2, _) = viz.get_bands();
    assert_eq!(b2.len(), 64);

    viz.set_fps(500); // above max 240
    let (b3, _) = viz.get_bands();
    assert_eq!(b3.len(), 64);
}





