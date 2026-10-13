use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Track {
    pub id: String,
    pub path: PathBuf,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub album_artist: Option<String>,
    pub genre: Option<String>,
    pub year: Option<u32>,
    #[serde(default)]
    pub release_date: Option<String>,
    pub track_number: Option<u32>,
    pub disc_number: Option<u32>,
    pub duration_secs: u64,
    pub format: String,
    pub sample_rate: Option<u32>,
    pub bit_depth: Option<u8>,
    pub bitrate_kbps: Option<u32>,
    pub has_cover_art: bool,
}

impl Track {
    pub fn duration(&self) -> Duration {
        Duration::from_secs(self.duration_secs)
    }

    pub fn formatted_duration(&self) -> String {
        let total_secs = self.duration_secs;
        let mins = total_secs / 60;
        let secs = total_secs % 60;
        format!("{:02}:{:02}", mins, secs)
    }

    pub fn display_artist(&self) -> &str {
        if self.artist.is_empty() {
            "Unknown Artist"
        } else {
            &self.artist
        }
    }

    pub fn display_album(&self) -> &str {
        if self.album.is_empty() {
            "Unknown Album"
        } else {
            &self.album
        }
    }

    pub fn display_title(&self) -> &str {
        if self.title.is_empty() {
            self.path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Unknown Title")
        } else {
            &self.title
        }
    }

    pub fn quality_info(&self) -> String {
        let mut parts = Vec::new();
        parts.push(self.format.to_uppercase());
        if let Some(rate) = self.sample_rate {
            if rate >= 1000 {
                parts.push(format!("{:.1} kHz", rate as f32 / 1000.0));
            } else {
                parts.push(format!("{} Hz", rate));
            }
        }
        if let Some(depth) = self.bit_depth {
            parts.push(format!("{}-bit", depth));
        } else if let Some(br) = self.bitrate_kbps {
            parts.push(format!("{} kbps", br));
        }
        parts.join(" • ")
    }

    pub fn matches_query(&self, query: &str) -> bool {
        let q = query.to_lowercase();
        self.title.to_lowercase().contains(&q)
            || self.artist.to_lowercase().contains(&q)
            || self.album.to_lowercase().contains(&q)
            || self.genre.as_ref().map(|g| g.to_lowercase().contains(&q)).unwrap_or(false)
            || self.path.to_string_lossy().to_lowercase().contains(&q)
    }

    pub fn generate_id(path: &Path) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        path.to_string_lossy().hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    }
}
