use crate::audio::visualizer::VisualizerBridge;
use crate::library::track::Track;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::SampleFormat;
use crossbeam_channel::{Receiver, Sender};
use std::fs::File;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{Decoder, DecoderOptions};
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::core::units::Time;

#[derive(Debug, Clone)]
pub struct TrackBoundary {
    pub track: Track,
    pub duration_secs: u64,
}

pub enum AudioItem {
    Sample(f32),
    Boundary(Box<TrackBoundary>),
}

pub struct PreloadedTrack {
    pub track: Track,
    pub format: Box<dyn FormatReader>,
    pub decoder: Box<dyn Decoder>,
    pub track_id: u32,
    pub sample_rate: u32,
    pub channels: usize,
}

#[derive(Debug, Clone)]
pub enum PlayerCommand {
    Play(Track),
    SetNextTrack(Option<Track>),
    Pause,
    Resume,
    TogglePause,
    Stop,
    SeekTo(Duration),
    SeekRelative(i64),
    SetVolume(f32),
}

#[derive(Debug, Clone)]
pub enum PlayerEvent {
    TrackFinished,
    TrackTransitioned(Track),
    Error(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackState {
    Stopped,
    Playing,
    Paused,
}

pub struct PlayerHandle {
    command_tx: Sender<PlayerCommand>,
    event_rx: Receiver<PlayerEvent>,
    state: Arc<std::sync::Mutex<PlaybackState>>,
    current_track: Arc<std::sync::Mutex<Option<Track>>>,
    elapsed_nanos: Arc<AtomicU64>,
    total_duration_secs: Arc<AtomicU64>,
    volume: Arc<std::sync::Mutex<f32>>,
    visualizer: VisualizerBridge,
}

impl PlayerHandle {
    pub fn new() -> anyhow::Result<Self> {
        let (command_tx, command_rx) = crossbeam_channel::unbounded();
        let (event_tx, event_rx) = crossbeam_channel::unbounded();

        let state = Arc::new(std::sync::Mutex::new(PlaybackState::Stopped));
        let current_track = Arc::new(std::sync::Mutex::new(None));
        let elapsed_nanos = Arc::new(AtomicU64::new(0));
        let total_duration_secs = Arc::new(AtomicU64::new(0));
        let volume = Arc::new(std::sync::Mutex::new(0.8f32));
        let visualizer = VisualizerBridge::new(44100);

        let worker = PlayerWorker {
            command_rx,
            event_tx,
            state: Arc::clone(&state),
            current_track: Arc::clone(&current_track),
            elapsed_nanos: Arc::clone(&elapsed_nanos),
            total_duration_secs: Arc::clone(&total_duration_secs),
            volume: Arc::clone(&volume),
            visualizer: visualizer.clone(),
        };

        std::thread::Builder::new()
            .name("omaflac-audio".into())
            .spawn(move || {
                worker.run();
            })?;

        Ok(Self {
            command_tx,
            event_rx,
            state,
            current_track,
            elapsed_nanos,
            total_duration_secs,
            volume,
            visualizer,
        })
    }

    pub fn play(&self, track: Track) {
        self.command_tx.send(PlayerCommand::Play(track)).ok();
    }

    pub fn set_next_track(&self, track: Option<Track>) {
        self.command_tx.send(PlayerCommand::SetNextTrack(track)).ok();
    }

    pub fn pause(&self) {
        self.command_tx.send(PlayerCommand::Pause).ok();
    }

    pub fn resume(&self) {
        self.command_tx.send(PlayerCommand::Resume).ok();
    }

    pub fn toggle_pause(&self) {
        self.command_tx.send(PlayerCommand::TogglePause).ok();
    }

    pub fn stop(&self) {
        self.command_tx.send(PlayerCommand::Stop).ok();
    }

    pub fn seek_to(&self, duration: Duration) {
        self.command_tx.send(PlayerCommand::SeekTo(duration)).ok();
    }

    pub fn seek_relative(&self, secs: i64) {
        self.command_tx.send(PlayerCommand::SeekRelative(secs)).ok();
    }

    pub fn set_volume(&self, vol: f32) {
        let clamped = vol.clamp(0.0, 1.5);
        if let Ok(mut v) = self.volume.lock() {
            *v = clamped;
        }
        self.command_tx.send(PlayerCommand::SetVolume(clamped)).ok();
    }

    pub fn volume(&self) -> f32 {
        self.volume.lock().map(|v| *v).unwrap_or(0.8)
    }

    pub fn state(&self) -> PlaybackState {
        self.state.lock().map(|s| *s).unwrap_or(PlaybackState::Stopped)
    }

    pub fn current_track(&self) -> Option<Track> {
        self.current_track.lock().ok().and_then(|t| t.clone())
    }

    pub fn elapsed(&self) -> Duration {
        Duration::from_nanos(self.elapsed_nanos.load(Ordering::Relaxed))
    }

    pub fn total_duration(&self) -> Duration {
        Duration::from_secs(self.total_duration_secs.load(Ordering::Relaxed))
    }

    pub fn visualizer(&self) -> &VisualizerBridge {
        &self.visualizer
    }

    pub fn try_recv_event(&self) -> Option<PlayerEvent> {
        self.event_rx.try_recv().ok()
    }
}

struct PlayerWorker {
    command_rx: Receiver<PlayerCommand>,
    event_tx: Sender<PlayerEvent>,
    state: Arc<std::sync::Mutex<PlaybackState>>,
    current_track: Arc<std::sync::Mutex<Option<Track>>>,
    elapsed_nanos: Arc<AtomicU64>,
    total_duration_secs: Arc<AtomicU64>,
    volume: Arc<std::sync::Mutex<f32>>,
    visualizer: VisualizerBridge,
}

struct StreamState {
    data: Vec<u8>,
    total_len: u64,
    is_complete: bool,
    error: Option<String>,
}

pub struct SharedStream {
    inner: Arc<(std::sync::Mutex<StreamState>, std::sync::Condvar)>,
    pos: u64,
    cancel: Arc<AtomicBool>,
}

impl SharedStream {
    pub fn open(path: &std::path::Path, cancel: Arc<AtomicBool>) -> anyhow::Result<Self> {
        let mut file = File::open(path)?;
        let total_len = file.metadata()?.len();

        let initial_cap = (total_len as usize).min(128 * 1024 * 1024);
        let mut data = Vec::with_capacity(initial_cap);

        // Read initial chunk (up to 512 KB) synchronously so Symphonia probes without delay
        let initial_read = (512 * 1024).min(total_len as usize);
        data.resize(initial_read, 0);
        let mut bytes_read = 0;
        while bytes_read < initial_read {
            match std::io::Read::read(&mut file, &mut data[bytes_read..initial_read]) {
                Ok(0) => break,
                Ok(n) => bytes_read += n,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e.into()),
            }
        }
        data.truncate(bytes_read);
        let is_complete = data.len() as u64 >= total_len;

        let inner = Arc::new((
            std::sync::Mutex::new(StreamState {
                data,
                total_len,
                is_complete,
                error: None,
            }),
            std::sync::Condvar::new(),
        ));

        if !is_complete {
            let inner_clone = Arc::clone(&inner);
            let cancel_clone = Arc::clone(&cancel);
            let mut bg_file = file;

            std::thread::Builder::new()
                .name("omaflac-preload".into())
                .spawn(move || {
                    let mut chunk = vec![0u8; 512 * 1024];
                    loop {
                        if cancel_clone.load(Ordering::Relaxed) {
                            break;
                        }
                        match std::io::Read::read(&mut bg_file, &mut chunk) {
                            Ok(0) => {
                                let (lock, cvar) = &*inner_clone;
                                if let Ok(mut state) = lock.lock() {
                                    state.is_complete = true;
                                }
                                cvar.notify_all();
                                break;
                            }
                            Ok(n) => {
                                let (lock, cvar) = &*inner_clone;
                                if let Ok(mut state) = lock.lock() {
                                    state.data.extend_from_slice(&chunk[..n]);
                                    if state.data.len() as u64 >= state.total_len {
                                        state.is_complete = true;
                                    }
                                }
                                cvar.notify_all();
                            }
                            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                            Err(e) => {
                                let (lock, cvar) = &*inner_clone;
                                if let Ok(mut state) = lock.lock() {
                                    state.error = Some(e.to_string());
                                    state.is_complete = true;
                                }
                                cvar.notify_all();
                                break;
                            }
                        }
                    }
                })?;
        }

        Ok(Self {
            inner,
            pos: 0,
            cancel,
        })
    }
}

impl std::io::Read for SharedStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let (lock, cvar) = &*self.inner;
        let mut state = lock.lock().unwrap();

