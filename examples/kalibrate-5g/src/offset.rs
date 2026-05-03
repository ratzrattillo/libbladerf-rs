use rustfft::FftPlanner;
use rustfft::num_complex::Complex32;

const OFFSET_FFT_SIZE: usize = 65_536;
const SNR_MIN_DB: f64 = 13.0;
const MAX_OFFSET_PPM: f64 = 200.0;
const DC_EXCLUSION_HZ: f64 = 250_000.0;

#[derive(Debug, Clone)]
pub struct OffsetResult {
    pub offset_hz: f64,
    pub snr_db: f64,
}

fn fftshift_inplace(buf: &mut [Complex32]) {
    let half = buf.len() / 2;
    let (left, right) = buf.split_at_mut(half);
    for i in 0..half {
        std::mem::swap(&mut left[i], &mut right[i]);
    }
}

pub fn measure_offset(
    samples: &[Complex32],
    sample_rate: f64,
    center_freq_hz: f64,
    ssb_offset_hz: f64,
) -> Option<OffsetResult> {
    if samples.len() < OFFSET_FFT_SIZE {
        return None;
    }

    let bin_width = sample_rate / OFFSET_FFT_SIZE as f64;
    let center_bin = OFFSET_FFT_SIZE / 2;

    let ssb_center_bin = (center_bin as f64 + ssb_offset_hz / bin_width).round() as usize;

    let max_offset_bins =
        (center_freq_hz * MAX_OFFSET_PPM / 1e6 / bin_width).ceil() as usize;
    let dc_excl_bins = (DC_EXCLUSION_HZ / bin_width).ceil() as usize;

    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(OFFSET_FFT_SIZE);

    let mut fft_buf = vec![Complex32::new(0.0, 0.0); OFFSET_FFT_SIZE];
    let take = samples.len().min(OFFSET_FFT_SIZE);
    fft_buf[..take].copy_from_slice(&samples[..take]);
    fft.process(&mut fft_buf);
    fftshift_inplace(&mut fft_buf);

    for b in center_bin.saturating_sub(dc_excl_bins)
        ..=(center_bin + dc_excl_bins).min(OFFSET_FFT_SIZE - 1)
    {
        fft_buf[b] = Complex32::new(0.0, 0.0);
    }

    let mut best_n_id_2: usize = 0;
    let mut best_shift: isize = 0;
    let mut best_corr_mag: f64 = 0.0;
    let mut corr_magnitudes: Vec<f64> = Vec::new();

    for n_id_2 in 0..crate::pss::NOF_PSS {
        let pss = crate::pss::generate_pss(n_id_2);

        for shift in -(max_offset_bins as isize)..=(max_offset_bins as isize) {
            let mut corr = Complex32::new(0.0, 0.0);
            let mut count: usize = 0;

            for sc in 0..crate::pss::PSS_LEN {
                let global_sc = sc as isize - crate::pss::PSS_LEN as isize / 2;
                let rx_bin = ssb_center_bin as isize + global_sc + shift;

                if rx_bin < 0 || rx_bin >= OFFSET_FFT_SIZE as isize {
                    continue;
                }
                let rx_bin = rx_bin as usize;

                if rx_bin >= center_bin.saturating_sub(dc_excl_bins)
                    && rx_bin <= (center_bin + dc_excl_bins).min(OFFSET_FFT_SIZE - 1)
                {
                    continue;
                }

                corr += fft_buf[rx_bin] * Complex32::new(pss[sc], 0.0).conj();
                count += 1;
            }

            let mag = corr.norm_sqr() as f64;
            if count > 0 {
                corr_magnitudes.push(mag);
            }

            if mag > best_corr_mag {
                best_corr_mag = mag;
                best_n_id_2 = n_id_2;
                best_shift = shift;
            }
        }
    }

    if corr_magnitudes.is_empty() || best_corr_mag <= 0.0 {
        return None;
    }

    corr_magnitudes.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median_corr = corr_magnitudes[corr_magnitudes.len() / 2];

    let snr_db = if median_corr > 0.0 {
        10.0 * (best_corr_mag / median_corr).log10()
    } else {
        f64::INFINITY
    };

    if snr_db < SNR_MIN_DB {
        return None;
    }

    let pss = crate::pss::generate_pss(best_n_id_2);

    let corr_at = |s: isize| -> f64 {
        let mut corr = Complex32::new(0.0, 0.0);
        for sc in 0..crate::pss::PSS_LEN {
            let global_sc = sc as isize - crate::pss::PSS_LEN as isize / 2;
            let rx_bin = ssb_center_bin as isize + global_sc + s;
            if rx_bin < 0 || rx_bin >= OFFSET_FFT_SIZE as isize {
                continue;
            }
            let rx_bin = rx_bin as usize;
            if rx_bin >= center_bin.saturating_sub(dc_excl_bins)
                && rx_bin <= (center_bin + dc_excl_bins).min(OFFSET_FFT_SIZE - 1)
            {
                continue;
            }
            corr += fft_buf[rx_bin] * Complex32::new(pss[sc], 0.0).conj();
        }
        corr.norm_sqr() as f64
    };

    let y_m1 = corr_at(best_shift - 1);
    let y_0 = corr_at(best_shift);
    let y_p1 = corr_at(best_shift + 1);

    let denom = y_m1 - 2.0 * y_0 + y_p1;
    let delta = if denom.abs() > 1e-20 {
        0.5 * (y_m1 - y_p1) / denom
    } else {
        0.0
    };

    let fine_shift = best_shift as f64 + delta.clamp(-0.5, 0.5);
    let refined_ssb_baseband = ssb_offset_hz + fine_shift * bin_width;

    Some(OffsetResult {
        offset_hz: refined_ssb_baseband,
        snr_db,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const TUNING_OFFSET_HZ: f64 = 1_500_000.0;

    fn make_noise(n: usize, amplitude: f32, seed: u32) -> Vec<Complex32> {
        let mut state = seed;
        let mut rng = || {
            state = state.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            ((state >> 16) as f32 / 32_768.0 - 1.0) * amplitude
        };
        (0..n).map(|_| Complex32::new(rng(), rng())).collect()
    }

    fn make_pss_ssb_signal(
        center_freq_hz: f64,
        sample_rate: f64,
        n_samples: usize,
        amplitude: f32,
    ) -> Vec<Complex32> {
        let pss_values = crate::pss::generate_pss(0);
        let mut planner = FftPlanner::new();
        let bin_width = sample_rate / OFFSET_FFT_SIZE as f64;
        let center_bin = OFFSET_FFT_SIZE / 2;

        let ssb_center_shifted =
            (center_bin as f64 + center_freq_hz / bin_width).round() as usize;
        let ssb_center_unshifted = (ssb_center_shifted + center_bin) % OFFSET_FFT_SIZE;

        let mut spectrum = vec![Complex32::new(0.0, 0.0); OFFSET_FFT_SIZE];
        for sc in 0..127 {
            let global_sc = sc as isize - 63;
            let bin_idx = (ssb_center_unshifted as isize + global_sc).rem_euclid(OFFSET_FFT_SIZE as isize)
                as usize;
            spectrum[bin_idx] = Complex32::new(pss_values[sc] * amplitude, 0.0);
        }

        let ifft = planner.plan_fft_inverse(OFFSET_FFT_SIZE);
        ifft.process(&mut spectrum);
        let scale = 1.0 / OFFSET_FFT_SIZE as f32;

        let mut result = Vec::with_capacity(n_samples);
        for i in 0..n_samples {
            result.push(spectrum[i % OFFSET_FFT_SIZE] * scale);
        }
        result
    }

    #[test]
    fn test_detects_ssb_offset() {
        let sample_rate = 40_000_000.0;
        let freq_error = 5_432.0;
        let ssb_baseband = -TUNING_OFFSET_HZ + freq_error;
        let ssb = make_pss_ssb_signal(ssb_baseband, sample_rate, 200_000, 10.0);
        let noise = make_noise(ssb.len(), 0.01, 99);
        let samples: Vec<Complex32> = ssb.iter().zip(noise.iter()).map(|(s, n)| s + n).collect();

        let result = measure_offset(&samples, sample_rate, 2_100_000_000.0, ssb_baseband);
        assert!(result.is_some(), "Should detect SSB with PSS");
        let r = result.unwrap();
        let error = (r.offset_hz - ssb_baseband).abs();
        assert!(
            error < 2_000.0,
            "Offset should be near {ssb_baseband} Hz, got {} Hz (error {error} Hz)",
            r.offset_hz
        );
    }

    #[test]
    fn test_noise_no_detection() {
        let sample_rate = 40_000_000.0;
        let noise = make_noise(200_000, 0.1, 42);
        let result = measure_offset(&noise, sample_rate, 2_100_000_000.0, -TUNING_OFFSET_HZ);
        assert!(
            result.is_none(),
            "Noise-only should not produce offset measurement"
        );
    }

    #[test]
    fn test_negative_offset() {
        let sample_rate = 40_000_000.0;
        let freq_error = -7890.0;
        let ssb_baseband = -TUNING_OFFSET_HZ + freq_error;
        let ssb = make_pss_ssb_signal(ssb_baseband, sample_rate, 200_000, 10.0);
        let noise = make_noise(ssb.len(), 0.01, 99);
        let samples: Vec<Complex32> = ssb.iter().zip(noise.iter()).map(|(s, n)| s + n).collect();

        let result = measure_offset(&samples, sample_rate, 2_100_000_000.0, ssb_baseband);
        assert!(result.is_some(), "Should detect SSB with PSS");
        let r = result.unwrap();
        let error = (r.offset_hz - ssb_baseband).abs();
        assert!(
            error < 2_000.0,
            "Offset should be near {ssb_baseband} Hz, got {} Hz (error {error} Hz)",
            r.offset_hz
        );
    }

    #[test]
    fn test_snr_reported() {
        let sample_rate = 40_000_000.0;
        let ssb_baseband = -TUNING_OFFSET_HZ + 100_000.0;
        let ssb = make_pss_ssb_signal(ssb_baseband, sample_rate, 200_000, 10.0);
        let noise = make_noise(ssb.len(), 0.01, 99);
        let samples: Vec<Complex32> = ssb.iter().zip(noise.iter()).map(|(s, n)| s + n).collect();

        let result = measure_offset(&samples, sample_rate, 2_100_000_000.0, ssb_baseband);
        assert!(result.is_some());
        let r = result.unwrap();
        assert!(
            r.snr_db > SNR_MIN_DB,
            "SNR should be >{SNR_MIN_DB} dB, got {:.1}",
            r.snr_db
        );
    }

    #[test]
    fn test_dc_spike_not_detected() {
        let sample_rate = 40_000_000.0;
        let mut samples = make_noise(200_000, 0.1, 42);
        for s in samples.iter_mut() {
            *s += Complex32::new(50.0, 0.0);
        }
        let result = measure_offset(&samples, sample_rate, 2_100_000_000.0, -TUNING_OFFSET_HZ);
        assert!(
            result.is_none(),
            "DC spike should not produce offset measurement"
        );
    }
}
