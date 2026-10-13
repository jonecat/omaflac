pub mod client;
pub mod lrc;

pub use client::{FetchResult, LyricsClient};
pub use lrc::{LyricLine, LyricsWindow, SyncedLyrics};

use crate::library::track::Track;
use crossbeam_channel::{unbounded, Receiver, Sender};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LyricsStatus {
    Idle,
    Loading,
    Found(SyncedLyrics),
    Instrumental,
    NotFound,
}

pub struct LyricsManager {
    current_track_id: Option<String>,
    current_status: LyricsStatus,
    sender: Sender<(u64, String, LyricsStatus)>,
    receiver: Receiver<(u64, String, LyricsStatus)>,
    request_counter: Arc<AtomicU64>,
}

impl Default for LyricsManager {
    fn default() -> Self {
        Self::new()
    }
}

impl LyricsManager {
    pub fn new() -> Self {
        let (sender, receiver) = unbounded();
        Self {
            current_track_id: None,
            current_status: LyricsStatus::Idle,
            sender,
            receiver,
            request_counter: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn status(&self) -> &LyricsStatus {
        &self.current_status
    }

    pub fn current_track_id(&self) -> Option<&str> {
        self.current_track_id.as_deref()
    }

    /// Request synchronized lyrics for currently playing track
    pub fn request_lyrics(&mut self, track: &Track) {
        if self.current_track_id.as_deref() == Some(&track.id) {
            return;
        }

        self.current_track_id = Some(track.id.clone());

        // 1. Check local .lrc file next to track audio file
        if let Some(FetchResult::Found(content)) = LyricsClient::check_local_file(&track.path) {
            if let Some(synced) = SyncedLyrics::from_lrc(&content) {
                self.current_status = LyricsStatus::Found(synced);
                return;
            }
        }

        // 2. Check local disk cache (~/.cache/omaflac/lyrics/)
        if let Some(cached) = LyricsClient::read_cache(&track.artist, &track.title) {
            match cached {
                FetchResult::Found(content) => {
                    if let Some(synced) = SyncedLyrics::from_lrc(&content) {
                        self.current_status = LyricsStatus::Found(synced);
                        return;
                    }
                }
                FetchResult::Instrumental => {
                    self.current_status = LyricsStatus::Instrumental;
                    return;
                }
                FetchResult::NotFound => {
                    self.current_status = LyricsStatus::NotFound;
                    return;
                }
            }
        }

        // 3. Not in cache: set status to Loading and spawn background fetcher
        self.current_status = LyricsStatus::Loading;

        let request_id = self.request_counter.fetch_add(1, Ordering::SeqCst) + 1;
        let track_id = track.id.clone();
        let artist = track.artist.clone();
        let title = track.title.clone();
        let album = track.album.clone();
        let duration_secs = track.duration_secs;
        let sender = self.sender.clone();

        std::thread::spawn(move || {
            let res = LyricsClient::fetch_remote(&artist, &title, &album, duration_secs);
            LyricsClient::write_cache(&artist, &title, &res);

            let status = match res {
                FetchResult::Found(content) => {
                    if let Some(synced) = SyncedLyrics::from_lrc(&content) {
                        LyricsStatus::Found(synced)
                    } else {
                        LyricsStatus::NotFound
                    }
                }
                FetchResult::Instrumental => LyricsStatus::Instrumental,
                FetchResult::NotFound => LyricsStatus::NotFound,
            };

            let _ = sender.send((request_id, track_id, status));
        });
    }

    /// Pre-fetch upcoming track lyrics to disk cache without modifying current playback state
    pub fn prefetch_lyrics(&self, track: &Track) {
        // If local file or cache already has it, skip prefetch
        if LyricsClient::check_local_file(&track.path).is_some() {
            return;
        }
        if LyricsClient::read_cache(&track.artist, &track.title).is_some() {
            return;
        }

        let artist = track.artist.clone();
        let title = track.title.clone();
        let album = track.album.clone();
        let duration_secs = track.duration_secs;

        std::thread::spawn(move || {
            let res = LyricsClient::fetch_remote(&artist, &title, &album, duration_secs);
            LyricsClient::write_cache(&artist, &title, &res);
        });
    }

    /// Poll incoming channel on tick
    pub fn poll(&mut self) {
        while let Ok((req_id, track_id, status)) = self.receiver.try_recv() {
            if Some(&track_id) == self.current_track_id.as_ref()
                && req_id == self.request_counter.load(Ordering::SeqCst)
            {
                self.current_status = status;
            }
        }
    }
}