        loop {
            if self.cancel.load(Ordering::Relaxed) {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Interrupted,
                    "Playback interrupted",
                ));
            }

            if self.pos < state.data.len() as u64 {
                let start = self.pos as usize;
                let count = (state.data.len() - start).min(buf.len());
                buf[..count].copy_from_slice(&state.data[start..start + count]);
                self.pos += count as u64;
                return Ok(count);
            }

            if state.is_complete {
                return Ok(0);
            }

            if let Some(ref e) = state.error {
                return Err(std::io::Error::new(std::io::ErrorKind::Other, e.clone()));
            }

            state = cvar.wait(state).unwrap();
        }
    }
}

impl std::io::Seek for SharedStream {
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        let (lock, cvar) = &*self.inner;
        let mut state = lock.lock().unwrap();

        let target_pos = match pos {
            std::io::SeekFrom::Start(offset) => offset as i64,
            std::io::SeekFrom::Current(offset) => self.pos as i64 + offset,
            std::io::SeekFrom::End(offset) => state.total_len as i64 + offset,
        };

        if target_pos < 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Negative seek position",
            ));
        }

        let target = target_pos as u64;

        while target > state.data.len() as u64 && !state.is_complete && !self.cancel.load(Ordering::Relaxed) {
            if let Some(ref e) = state.error {
                return Err(std::io::Error::new(std::io::ErrorKind::Other, e.clone()));
            }
            state = cvar.wait(state).unwrap();
        }

        if self.cancel.load(Ordering::Relaxed) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Interrupted,
                "Playback interrupted",
            ));
        }

        self.pos = target;
        Ok(self.pos)
    }
}

