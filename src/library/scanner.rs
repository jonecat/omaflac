use crate::library::track::Track;
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::probe::Probe;
use lofty::tag::Accessor;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub const SUPPORTED_EXTENSIONS: &[&str] = &[
    "flac", "mp3", "ogg", "oga", "wav", "wave", "m4a", "aac", "opus", "alac", "aiff", "aif",
];

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

    pub fn scan_directories<P: AsRef<Path>>(directories: &[P]) -> Vec<Track> {
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
                        tracks.push(track);
                    }
                }
            }
        }

        // Sort by Album, Disc, Track number, Title
        tracks.sort_by(|a, b| {
            a.artist
                .cmp(&b.artist)
                .then_with(|| a.album.cmp(&b.album))
                .then_with(|| a.disc_number.cmp(&b.disc_number))
                .then_with(|| a.track_number.cmp(&b.track_number))
                .then_with(|| a.title.cmp(&b.title))
        });

        tracks
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
            let year = tag.and_then(|t| t.year());
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
                track_number,
                disc_number,
                duration_secs,
                format: ext.to_uppercase(),
                sample_rate,
                bit_depth,
                bitrate_kbps,
                has_cover_art,
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
            track_number: None,
            disc_number: None,
            duration_secs: 0,
            format: ext.to_uppercase(),
            sample_rate: None,
            bit_depth: None,
            bitrate_kbps: None,
            has_cover_art: folder_has_cover,
        })
    }
}
