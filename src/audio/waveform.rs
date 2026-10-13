use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

pub const DEFAULT_WAVEFORM_BINS: usize = 512;

#[derive(Debug, Clone)]
pub struct WaveformData {
    pub path: PathBuf,
    pub peaks: Vec<f32>,
    pub is_complete: bool,
}

impl WaveformData {
    pub fn empty(path: PathBuf) -> Self {
        Self {
            path,
            peaks: vec![0.1; DEFAULT_WAVEFORM_BINS],
            is_complete: false,
        }
    }

    pub fn generate_fast_estimate(path: PathBuf, duration_secs: u64) -> Self {
        // Generate an initial aesthetic pseudo-waveform while accurate scan computes
        let mut peaks = Vec::with_capacity(DEFAULT_WAVEFORM_BINS);
        let n = DEFAULT_WAVEFORM_BINS as f32;
        let seed = duration_secs as f32;
        for i in 0..DEFAULT_WAVEFORM_BINS {
            let x = i as f32 / n;
            let shape = (x * std::f32::consts::PI).sin();
            let chorus1 = (x * 16.0 + seed * 0.1).sin().abs() * 0.25;
            let chorus2 = (x * 32.0 + seed * 0.3).cos().abs() * 0.15;
            let ripple = (x * 64.0 + seed * 0.7).sin().abs() * 0.1;
            let val = (shape * 0.6 + chorus1 + chorus2 + ripple + 0.1).powf(0.8).clamp(0.06, 0.98);
            peaks.push(val);
        }
        Self {
            path,
            peaks,
            is_complete: false,
        }
    }

    pub fn compute_accurate(
        path: &Path,
        num_bins: usize,
        cancel_token: Arc<AtomicBool>,
    ) -> Option<Vec<f32>> {
        let file = File::open(path).ok()?;
        let mss = MediaSourceStream::new(Box::new(file), Default::default());

        let mut hint = Hint::new();
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            hint.with_extension(ext);
        }

        let meta_opts: MetadataOptions = Default::default();
        let fmt_opts: FormatOptions = Default::default();
        let probed = symphonia::default::get_probe()
            .format(&hint, mss, &fmt_opts, &meta_opts)
            .ok()?;

        let mut format = probed.format;
        let track = format.default_track()?;
        let track_id = track.id;

        let dec_opts: DecoderOptions = Default::default();
        let mut decoder = symphonia::default::get_codecs()
            .make(&track.codec_params, &dec_opts)
            .ok()?;

        let mut all_chunk_peaks = Vec::new();
        let mut sample_buf: Option<SampleBuffer<f32>> = None;

        let mut packet_count = 0;
        while !cancel_token.load(Ordering::Relaxed) {
            let packet = match format.next_packet() {
                Ok(p) => p,
                Err(symphonia::core::errors::Error::IoError(err))
                    if err.kind() == std::io::ErrorKind::UnexpectedEof =>
                {
                    break;
                }
                Err(_) => break,
            };

            if packet.track_id() != track_id {
                continue;
            }

            packet_count += 1;
            // Downsample: sample every 2nd packet for high-fidelity fast scan
            if packet_count % 2 != 0 {
                continue;
            }

            match decoder.decode(&packet) {
                Ok(decoded) => {
                    if sample_buf.is_none() {
                        let spec = *decoded.spec();
                        let duration = decoded.capacity() as u64;
                        sample_buf = Some(SampleBuffer::new(duration, spec));
                    }

                    if let Some(buf) = sample_buf.as_mut() {
                        buf.copy_interleaved_ref(decoded);
                        let samples = buf.samples();
                        if !samples.is_empty() {
                            let mut max_abs = 0.0f32;
                            let mut sum_sq = 0.0f32;
                            for &s in samples {
                                let abs = s.abs();
                                if abs > max_abs {
                                    max_abs = abs;
                                }
                                sum_sq += abs * abs;
                            }
                            let rms = (sum_sq / samples.len() as f32).sqrt();
                            // Combined peak and RMS energy
                            let energy = max_abs * 0.7 + rms * 0.3;
                            all_chunk_peaks.push(energy);
                        }
                    }
                }
                Err(symphonia::core::errors::Error::DecodeError(_)) => continue,
                Err(_) => break,
            }
        }

        if cancel_token.load(Ordering::Relaxed) || all_chunk_peaks.is_empty() {
            return None;
        }

        // Resample all_chunk_peaks into num_bins
        let total_chunks = all_chunk_peaks.len();
        let mut final_bins = vec![0.0f32; num_bins];
        let chunks_per_bin = total_chunks as f32 / num_bins as f32;

        let mut global_max = 0.001f32;
        for i in 0..num_bins {
            let start = ((i as f32 * chunks_per_bin).floor() as usize).min(total_chunks - 1);
            let end = (((i + 1) as f32 * chunks_per_bin).ceil() as usize).max(start + 1).min(total_chunks);

            let mut peak = 0.0f32;
            for j in start..end {
                if all_chunk_peaks[j] > peak {
                    peak = all_chunk_peaks[j];
                }
            }
            final_bins[i] = peak;
            if peak > global_max {
                global_max = peak;
            }
        }

        // Perceptual dynamic normalization (logarithmic/power curve)
        for b in &mut final_bins {
            *b = ((*b / global_max).powf(0.75)).clamp(0.04, 1.0);
        }

        Some(final_bins)
    }
}
