pub const PSS_LEN: usize = 127;
pub const NOF_PSS: usize = 3;

fn generate_m_sequence() -> [u8; PSS_LEN] {
    let mut x = [0u8; PSS_LEN + 7];
    x[0] = 0;
    x[1] = 1;
    x[2] = 1;
    x[3] = 0;
    x[4] = 1;
    x[5] = 1;
    x[6] = 1;
    for i in 0..PSS_LEN {
        x[i + 7] = (x[i + 4] + x[i]) % 2;
    }
    let mut seq = [0u8; PSS_LEN];
    seq[..PSS_LEN].copy_from_slice(&x[..PSS_LEN]);
    seq
}

pub fn generate_pss(n_id_2: usize) -> [f32; PSS_LEN] {
    assert!(n_id_2 < NOF_PSS, "n_id_2 must be 0, 1, or 2");
    let x = generate_m_sequence();
    let m = (43 * (n_id_2 as u32)) as usize % PSS_LEN;
    let mut d = [0.0f32; PSS_LEN];
    for n in 0..PSS_LEN {
        d[n] = 1.0 - 2.0 * x[(n + m) % PSS_LEN] as f32;
    }
    d
}

use rustfft::num_complex::Complex32;

const PSS_SCS_HZ: f64 = 15_000.0;
const PSS_SNR_MIN_DB: f64 = 8.0;
const PSS_C2B_MIN_DB: f64 = 3.0;
const PSS_AVG_BLOCKS: usize = 64;

#[derive(Debug, Clone)]
pub struct PssCorrelationResult {
    pub n_id_2: usize,
    pub freq_offset_hz: f64,
    pub snr_db: f64,
}

fn lerp_spectrum(buf: &[Complex32], k_frac: f64, fft_size: usize) -> Complex32 {
    let k0 = k_frac.floor() as isize;
    let frac = k_frac - k0 as f64;
    let k0 = k0.rem_euclid(fft_size as isize) as usize;
    let k1 = (k0 + 1) % fft_size;
    buf[k0] * (1.0 - frac as f32) + buf[k1] * frac as f32
}

pub fn cancel_dominant_tone(_samples: &mut [Complex32], _sample_rate: f64) {}

