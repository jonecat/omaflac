pub mod player;
pub mod visualizer;
pub mod waveform;

use crate::library::track::Track;
pub use player::{PlaybackState, PlayerEvent, PlayerHandle};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use visualizer::VisualizerBridge;
use waveform::WaveformData;

pub struct AudioEngine {
    player: PlayerHandle,
    current_waveform: Arc<Mutex<Option<WaveformData>>>,
    waveform_cancel_token: Arc<AtomicBool>,
}

impl AudioEngine {
    pub fn new() -> anyhow::Result<Self> {
        let player = PlayerHandle::new()?;
        Ok(Self {
            player,
            current_waveform: Arc::new(Mutex::new(None)),
            waveform_cancel_token: Arc::new(AtomicBool::new(false)),
        })
    }

    pub fn play_track(&self, track: Track) {
        // Cancel any pending waveform generation
        self.waveform_cancel_token.store(true, Ordering::Relaxed);

        // Immediately set estimated waveform
        let fast_waveform = WaveformData::generate_fast_estimate(track.path.clone(), track.duration_secs);
        if let Ok(mut wf) = self.current_waveform.lock() {
            *wf = Some(fast_waveform);
        }

        // Spawn background task to compute accurate waveform
        let cancel_token = Arc::new(AtomicBool::new(false));
        self.waveform_cancel_token.store(false, Ordering::Relaxed);
        let track_path = track.path.clone();
        let wf_target = Arc::clone(&self.current_waveform);
        let cancel_clone = Arc::clone(&cancel_token);

        std::thread::Builder::new()
            .name("waveform-scanner".into())
            .spawn(move || {
                if let Some(peaks) = WaveformData::compute_accurate(&track_path, waveform::DEFAULT_WAVEFORM_BINS, cancel_clone) {
                    if let Ok(mut wf) = wf_target.lock() {
                        *wf = Some(WaveformData {
                            path: track_path,
                            peaks,
                            is_complete: true,
                        });
                    }
                }
            })
            .ok();

        self.player.play(track);
    }

    pub fn set_next_track(&self, track: Option<Track>) {
        self.player.set_next_track(track);
    }

    pub fn on_track_transitioned(&self, track: &Track) {
        // Cancel previous waveform scanner and start for new track
        self.waveform_cancel_token.store(true, Ordering::Relaxed);
        let fast_waveform = WaveformData::generate_fast_estimate(track.path.clone(), track.duration_secs);
        if let Ok(mut wf) = self.current_waveform.lock() {
            *wf = Some(fast_waveform);
        }

        let cancel_token = Arc::new(AtomicBool::new(false));
        self.waveform_cancel_token.store(false, Ordering::Relaxed);
        let track_path = track.path.clone();
        let wf_target = Arc::clone(&self.current_waveform);
        let cancel_clone = Arc::clone(&cancel_token);

        std::thread::Builder::new()
            .name("waveform-scanner".into())
            .spawn(move || {
                if let Some(peaks) = WaveformData::compute_accurate(&track_path, waveform::DEFAULT_WAVEFORM_BINS, cancel_clone) {
                    if let Ok(mut wf) = wf_target.lock() {
                        *wf = Some(WaveformData {
                            path: track_path,
                            peaks,
                            is_complete: true,
                        });
                    }
                }
            })
            .ok();
    }

    pub fn pause(&self) {
        self.player.pause();
    }

    pub fn resume(&self) {
        self.player.resume();
    }

    pub fn toggle_pause(&self) {
        self.player.toggle_pause();
    }

    pub fn stop(&self) {
        self.player.stop();
        if let Ok(mut wf) = self.current_waveform.lock() {
            *wf = None;
        }
    }

    pub fn seek_to(&self, duration: Duration) {
        self.player.seek_to(duration);
    }

    pub fn seek_relative(&self, secs: i64) {
        self.player.seek_relative(secs);
    }

    pub fn set_volume(&self, vol: f32) {
        self.player.set_volume(vol);
    }

    pub fn volume(&self) -> f32 {
        self.player.volume()
    }

    pub fn state(&self) -> PlaybackState {
        self.player.state()
    }

    pub fn current_track(&self) -> Option<Track> {
        self.player.current_track()
    }

    pub fn elapsed(&self) -> Duration {
        self.player.elapsed()
    }

    pub fn total_duration(&self) -> Duration {
        self.player.total_duration()
    }

    pub fn visualizer(&self) -> &VisualizerBridge {
        self.player.visualizer()
    }

    pub fn current_waveform(&self) -> Option<WaveformData> {
        self.current_waveform.lock().ok().and_then(|w| w.clone())
    }

    pub fn try_recv_event(&self) -> Option<PlayerEvent> {
        self.player.try_recv_event()
    }
}