impl symphonia::core::io::MediaSource for SharedStream {
    fn is_seekable(&self) -> bool {
        true
    }
    fn byte_len(&self) -> Option<u64> {
        let (lock, _) = &*self.inner;
        lock.lock().ok().map(|s| s.total_len)
    }
}


impl PlayerWorker {
    fn run(self) {
        let host = cpal::default_host();
        let device = match host.default_output_device() {
            Some(dev) => dev,
            None => {
                self.event_tx
                    .send(PlayerEvent::Error("No default audio output device".into()))
                    .ok();
                return;
            }
        };

        let config = match device.default_output_config() {
            Ok(cfg) => cfg,
            Err(e) => {
                self.event_tx
                    .send(PlayerEvent::Error(format!("Failed to query device config: {}", e)))
                    .ok();
                return;
            }
        };

        let sample_rate = config.sample_rate().0;
        let channels = config.channels() as usize;

        // Shared ring buffer between decode loop and CPAL stream (10 seconds deep buffer)
        let ring_buffer_capacity = sample_rate as usize * channels * 10;
        let (audio_tx, audio_rx) = crossbeam_channel::bounded::<AudioItem>(ring_buffer_capacity);
        let audio_rx_worker = audio_rx.clone();

        let is_stream_paused = Arc::new(AtomicBool::new(false));
        let is_stream_paused_clone = Arc::clone(&is_stream_paused);

        let elapsed_nanos_clone = Arc::clone(&self.elapsed_nanos);
        let total_duration_secs_clone = Arc::clone(&self.total_duration_secs);
        let current_track_clone = Arc::clone(&self.current_track);
        let event_tx_clone = self.event_tx.clone();
        let visualizer_clone = self.visualizer.clone();

        let err_tx = self.event_tx.clone();
        let stream = match config.sample_format() {
            SampleFormat::F32 => device.build_output_stream(
                &config.into(),
                move |data: &mut [f32], _| {
                    if is_stream_paused_clone.load(Ordering::Relaxed) {
                        data.fill(0.0);
                        return;
                    }
                    let mut played_samples = 0;
                    for sample in data.iter_mut() {
                        loop {
                            match audio_rx.try_recv() {
                                Ok(AudioItem::Sample(s)) => {
                                    *sample = s;
                                    played_samples += 1;
                                    break;
                                }
                                Ok(AudioItem::Boundary(boundary)) => {
                                    elapsed_nanos_clone.store(0, Ordering::Relaxed);
                                    total_duration_secs_clone.store(boundary.duration_secs, Ordering::Relaxed);
                                    if let Ok(mut cur) = current_track_clone.lock() {
                                        *cur = Some(boundary.track.clone());
                                    }
                                    event_tx_clone.send(PlayerEvent::TrackTransitioned(boundary.track)).ok();
                                }
                                Err(_) => {
                                    *sample = 0.0;
                                    break;
                                }
                            }
                        }
                    }

                    if played_samples > 0 {
                        let ch = channels.max(1);
                        let frames_played = played_samples / ch;
                        let added_nanos = (frames_played as f64 / sample_rate as f64 * 1_000_000_000.0) as u64;
                        elapsed_nanos_clone.fetch_add(added_nanos, Ordering::Relaxed);

                        let mut mono_buf = [0.0f32; 2048];
                        let num_frames = frames_played.min(mono_buf.len());
                        if ch == 1 {
                            mono_buf[..num_frames].copy_from_slice(&data[..num_frames]);
                        } else {
                            for f in 0..num_frames {
                                let mut sum = 0.0;
                                for c in 0..ch {
                                    sum += data[f * ch + c];
                                }
                                mono_buf[f] = sum / ch as f32;
                            }
                        }
                        visualizer_clone.push_samples(&mono_buf[..num_frames]);
                    }
                },
                move |err| {
                    err_tx.send(PlayerEvent::Error(format!("Audio stream error: {}", err))).ok();
                },
                None,
            ),
            _ => {
                self.event_tx
                    .send(PlayerEvent::Error("Unsupported audio sample format".into()))
                    .ok();
                return;
            }
        };

        let stream = match stream {
            Ok(s) => s,
            Err(e) => {
                self.event_tx
                    .send(PlayerEvent::Error(format!("Failed to build audio stream: {}", e)))
                    .ok();
                return;
            }
        };

        if let Err(e) = stream.play() {
            self.event_tx
                .send(PlayerEvent::Error(format!("Failed to start audio stream: {}", e)))
                .ok();
            return;
        }

        self.visualizer.set_sample_rate(sample_rate);

        // Playback state variables
        let mut active_format: Option<Box<dyn FormatReader>> = None;
        let mut active_decoder: Option<Box<dyn Decoder>> = None;
        let mut active_track_id: u32 = 0;
        let mut active_track_sample_rate: u32 = sample_rate;
        let mut active_track_channels: usize = 2;
        let mut sample_buf: Option<SampleBuffer<f32>> = None;

        let prebuffer_target = sample_rate as usize * channels / 4; // 250ms pre-buffer threshold for instant playback
        let high_water_mark = sample_rate as usize * channels * 6; // 6 seconds high-water mark

        let mut is_playing = false;
        let mut is_paused = false;
        let mut is_prebuffering = false;
        let mut is_eof = false;

        let mut resample_ratio: f64 = 1.0;
        let mut active_preload_cancel: Option<Arc<AtomicBool>> = None;

        let mut queued_next_track: Option<Track> = None;
        let mut pending_preload: Option<(Track, Receiver<anyhow::Result<PreloadedTrack>>, Arc<AtomicBool>)> = None;
        let mut preloaded_next_track: Option<PreloadedTrack> = None;

        loop {
            // Drain incoming commands
            while let Ok(cmd) = self.command_rx.try_recv() {
                match cmd {
                    PlayerCommand::Play(track) => {
                        // Cancel previous background preloader threads
                        if let Some(prev_cancel) = active_preload_cancel.take() {
                            prev_cancel.store(true, Ordering::Relaxed);
                        }
                        if let Some((_, _, cancel)) = pending_preload.take() {
                            cancel.store(true, Ordering::Relaxed);
                        }
                        preloaded_next_track = None;
                        queued_next_track = None;

                        let cancel_token = Arc::new(AtomicBool::new(false));
                        active_preload_cancel = Some(Arc::clone(&cancel_token));

                        // Stop current playback & pause output stream immediately
                        is_stream_paused.store(true, Ordering::SeqCst);
                        active_format = None;
                        active_decoder = None;
                        is_eof = false;
                        sample_buf = None;

                        // Synchronously drain old samples from ring buffer before decoding new track
                        while audio_rx_worker.try_recv().is_ok() {}

                        is_prebuffering = true;
                        self.visualizer.clear();

                        match self.open_track(&track, cancel_token) {
                            Ok(preloaded) => {
                                active_format = Some(preloaded.format);
                                active_decoder = Some(preloaded.decoder);
                                active_track_id = preloaded.track_id;
                                active_track_sample_rate = preloaded.sample_rate;
                                active_track_channels = preloaded.channels;
                                resample_ratio = preloaded.sample_rate as f64 / sample_rate as f64;
                                sample_buf = None;

                                if let Ok(mut cur) = self.current_track.lock() {
                                    *cur = Some(track.clone());
                                }
                                self.total_duration_secs.store(track.duration_secs, Ordering::Relaxed);
                                self.elapsed_nanos.store(0, Ordering::SeqCst);

                                is_playing = true;
                                is_paused = false;
                                if let Ok(mut st) = self.state.lock() {
                                    *st = PlaybackState::Playing;
                                }
                            }
                            Err(e) => {
                                self.event_tx
                                    .send(PlayerEvent::Error(format!("Failed to open track: {}", e)))
                                    .ok();
                                is_playing = false;
                                is_prebuffering = false;
                                if let Ok(mut st) = self.state.lock() {
                                    *st = PlaybackState::Stopped;
                                }
                            }
                        }
                    }
                    PlayerCommand::SetNextTrack(opt_track) => {
                        let changed = match (&queued_next_track, &opt_track) {
                            (Some(c), Some(n)) => c.id != n.id,
                            (None, None) => false,
                            _ => true,
                        };
                        if changed {
                            if let Some((_, _, cancel)) = pending_preload.take() {
                                cancel.store(true, Ordering::Relaxed);
                            }
                            preloaded_next_track = None;
                            queued_next_track = opt_track.clone();

                            if let Some(track) = opt_track {
                                let (tx, rx) = crossbeam_channel::bounded(1);
                                let cancel = Arc::new(AtomicBool::new(false));
                                let cancel_clone = Arc::clone(&cancel);
                                let trk = track.clone();

                                std::thread::Builder::new()
                                    .name("omaflac-next-preload".into())
                                    .spawn(move || {
                                        let res = Self::open_track_static(&trk, cancel_clone);
                                        tx.send(res).ok();
                                    })
                                    .ok();

                                pending_preload = Some((track, rx, cancel));
                            }
                        }
                    }
                    PlayerCommand::Pause => {
                        if is_playing && !is_paused {
                            is_paused = true;
                            is_stream_paused.store(true, Ordering::Relaxed);
                            if let Ok(mut st) = self.state.lock() {
                                *st = PlaybackState::Paused;
                            }
                        }
                    }
                    PlayerCommand::Resume => {
                        if is_playing && is_paused {
                            is_paused = false;
                            if !is_prebuffering {
                                is_stream_paused.store(false, Ordering::Relaxed);
                            }
                            if let Ok(mut st) = self.state.lock() {
                                *st = PlaybackState::Playing;
                            }
                        }
                    }
                    PlayerCommand::TogglePause => {
                        if is_playing {
                            is_paused = !is_paused;
                            if is_paused {
                                is_stream_paused.store(true, Ordering::Relaxed);
                                self.visualizer.clear();
                            } else if !is_prebuffering {
                                is_stream_paused.store(false, Ordering::Relaxed);
                            }
                            if let Ok(mut st) = self.state.lock() {
                                *st = if is_paused {
                                    PlaybackState::Paused
                                } else {
                                    PlaybackState::Playing
                                };
                            }
                        }
                    }
                    PlayerCommand::Stop => {
                        if let Some(prev_cancel) = active_preload_cancel.take() {
                            prev_cancel.store(true, Ordering::Relaxed);
                        }
                        if let Some((_, _, cancel)) = pending_preload.take() {
                            cancel.store(true, Ordering::Relaxed);
                        }
                        preloaded_next_track = None;
                        queued_next_track = None;
                        active_format = None;
                        active_decoder = None;
                        is_playing = false;
                        is_paused = false;
                        is_prebuffering = false;
                        is_eof = false;
                        is_stream_paused.store(true, Ordering::SeqCst);
                        while audio_rx_worker.try_recv().is_ok() {}
                        sample_buf = None;
                        self.visualizer.clear();
                        if let Ok(mut st) = self.state.lock() {
                            *st = PlaybackState::Stopped;
                        }
                    }
                    PlayerCommand::SeekTo(target_duration) => {
                        if let Some(format) = active_format.as_mut() {
                            let time = Time::from(target_duration.as_secs_f64());
                            let seek_to = SeekTo::Time {
                                time,
                                track_id: Some(active_track_id),
                            };
                            if let Ok(seeked) = format.seek(SeekMode::Coarse, seek_to) {
                                is_stream_paused.store(true, Ordering::SeqCst);
                                while audio_rx_worker.try_recv().is_ok() {}
                                is_prebuffering = true;
                                is_eof = false;
                                sample_buf = None;
                                self.visualizer.clear();
                                let actual_secs = seeked.actual_ts as f64 / active_track_sample_rate as f64;
                                self.elapsed_nanos.store((actual_secs * 1_000_000_000.0) as u64, Ordering::SeqCst);
                            }
                        }
                    }
                    PlayerCommand::SeekRelative(delta_secs) => {
                        let cur_nanos = self.elapsed_nanos.load(Ordering::Relaxed);
                        let cur_secs = cur_nanos as f64 / 1_000_000_000.0;
                        let target_secs = (cur_secs + delta_secs as f64).max(0.0);
                        let total_secs = self.total_duration_secs.load(Ordering::Relaxed) as f64;
                        let clamped = target_secs.min(total_secs);
                        if let Some(format) = active_format.as_mut() {
                            let time = Time::from(clamped);
                            let seek_to = SeekTo::Time {
                                time,
                                track_id: Some(active_track_id),
                            };
                            if let Ok(seeked) = format.seek(SeekMode::Coarse, seek_to) {
                                is_stream_paused.store(true, Ordering::SeqCst);
                                while audio_rx_worker.try_recv().is_ok() {}
                                is_prebuffering = true;
                                is_eof = false;
                                sample_buf = None;
                                self.visualizer.clear();
                                let actual_secs = seeked.actual_ts as f64 / active_track_sample_rate as f64;
                                self.elapsed_nanos.store((actual_secs * 1_000_000_000.0) as u64, Ordering::SeqCst);
                            }
                        }
                    }
                    PlayerCommand::SetVolume(vol) => {
                        if let Ok(mut v) = self.volume.lock() {
                            *v = vol;
                        }
                    }
                }
            }

            if !is_playing || is_paused {
                std::thread::sleep(Duration::from_millis(10));
                continue;
            }

            // Check if next track background preload has completed
            if preloaded_next_track.is_none() {
                if let Some((_, rx, _)) = &pending_preload {
                    if let Ok(res) = rx.try_recv() {
                        if let Ok(preloaded) = res {
                            preloaded_next_track = Some(preloaded);
                        }
                        pending_preload = None;
                    }
                }
            }

            // EOF handling: wait until all queued audio in ring buffer finishes playing
            if is_eof {
                if audio_tx.is_empty() {
                    is_playing = false;
                    is_eof = false;
                    if let Ok(mut st) = self.state.lock() {
                        *st = PlaybackState::Stopped;
                    }
                    self.event_tx.send(PlayerEvent::TrackFinished).ok();
                } else {
                    std::thread::sleep(Duration::from_millis(15));
                }
                continue;
            }

            // Pre-buffering and high-water mark throttling
            if is_prebuffering {
                if audio_tx.len() >= prebuffer_target {
                    is_prebuffering = false;
                    if !is_paused {
                        is_stream_paused.store(false, Ordering::Relaxed);
                    }
                }
            } else if audio_tx.len() > high_water_mark {
                // Deep buffer filled (6 seconds), throttle decode loop to keep CPU near zero
                std::thread::sleep(Duration::from_millis(15));
                continue;
            }

            // Decode next packet
            let format = active_format.as_mut().unwrap();
            let decoder = active_decoder.as_mut().unwrap();

            let packet = match format.next_packet() {
                Ok(p) => p,
                Err(symphonia::core::errors::Error::IoError(err))
                    if err.kind() == std::io::ErrorKind::UnexpectedEof =>
                {
                    // Track reached EOF! Check if preloaded next track is available for gapless playback
                    if preloaded_next_track.is_none() {
                        if let Some((_, rx, _)) = pending_preload.take() {
                            if let Ok(Ok(preloaded)) = rx.recv_timeout(Duration::from_millis(300)) {
                                preloaded_next_track = Some(preloaded);
                            }
                        }
                    }

                    if let Some(next) = preloaded_next_track.take() {
                        queued_next_track = None;

                        // Insert gapless boundary marker into audio ring buffer
                        let boundary = TrackBoundary {
                            track: next.track.clone(),
                            duration_secs: next.track.duration_secs,
                        };
                        audio_tx.send(AudioItem::Boundary(Box::new(boundary))).ok();

                        // Seamlessly transition decoder to next track
                        active_format = Some(next.format);
                        active_decoder = Some(next.decoder);
                        active_track_id = next.track_id;
                        active_track_sample_rate = next.sample_rate;
                        active_track_channels = next.channels;
                        resample_ratio = next.sample_rate as f64 / sample_rate as f64;
                        sample_buf = None;
                        is_eof = false;

                        // Continue decoding immediately with zero audio gap!
                        continue;
                    } else {
                        is_eof = true;
                        if is_prebuffering {
                            is_prebuffering = false;
                            if !is_paused {
                                is_stream_paused.store(false, Ordering::Relaxed);
                            }
                        }
                        continue;
                    }
                }
                Err(symphonia::core::errors::Error::ResetRequired) => {
                    decoder.reset();
                    continue;
                }
                Err(e) => {
                    self.event_tx
                        .send(PlayerEvent::Error(format!("Decode packet error: {}", e)))
                        .ok();
                    is_playing = false;
                    is_prebuffering = false;
                    if let Ok(mut st) = self.state.lock() {
                        *st = PlaybackState::Stopped;
                    }
                    continue;
                }
            };

            if packet.track_id() != active_track_id {
                continue;
            }

            let decoded = match decoder.decode(&packet) {
                Ok(d) => d,
                Err(symphonia::core::errors::Error::DecodeError(_)) => continue,
                Err(e) => {
                    self.event_tx
                        .send(PlayerEvent::Error(format!("Decode error: {}", e)))
                        .ok();
                    continue;
                }
            };

            if sample_buf.is_none() {
                let spec = *decoded.spec();
                let duration = decoded.capacity() as u64;
                sample_buf = Some(SampleBuffer::new(duration, spec));
            }

            let buf = sample_buf.as_mut().unwrap();
            buf.copy_interleaved_ref(decoded);
            let samples = buf.samples();

            let current_vol = self.volume.lock().map(|v| *v).unwrap_or(0.8);
            let in_channels = active_track_channels.max(1);

            // Channel adapt and push to ring buffer
            if (resample_ratio - 1.0).abs() < 0.001 && in_channels == channels {
                for &s in samples {
                    let out_s = s * current_vol;
                    audio_tx.send(AudioItem::Sample(out_s)).ok();
                }
            } else {
                let num_frames = samples.len() / in_channels;
                for frame_idx in 0..num_frames {
                    let s_left = samples[frame_idx * in_channels];
                    let s_right = if in_channels > 1 {
                        samples[frame_idx * in_channels + 1]
                    } else {
                        s_left
                    };

                    let out_left = s_left * current_vol;
                    let out_right = s_right * current_vol;

                    if channels == 1 {
                        let mono = (out_left + out_right) * 0.5;
                        audio_tx.send(AudioItem::Sample(mono)).ok();
                    } else {
                        audio_tx.send(AudioItem::Sample(out_left)).ok();
                        audio_tx.send(AudioItem::Sample(out_right)).ok();
                    }
                }
            }
        }
    }

