pub mod playlist;
pub mod scanner;
pub mod track;

use scanner::Scanner;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
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

    pub fn load() -> Self {
        let path = Self::library_path();
        if let Ok(file) = std::fs::File::open(&path) {
            if let Ok(lib) = serde_json::from_reader::<_, Self>(file) {
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
            a.artist
                .cmp(&b.artist)
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
        set.into_iter().collect()
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

    pub fn all_albums(&self) -> Vec<AlbumSummary> {
        let mut map: BTreeMap<(String, String), (usize, u64, Option<u32>, bool)> = BTreeMap::new();
        for track in &self.tracks {
            let key = (track.display_artist().to_string(), track.display_album().to_string());
            let entry = map.entry(key).or_insert((0, 0, track.year, track.has_cover_art));
            entry.0 += 1;
            entry.1 += track.duration_secs;
            if entry.2.is_none() && track.year.is_some() {
                entry.2 = track.year;
            }
            if track.has_cover_art {
                entry.3 = true;
            }
        }

        map.into_iter()
            .map(|((artist, album), (track_count, total_duration_secs, year, has_cover_art))| {
                AlbumSummary {
                    artist,
                    album,
                    track_count,
                    total_duration_secs,
                    year,
                    has_cover_art,
                }
            })
            .collect()
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
