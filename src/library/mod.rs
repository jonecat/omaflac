pub mod playlist;
pub mod scanner;
pub mod track;

use scanner::Scanner;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use track::Track;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Library {
    pub tracks: Vec<Track>,
    #[serde(skip)]
    pub scanned_at: Option<std::time::SystemTime>,
}

impl Library {
    pub fn library_path() -> PathBuf {
        let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        path.push("omaflac");
        std::fs::create_dir_all(&path).ok();
        path.push("library.json");
        path
    }

    pub fn strip_leading_the(name: &str) -> &str {
        let trimmed = name.trim();
        if trimmed.len() > 4 && trimmed.as_bytes()[..4].eq_ignore_ascii_case(b"the ") {
            trimmed[4..].trim_start()
        } else {
            trimmed
        }
    }

    pub fn compare_artist_names(a: &str, b: &str) -> std::cmp::Ordering {
        let key_a = Self::strip_leading_the(a);
        let key_b = Self::strip_leading_the(b);
        key_a
            .to_lowercase()
            .cmp(&key_b.to_lowercase())
            .then_with(|| a.to_lowercase().cmp(&b.to_lowercase()))
            .then_with(|| a.cmp(b))
    }

    pub fn load() -> Self {
        let path = Self::library_path();
        if let Ok(file) = std::fs::File::open(&path) {
            if let Ok(mut lib) = serde_json::from_reader::<_, Self>(file) {
                lib.sort_tracks();
                return lib;
            }
        }
        Self::default()
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::library_path();
        let file = std::fs::File::create(path)?;
        serde_json::to_writer_pretty(file, self)?;
        Ok(())
    }

    pub fn sort_tracks(&mut self) {
        self.tracks.sort_by(|a, b| {
            Self::compare_artist_names(a.display_artist(), b.display_artist())
                .then_with(|| a.album.to_lowercase().cmp(&b.album.to_lowercase()))
                .then_with(|| a.album.cmp(&b.album))
                .then_with(|| a.disc_number.cmp(&b.disc_number))
                .then_with(|| a.track_number.cmp(&b.track_number))
                .then_with(|| a.title.cmp(&b.title))
        });
    }

    /// Merges a batch of new or updated tracks into the library without wiping existing tracks.
    /// Returns `(added_count, updated_count)`.
    pub fn upsert_tracks(&mut self, batch: Vec<Track>) -> (usize, usize) {
        if batch.is_empty() {
            return (0, 0);
        }
        let mut added = 0usize;
        let mut updated = 0usize;
        for track in batch {
            if let Some(existing) = self.tracks.iter_mut().find(|t| t.path == track.path) {
                if *existing != track {
                    *existing = track;
                    updated += 1;
                }
            } else {
                self.tracks.push(track);
                added += 1;
            }
        }
        if added > 0 || updated > 0 {
            self.sort_tracks();
        }
        (added, updated)
    }

    /// Removes tracks matching any of `removed_paths`. Returns the number of tracks removed.
    pub fn remove_tracks_by_path(&mut self, removed_paths: &[PathBuf]) -> usize {
        if removed_paths.is_empty() {
            return 0;
        }
        let set: std::collections::HashSet<&PathBuf> = removed_paths.iter().collect();
        let before = self.tracks.len();
        self.tracks.retain(|t| !set.contains(&t.path));
        before.saturating_sub(self.tracks.len())
    }

    /// Updates `file_size` and `modified_secs` in-place on existing tracks.
    pub fn backfill_stats(&mut self, stats: &[(PathBuf, Option<u64>, Option<u64>)]) -> bool {
        if stats.is_empty() {
            return false;
        }
        let map: std::collections::HashMap<&PathBuf, (Option<u64>, Option<u64>)> = stats
            .iter()
            .map(|(p, sz, mt)| (p, (*sz, *mt)))
            .collect();
        let mut changed = false;
        for track in &mut self.tracks {
            if let Some(&(sz, mt)) = map.get(&track.path) {
                if track.file_size != sz || track.modified_secs != mt {
                    track.file_size = sz;
                    track.modified_secs = mt;
                    changed = true;
                }
            }
        }
        changed
    }

