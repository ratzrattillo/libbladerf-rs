use rustfft::FftPlanner;
use rustfft::num_complex::Complex32;

/// GSM symbol rate: 1_625_000 / 6 = 270_833.333... Hz
pub const GSM_RATE: f64 = 1_625_000.0 / 6.0;

/// FFT size for frequency detection
const FFT_SIZE: usize = 1_024;

/// Minimum peak-to-mean ratio to confirm a pure tone
const MIN_PM: f32 = 50.0;

/// FCCH detector using an LMS adaptive filter + FFT frequency estimation.
///
/// Based on: Varma, Sahu, and Charan, "Robust Frequency Burst Detection
/// Algorithm for GSM/GPRS."
pub struct FcchDetector {
    /// Filter tap weights (17 taps)
    w: Vec<Complex32>,
    /// Filter length = 2 * filter_delay + 1 = 17
    w_len: usize,
    /// Prediction delay D = 8
    d: usize,
    /// Error smoothing factor p = 1/32
    p: f32,
    /// LMS step-size G = 1/12.5 (adaptively adjusted)
    g: f32,
    /// Running error estimate (exponential moving average)
    e: f32,
    /// Expected FCCH burst length in samples
    fcch_burst_len: usize,
    /// Sample rate
    sample_rate: f32,
    /// FFT (shared via Arc — rustfft returns Arc<dyn Fft>)
    fft: std::sync::Arc<dyn rustfft::Fft<f32>>,
}

impl FcchDetector {
    pub fn new(sample_rate: f32) -> Self {
        let filter_delay = 8usize;
        let w_len = 2 * filter_delay + 1; // 17
        let d = 8;
        let p = 1.0 / 32.0;
        let g = 1.0 / 12.5;
        let fcch_burst_len = (148.0 * (sample_rate as f64 / GSM_RATE)) as usize;

        let mut planner = FftPlanner::new();
        let fft = planner.plan_fft_forward(FFT_SIZE);

        Self {
            w: vec![Complex32::new(0.0, 0.0); w_len],
            w_len,
            d,
            p,
            g,
            e: 0.0,
            fcch_burst_len,
            sample_rate,
            fft,
        }
    }

    /// Process one sample through the LMS adaptive filter.
    /// Returns the normalized error ratio, or `None` if not enough samples yet.
    ///
    /// `x` is the full input buffer; `n` is the current write position.
    fn next_norm_error(&mut self, x: &[Complex32], n: usize) -> Option<f32> {
        // Need at least w_len samples, and D more for the desired signal
        if n + 1 < self.w_len || n + self.d + 1 > x.len() {
            return None;
        }

        let start = n + 1 - self.w_len;

        // Compute input power
        let e_val: f32 = x[start..=n].iter().map(|s| s.norm_sqr()).sum();

        // Normalized LMS: clamp step size for stability
        if self.g >= 2.0 / e_val {
            self.g = 1.0 / e_val;
        }

        // Compute filter output: y = sum(conj(w[i]) * x[n - i])
        let mut y = Complex32::new(0.0, 0.0);
        for i in 0..self.w_len {
            y += self.w[i].conj() * x[n - i];
        }

        // Error: desired (x[n + D]) minus predicted (y)
        let desired = x[n + self.d];
        let err = desired - y;

        // Update filter weights: w[i] += G * conj(e) * x[n - i]
        for i in 0..self.w_len {
            self.w[i] += self.g * err.conj() * x[n - i];
        }

        // Update error power (exponential moving average)
        let e_avg = e_val / self.w_len as f32;
        self.e = (1.0 - self.p) * self.e + self.p * err.norm_sqr();

        Some(self.e / e_avg)
    }

