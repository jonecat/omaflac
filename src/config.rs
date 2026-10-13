use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RepeatMode {
    Off,
    Track,
    All,
}

impl RepeatMode {
    pub fn next(&self) -> Self {
        match self {
            RepeatMode::Off => RepeatMode::All,
            RepeatMode::All => RepeatMode::Track,
            RepeatMode::Track => RepeatMode::Off,
        }
    }

    pub fn display(&self) -> &'static str {
        match self {
            RepeatMode::Off => "Off",
            RepeatMode::All => "All",
            RepeatMode::Track => "1",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub library_folders: Vec<PathBuf>,
    pub volume: f32,
    pub repeat_mode: RepeatMode,
    pub shuffle: bool,
    pub visualizer_enabled: bool,
    pub visualizer_fps: u64,
    #[serde(default = "default_theme")]
    pub theme: String,
}

fn default_theme() -> String {
    "auto".to_string()
}

impl Default for AppConfig {
    fn default() -> Self {
        let mut folders = Vec::new();
        if let Some(audio_dir) = dirs::audio_dir() {
            if audio_dir.exists() {
                folders.push(audio_dir);
            }
        }
        if let Some(home) = dirs::home_dir() {
            let music = home.join("Music");
            if music.exists() && !folders.contains(&music) {
                folders.push(music);
            }
        }

        Self {
            library_folders: folders,
            volume: 0.8,
            repeat_mode: RepeatMode::Off,
            shuffle: false,
            visualizer_enabled: true,
            visualizer_fps: 30,
            theme: default_theme(),
        }
    }
}

impl AppConfig {
    pub fn config_path() -> PathBuf {
        let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        path.push("omaflac");
        std::fs::create_dir_all(&path).ok();
        path.push("config.json");
        path
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        if let Ok(file) = std::fs::File::open(path) {
            if let Ok(config) = serde_json::from_reader(file) {
                return config;
            }
        }
        let default_config = Self::default();
        default_config.save().ok();
        default_config
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::config_path();
        let file = std::fs::File::create(path)?;
        serde_json::to_writer_pretty(file, self)?;
        Ok(())
    }
}
