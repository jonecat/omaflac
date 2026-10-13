use omaflac::library::track::Track;
use omaflac::lyrics::{FetchResult, LyricsClient, LyricsManager, LyricsStatus, SyncedLyrics};
use std::fs::File;
use std::io::Write;
use std::time::Duration;

#[test]
fn test_lrc_parser_timestamps_and_offsets() {
    let raw = r#"
[ti:Echoes]
[ar:Pink Floyd]
[al:Meddle]
[offset:+500]
[00:05.50]Overhead the albatross
[00:10.00][00:15.00]Hangs motionless upon the air
[00:20.123]And deep beneath the rolling waves
"#;

    let synced = SyncedLyrics::from_lrc(raw).expect("LRC should parse");
    assert_eq!(synced.lines.len(), 4);

    // [00:05.50] = 5500ms + offset 500ms = 6000ms
    assert_eq!(synced.lines[0].timestamp, Duration::from_millis(6000));
    assert_eq!(synced.lines[0].text, "Overhead the albatross");

    // [00:10.00] = 10000ms + 500ms = 10500ms
    assert_eq!(synced.lines[1].timestamp, Duration::from_millis(10500));
    assert_eq!(synced.lines[1].text, "Hangs motionless upon the air");

    // [00:15.00] = 15000ms + 500ms = 15500ms
    assert_eq!(synced.lines[2].timestamp, Duration::from_millis(15500));
    assert_eq!(synced.lines[2].text, "Hangs motionless upon the air");

    // [00:20.123] = 20123ms + 500ms = 20623ms
    assert_eq!(synced.lines[3].timestamp, Duration::from_millis(20623));
    assert_eq!(synced.lines[3].text, "And deep beneath the rolling waves");
}

#[test]
fn test_lrc_window_progression_and_gaps() {
    let raw = r#"
[00:10.00]First line
[00:20.00]Second line
[00:50.00]Third line after long guitar solo
"#;
    let synced = SyncedLyrics::from_lrc(raw).unwrap();

    // 1. Before song lyrics start (intro)
    let w0 = synced.get_window(Duration::from_secs(4));
    assert!(w0.prev.is_none());
    assert!(w0.curr.is_none());
    assert_eq!(w0.next.unwrap().text, "First line");
    assert!(w0.is_instrumental_gap);

    // 2. Exactly at line 1
    let w1 = synced.get_window(Duration::from_secs(12));
    assert!(w1.prev.is_none());
    assert_eq!(w1.curr.unwrap().text, "First line");
    assert_eq!(w1.next.unwrap().text, "Second line");
    assert!(!w1.is_instrumental_gap);

    // 3. During line 2
    let w2 = synced.get_window(Duration::from_secs(22));
    assert_eq!(w2.prev.unwrap().text, "First line");
    assert_eq!(w2.curr.unwrap().text, "Second line");
    assert_eq!(w2.next.unwrap().text, "Third line after long guitar solo");
    assert!(!w2.is_instrumental_gap);

    // 4. During long guitar solo (at 38s: 18s after line 2, 12s before line 3)
    let w3 = synced.get_window(Duration::from_secs(38));
    assert_eq!(w3.curr.unwrap().text, "Second line");
    assert!(w3.is_instrumental_gap);

    // 5. At line 3
    let w4 = synced.get_window(Duration::from_secs(51));
    assert_eq!(w4.prev.unwrap().text, "Second line");
    assert_eq!(w4.curr.unwrap().text, "Third line after long guitar solo");
    assert!(w4.next.is_none());
    assert!(!w4.is_instrumental_gap);
}

#[test]
fn test_lyrics_cache_and_negative_markers() {
    let artist = "TestArtistX";
    let title = "TestTitleY";

    // 1. Write found lyrics
    let sample = "[00:01.00] Hello world\n";
    LyricsClient::write_cache(artist, title, &FetchResult::Found(sample.to_string()));
    assert_eq!(
        LyricsClient::read_cache(artist, title),
        Some(FetchResult::Found(sample.to_string()))
    );

    // 2. Write instrumental marker
    LyricsClient::write_cache(artist, title, &FetchResult::Instrumental);
    assert_eq!(
        LyricsClient::read_cache(artist, title),
        Some(FetchResult::Instrumental)
    );

    // 3. Write not found marker
    LyricsClient::write_cache(artist, title, &FetchResult::NotFound);
    assert_eq!(
        LyricsClient::read_cache(artist, title),
        Some(FetchResult::NotFound)
    );

    // Clean up test file
    let path = LyricsClient::cache_path(artist, title);
    let _ = std::fs::remove_file(path);
}

#[test]
fn test_local_lrc_file_discovery() {
    let temp_dir = std::env::temp_dir().join("omaflac_lyrics_test");
    let _ = std::fs::create_dir_all(&temp_dir);

    let audio_file = temp_dir.join("track_test.flac");
    let lrc_file = temp_dir.join("track_test.lrc");

    let lrc_content = "[00:02.00] Local file lyrics test\n";
    {
        let mut f = File::create(&lrc_file).unwrap();
        f.write_all(lrc_content.as_bytes()).unwrap();
    }

    let result = LyricsClient::check_local_file(&audio_file);
    assert_eq!(result, Some(FetchResult::Found(lrc_content.to_string())));

    // Clean up
    let _ = std::fs::remove_file(lrc_file);
    let _ = std::fs::remove_dir(temp_dir);
}

#[test]
fn test_lyrics_manager_local_instant_load() {
    let temp_dir = std::env::temp_dir().join("omaflac_manager_test");
    let _ = std::fs::create_dir_all(&temp_dir);

    let audio_file = temp_dir.join("instant.flac");
    let lrc_file = temp_dir.join("instant.lrc");

    let lrc_content = "[00:05.00] Instant local lyrics\n";
    {
        let mut f = File::create(&lrc_file).unwrap();
        f.write_all(lrc_content.as_bytes()).unwrap();
    }

    let track = Track {
        id: "instant-1".into(),
        path: audio_file,
        title: "Instant Song".into(),
        artist: "Instant Artist".into(),
        album: "Instant Album".into(),
        album_artist: None,
        genre: None,
        year: Some(2025),
        release_date: None,
        track_number: Some(1),
        disc_number: Some(1),
        duration_secs: 180,
        format: "FLAC".into(),
        sample_rate: Some(44100),
        bit_depth: Some(16),
        bitrate_kbps: Some(900),
        has_cover_art: false,
        file_size: None,
        modified_secs: None,
    };

    let mut manager = LyricsManager::new();
    manager.request_lyrics(&track);

    match manager.status() {
        LyricsStatus::Found(synced) => {
            assert_eq!(synced.lines.len(), 1);
            assert_eq!(synced.lines[0].text, "Instant local lyrics");
        }
        other => panic!("Expected LyricsStatus::Found, got {:?}", other),
    }

    // Clean up
    let _ = std::fs::remove_file(lrc_file);
    let _ = std::fs::remove_dir(temp_dir);
}
