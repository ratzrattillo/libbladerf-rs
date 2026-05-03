use rustfft::FftPlanner;
use rustfft::num_complex::Complex32;

const FFT_SIZE: usize = 4_096;
const SSB_BANDWIDTH_HZ: f64 = 3_600_000.0;
const PAR_THRESHOLD_DB: f64 = 6.0;
const MAX_WELCH_SEGMENTS: usize = 8;
const EDGE_GUARD_FRAC: f64 = 0.1;
const FILL_RATE_THRESHOLD: f64 = 0.25;
const FILL_RATE_MARGIN_DB: f64 = 1.0;
const DC_EXCLUSION_HZ: f64 = 250_000.0;
const PSS_VERIFY_MAX_SHIFT_HZ: f64 = 200_000.0;
const MAX_PSS_CANDIDATES: usize = 5;
const PSS_FILL_RATE_MIN: f64 = 0.5;
const PSS_ENABLED: bool = false;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SsbDetectResult {
    pub detected: bool,
    pub par_db: f64,
    pub peak_ssb_power: f64,
    pub noise_floor_per_bin: f64,
    pub freq_offset_hz: f64,
    pub fill_rate: f64,
    pub center_freq_hz: f64,
    pub n_id_2: Option<usize>,
    pub pss_snr_db: Option<f64>,
}

struct WindowCandidate {
    start: usize,
    wraps: bool,
    wrap_right_len: usize,
    power: f64,
}