    /// Scan a buffer of samples for an FCCH burst.
    ///
    /// Returns the detected frequency offset in Hz if a valid FCCH burst is found,
    /// or `None` if no burst is detected.
    pub fn scan(&mut self, samples: &[Complex32]) -> Option<f32> {
        let sps = self.sample_rate as f64 / GSM_RATE;
        let min_fb_len = (100.0 * sps) as usize;

        // Phase 1: Feed all samples through LMS filter, collect error values
        let mut errors: Vec<f32> = Vec::with_capacity(samples.len());
        let mut sum: f64 = 0.0;

        // We need w_len + D samples before the first error output
        for n in 0..samples.len() {
            if let Some(e) = self.next_norm_error(samples, n) {
                errors.push(e);
                sum += e as f64;
            }
        }

        if errors.is_empty() {
            return None;
        }

        // Phase 2: Compute threshold (70% of average error)
        let avg = sum / errors.len() as f64;
        let limit = 0.7 * avg as f32;

        // Phase 3: Find low-error neighborhoods (state machine)
        let mut state = BurstState::High;
        let mut count: usize = 0;

        for (i, &e) in errors.iter().enumerate() {
            let transition_count = state.transition(e, limit, &mut count);

            if transition_count >= min_fb_len {
                // Found a low-error region long enough for an FCCH burst
                let y_offset = i + 1 - transition_count;
                let y_len = transition_count.min(self.fcch_burst_len);
                let y_start = y_offset + (self.w_len - 1 + self.d);
                let y_end = y_start + y_len;

                if y_end <= samples.len() {
                    let (freq, pm) = self.freq_detect(&samples[y_start..y_end]);
                    if pm > MIN_PM {
                        return Some(freq);
                    }
                }
            }
        }

        None
    }

    /// Detect the frequency of a pure tone using FFT + sinc-kernel interpolation.
    /// Returns (frequency_hz, peak_to_mean_ratio).
    fn freq_detect(&mut self, s: &[Complex32]) -> (f32, f32) {
        let len = s.len().min(FFT_SIZE);

        // Prepare FFT input (zero-padded)
        let mut fft_input: Vec<Complex32> = vec![Complex32::new(0.0, 0.0); FFT_SIZE];
        fft_input[..len].copy_from_slice(&s[..len]);

        // Execute FFT
        self.fft.process(&mut fft_input);

        // Peak detection with sub-bin interpolation
        let (max_i, peak, avg_power) = peak_detect(&fft_input);

        let pm = peak.norm_sqr() / avg_power;
        let freq = max_i * (self.sample_rate / FFT_SIZE as f32);

        (freq, pm)
    }
}

/// State machine for detecting low-error regions (FCCH bursts).
#[derive(PartialEq)]
enum BurstState {
    Low,
    High,
}

impl BurstState {
    /// Process one error value. Returns the length of a LOW-run when
    /// transitioning from LOW to HIGH, or 0 otherwise.
    fn transition(&mut self, e: f32, threshold: f32, count: &mut usize) -> usize {
        if e > threshold {
            if *self == Self::Low {
                let r = *count;
                *self = Self::High;
                *count = 1;
                return r;
            }
            *count += 1;
        } else {
            if *self == Self::High {
                *self = Self::Low;
                *count = 1;
            } else {
                *count += 1;
            }
        }
        0
    }
}

/// Find the peak bin with sub-bin interpolation using sinc reconstruction.
/// Returns (fractional_bin_index, peak_value, average_power_excluding_peak).
fn peak_detect(fft: &[Complex32]) -> (f32, Complex32, f32) {
    let s_len = fft.len();

    // Coarse peak: find bin with maximum magnitude squared
    let (max_i, _max_power) = fft
        .iter()
        .enumerate()
        .map(|(i, s)| (i, s.norm_sqr()))
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
        .unwrap();

    let sum_power: f32 = fft.iter().map(|s| s.norm_sqr()).sum();

    // Fine peak: binary interpolation using sinc reconstruction
    let mut early_i = if max_i >= 1 { max_i as f32 - 1.0 } else { 0.0 };
    let mut late_i = if max_i + 1 < s_len {
        max_i as f32 + 1.0
    } else {
        (s_len - 1) as f32
    };

    let mut incr: f32 = 0.5;
    while incr > 1.0 / 1_024.0 {
        let early_p = interpolate_point(fft, early_i);
        let late_p = interpolate_point(fft, late_i);
        if early_p.norm_sqr() < late_p.norm_sqr() {
            early_i += incr;
        } else if early_p.norm_sqr() > late_p.norm_sqr() {
            early_i -= incr;
        } else {
            break;
        }
        incr /= 2.0;
        late_i = early_i + 2.0;
    }

    let max_i_fine = early_i + 1.0;
    let peak = interpolate_point(fft, max_i_fine);
    let avg_power = (sum_power - peak.norm_sqr()) / (s_len - 1) as f32;

    (max_i_fine, peak, avg_power)
}