    pub fn scan(&mut self, directories: &[PathBuf]) {
        self.tracks = Scanner::scan_directories(directories);
        self.scanned_at = Some(std::time::SystemTime::now());
        self.save().ok();
    }

    pub fn get_track(&self, id: &str) -> Option<&Track> {
        self.tracks.iter().find(|t| t.id == id)
    }

    pub fn get_track_by_path(&self, path: &Path) -> Option<&Track> {
        self.tracks.iter().find(|t| t.path == path)
    }

    pub fn artists(&self) -> Vec<String> {
        let mut set = BTreeSet::new();
        for track in &self.tracks {
            set.insert(track.display_artist().to_string());
        }
        let mut list: Vec<String> = set.into_iter().collect();
        list.sort_by(|a, b| Self::compare_artist_names(a, b));
        list
    }

    pub fn albums_by_artist(&self, artist: &str) -> Vec<String> {
        let mut set = BTreeSet::new();
        for track in &self.tracks {
            if track.display_artist().eq_ignore_ascii_case(artist) {
                set.insert(track.display_album().to_string());
            }
        }
        set.into_iter().collect()
    }

    pub fn tracks_by_artist(&self, artist: &str) -> Vec<&Track> {
        self.tracks
            .iter()
            .filter(|t| t.display_artist().eq_ignore_ascii_case(artist))
            .collect()
    }

    pub fn tracks_by_artist_and_album(&self, artist: &str, album: &str) -> Vec<&Track> {
        self.tracks
            .iter()
            .filter(|t| {
                t.display_artist().eq_ignore_ascii_case(artist)
                    && t.display_album().eq_ignore_ascii_case(album)
            })
            .collect()
    }

    /// Determines the album artist for a track using explicit tags and smart heuristics.
    pub fn track_album_artist(&self, track: &Track) -> String {
        // 1. Direct explicit album_artist on this track
        if let Some(ref aa) = track.album_artist {
            let trimmed = aa.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }

        // 2. Check sibling tracks sharing the same parent directory (or matching artist)
        // that have an explicit album_artist for this album
        for sibling in &self.tracks {
            let same_dir = match (track.path.parent(), sibling.path.parent()) {
                (Some(p1), Some(p2)) => p1 == p2,
                _ => false,
            };
            let same_artist = sibling.display_artist().eq_ignore_ascii_case(track.display_artist());
            if (same_dir || same_artist) && sibling.display_album().eq_ignore_ascii_case(track.display_album()) {
                if let Some(ref aa) = sibling.album_artist {
                    let trimmed = aa.trim();
                    if !trimmed.is_empty() {
                        return trimmed.to_string();
                    }
                }
            }
        }

        // 3. Majority artist heuristic on tracks of the same album
        let album_tracks: Vec<&Track> = {
            let by_dir: Vec<&Track> = self
                .tracks
                .iter()
                .filter(|t| {
                    if !t.display_album().eq_ignore_ascii_case(track.display_album()) {
                        return false;
                    }
                    match (track.path.parent(), t.path.parent()) {
                        (Some(p1), Some(p2)) => p1 == p2 || p1.parent() == p2.parent(),
                        _ => false,
                    }
                })
                .collect();
            if !by_dir.is_empty() {
                by_dir
            } else {
                self.tracks
                    .iter()
                    .filter(|t| t.display_album().eq_ignore_ascii_case(track.display_album()))
                    .collect()
            }
        };

        if !album_tracks.is_empty() {
            let mut counts: HashMap<&str, usize> = HashMap::new();
            for t in &album_tracks {
                *counts.entry(t.display_artist()).or_insert(0) += 1;
            }
            if let Some((&majority_artist, &count)) = counts.iter().max_by_key(|(_, &c)| c) {
                if count * 2 >= album_tracks.len() {
                    return majority_artist.to_string();
                }
            }
            // If multiple diverse artists and at least 3 tracks, treat as "Various Artists"
            if counts.len() >= 3 {
                return "Various Artists".to_string();
            }
        }

        // 4. Fallback to track's own display_artist
        track.display_artist().to_string()
    }

