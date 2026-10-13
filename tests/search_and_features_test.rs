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