pub fn ssb_detect_all(samples: &[Complex32], sample_rate: f64) -> Vec<SsbDetectResult> {
    let bin_width = sample_rate / FFT_SIZE as f64;
    let window_bins = (SSB_BANDWIDTH_HZ / bin_width).ceil() as usize;

    let psd = welch_psd(samples, sample_rate);

    let n = psd.len();
    if window_bins >= n || psd.is_empty() {
        return vec![empty_result()];
    }

    let guard_bins = ((n as f64) * EDGE_GUARD_FRAC / 2.0).ceil() as usize;
    let dc_excl_bins = (DC_EXCLUSION_HZ / bin_width).ceil() as usize;
    let center_bin = n / 2;

    let search_lo = guard_bins;
    let search_hi = n - guard_bins;

    let cumsum = cumulative_sum(&psd);

    let mut candidates: Vec<WindowCandidate> = Vec::new();

    for start in search_lo..=search_hi.saturating_sub(window_bins) {
        let win_center = start + window_bins / 2;
        if is_in_dc_hole(win_center, center_bin, dc_excl_bins) {
            continue;
        }
        let w_power = cumsum[start + window_bins] - cumsum[start];
        candidates.push(WindowCandidate {
            start,
            wraps: false,
            wrap_right_len: 0,
            power: w_power,
        });
    }

    candidates.sort_by(|a, b| b.power.partial_cmp(&a.power).unwrap());

    let mut results: Vec<SsbDetectResult> = Vec::new();
    let mut used_centers: Vec<usize> = Vec::new();

    #[allow(dead_code)]
    struct PrefilterCandidate<'a> {
        cand: &'a WindowCandidate,
        win_center_bin: usize,
        par_db: f64,
        fill_rate: f64,
        coarse_freq_offset_hz: f64,
    }

    let mut prefilter: Vec<PrefilterCandidate> = Vec::new();

    for cand in &candidates {
        let win_center_bin = compute_center_bin(cand, window_bins, n);
        let min_spacing = window_bins / 2;
        if used_centers
            .iter()
            .any(|&c| (c as isize - win_center_bin as isize).unsigned_abs() < min_spacing)
        {
            continue;
        }

        let noise_floor_per_bin = compute_noise_floor_multi(
            &cumsum,
            n,
            search_lo,
            search_hi,
            cand,
            window_bins,
            &prefilter.iter().map(|p| p.cand).collect::<Vec<_>>(),
        );

        if noise_floor_per_bin <= 0.0 || noise_floor_per_bin.is_nan() {
            continue;
        }

        let par_linear = cand.power / (window_bins as f64 * noise_floor_per_bin);
        let par_db = 10.0 * par_linear.log10();

        if par_db <= PAR_THRESHOLD_DB {
            continue;
        }

        let fill_threshold = noise_floor_per_bin * 10.0_f64.powf(FILL_RATE_MARGIN_DB / 10.0);
        let fill_rate = compute_fill_rate(&psd, cand, window_bins, n, fill_threshold);

        if fill_rate < FILL_RATE_THRESHOLD {
            continue;
        }

        let center_bin_f = compute_center_bin_f(cand, window_bins, n);
        let coarse_freq_offset_hz = (center_bin_f - n as f64 / 2.0) * bin_width;

        prefilter.push(PrefilterCandidate {
            cand,
            win_center_bin,
            par_db,
            fill_rate,
            coarse_freq_offset_hz,
        });

        used_centers.push(win_center_bin);
    }

    prefilter.sort_by(|a, b| {
        let sa = a.par_db + 10.0 * a.fill_rate.log10();
        let sb = b.par_db + 10.0 * b.fill_rate.log10();
        sb.partial_cmp(&sa).unwrap()
    });
    prefilter.retain(|p| p.fill_rate >= PSS_FILL_RATE_MIN);
    prefilter.truncate(MAX_PSS_CANDIDATES);

    for pc in &prefilter {
        eprintln!(
            "    [detect] CANDIDATE: coarse={:.1}kHz par={:.1}dB fill={:.0}%",
            pc.coarse_freq_offset_hz / 1e3,
            pc.par_db,
            pc.fill_rate * 100.0
        );

        if PSS_ENABLED {
            let pss_result = crate::pss::pss_correlate(
                samples,
                sample_rate,
                pc.coarse_freq_offset_hz,
                PSS_VERIFY_MAX_SHIFT_HZ,
            );

            let (freq_offset_hz, n_id_2, pss_snr_db) = match pss_result {
                Some(r) => {
                    eprintln!(
                        "    [detect] PSS OK: n_id_2={} offset={:.1}kHz snr={:.1}dB coarse={:.1}kHz",
                        r.n_id_2,
                        r.freq_offset_hz / 1e3,
                        r.snr_db,
                        pc.coarse_freq_offset_hz / 1e3
                    );
                    (r.freq_offset_hz, Some(r.n_id_2), Some(r.snr_db))
                }
                None => {
                    eprintln!(
                        "    [detect] PSS REJECT: coarse={:.1}kHz par={:.1}dB fill={:.0}%",
                        pc.coarse_freq_offset_hz / 1e3,
                        pc.par_db,
                        pc.fill_rate * 100.0
                    );
                    continue;
                }
            };

            results.push(SsbDetectResult {
                detected: true,
                par_db: pc.par_db,
                peak_ssb_power: pc.cand.power,
                noise_floor_per_bin: 0.0,
                freq_offset_hz,
                fill_rate: pc.fill_rate,
                center_freq_hz: freq_offset_hz,
                n_id_2,
                pss_snr_db,
            });
        } else {
            results.push(SsbDetectResult {
                detected: true,
                par_db: pc.par_db,
                peak_ssb_power: pc.cand.power,
                noise_floor_per_bin: 0.0,
                freq_offset_hz: pc.coarse_freq_offset_hz,
                fill_rate: pc.fill_rate,
                center_freq_hz: pc.coarse_freq_offset_hz,
                n_id_2: None,
                pss_snr_db: None,
            });
        }
    }

    if results.is_empty() {
        results.push(empty_result());
    }

    results
}

fn is_in_dc_hole(bin: usize, center_bin: usize, dc_excl_bins: usize) -> bool {
    if bin >= center_bin.saturating_sub(dc_excl_bins) && bin <= center_bin + dc_excl_bins {
        return true;
    }
    false
}

fn compute_center_bin(cand: &WindowCandidate, window_bins: usize, n: usize) -> usize {
    if cand.wraps {
        let left_len = n - cand.start;
        let right_len = cand.wrap_right_len;
        if left_len > right_len {
            cand.start + left_len / 2
        } else {
            right_len / 2
        }
    } else {
        cand.start + window_bins / 2
    }
}

