use omaflac::library::scanner::Scanner;
use omaflac::library::Library;
use omaflac::library::playlist::PlaylistsManager;
use omaflac::audio::waveform::WaveformData;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

#[test]
fn test_library_scan_and_hierarchy() {
    let music_dir = PathBuf::from("/home/jon/Music");
    if !music_dir.exists() {
        return;
    }

    let tracks = Scanner::scan_directories(&[music_dir.clone()]);
    assert!(!tracks.is_empty(), "Should discover audio tracks in ~/Music");

    let mut library = Library::default();
    library.tracks = tracks;

    let artists = library.artists();
    assert!(!artists.is_empty(), "Should extract artist names");
    println!("Artists discovered: {:?}", artists);

    // Verify hierarchical structure: Artist -> Albums -> Songs
    for artist in &artists {
        let albums = library.albums_by_artist(artist);
        assert!(!albums.is_empty(), "Artist {} should have albums", artist);
        for album in &albums {
            let songs = library.tracks_by_artist_and_album(artist, album);
            assert!(!songs.is_empty(), "Album {} should have songs", album);
            for song in &songs {
                assert_eq!(song.display_artist(), artist.as_str());
                assert_eq!(song.display_album(), album.as_str());
            }
        }
    }

    // Verify search
    let matches = library.search("Pink Floyd");
    assert!(!matches.is_empty(), "Search for Pink Floyd should return tracks");

    // Verify playlist operations
    let mut pm = PlaylistsManager::default();
    let p_id = pm.create_playlist("Classic Rock Hits");
    let first_track = &library.tracks[0];
    pm.add_track(&p_id, &first_track.id);
    let playlist = pm.get_playlist(&p_id).expect("Playlist should exist");
    assert_eq!(playlist.track_ids.len(), 1);
    assert_eq!(playlist.track_ids[0], first_track.id);

    // Verify waveform computation on first track
    let cancel = Arc::new(AtomicBool::new(false));
    let waveform_bins = WaveformData::compute_accurate(&first_track.path, 100, cancel);
    assert!(waveform_bins.is_some(), "Waveform extraction should succeed");
    let peaks = waveform_bins.unwrap();
    assert_eq!(peaks.len(), 100);
    println!("Successfully computed waveform with {} bins for {}", peaks.len(), first_track.display_title());
}
