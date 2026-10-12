use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Playlist {
    pub id: String,
    pub name: String,
    pub track_ids: Vec<String>,
}

impl Playlist {
    pub fn new(name: impl Into<String>) -> Self {
        let name = name.into();
        let id = format!("{}_{}", name.to_lowercase().replace(' ', "_"), std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs());
        Self {
            id,
            name,
            track_ids: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PlaylistsManager {
    pub playlists: Vec<Playlist>,
    pub favorites: Vec<String>,
    pub recently_played: Vec<String>,
}

impl PlaylistsManager {
    pub fn playlists_path() -> PathBuf {
        let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        path.push("omaflac");
        std::fs::create_dir_all(&path).ok();
        path.push("playlists.json");
        path
    }

    pub fn load() -> Self {
        let path = Self::playlists_path();
        if let Ok(file) = std::fs::File::open(&path) {
            if let Ok(manager) = serde_json::from_reader(file) {
                return manager;
            }
        }
        let manager = Self::default();
        manager.save().ok();
        manager
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::playlists_path();
        let file = std::fs::File::create(path)?;
        serde_json::to_writer_pretty(file, self)?;
        Ok(())
    }

    pub fn create_playlist(&mut self, name: &str) -> String {
        let playlist = Playlist::new(name);
        let id = playlist.id.clone();
        self.playlists.push(playlist);
        self.save().ok();
        id
    }

    pub fn delete_playlist(&mut self, id: &str) {
        self.playlists.retain(|p| p.id != id);
        self.save().ok();
    }

    pub fn rename_playlist(&mut self, id: &str, new_name: &str) {
        if let Some(p) = self.playlists.iter_mut().find(|p| p.id == id) {
            p.name = new_name.to_string();
            self.save().ok();
        }
    }

    pub fn add_track(&mut self, playlist_id: &str, track_id: &str) {
        if let Some(p) = self.playlists.iter_mut().find(|p| p.id == playlist_id) {
            if !p.track_ids.contains(&track_id.to_string()) {
                p.track_ids.push(track_id.to_string());
                self.save().ok();
            }
        }
    }

    pub fn remove_track(&mut self, playlist_id: &str, index: usize) {
        if let Some(p) = self.playlists.iter_mut().find(|p| p.id == playlist_id) {
            if index < p.track_ids.len() {
                p.track_ids.remove(index);
                self.save().ok();
            }
        }
    }

    pub fn toggle_favorite(&mut self, track_id: &str) -> bool {
        let tid = track_id.to_string();
        let is_fav = if let Some(pos) = self.favorites.iter().position(|id| id == &tid) {
            self.favorites.remove(pos);
            false
        } else {
            self.favorites.push(tid);
            true
        };
        self.save().ok();
        is_fav
    }

    pub fn is_favorite(&self, track_id: &str) -> bool {
        self.favorites.iter().any(|id| id == track_id)
    }

    pub fn add_recently_played(&mut self, track_id: &str) {
        let tid = track_id.to_string();
        self.recently_played.retain(|id| id != &tid);
        self.recently_played.insert(0, tid);
        if self.recently_played.len() > 100 {
            self.recently_played.truncate(100);
        }
        self.save().ok();
    }

    pub fn get_playlist(&self, id: &str) -> Option<&Playlist> {
        self.playlists.iter().find(|p| p.id == id)
    }
}