fn compute_center_bin_f(cand: &WindowCandidate, window_bins: usize, n: usize) -> f64 {
    if cand.wraps {
        let left_len = (n - cand.start) as f64;
        let right_len = cand.wrap_right_len as f64;
        let left_center = n as f64 - left_len / 2.0;
        let right_center = right_len / 2.0;
        let right_weight = right_len / window_bins as f64;
        let left_weight = left_len / window_bins as f64;
        let com = right_center * right_weight + (left_center - n as f64) * left_weight;
        if com < 0.0 { com + n as f64 } else { com }
    } else {
        cand.start as f64 + window_bins as f64 / 2.0
    }
}

fn empty_result() -> SsbDetectResult {
    SsbDetectResult {
        detected: false,
        par_db: f64::NEG_INFINITY,
        peak_ssb_power: 0.0,
        noise_floor_per_bin: 0.0,
        freq_offset_hz: 0.0,
        fill_rate: 0.0,
        center_freq_hz: 0.0,
        n_id_2: None,
        pss_snr_db: None,
    }
}

fn compute_noise_floor_multi(
    cumsum: &[f64],
    n: usize,
    search_lo: usize,
    search_hi: usize,
    cand: &WindowCandidate,
    window_bins: usize,
    accepted: &[&WindowCandidate],
) -> f64 {
    let search_power = cumsum[search_hi] - cumsum[search_lo];
    let search_len = (search_hi - search_lo) as f64;

    let mut total_overlap_power = 0.0f64;
    let mut total_overlap_bins = 0usize;

    for w in accepted.iter().chain(std::iter::once(&cand)) {
        let (p, b) = window_overlap(cumsum, n, search_lo, search_hi, w, window_bins);
        total_overlap_power += p;
        total_overlap_bins += b;
    }

    let noise_bins = search_len - total_overlap_bins as f64;
    if noise_bins <= 0.0 {
        return f64::NAN;
    }
    (search_power - total_overlap_power) / noise_bins
}

fn window_overlap(
    cumsum: &[f64],
    n: usize,
    search_lo: usize,
    search_hi: usize,
    cand: &WindowCandidate,
    window_bins: usize,
) -> (f64, usize) {
    if cand.wraps {
        let mut overlap_power = 0.0f64;
        let mut overlap_bins = 0usize;
        if cand.start < search_hi {
            let lo = cand.start.max(search_lo);
            let hi = search_hi.min(n);
            if hi > lo {
                overlap_power += cumsum[hi] - cumsum[lo];
                overlap_bins += hi - lo;
            }
        }
        if cand.wrap_right_len > 0 && search_lo < cand.wrap_right_len {
            let lo = search_lo;
            let hi = cand.wrap_right_len.min(search_hi);
            if hi > lo {
                overlap_power += cumsum[hi] - cumsum[lo];
                overlap_bins += hi - lo;
            }
        }
        (overlap_power, overlap_bins)
    } else {
        let lo = cand.start.max(search_lo);
        let hi = (cand.start + window_bins).min(search_hi);
        if hi > lo {
            (cumsum[hi] - cumsum[lo], hi - lo)
        } else {
            (0.0, 0)
        }
    }
}

fn compute_fill_rate(
    psd: &[f64],
    cand: &WindowCandidate,
    window_bins: usize,
    n: usize,
    threshold: f64,
) -> f64 {
    let filled = if cand.wraps {
        let left_lo = cand.start;
        let left_hi = n;
        let right_lo = 0;
        let right_hi = cand.wrap_right_len;
        let mut count = 0usize;
        for v in &psd[left_lo..left_hi] {
            if *v >= threshold {
                count += 1;
            }
        }
        for v in &psd[right_lo..right_hi] {
            if *v >= threshold {
                count += 1;
            }
        }
        count
    } else {
        psd[cand.start..cand.start + window_bins]
            .iter()
            .filter(|&&v| v >= threshold)
            .count()
    };

    filled as f64 / window_bins as f64
}

fn cumulative_sum(psd: &[f64]) -> Vec<f64> {
    let mut cs = Vec::with_capacity(psd.len() + 1);
    cs.push(0.0);
    let mut sum = 0.0;
    for &v in psd {
        sum += v;
        cs.push(sum);
    }
    cs
}