#[allow(clippy::needless_range_loop, clippy::manual_memcpy)]
pub fn pss_correlate(
    samples: &[Complex32],
    sample_rate: f64,
    search_center_hz: f64,
    max_shift_hz: f64,
) -> Option<PssCorrelationResult> {
    let fft_size = 65536usize;
    let bin_width = sample_rate / fft_size as f64;
    let scs_bins = PSS_SCS_HZ / bin_width;

    if samples.len() < fft_size || scs_bins < 1.0 {
        return None;
    }

    let search_center_bin = search_center_hz / bin_width;

    let shift_range_bins = (max_shift_hz / bin_width).ceil() as usize;
    let num_shifts = 2 * shift_range_bins + 1;

    let max_offset = samples.len().saturating_sub(fft_size);
    let num_blocks = PSS_AVG_BLOCKS.min(if max_offset > 0 {
        max_offset / (fft_size / 2).max(1) + 1
    } else {
        1
    });
    let hop = if num_blocks > 1 && max_offset > 0 {
        max_offset / (num_blocks - 1)
    } else {
        fft_size / 2
    };

    let pss_seq: Vec<[Complex32; PSS_LEN]> = (0..NOF_PSS)
        .map(|n_id_2| {
            let pss = generate_pss(n_id_2);
            let mut seq = [Complex32::new(0.0, 0.0); PSS_LEN];
            for sc in 0..PSS_LEN {
                seq[sc] = Complex32::new(pss[sc], 0.0);
            }
            seq
        })
        .collect();

    let mut total_corr_mag: Vec<f64> = vec![0.0; NOF_PSS * num_shifts];
    let mut total_band_power: Vec<f64> = vec![0.0; num_shifts];

    let mut planner = rustfft::FftPlanner::new();
    let fft = planner.plan_fft_forward(fft_size);

    for blk in 0..num_blocks {
        let offset = blk * hop;
        let mut buf = vec![Complex32::new(0.0, 0.0); fft_size];
        let end = (offset + fft_size).min(samples.len());
        buf[..(end - offset)].copy_from_slice(&samples[offset..end]);

        fft.process(&mut buf);

        for si in 0..num_shifts {
            let shift = si as isize - shift_range_bins as isize;
            let mut band_power = 0.0f64;
            for sc in 0..PSS_LEN {
                let sc_offset = sc as f64 - (PSS_LEN as f64 - 1.0) / 2.0;
                let k_frac = search_center_bin + shift as f64 + sc_offset * scs_bins;
                let val = lerp_spectrum(&buf, k_frac, fft_size);
                band_power += val.norm_sqr() as f64;
            }
            total_band_power[si] += band_power;
        }

        for n_id_2 in 0..NOF_PSS {
            let pss = &pss_seq[n_id_2];
            for si in 0..num_shifts {
                let shift = si as isize - shift_range_bins as isize;

                let mut corr = Complex32::new(0.0, 0.0);
                for sc in 0..PSS_LEN {
                    let sc_offset = sc as f64 - (PSS_LEN as f64 - 1.0) / 2.0;
                    let k_frac = search_center_bin + shift as f64 + sc_offset * scs_bins;
                    let val = lerp_spectrum(&buf, k_frac, fft_size);
                    corr += val * pss[sc].conj();
                }
                total_corr_mag[n_id_2 * num_shifts + si] += corr.norm_sqr() as f64;
            }
        }
    }

    let nb = num_blocks as f64;
    for v in &mut total_corr_mag {
        *v /= nb;
    }
    for v in &mut total_band_power {
        *v /= nb;
    }

    let mut best_idx: usize = 0;
    let mut best_mag: f64 = 0.0;
    for (i, &m) in total_corr_mag.iter().enumerate() {
        if m > best_mag {
            best_mag = m;
            best_idx = i;
        }
    }

    if best_mag <= 0.0 {
        return None;
    }

    let mut sorted: Vec<f64> = total_corr_mag.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = sorted[sorted.len() / 2];

    let snr_db = if median > 0.0 {
        10.0 * (best_mag / median).log10()
    } else {
        f64::INFINITY
    };

    let best_n_id_2 = best_idx / num_shifts;
    let best_si = best_idx % num_shifts;
    let best_shift = best_si as isize - shift_range_bins as isize;
    let coarse_offset_hz = search_center_hz + best_shift as f64 * bin_width;

    let band_power = total_band_power[best_si];
    let corr_to_band = if band_power > 0.0 {
        10.0 * (best_mag / band_power).log10()
    } else {
        f64::NEG_INFINITY
    };

    eprintln!(
        "    [pss] n_id_2={} snr={:.1}dB c2b={:.1}dB snr_min={:.0}dB c2b_min={:.0}dB coarse={:.1}kHz shift={}bins ({:.1}kHz) scs_bins={:.2}",
        best_n_id_2,
        snr_db,
        corr_to_band,
        PSS_SNR_MIN_DB,
        PSS_C2B_MIN_DB,
        coarse_offset_hz / 1e3,
        best_shift,
        best_shift as f64 * bin_width / 1e3,
        scs_bins
    );

    if snr_db < PSS_SNR_MIN_DB {
        eprintln!("    [pss] REJECT: snr={snr_db:.1}dB < {PSS_SNR_MIN_DB:.0}dB");
        return None;
    }

    if corr_to_band < PSS_C2B_MIN_DB {
        eprintln!("    [pss] REJECT: c2b={corr_to_band:.1}dB < {PSS_C2B_MIN_DB:.0}dB (likely CW)");
        return None;
    }

    if corr_to_band < PSS_C2B_MIN_DB {
        eprintln!("    [pss] REJECT: c2b={corr_to_band:.1}dB < {PSS_C2B_MIN_DB:.0}dB (likely CW)");
        return None;
    }

    let fine_offset_hz = {
        let lo = best_si.saturating_sub(5);
        let hi = (best_si + 6).min(num_shifts);
        let mut weighted_sum = 0.0f64;
        let mut weight_total = 0.0f64;
        for si in lo..hi {
            let shift = si as isize - shift_range_bins as isize;
            let m = total_corr_mag[best_n_id_2 * num_shifts + si];
            weighted_sum += shift as f64 * m;
            weight_total += m;
        }
        let cog_shift = if weight_total > 0.0 {
            weighted_sum / weight_total
        } else {
            best_shift as f64
        };
        search_center_hz + cog_shift * bin_width
    };

    Some(PssCorrelationResult {
        n_id_2: best_n_id_2,
        freq_offset_hz: fine_offset_hz,
        snr_db,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_m_sequence_length() {
        let x = generate_m_sequence();
        assert_eq!(x.len(), PSS_LEN);
    }

    #[test]
    fn test_pss_values_in_range() {
        for n_id_2 in 0..NOF_PSS {
            let pss = generate_pss(n_id_2);
            for &v in &pss {
                assert!(v == 1.0 || v == -1.0);
            }
        }
    }

    #[test]
    fn test_pss_sequences_differ() {
        let pss0 = generate_pss(0);
        let pss1 = generate_pss(1);
        let pss2 = generate_pss(2);
        assert!(pss0.iter().zip(pss1.iter()).any(|(&a, &b)| a != b));
        assert!(pss0.iter().zip(pss2.iter()).any(|(&a, &b)| a != b));
        assert!(pss1.iter().zip(pss2.iter()).any(|(&a, &b)| a != b));
    }

    #[test]
    fn test_pss_autocorrelation_peak() {
        let pss = generate_pss(0);
        let mut auto_corr = vec![0.0f64; PSS_LEN];
        for shift in 0..PSS_LEN {
            let mut sum = 0.0f64;
            for n in 0..PSS_LEN {
                sum += pss[n] as f64 * pss[(n + shift) % PSS_LEN] as f64;
            }
            auto_corr[shift] = sum;
        }
        assert!(auto_corr[0] > 0.0);
        for shift in 1..PSS_LEN {
            assert!(auto_corr[shift] < auto_corr[0]);
        }
    }

    #[test]
    #[should_panic]
    fn test_pss_invalid_n_id_2() {
        generate_pss(3);
    }

    fn make_pss_signal_with_noise(
        sample_rate: f64,
        ssb_offset: f64,
        freq_error: f64,
        n_id_2: usize,
        n_samples: usize,
        signal_amp: f32,
        noise_amp: f32,
        seed: u32,
    ) -> Vec<Complex32> {
        let pss = generate_pss(n_id_2);
        let mut state = seed;
        let mut rng = || {
            state = state.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            ((state >> 16) as f32 / 32_768.0 - 1.0) * noise_amp
        };

        let mut samples = Vec::with_capacity(n_samples);
        for i in 0..n_samples {
            let t = i as f64 / sample_rate;
            let mut val = Complex32::new(0.0, 0.0);
            for sc in 0..PSS_LEN {
                let sc_offset = sc as f64 - (PSS_LEN as f64 - 1.0) / 2.0;
                let freq = ssb_offset + sc_offset * PSS_SCS_HZ;
                let phase = 2.0 * std::f64::consts::PI * (freq + freq_error) * t;
                val += Complex32::new(pss[sc] * phase.cos() as f32, pss[sc] * phase.sin() as f32);
            }
            val = val / PSS_LEN as f32 * signal_amp;
            samples.push(val + Complex32::new(rng(), rng()));
        }
        samples
    }

    #[test]
    fn test_detect_synth_ssb_moderate_snr() {
        let sample_rate = 40_000_000.0;
        let ssb_offset = -1_500_000.0;
        let freq_error = 5_432.0;
        let n_id_2 = 0;

        let samples = make_pss_signal_with_noise(
            sample_rate,
            ssb_offset,
            freq_error,
            n_id_2,
            200_000,
            0.7,
            0.3,
            42,
        );

        let result = pss_correlate(&samples, sample_rate, ssb_offset + freq_error, 200_000.0);
        assert!(result.is_some(), "Should detect SSB with PSS");
    }

    #[test]
    fn test_noise_no_detection() {
        let sample_rate = 40_000_000.0;
        let mut state: u32 = 42;
        let mut rng = || {
            state = state.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            ((state >> 16) as f32 / 32_768.0 - 1.0) * 0.3
        };
        let noise: Vec<Complex32> = (0..200_000).map(|_| Complex32::new(rng(), rng())).collect();
        let result = pss_correlate(&noise, sample_rate, -1_500_000.0, 200_000.0);
        assert!(
            result.is_none() || result.as_ref().unwrap().snr_db < PSS_SNR_MIN_DB,
            "Noise-only should not produce detection above threshold"
        );
    }

    #[test]
    fn test_cw_rejected_by_snr_cap() {
        let sample_rate = 40_000_000.0;
        let n_samples = 200_000usize;
        let mut state: u32 = 42;
        let mut rng = || {
            state = state.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            ((state >> 16) as f32 / 32_768.0 - 1.0) * 0.1
        };

        let tone_freq: f64 = -1_500_000.0;
        let phase_step = 2.0 * std::f64::consts::PI * tone_freq / sample_rate;
        let mut samples = Vec::with_capacity(n_samples);
        for i in 0..n_samples {
            let phase = phase_step * i as f64;
            let tone = Complex32::new(phase.cos() as f32, phase.sin() as f32) * 5.0;
            samples.push(tone + Complex32::new(rng(), rng()));
        }

        let result = pss_correlate(&samples, sample_rate, tone_freq, 200_000.0);
        assert!(
            result.is_none(),
            "CW tone should be rejected by SNR cap, got n_id_2={:?} snr={:?}",
            result.as_ref().map(|r| r.n_id_2),
            result.as_ref().map(|r| r.snr_db),
        );
    }
}
