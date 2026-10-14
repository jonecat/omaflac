use rustfft::num_complex::Complex;
use rustfft::FftPlanner;
use std::f32::consts::PI;
use std::sync::{Arc, Mutex};

pub const FFT_SIZE: usize = 2048;
pub const NUM_BANDS: usize = 64;

#[derive(Clone)]
pub struct VisualizerBridge {
    state: Arc<Mutex<VisualizerState>>,
}

struct VisualizerState {
    sample_buffer: Vec<f32>,
    write_pos: usize,
    sample_rate: u32,
    fps: u32,
    band_values: Vec<f32>,
    peak_values: Vec<f32>,
    fft: Arc<dyn rustfft::Fft<f32>>,
    complex_buffer: Vec<Complex<f32>>,
}

impl Default for VisualizerBridge {
    fn default() -> Self {
        Self::new(44100)
    }
}

impl VisualizerBridge {
    pub fn new(sample_rate: u32) -> Self {
        let mut planner = FftPlanner::new();
        let fft = planner.plan_fft_forward(FFT_SIZE);
        Self {
            state: Arc::new(Mutex::new(VisualizerState {
                sample_buffer: vec![0.0; FFT_SIZE],
                write_pos: 0,
                sample_rate: sample_rate.max(1),
                fps: 60,
                band_values: vec![0.0; NUM_BANDS],
                peak_values: vec![0.0; NUM_BANDS],
                fft,
                complex_buffer: vec![Complex::new(0.0, 0.0); FFT_SIZE],
            })),
        }
    }

    pub fn set_fps(&self, fps: u64) {
        if let Ok(mut state) = self.state.lock() {
            state.fps = (fps as u32).clamp(10, 240);
        }
    }

    pub fn set_sample_rate(&self, rate: u32) {
        if let Ok(mut state) = self.state.lock() {
            state.sample_rate = rate.max(1);
        }
    }

    pub fn push_samples(&self, samples: &[f32]) {
        if let Ok(mut state) = self.state.try_lock() {
            let buf_len = state.sample_buffer.len();
            for &s in samples {
                let pos = state.write_pos;
                state.sample_buffer[pos] = s;
                state.write_pos = (pos + 1) % buf_len;
            }
        }
    }

