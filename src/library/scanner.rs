use crate::library::track::Track;
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::probe::Probe;
use lofty::tag::Accessor;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub const SUPPORTED_EXTENSIONS: &[&str] = &[
    "flac", "mp3", "ogg", "oga", "wav", "wave", "m4a", "aac", "opus", "alac", "aiff", "aif",
];

#[derive(Debug, Clone)]
pub enum ScanUpdate {
    Progress {
        current_file: String,
        found_count: usize,
    },
    /// Incremental batch of newly discovered or updated tracks
    UpsertBatch {
        tracks: Vec<Track>,
    },
    /// Backfills file_size and modified_secs on existing cached tracks without re-reading the file
    BackfillStat {
        stats: Vec<(PathBuf, Option<u64>, Option<u64>)>,
    },
    /// Tracks whose paths were confirmed deleted from an online library folder
    RemoveTracks {
        removed_paths: Vec<PathBuf>,
    },
    /// Full replacement completion (used by `scan_directories`)
    Completed {
        tracks: Vec<Track>,
    },
    /// Incremental sync completion summary
    SyncCompleted {
        checked_count: usize,
        added_count: usize,
        updated_count: usize,
        removed_count: usize,
        offline_folders: usize,
    },
}

pub struct Scanner;

impl Scanner {
    pub fn is_audio_file(path: &Path) -> bool {
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| {
                let lower = ext.to_lowercase();
                SUPPORTED_EXTENSIONS.contains(&lower.as_str())
            })
            .unwrap_or(false)
    }

    pub fn extract_stat_from_meta(meta: &std::fs::Metadata) -> (Option<u64>, Option<u64>) {
        let size = Some(meta.len());
        let mtime = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs());
        (size, mtime)
    }

    pub fn extract_file_stat(path: &Path) -> (Option<u64>, Option<u64>) {
        std::fs::metadata(path)
            .ok()
            .map(|m| Self::extract_stat_from_meta(&m))
            .unwrap_or((None, None))
    }

    pub fn scan_directories<P: AsRef<Path>>(directories: &[P]) -> Vec<Track> {
        let (tx, rx) = crossbeam_channel::unbounded();
        Self::scan_directories_with_sender(directories, tx);
        while let Ok(update) = rx.recv() {
            if let ScanUpdate::Completed { tracks } = update {
                return tracks;
            }
        }
        Vec::new()
    }

    pub fn scan_directories_with_sender<P: AsRef<Path>>(
        directories: &[P],
        sender: crossbeam_channel::Sender<ScanUpdate>,
    ) {
        let mut tracks = Vec::new();
        for dir in directories {
            let dir_path = dir.as_ref();
            if !dir_path.exists() {
                continue;
            }
            for entry in WalkDir::new(dir_path)
                .follow_links(true)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                let path = entry.path();
                if path.is_file() && Self::is_audio_file(path) {
                    if let Some(track) = Self::read_track(path) {
                        let file_name = path
                            .file_name()
                            .map(|s| s.to_string_lossy().to_string())
                            .unwrap_or_default();
                        tracks.push(track);
                        if tracks.len() == 1 || tracks.len() % 5 == 0 {
                            let _ = sender.send(ScanUpdate::Progress {
                                current_file: file_name,
                                found_count: tracks.len(),
                            });
                        }
                    }
                }
            }
        }

        // Sort by Artist (ignoring leading 'The '), Album, Disc, Track number, Title
        tracks.sort_by(|a, b| {
            crate::library::Library::compare_artist_names(a.display_artist(), b.display_artist())
                .then_with(|| a.album.to_lowercase().cmp(&b.album.to_lowercase()))
                .then_with(|| a.album.cmp(&b.album))
                .then_with(|| a.disc_number.cmp(&b.disc_number))
                .then_with(|| a.track_number.cmp(&b.track_number))
                .then_with(|| a.title.cmp(&b.title))
        });

        let _ = sender.send(ScanUpdate::Completed { tracks });
    }

    /// Incremental background library sync.
    /// - Preserves existing cached tracks immediately.
    /// - Uses `stat` (`file_size` + `modified_secs`) to skip opening unchanged files over the network.
    /// - Streams newly discovered or modified tracks in small batches via `ScanUpdate::UpsertBatch`.
    /// - Protects against unmounted/offline network shares by refusing to prune a folder if it is
    ///   missing or yields 0 audio files when it previously had tracks.
    pub fn sync_directories_with_sender(
        directories: &[PathBuf],
        existing_tracks: Vec<Track>,
        force_full_rescan: bool,
        sender: crossbeam_channel::Sender<ScanUpdate>,
    ) {
        use std::collections::{HashMap, HashSet};
        use std::time::{Duration, Instant};

        let mut removed_count = 0usize;

        // 1. Prune tracks belonging to folders that were explicitly removed from config.library_folders
        let mut orphaned_paths = Vec::new();
        let mut active_existing = Vec::with_capacity(existing_tracks.len());
        for track in existing_tracks {
            if directories.iter().any(|dir| track.path.starts_with(dir)) {
                active_existing.push(track);
            } else {
                orphaned_paths.push(track.path);
            }
        }
        if !orphaned_paths.is_empty() {
            removed_count += orphaned_paths.len();
            let _ = sender.send(ScanUpdate::RemoveTracks {
                removed_paths: orphaned_paths,
            });
        }

        let existing_map: HashMap<PathBuf, Track> = active_existing
            .into_iter()
            .map(|t| (t.path.clone(), t))
            .collect();

        let mut checked_count = 0usize;
        let mut added_count = 0usize;
        let mut updated_count = 0usize;
        let mut offline_folders = 0usize;

        let mut upsert_batch: Vec<Track> = Vec::new();
        let mut backfill_stats: Vec<(PathBuf, Option<u64>, Option<u64>)> = Vec::new();
        let mut last_flush = Instant::now();

        for dir in directories {
            let existing_in_dir: Vec<PathBuf> = existing_map
                .keys()
                .filter(|p| p.starts_with(dir))
                .cloned()
                .collect();

            if !dir.exists() {
                if !existing_in_dir.is_empty() {
                    offline_folders += 1;
                }
                continue;
            }

            let mut seen_in_dir: HashSet<PathBuf> = HashSet::new();

            for entry in WalkDir::new(dir)
                .follow_links(true)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                let path = entry.path();
                if !path.is_file() || !Self::is_audio_file(path) {
                    continue;
                }

                let path_buf = path.to_path_buf();
                seen_in_dir.insert(path_buf.clone());
                checked_count += 1;

                let (cur_size, cur_mtime) = entry
                    .metadata()
                    .ok()
                    .map(|m| Self::extract_stat_from_meta(&m))
                    .unwrap_or_else(|| Self::extract_file_stat(path));

                let file_name = path
                    .file_name()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default();

                let mut needs_read = true;
                let mut is_existing = false;

                if let Some(existing) = existing_map.get(path) {
                    is_existing = true;
                    if !force_full_rescan && existing.release_date.is_some() {
                        if existing.file_size.is_some()
                            && existing.modified_secs.is_some()
                            && existing.file_size == cur_size
                            && existing.modified_secs == cur_mtime
                        {
                            // Unchanged file: skip opening over the network
                            needs_read = false;
                        } else if existing.file_size.is_none() || existing.modified_secs.is_none() {
                            // First sync after upgrade: backfill stat metadata without opening the file
                            needs_read = false;
                            if cur_size.is_some() || cur_mtime.is_some() {
                                backfill_stats.push((path_buf.clone(), cur_size, cur_mtime));
                            }
                        }
                    }
                }

                if needs_read {
                    if let Some(mut track) = Self::read_track(path) {
                        if cur_size.is_some() {
                            track.file_size = cur_size;
                        }
                        if cur_mtime.is_some() {
                            track.modified_secs = cur_mtime;
                        }
                        if is_existing {
                            if existing_map.get(path) != Some(&track) {
                                updated_count += 1;
                                upsert_batch.push(track);
                            } else if cur_size.is_some() || cur_mtime.is_some() {
                                backfill_stats.push((path_buf, cur_size, cur_mtime));
                            }
                        } else {
                            added_count += 1;
                            upsert_batch.push(track);
                        }
                    }
                }

                if upsert_batch.len() >= 8
                    || (!upsert_batch.is_empty()
                        && last_flush.elapsed() >= Duration::from_millis(250))
                {
                    let _ = sender.send(ScanUpdate::UpsertBatch {
                        tracks: std::mem::take(&mut upsert_batch),
                    });
                    last_flush = Instant::now();
                }

                if backfill_stats.len() >= 50 {
                    let _ = sender.send(ScanUpdate::BackfillStat {
                        stats: std::mem::take(&mut backfill_stats),
                    });
                }

                if checked_count == 1 || checked_count % 10 == 0 {
                    let _ = sender.send(ScanUpdate::Progress {
                        current_file: file_name,
                        found_count: checked_count,
                    });
                }
            }

            if !upsert_batch.is_empty() {
                let _ = sender.send(ScanUpdate::UpsertBatch {
                    tracks: std::mem::take(&mut upsert_batch),
                });
                last_flush = Instant::now();
            }

            if !backfill_stats.is_empty() {
                let _ = sender.send(ScanUpdate::BackfillStat {
                    stats: std::mem::take(&mut backfill_stats),
                });
            }

            // Offline / unmounted remote folder protection:
            // If `dir` previously had tracks but yielded 0 audio files now, treat it as unmounted/offline.
            if seen_in_dir.is_empty() && !existing_in_dir.is_empty() {
                offline_folders += 1;
            } else {
                let removed_in_dir: Vec<PathBuf> = existing_in_dir
                    .into_iter()
                    .filter(|p| !seen_in_dir.contains(p))
                    .collect();
                if !removed_in_dir.is_empty() {
                    removed_count += removed_in_dir.len();
                    let _ = sender.send(ScanUpdate::RemoveTracks {
                        removed_paths: removed_in_dir,
                    });
                }
            }
        }

        let _ = sender.send(ScanUpdate::SyncCompleted {
            checked_count,
            added_count,
            updated_count,
            removed_count,
            offline_folders,
        });
    }

    pub fn has_folder_cover(dir: &Path) -> bool {
        let common_names = [
            "cover.jpg",
            "cover.png",
            "cover.jpeg",
            "folder.jpg",
            "folder.png",
            "front.jpg",
            "front.png",
            "album.jpg",
            "album.png",
            "albumart.jpg",
        ];
        for name in &common_names {
            if dir.join(name).exists() {
                return true;
            }
        }
        false
    }

    pub fn find_folder_cover(dir: &Path) -> Option<PathBuf> {
        let common_names = [
            "cover.jpg",
            "cover.png",
            "cover.jpeg",
            "folder.jpg",
            "folder.png",
            "front.jpg",
            "front.png",
            "album.jpg",
            "album.png",
            "albumart.jpg",
        ];
        for name in &common_names {
            let path = dir.join(name);
            if path.exists() {
                return Some(path);
            }
        }
        None
    }

    pub fn read_track(path: &Path) -> Option<Track> {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        let id = Track::generate_id(path);
        let parent_dir = path.parent();
        let folder_has_cover = parent_dir.map(Self::has_folder_cover).unwrap_or(false);
        let (file_size, modified_secs) = Self::extract_file_stat(path);

        // Try reading with Lofty
        if let Ok(tagged_file) = Probe::open(path).and_then(|p| p.read()) {
            let tag = tagged_file.primary_tag().or_else(|| tagged_file.first_tag());
            let properties = tagged_file.properties();

            let title = tag
                .and_then(|t| t.title().map(|s| s.to_string()))
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| {
                    path.file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("Unknown")
                        .to_string()
                });

            let artist = tag
                .and_then(|t| t.artist().map(|s| s.to_string()))
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| {
                    // Try getting from directory structure Artist/Album/Song
                    parent_dir
                        .and_then(|p| p.parent())
                        .and_then(|p| p.file_name())
                        .and_then(|s| s.to_str())
                        .unwrap_or("Unknown Artist")
                        .to_string()
                });

            let album = tag
                .and_then(|t| t.album().map(|s| s.to_string()))
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| {
                    parent_dir
                        .and_then(|p| p.file_name())
                        .and_then(|s| s.to_str())
                        .unwrap_or("Unknown Album")
                        .to_string()
                });

            let album_artist = tag.and_then(|t| t.get_string(&lofty::tag::ItemKey::AlbumArtist).map(|s| s.to_string()));
            let genre = tag.and_then(|t| t.genre().map(|s| s.to_string()));
            let release_date = tag.and_then(Self::extract_original_release_date);
            let year = release_date
                .as_deref()
                .and_then(Self::parse_year_prefix)
                .or_else(|| tag.and_then(|t| t.year()));
            let track_number = tag.and_then(|t| t.track());
            let disc_number = tag.and_then(|t| t.disk());

            let has_embedded_cover = tag.map(|t| !t.pictures().is_empty()).unwrap_or(false);
            let has_cover_art = has_embedded_cover || folder_has_cover;

            let duration_secs = properties.duration().as_secs();
            let sample_rate = properties.sample_rate();
            let bit_depth = properties.bit_depth();
            let bitrate_kbps = properties.audio_bitrate();

            return Some(Track {
                id,
                path: path.to_path_buf(),
                title,
                artist,
                album,
                album_artist,
                genre,
                year,
                release_date,
                track_number,
                disc_number,
                duration_secs,
                format: ext.to_uppercase(),
                sample_rate,
                bit_depth,
                bitrate_kbps,
                has_cover_art,
                file_size,
                modified_secs,
            });
        }

        // Fallback if metadata cannot be parsed by lofty
        Some(Track {
            id,
            path: path.to_path_buf(),
            title: path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Unknown")
                .to_string(),
            artist: parent_dir
                .and_then(|p| p.parent())
                .and_then(|p| p.file_name())
                .and_then(|s| s.to_str())
                .unwrap_or("Unknown Artist")
                .to_string(),
            album: parent_dir
                .and_then(|p| p.file_name())
                .and_then(|s| s.to_str())
                .unwrap_or("Unknown Album")
                .to_string(),
            album_artist: None,
            genre: None,
            year: None,
            release_date: None,
            track_number: None,
            disc_number: None,
            duration_secs: 0,
            format: ext.to_uppercase(),
            sample_rate: None,
            bit_depth: None,
            bitrate_kbps: None,
            has_cover_art: folder_has_cover,
            file_size,
            modified_secs,
        })
    }

    fn parse_year_prefix(date_str: &str) -> Option<u32> {
        let trimmed = date_str.trim();
        if trimmed.len() >= 4 {
            trimmed[..4].parse::<u32>().ok()
        } else {
            None
        }
    }

    fn extract_original_release_date(tag: &lofty::tag::Tag) -> Option<String> {
        use lofty::tag::ItemKey;

        // 1. Explicit Original Release Date tags (ORIGINALDATE, ORIGINALYEAR, TDOR, TORY)
        if let Some(orig) = tag
            .get_string(&ItemKey::OriginalReleaseDate)
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
        {
            return Some(orig.to_string());
        }

        for custom_key in ["ORIGINALDATE", "ORIGINALYEAR", "ORIGINAL_DATE", "ORIGYEAR"] {
            if let Some(orig) = tag
                .get_string(&ItemKey::Unknown(custom_key.to_string()))
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
            {
                return Some(orig.to_string());
            }
        }

        // 2. MusicBrainz Release Group ID -> first-release-date (original album release date)
        let rg_id = tag
            .get_string(&ItemKey::MusicBrainzReleaseGroupId)
            .or_else(|| tag.get_string(&ItemKey::Unknown("MUSICBRAINZ_RELEASEGROUPID".to_string())))
            .map(|s| s.trim())
            .filter(|s| !s.is_empty());

        if let Some(rg_id) = rg_id {
            if let Some(first_date) = Self::resolve_musicbrainz_release_group_date(rg_id) {
                return Some(first_date);
            }
        }

        // 3. Fallback to standard ReleaseDate / RecordingDate (DATE) / Year
        if let Some(rel) = tag
            .get_string(&ItemKey::ReleaseDate)
            .or_else(|| tag.get_string(&ItemKey::RecordingDate))
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
        {
            return Some(rel.to_string());
        }

        tag.year().map(|y| y.to_string())
    }

    fn release_dates_cache_path() -> PathBuf {
        let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        path.push("omaflac");
        std::fs::create_dir_all(&path).ok();
        path.push("release_dates.json");
        path
    }

    fn resolve_musicbrainz_release_group_date(rg_id: &str) -> Option<String> {
        use std::collections::BTreeMap;
        use std::sync::{Mutex, OnceLock};

        static CACHE: OnceLock<Mutex<BTreeMap<String, String>>> = OnceLock::new();
        let mutex = CACHE.get_or_init(|| {
            let mut map = BTreeMap::new();
            // Seed known MusicBrainz Release Group original release dates
            map.insert(
                "416bb5e5-c7d1-3977-8fd7-7c9daf6c2be6".to_string(),
                "1977-02-04".to_string(),
            );
            map.insert(
                "89e0c763-1b48-35c7-a7a6-1f4c1b7fbf37".to_string(),
                "1990-10-08".to_string(),
            );
            map.insert(
                "db5cf893-a9c0-35e0-86ab-433f630273b7".to_string(),
                "1991-03-04".to_string(),
            );
            map.insert(
                "f5093c06-23e3-404f-aeaa-40f72885ee3a".to_string(),
                "1973-03-24".to_string(),
            );
            map.insert(
                "1a272023-10d3-38ee-bab3-317b55fcc21d".to_string(),
                "1975-09-12".to_string(),
            );
            map.insert(
                "9f7a4c28-8fa2-3113-929c-c47a9f7982c3".to_string(),
                "1967-05-26".to_string(),
            );

            let cache_path = Self::release_dates_cache_path();
            if let Ok(file) = std::fs::File::open(&cache_path) {
                if let Ok(disk_map) = serde_json::from_reader::<_, BTreeMap<String, String>>(file) {
                    map.extend(disk_map);
                }
            }
            Mutex::new(map)
        });

        if let Ok(guard) = mutex.lock() {
            if let Some(cached) = guard.get(rg_id) {
                return if cached.is_empty() {
                    None
                } else {
                    Some(cached.clone())
                };
            }
        }

        // Fetch from MusicBrainz API silently via curl if not cached
        let url = format!("https://musicbrainz.org/ws/2/release-group/{}?fmt=json", rg_id);
        let fetched = std::process::Command::new("curl")
            .args([
                "-s",
                "--max-time",
                "3",
                "-A",
                "OmaFLAC/0.1.0 ( https://github.com/jonecat/omaflac )",
                &url,
            ])
            .output()
            .ok()
            .and_then(|out| {
                if out.status.success() {
                    serde_json::from_slice::<serde_json::Value>(&out.stdout).ok()
                } else {
                    None
                }
            })
            .and_then(|json| {
                json.get("first-release-date")
                    .and_then(|v| v.as_str())
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
            });

        if let Ok(mut guard) = mutex.lock() {
            guard.insert(rg_id.to_string(), fetched.clone().unwrap_or_default());
            if let Ok(file) = std::fs::File::create(Self::release_dates_cache_path()) {
                let _ = serde_json::to_writer_pretty(file, &*guard);
            }
        }

        fetched
    }
}
