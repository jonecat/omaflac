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
pub enum PlayerCommand {
    Play(Track),
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

        // Shared ring buffer between decode loop and CPAL stream
        let ring_buffer_capacity = sample_rate as usize * channels * 2; // 2 seconds of audio
        let (audio_tx, audio_rx) = crossbeam_channel::bounded::<f32>(ring_buffer_capacity);

        let is_stream_paused = Arc::new(AtomicBool::new(false));
        let is_stream_paused_clone = Arc::clone(&is_stream_paused);

        let should_flush = Arc::new(AtomicBool::new(false));
        let should_flush_clone = Arc::clone(&should_flush);

        let err_tx = self.event_tx.clone();
        let stream = match config.sample_format() {
            SampleFormat::F32 => device.build_output_stream(
                &config.into(),
                move |data: &mut [f32], _| {
                    if is_stream_paused_clone.load(Ordering::Relaxed) {
                        data.fill(0.0);
                        return;
                    }
                    if should_flush_clone.swap(false, Ordering::Relaxed) {
                        while audio_rx.try_recv().is_ok() {}
                    }
                    for sample in data.iter_mut() {
                        *sample = audio_rx.try_recv().unwrap_or(0.0);
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

        let mut is_playing = false;
        let mut is_paused = false;

        let mut resample_ratio: f64 = 1.0;

        loop {
            // Drain incoming commands
            while let Ok(cmd) = self.command_rx.try_recv() {
                match cmd {
                    PlayerCommand::Play(track) => {
                        // Stop current playback
                        active_format = None;
                        active_decoder = None;
                        // Flush ring buffer
                        should_flush.store(true, Ordering::SeqCst);
                        self.visualizer.clear();

                        match self.open_track(&track) {
                            Ok((reader, decoder, trk_id, sr, ch)) => {
                                active_format = Some(reader);
                                active_decoder = Some(decoder);
                                active_track_id = trk_id;
                                active_track_sample_rate = sr;
                                active_track_channels = ch;
                                resample_ratio = sr as f64 / sample_rate as f64;
                                sample_buf = None;

                                if let Ok(mut cur) = self.current_track.lock() {
                                    *cur = Some(track.clone());
                                }
                                self.total_duration_secs.store(track.duration_secs, Ordering::Relaxed);
                                self.elapsed_nanos.store(0, Ordering::Relaxed);

                                is_playing = true;
                                is_paused = false;
                                is_stream_paused.store(false, Ordering::Relaxed);
                                if let Ok(mut st) = self.state.lock() {
                                    *st = PlaybackState::Playing;
                                }
                            }
                            Err(e) => {
                                self.event_tx
                                    .send(PlayerEvent::Error(format!("Failed to open track: {}", e)))
                                    .ok();
                                is_playing = false;
                                if let Ok(mut st) = self.state.lock() {
                                    *st = PlaybackState::Stopped;
                                }
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
                            is_stream_paused.store(false, Ordering::Relaxed);
                            if let Ok(mut st) = self.state.lock() {
                                *st = PlaybackState::Playing;
                            }
                        }
                    }
                    PlayerCommand::TogglePause => {
                        if is_playing {
                            is_paused = !is_paused;
                            is_stream_paused.store(is_paused, Ordering::Relaxed);
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
                        active_format = None;
                        active_decoder = None;
                        is_playing = false;
                        is_paused = false;
                        is_stream_paused.store(true, Ordering::Relaxed);
                        should_flush.store(true, Ordering::SeqCst);
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
                                // Flush audio buffer
                                should_flush.store(true, Ordering::SeqCst);
                                self.visualizer.clear();
                                let actual_secs = seeked.actual_ts as f64 / active_track_sample_rate as f64;
                                self.elapsed_nanos.store((actual_secs * 1_000_000_000.0) as u64, Ordering::Relaxed);
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
                                should_flush.store(true, Ordering::SeqCst);
                                self.visualizer.clear();
                                let actual_secs = seeked.actual_ts as f64 / active_track_sample_rate as f64;
                                self.elapsed_nanos.store((actual_secs * 1_000_000_000.0) as u64, Ordering::Relaxed);
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

            // If the buffer already has enough audio queued (> 0.5s), sleep a bit to throttle CPU
            if audio_tx.len() > sample_rate as usize * channels / 2 {
                std::thread::sleep(Duration::from_millis(10));
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
                    // Track reached end
                    is_playing = false;
                    if let Ok(mut st) = self.state.lock() {
                        *st = PlaybackState::Stopped;
                    }
                    self.event_tx.send(PlayerEvent::TrackFinished).ok();
                    continue;
                }
                Err(e) => {
                    self.event_tx
                        .send(PlayerEvent::Error(format!("Decode packet error: {}", e)))
                        .ok();
                    is_playing = false;
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

            let mut visualizer_samples = Vec::new();

            // Simple resample & channel adapt to output device
            if (resample_ratio - 1.0).abs() < 0.001 && in_channels == channels {
                // Same sample rate and channels: direct push
                for &s in samples {
                    let out_s = s * current_vol;
                    audio_tx.send(out_s).ok();
                    visualizer_samples.push(s);
                }
            } else {
                // Adapt channels & sample rate
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
                        audio_tx.send(mono).ok();
                    } else {
                        audio_tx.send(out_left).ok();
                        audio_tx.send(out_right).ok();
                    }

                    visualizer_samples.push((s_left + s_right) * 0.5);
                }
            }

            if !visualizer_samples.is_empty() {
                self.visualizer.push_samples(&visualizer_samples);
            }

            // Advance elapsed time estimate
            let frames_decoded = (samples.len() / in_channels) as u64;
            let added_nanos = (frames_decoded as f64 / active_track_sample_rate as f64 * 1_000_000_000.0) as u64;
            self.elapsed_nanos.fetch_add(added_nanos, Ordering::Relaxed);
        }
    }

    fn open_track(
        &self,
        track: &Track,
    ) -> anyhow::Result<(
        Box<dyn FormatReader>,
        Box<dyn Decoder>,
        u32,
        u32,
        usize,
    )> {
        let file = File::open(&track.path)?;
        let mss = MediaSourceStream::new(Box::new(file), Default::default());

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

        Ok((format, decoder, trk_id, sample_rate, channels))
    }
}