    pub fn clear(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.sample_buffer.fill(0.0);
            state.complex_buffer.fill(Complex::new(0.0, 0.0));
            state.band_values.fill(0.0);
            state.peak_values.fill(0.0);
        }
    }

    pub fn get_bands(&self) -> (Vec<f32>, Vec<f32>) {
        let mut state = match self.state.lock() {
            Ok(s) => s,
            Err(_) => return (vec![0.0; NUM_BANDS], vec![0.0; NUM_BANDS]),
        };

        // Extract ordered sample window with Hann windowing into preallocated complex buffer
        let write_pos = state.write_pos;
        for i in 0..FFT_SIZE {
            let idx = (write_pos + i) % FFT_SIZE;
            let sample = state.sample_buffer[idx];
            let hann = 0.5 * (1.0 - (2.0 * PI * i as f32 / (FFT_SIZE - 1) as f32).cos());
            state.complex_buffer[i] = Complex::new(sample * hann, 0.0);
        }

        // Perform in-place FFT using pre-planned transform
        let fft = Arc::clone(&state.fft);
        fft.process(&mut state.complex_buffer);

        let half_size = FFT_SIZE / 2;
        let sample_rate = state.sample_rate as f32;
        let bin_width = sample_rate / FFT_SIZE as f32;

        let min_freq = 40.0f32;
        let max_freq = (sample_rate / 2.0).min(14000.0);

        let mut raw_bands = [0.0f32; NUM_BANDS];
        let norm_factor = FFT_SIZE as f32 / 2.0;

        for band in 0..NUM_BANDS {
            let t0 = band as f32 / NUM_BANDS as f32;
            let t1 = (band + 1) as f32 / NUM_BANDS as f32;

            // Logarithmic frequency scale
            let f_low = min_freq * (max_freq / min_freq).powf(t0);
            let f_high = min_freq * (max_freq / min_freq).powf(t1);
            let freq_center = (f_low * f_high).sqrt();

            let exact_low = (f_low / bin_width).clamp(1.0, (half_size - 2) as f32);
            let exact_high = (f_high / bin_width).clamp(exact_low, (half_size - 1) as f32);

            let avg_mag = if exact_high - exact_low < 1.0 {
                // Smoothly interpolate between adjacent FFT bins at low frequencies
                let exact_center = (freq_center / bin_width).clamp(1.0, (half_size - 2) as f32);
                let idx0 = exact_center.floor() as usize;
                let idx1 = (idx0 + 1).min(half_size - 1);
                let frac = exact_center - idx0 as f32;
                let m0 = state.complex_buffer[idx0].norm() / norm_factor;
                let m1 = state.complex_buffer[idx1].norm() / norm_factor;
                m0 * (1.0 - frac) + m1 * frac
            } else {
                let bin_low = (exact_low.floor() as usize).clamp(1, half_size - 1);
                let bin_high = (exact_high.ceil() as usize).clamp(bin_low + 1, half_size);
                let mut sum_mag = 0.0f32;
                let mut max_mag = 0.0f32;
                let mut count = 0;
                for i in bin_low..bin_high {
                    let mag = state.complex_buffer[i].norm() / norm_factor;
                    sum_mag += mag;
                    if mag > max_mag {
                        max_mag = mag;
                    }
                    count += 1;
                }
                if count > 0 {
                    (sum_mag / count as f32) * 0.5 + max_mag * 0.5
                } else {
                    0.0
                }
            };

            // Equal loudness / treble boost curve
            let eq_boost = 1.0 + (freq_center / 500.0).log10().max(0.0) * 1.8;

            // Logarithmic dB compression
            let scaled = (avg_mag * 14.0 * eq_boost).powf(0.85).min(1.0);
            raw_bands[band] = scaled;
        }

        // Compute frame-rate delta scaling relative to 30 FPS baseline
        let fps = state.fps.max(10) as f32;
        let dt_ratio = 30.0 / fps;
        let attack_factor = (1.0 - (1.0 - 0.65f32).powf(dt_ratio)).clamp(0.01, 1.0);
        let decay_factor = (1.0 - (1.0 - 0.20f32).powf(dt_ratio)).clamp(0.01, 1.0);
        let peak_falloff = 0.02 * dt_ratio;

        // Decay & smoothing with hysteresis snap (eliminates limit-cycle jitter on static signals)
        for b in 0..NUM_BANDS {
            let target = raw_bands[b];
            let current = state.band_values[b];

            let diff = target - current;
            let next_val = if diff.abs() < 1e-4 {
                target
            } else if diff > 0.0 {
                current + diff * attack_factor
            } else {
                current + diff * decay_factor
            };
            let next_val = next_val.clamp(0.0, 1.0);
            state.band_values[b] = next_val;

            // Peak cap decay: smooth falloff and no bounce
            let peak = state.peak_values[b];
            let next_peak = if next_val >= peak || (peak - next_val).abs() < 1e-4 {
                next_val
            } else {
                (peak - peak_falloff).max(next_val)
            };
            state.peak_values[b] = next_peak.clamp(0.0, 1.0);
        }

        (state.band_values.clone(), state.peak_values.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_visualizer_static_pause_stability() {
        let viz = VisualizerBridge::new(44100);

        // Feed some test samples (e.g. 440 Hz tone)
        let mut samples = Vec::new();
        for i in 0..FFT_SIZE {
            let t = i as f32 / 44100.0;
            samples.push((2.0 * PI * 440.0 * t).sin() * 0.8);
        }
        viz.push_samples(&samples);

        // Run several frames to let the envelope follower rise to steady state
        for _ in 0..25 {
            viz.get_bands();
        }

        // Now pause audio (no more samples pushed)
        // Check that subsequent frames produce identical, jitter-free results
        let (first_bands, first_peaks) = viz.get_bands();
        for _ in 0..20 {
            let (next_bands, next_peaks) = viz.get_bands();
            for b in 0..NUM_BANDS {
                assert!(
                    (first_bands[b] - next_bands[b]).abs() < 1e-4,
                    "Band {} jittered: {} vs {}",
                    b,
                    first_bands[b],
                    next_bands[b]
                );
                assert!(
                    (first_peaks[b] - next_peaks[b]).abs() < 1e-4,
                    "Peak {} jittered: {} vs {}",
                    b,
                    first_peaks[b],
                    next_peaks[b]
                );
            }
        }
    }
}
