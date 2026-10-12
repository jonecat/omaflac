use rustfft::num_complex::Complex;
use rustfft::FftPlanner;
use std::f32::consts::PI;
use std::sync::{Arc, Mutex};

pub const FFT_SIZE: usize = 1024;
pub const NUM_BANDS: usize = 48;

#[derive(Clone)]
pub struct VisualizerBridge {
    state: Arc<Mutex<VisualizerState>>,
}

struct VisualizerState {
    sample_buffer: Vec<f32>,
    write_pos: usize,
    sample_rate: u32,
    band_values: Vec<f32>,
    peak_values: Vec<f32>,
}

impl Default for VisualizerBridge {
    fn default() -> Self {
        Self::new(44100)
    }
}

impl VisualizerBridge {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            state: Arc::new(Mutex::new(VisualizerState {
                sample_buffer: vec![0.0; FFT_SIZE],
                write_pos: 0,
                sample_rate: sample_rate.max(1),
                band_values: vec![0.0; NUM_BANDS],
                peak_values: vec![0.0; NUM_BANDS],
            })),
        }
    }

    pub fn set_sample_rate(&self, rate: u32) {
        if let Ok(mut state) = self.state.lock() {
            state.sample_rate = rate.max(1);
        }
    }

    pub fn push_samples(&self, samples: &[f32]) {
        if let Ok(mut state) = self.state.lock() {
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
            state.band_values.fill(0.0);
            state.peak_values.fill(0.0);
        }
    }

    pub fn get_bands(&self) -> (Vec<f32>, Vec<f32>) {
        let mut state = match self.state.lock() {
            Ok(s) => s,
            Err(_) => return (vec![0.0; NUM_BANDS], vec![0.0; NUM_BANDS]),
        };

        // Extract ordered sample window
        let mut window = vec![0.0f32; FFT_SIZE];
        let write_pos = state.write_pos;
        for i in 0..FFT_SIZE {
            let idx = (write_pos + i) % FFT_SIZE;
            let sample = state.sample_buffer[idx];
            // Hann windowing
            let hann = 0.5 * (1.0 - (2.0 * PI * i as f32 / (FFT_SIZE - 1) as f32).cos());
            window[i] = sample * hann;
        }

        // Perform FFT
        let mut planner = FftPlanner::new();
        let fft = planner.plan_fft_forward(FFT_SIZE);
        let mut complex_buffer: Vec<Complex<f32>> = window.iter().map(|&s| Complex::new(s, 0.0)).collect();
        fft.process(&mut complex_buffer);

        let half_size = FFT_SIZE / 2;
        let sample_rate = state.sample_rate as f32;
        let bin_width = sample_rate / FFT_SIZE as f32;

        let min_freq = 30.0f32;
        let max_freq = (sample_rate / 2.0).min(18000.0);

        let mut raw_bands = vec![0.0f32; NUM_BANDS];

        for band in 0..NUM_BANDS {
            let t0 = band as f32 / NUM_BANDS as f32;
            let t1 = (band + 1) as f32 / NUM_BANDS as f32;

            // Logarithmic frequency scale
            let f_low = min_freq * (max_freq / min_freq).powf(t0);
            let f_high = min_freq * (max_freq / min_freq).powf(t1);

            let bin_low = ((f_low / bin_width).floor() as usize).clamp(0, half_size - 1);
            let bin_high = ((f_high / bin_width).ceil() as usize).clamp(bin_low + 1, half_size);

            let mut sum_mag = 0.0f32;
            let mut count = 0;
            for i in bin_low..bin_high {
                let mag = complex_buffer[i].norm() / (FFT_SIZE as f32 / 2.0);
                sum_mag += mag;
                count += 1;
            }

            let avg_mag = if count > 0 { sum_mag / count as f32 } else { 0.0 };

            // Equal loudness / treble boost curve (high frequencies naturally have lower amplitude)
            let freq_center = (f_low * f_high).sqrt();
            let eq_boost = 1.0 + (freq_center / 1000.0).log10().max(0.0) * 1.5;

            // Logarithmic dB compression
            let scaled = (avg_mag * 12.0 * eq_boost).min(1.0);
            raw_bands[band] = scaled;
        }

        // Decay & smoothing
        for b in 0..NUM_BANDS {
            let target = raw_bands[b];
            let current = state.band_values[b];

            // Smooth attack and decay
            let next_val = if target > current {
                current + (target - current) * 0.75
            } else {
                current * 0.82
            };
            state.band_values[b] = next_val.max(0.0);

            // Peak cap decay
            let peak = state.peak_values[b];
            if next_val >= peak {
                state.peak_values[b] = next_val;
            } else {
                state.peak_values[b] = (peak - 0.03).max(0.0);
            }
        }

        (state.band_values.clone(), state.peak_values.clone())
    }
}