    pub fn all_albums(&self) -> Vec<AlbumSummary> {
        let mut albums: Vec<AlbumSummary> = Vec::new();
        let mut key_index: HashMap<(String, String), usize> = HashMap::new();

        for track in &self.tracks {
            let album_artist = self.track_album_artist(track);
            let album_name = track.display_album();
            let key = (album_artist.to_lowercase(), album_name.to_lowercase());

            if let Some(&idx) = key_index.get(&key) {
                let existing = &mut albums[idx];
                existing.track_count += 1;
                existing.total_duration_secs += track.duration_secs;
                if existing.year.is_none() && track.year.is_some() {
                    existing.year = track.year;
                }
                if track.has_cover_art {
                    existing.has_cover_art = true;
                }
            } else {
                let idx = albums.len();
                key_index.insert(key, idx);
                albums.push(AlbumSummary {
                    artist: album_artist,
                    album: album_name.to_string(),
                    track_count: 1,
                    total_duration_secs: track.duration_secs,
                    year: track.year,
                    has_cover_art: track.has_cover_art,
                });
            }
        }

        albums.sort_by(|a, b| {
            Self::compare_artist_names(&a.artist, &b.artist)
                .then_with(|| a.album.to_lowercase().cmp(&b.album.to_lowercase()))
                .then_with(|| a.album.cmp(&b.album))
        });

        albums
    }

    pub fn tracks_by_album(&self, album_artist: &str, album: &str) -> Vec<&Track> {
        let mut tracks: Vec<&Track> = self
            .tracks
            .iter()
            .filter(|t| {
                if !t.display_album().eq_ignore_ascii_case(album) {
                    return false;
                }
                let resolved_aa = self.track_album_artist(t);
                resolved_aa.eq_ignore_ascii_case(album_artist)
                    || t.display_artist().eq_ignore_ascii_case(album_artist)
                    || t.album_artist.as_deref().map(|aa| aa.trim().eq_ignore_ascii_case(album_artist)).unwrap_or(false)
            })
            .collect();

        tracks.sort_by(|a, b| {
            a.disc_number
                .unwrap_or(1)
                .cmp(&b.disc_number.unwrap_or(1))
                .then_with(|| {
                    a.track_number
                        .unwrap_or(u32::MAX)
                        .cmp(&b.track_number.unwrap_or(u32::MAX))
                })
                .then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
                .then_with(|| a.title.cmp(&b.title))
        });

        tracks
    }

    pub fn search(&self, query: &str) -> Vec<&Track> {
        if query.trim().is_empty() {
            return self.tracks.iter().collect();
        }
        self.tracks.iter().filter(|t| t.matches_query(query)).collect()
    }

    pub fn search_artists(&self, query: &str) -> Vec<ArtistSummary> {
        let q = query.trim().to_lowercase();
        let mut results = Vec::new();
        for artist in self.artists() {
            if q.is_empty() || artist.to_lowercase().contains(&q) {
                let albums = self.albums_by_artist(&artist);
                let tracks = self.tracks_by_artist(&artist);
                results.push(ArtistSummary {
                    name: artist,
                    album_count: albums.len(),
                    track_count: tracks.len(),
                });
            }
        }
        results
    }

    pub fn search_albums(&self, query: &str) -> Vec<AlbumSummary> {
        let q = query.trim().to_lowercase();
        self.all_albums()
            .into_iter()
            .filter(|a| {
                q.is_empty()
                    || a.album.to_lowercase().contains(&q)
                    || a.artist.to_lowercase().contains(&q)
            })
            .collect()
    }

    pub fn search_tracks(&self, query: &str) -> Vec<&Track> {
        self.search(query)
    }

    pub fn tracks_for_playlist<'a>(
        &'a self,
        track_ids: &[String],
    ) -> Vec<&'a Track> {
        track_ids
            .iter()
            .filter_map(|id| self.get_track(id))
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtistSummary {
    pub name: String,
    pub album_count: usize,
    pub track_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlbumSummary {
    pub artist: String,
    pub album: String,
    pub track_count: usize,
    pub total_duration_secs: u64,
    pub year: Option<u32>,
    pub has_cover_art: bool,
}

impl AlbumSummary {
    pub fn formatted_duration(&self) -> String {
        let mins = self.total_duration_secs / 60;
        let secs = self.total_duration_secs % 60;
        format!("{:02}:{:02}", mins, secs)
    }
}