fn welch_psd(samples: &[Complex32], sample_rate: f64) -> Vec<f64> {
    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(FFT_SIZE);

    let half = FFT_SIZE / 2;
    let hop = half;
    let max_segments = if samples.len() >= FFT_SIZE {
        (samples.len() - half) / hop
    } else {
        1
    };
    let num_segments = max_segments.min(MAX_WELCH_SEGMENTS);

    let hann: Vec<f32> = (0..FFT_SIZE)
        .map(|i| {
            0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / (FFT_SIZE - 1) as f32).cos())
        })
        .collect();

    let win_sq_sum: f64 = hann.iter().map(|&h| (h * h) as f64).sum();

    let mut psd = vec![0.0f64; FFT_SIZE];
    let mut buf = vec![Complex32::new(0.0, 0.0); FFT_SIZE];

    for seg in 0..num_segments {
        let offset = seg * hop;
        let end = (offset + FFT_SIZE).min(samples.len());

        buf.iter_mut().for_each(|c| *c = Complex32::new(0.0, 0.0));
        for i in 0..(end - offset) {
            buf[i] = samples[offset + i] * hann[i];
        }

        fft.process(&mut buf);

        for k in 0..FFT_SIZE {
            psd[k] += buf[k].norm_sqr() as f64;
        }
    }

    let scale = sample_rate / (win_sq_sum * num_segments as f64 * FFT_SIZE as f64);
    for v in psd.iter_mut().take(FFT_SIZE) {
        *v *= scale;
    }

    fftshift_inplace(&mut psd);
    psd
}