/// Sinc-kernel interpolation at a fractional index.
/// Uses a 21-tap sinc kernel for reconstruction.
fn interpolate_point(s: &[Complex32], s_i: f32) -> Complex32 {
    const FILTER_LEN: usize = 21;
    let half = (FILTER_LEN - 1) as i32 / 2; // 10

    let start = ((s_i.floor() as i32) - half).max(0) as usize;
    let end = ((s_i.floor() as i32) + half + 1).min(s.len() as i32 - 1) as usize;

    (start..=end)
        .map(|i| {
            let x = std::f32::consts::PI * (i as f32 - s_i);
            s[i] * sinc(x)
        })
        .sum()
}

/// sinc(x) = sin(x)/x, or 1.0 if |x| is very small.
fn sinc(x: f32) -> f32 {
    if x.abs() >= 0.1 { x.sin() / x } else { 1.0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sinc() {
        assert!((sinc(0.0) - 1.0).abs() < 1e-6);
        assert!((sinc(std::f32::consts::PI) - 0.0).abs() < 1e-5);
    }

    #[test]
    fn test_peak_detect_single_tone() {
        // Create a frequency-domain impulse at bin 100 (simulate FFT output)
        // Add small noise floor so avg_power is non-zero
        let mut fft_output = vec![Complex32::new(0.0, 0.0); FFT_SIZE];
        fft_output[100] = Complex32::new(100.0, 50.0);
        for i in 0..FFT_SIZE {
            if i != 100 {
                fft_output[i] = Complex32::new(0.01, 0.01);
            }
        }

        let (max_i, peak, avg_power) = peak_detect(&fft_output);
        let pm = peak.norm_sqr() / avg_power;
        assert!((max_i - 100.0).abs() < 0.5, "Expected ~100, got {max_i}");
        assert!(pm > 10.0, "Peak-to-mean ratio too low: {pm}");
    }

    #[test]
    fn test_fcch_scan_detects_tone() {
        let sample_rate = (4.0 * 13e6 / 48.0) as f32; // ~1_083_333 Hz
        let mut detector = FcchDetector::new(sample_rate);

        let sps = sample_rate as f64 / GSM_RATE;
        let fcch_freq = GSM_RATE / 4.0; // ~67_708 Hz
        let s_len = ((12.0 * 8.0 * 156.25 + 156.25) * sps) as usize;

        // Generate a signal with an FCCH-like burst (pure tone) embedded in noise
        let mut samples = Vec::with_capacity(s_len);
        let burst_start = s_len / 4;
        let burst_len = (148.0 * sps) as usize;

        // Use a simple RNG for reproducible noise
        let mut rng_state: u32 = 42;
        let mut noise = || {
            rng_state = rng_state.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            ((rng_state >> 16) as f32 / 32_768.0 - 1.0) * 0.01
        };

        for i in 0..s_len {
            let is_burst = i >= burst_start && i < burst_start + burst_len;
            let phase = 2.0 * std::f32::consts::PI * fcch_freq as f32 * i as f32 / sample_rate;
            let signal = if is_burst {
                Complex32::new(phase.cos(), phase.sin()) * 1.0
            } else {
                Complex32::new(0.0, 0.0)
            };
            let noise_c = Complex32::new(noise(), noise());
            samples.push(signal + noise_c);
        }

        let result = detector.scan(&samples);
        assert!(result.is_some(), "FCCH detector should find the tone");
    }
}