    fn open_track_static(
        track: &Track,
        cancel: Arc<AtomicBool>,
    ) -> anyhow::Result<PreloadedTrack> {
        let stream = SharedStream::open(&track.path, cancel)?;
        let mss_opts = symphonia::core::io::MediaSourceStreamOptions {
            buffer_len: 512 * 1024,
        };
        let mss = MediaSourceStream::new(Box::new(stream), mss_opts);

        let mut hint = Hint::new();
        if let Some(ext) = track.path.extension().and_then(|e| e.to_str()) {
            hint.with_extension(ext);
        }

        let fmt_opts: FormatOptions = Default::default();
        let meta_opts: MetadataOptions = Default::default();

        let probed = symphonia::default::get_probe().format(&hint, mss, &fmt_opts, &meta_opts)?;
        let format = probed.format;

        let trk = format
            .default_track()
            .ok_or_else(|| anyhow::anyhow!("No default audio track found"))?;
        let trk_id = trk.id;
        let sample_rate = trk.codec_params.sample_rate.unwrap_or(44100);
        let channels = trk.codec_params.channels.map(|c| c.count()).unwrap_or(2);

        let dec_opts: DecoderOptions = Default::default();
        let decoder = symphonia::default::get_codecs()
            .make(&trk.codec_params, &dec_opts)?;

        Ok(PreloadedTrack {
            track: track.clone(),
            format,
            decoder,
            track_id: trk_id,
            sample_rate,
            channels,
        })
    }

    fn open_track(
        &self,
        track: &Track,
        cancel: Arc<AtomicBool>,
    ) -> anyhow::Result<PreloadedTrack> {
        Self::open_track_static(track, cancel)
    }
}