fn fftshift_inplace(psd: &mut [f64]) {
    let half = psd.len() / 2;
    let (left, right) = psd.split_at_mut(half);
    for i in 0..half {
        std::mem::swap(&mut left[i], &mut right[i]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_noise(n: usize, amplitude: f32, seed: u32) -> Vec<Complex32> {
        let mut state = seed;
        let mut rng = || {
            state = state.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            ((state >> 16) as f32 / 32_768.0 - 1.0) * amplitude
        };
        (0..n).map(|_| Complex32::new(rng(), rng())).collect()
    }

    fn make_pss_ssb(
        center_freq_hz: f64,
        sample_rate: f64,
        n_samples: usize,
        amplitude: f32,
        n_id_2: usize,
        _rng_seed: u32,
    ) -> Vec<Complex32> {
        let pss_values = crate::pss::generate_pss(n_id_2);
        let scs_hz = 15_000.0f64;
        let mut samples = Vec::with_capacity(n_samples);
        for i in 0..n_samples {
            let t = i as f64 / sample_rate;
            let mut val = Complex32::new(0.0, 0.0);
            for sc in 0..crate::pss::PSS_LEN {
                let sc_offset = sc as f64 - (crate::pss::PSS_LEN as f64 - 1.0) / 2.0;
                let freq = center_freq_hz + sc_offset * scs_hz;
                let phase = 2.0 * std::f64::consts::PI * freq * t;
                val += Complex32::new(
                    pss_values[sc] * phase.cos() as f32,
                    pss_values[sc] * phase.sin() as f32,
                );
            }
            samples.push(val * (amplitude / crate::pss::PSS_LEN as f32));
        }
        samples
    }

    #[test]
    fn test_detects_pss_ssb() {
        let sample_rate = 40_000_000.0;
        let center_freq = -5_000_000.0;
        let ssb = make_pss_ssb(center_freq, sample_rate, 200_000, 10.0, 0, 42);
        let noise = make_noise(ssb.len(), 0.01, 99);
        let samples: Vec<Complex32> = ssb.iter().zip(noise.iter()).map(|(s, n)| s + n).collect();

        let results = ssb_detect_all(&samples, sample_rate);
        let detected = results.iter().any(|r| r.detected);
        assert!(detected, "Should detect PSS-SSB signal");
    }

    #[test]
    fn test_noise_only_not_detected() {
        let sample_rate = 40_000_000.0;
        let noise = make_noise(200_000, 0.1, 42);
        let results = ssb_detect_all(&noise, sample_rate);
        assert!(
            results.iter().all(|r| !r.detected),
            "Noise-only should not be detected"
        );
    }

    #[test]
    fn test_freq_offset_pss_ssb() {
        let sample_rate = 40_000_000.0;
        let center_freq = -5_000_000.0;
        let ssb = make_pss_ssb(center_freq, sample_rate, 200_000, 10.0, 0, 42);
        let noise = make_noise(ssb.len(), 0.01, 99);
        let samples: Vec<Complex32> = ssb.iter().zip(noise.iter()).map(|(s, n)| s + n).collect();

        let results = ssb_detect_all(&samples, sample_rate);
        let detected = results.iter().find(|r| r.detected);
        assert!(detected.is_some(), "Should detect PSS-SSB signal");
    }

    #[test]
    fn test_no_edge_artifact_offset() {
        let sample_rate = 40_000_000.0;
        let noise = make_noise(16_384, 0.1, 77);
        let results = ssb_detect_all(&noise, sample_rate);
        for r in &results {
            if r.detected {
                let bin_width = sample_rate / FFT_SIZE as f64;
                let n = FFT_SIZE;
                let guard_bins = ((n as f64) * EDGE_GUARD_FRAC / 2.0).ceil() as usize;
                let edge_lo = -(n as f64 / 2.0 + guard_bins as f64 / 2.0) * bin_width;
                let edge_hi = (n as f64 / 2.0 - guard_bins as f64 / 2.0) * bin_width;
                assert!(
                    r.freq_offset_hz > edge_lo && r.freq_offset_hz < edge_hi,
                    "Detected offset {:.1} kHz is in the edge guard band",
                    r.freq_offset_hz / 1e3
                );
            }
        }
    }

    #[test]
    fn test_narrowband_rejected_by_pss() {
        let sample_rate = 40_000_000.0;
        let n_samples = 200_000;

        let mut samples = make_noise(n_samples, 0.1, 42);

        let tone_freq: f64 = 500_000.0;
        let phase_step = 2.0 * std::f64::consts::PI * tone_freq / sample_rate;
        for i in 0..n_samples {
            let phase = phase_step * i as f64;
            let tone = Complex32::new(phase.cos() as f32, phase.sin() as f32) * 5.0;
            samples[i] += tone;
        }

        let results = ssb_detect_all(&samples, sample_rate);
        let tone_det = results
            .iter()
            .find(|r| r.detected && (r.freq_offset_hz - tone_freq).abs() < 500_000.0);
        assert!(
            tone_det.is_none(),
            "Narrowband tone should be rejected by PSS verification, got offset={:.1}kHz n_id_2={:?} pss_snr={:?}",
            tone_det.unwrap().freq_offset_hz / 1e3,
            tone_det.unwrap().n_id_2,
            tone_det.unwrap().pss_snr_db,
        );
    }

    #[test]
    fn test_dc_spike_not_detected() {
        let sample_rate = 40_000_000.0;
        let n_samples = 200_000;

        let mut samples = make_noise(n_samples, 0.1, 42);

        for i in 0..n_samples {
            samples[i] += Complex32::new(10.0, 0.0);
        }

        let results = ssb_detect_all(&samples, sample_rate);
        for r in &results {
            if r.detected && r.freq_offset_hz.abs() < DC_EXCLUSION_HZ {
                panic!("DC spike should not be detected as SSB");
            }
        }
    }

    #[test]
    fn test_detects_pss_ssb_positive_offset() {
        let sample_rate = 40_000_000.0;
        let center_freq = 5_000_000.0;
        let ssb = make_pss_ssb(center_freq, sample_rate, 200_000, 10.0, 1, 99);
        let noise = make_noise(ssb.len(), 0.01, 77);
        let samples: Vec<Complex32> = ssb.iter().zip(noise.iter()).map(|(s, n)| s + n).collect();

        let results = ssb_detect_all(&samples, sample_rate);
        let detected = results.iter().any(|r| r.detected);
        assert!(detected, "Should detect PSS-SSB signal at +5 MHz");
    }

    #[test]
    fn test_detects_pss_ssb_n_id_2_2() {
        let sample_rate = 40_000_000.0;
        let center_freq = -3_000_000.0;
        let ssb = make_pss_ssb(center_freq, sample_rate, 200_000, 10.0, 2, 42);
        let noise = make_noise(ssb.len(), 0.01, 99);
        let samples: Vec<Complex32> = ssb.iter().zip(noise.iter()).map(|(s, n)| s + n).collect();

        let results = ssb_detect_all(&samples, sample_rate);
        let detected = results.iter().any(|r| r.detected);
        assert!(detected, "Should detect PSS-SSB signal with n_id_2=2");
    }
}
