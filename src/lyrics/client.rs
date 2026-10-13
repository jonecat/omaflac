use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub const LRCLIB_GET_URL: &str = "https://lrclib.net/api/get";
pub const LRCLIB_SEARCH_URL: &str = "https://lrclib.net/api/search";

const NEGATIVE_MARKER_INSTRUMENTAL: &str = "[omaflac:instrumental]";
const NEGATIVE_MARKER_NOT_FOUND: &str = "[omaflac:not_found]";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FetchResult {
    Found(String),
    Instrumental,
    NotFound,
}

pub struct LyricsClient;

impl LyricsClient {
    pub fn cache_dir() -> PathBuf {
        let mut dir = dirs::cache_dir().unwrap_or_else(|| PathBuf::from("."));
        dir.push("omaflac");
        dir.push("lyrics");
        let _ = std::fs::create_dir_all(&dir);
        dir
    }

    /// Generate safe filename for disk caching based on artist and title
    pub fn cache_path(artist: &str, title: &str) -> PathBuf {
        let clean_artist: String = artist
            .chars()
            .map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' { c } else { '_' })
            .collect();
        let clean_title: String = title
            .chars()
            .map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' { c } else { '_' })
            .collect();

        let base = format!("{}_{}", clean_artist.trim(), clean_title.trim());
        let filename = if base.trim().is_empty() || base == "_" {
            "unknown.lrc".to_string()
        } else {
            format!("{}.lrc", base)
        };
        Self::cache_dir().join(filename)
    }

    /// Check if a local .lrc sidecar file exists next to the audio file
    pub fn check_local_file(track_path: &Path) -> Option<FetchResult> {
        let lrc_path = track_path.with_extension("lrc");
        if lrc_path.exists() && lrc_path.is_file() {
            if let Ok(content) = std::fs::read_to_string(&lrc_path) {
                if !content.trim().is_empty() {
                    return Some(FetchResult::Found(content));
                }
            }
        }
        None
    }

    /// Check local disk cache (~/.cache/omaflac/lyrics/)
    pub fn read_cache(artist: &str, title: &str) -> Option<FetchResult> {
        let path = Self::cache_path(artist, title);
        if !path.exists() {
            return None;
        }

        let content = std::fs::read_to_string(&path).ok()?;
        let trimmed = content.trim();

        if trimmed == NEGATIVE_MARKER_INSTRUMENTAL {
            Some(FetchResult::Instrumental)
        } else if trimmed == NEGATIVE_MARKER_NOT_FOUND {
            Some(FetchResult::NotFound)
        } else if !trimmed.is_empty() {
            Some(FetchResult::Found(content))
        } else {
            None
        }
    }

    /// Write result to local disk cache (including negative cache markers)
    pub fn write_cache(artist: &str, title: &str, result: &FetchResult) {
        let path = Self::cache_path(artist, title);
        match result {
            FetchResult::Found(content) => {
                let _ = std::fs::write(&path, content);
            }
            FetchResult::Instrumental => {
                let _ = std::fs::write(&path, NEGATIVE_MARKER_INSTRUMENTAL);
            }
            FetchResult::NotFound => {
                let _ = std::fs::write(&path, NEGATIVE_MARKER_NOT_FOUND);
            }
        }
    }

    /// Fetch lyrics from LRCLIB via background curl call
    pub fn fetch_remote(
        artist: &str,
        title: &str,
        album: &str,
        duration_secs: u64,
    ) -> FetchResult {
        if artist.trim().is_empty() && title.trim().is_empty() {
            return FetchResult::NotFound;
        }

        // 1. Try exact match with GET /api/get
        let mut cmd = Command::new("curl");
        cmd.args([
            "-fsSL",
            "--max-time",
            "4",
            "-A",
            "OmaFLAC/0.1.0",
            "-G",
            LRCLIB_GET_URL,
            "--data-urlencode",
            &format!("artist_name={}", artist),
            "--data-urlencode",
            &format!("track_name={}", title),
        ]);

        if !album.trim().is_empty() {
            cmd.args(["--data-urlencode", &format!("album_name={}", album)]);
        }
        if duration_secs > 0 {
            cmd.args(["--data-urlencode", &format!("duration={}", duration_secs)]);
        }

        cmd.stdin(Stdio::null());
        cmd.stderr(Stdio::null());

        if let Ok(output) = cmd.output() {
            if output.status.success() {
                if let Ok(json_str) = String::from_utf8(output.stdout) {
                    if let Some(res) = Self::parse_lrclib_single(&json_str) {
                        return res;
                    }
                }
            }
        }

        // 2. Fallback: Search /api/search without strict duration match
        let mut search_cmd = Command::new("curl");
        search_cmd.args([
            "-fsSL",
            "--max-time",
            "4",
            "-A",
            "OmaFLAC/0.1.0",
            "-G",
            LRCLIB_SEARCH_URL,
            "--data-urlencode",
            &format!("artist_name={}", artist),
            "--data-urlencode",
            &format!("track_name={}", title),
        ]);

        search_cmd.stdin(Stdio::null());
        search_cmd.stderr(Stdio::null());

        if let Ok(output) = search_cmd.output() {
            if output.status.success() {
                if let Ok(json_str) = String::from_utf8(output.stdout) {
                    if let Some(res) = Self::parse_lrclib_search_list(&json_str) {
                        return res;
                    }
                }
            }
        }

        FetchResult::NotFound
    }

    fn parse_lrclib_single(json_str: &str) -> Option<FetchResult> {
        let val: serde_json::Value = serde_json::from_str(json_str).ok()?;
        
        // Check for 404 error response
        if let Some(status) = val.get("statusCode").and_then(|s| s.as_u64()) {
            if status == 404 {
                return None;
            }
        }

        if val.get("instrumental").and_then(|v| v.as_bool()).unwrap_or(false) {
            return Some(FetchResult::Instrumental);
        }

        if let Some(synced) = val.get("syncedLyrics").and_then(|v| v.as_str()) {
            let trimmed = synced.trim();
            if !trimmed.is_empty() {
                return Some(FetchResult::Found(trimmed.to_string()));
            }
        }

        None
    }

    fn parse_lrclib_search_list(json_str: &str) -> Option<FetchResult> {
        let val: serde_json::Value = serde_json::from_str(json_str).ok()?;
        let list = val.as_array()?;

        for item in list {
            if item.get("instrumental").and_then(|v| v.as_bool()).unwrap_or(false) {
                return Some(FetchResult::Instrumental);
            }
            if let Some(synced) = item.get("syncedLyrics").and_then(|v| v.as_str()) {
                let trimmed = synced.trim();
                if !trimmed.is_empty() {
                    return Some(FetchResult::Found(trimmed.to_string()));
                }
            }
        }

        None
    }
}
