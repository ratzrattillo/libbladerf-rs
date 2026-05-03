//! GSCN (Global Synchronization Channel Number) — 3GPP TS 38.104 §5.4.3
//!
//! Maps GSCN indices to SSB center frequencies and back.
//! Only includes NR bands relevant to Germany that fall within BladeRF1 range.
//!
//! Reference: ocudu C++ implementation (resources/ocudu/lib/ran/ssb/ssb_gscn.cpp)
//! and Table 5.4.3.3-1, TS 38.104.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Band {
    N1,
    N3,
    N7,
    N8,
    N20,
    N28,
    N78,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DuplexMode {
    Fdd,
    Tdd,
}

#[allow(dead_code)]
impl Band {
    pub fn dl_low_mhz(self) -> f64 {
        match self {
            Self::N1 => 2_110.0,
            Self::N3 => 1_805.0,
            Self::N7 => 2_620.0,
            Self::N8 => 925.0,
            Self::N20 => 791.0,
            Self::N28 => 758.0,
            Self::N78 => 3_300.0,
        }
    }

    pub fn dl_high_mhz(self) -> f64 {
        match self {
            Self::N1 => 2_170.0,
            Self::N3 => 1_880.0,
            Self::N7 => 2_690.0,
            Self::N8 => 960.0,
            Self::N20 => 821.0,
            Self::N28 => 803.0,
            Self::N78 => 3_800.0,
        }
    }

    pub fn duplex_mode(self) -> DuplexMode {
        match self {
            Self::N1 | Self::N3 | Self::N7 | Self::N8 | Self::N20 | Self::N28 => DuplexMode::Fdd,
            Self::N78 => DuplexMode::Tdd,
        }
    }

    pub fn all() -> &'static [Band] {
        &[
            Band::N8,
            Band::N20,
            Band::N28,
            Band::N3,
            Band::N1,
            Band::N7,
            Band::N78,
        ]
    }

    pub fn from_name(s: &str) -> Option<Self> {
        match s {
            "n1" | "N1" | "1" => Some(Self::N1),
            "n3" | "N3" | "3" => Some(Self::N3),
            "n7" | "N7" | "7" => Some(Self::N7),
            "n8" | "N8" | "8" => Some(Self::N8),
            "n20" | "N20" | "20" => Some(Self::N20),
            "n28" | "N28" | "28" => Some(Self::N28),
            "n78" | "N78" | "78" => Some(Self::N78),
            _ => None,
        }
    }

    pub fn gscns(self) -> Vec<u32> {
        match self {
            Self::N1 => gscn_range(5_279, 1, 5_419),
            Self::N3 => gscn_range(4_517, 1, 4_693),
            Self::N7 => gscn_range(6_554, 1, 6_718),
            Self::N8 => gscn_range(2_318, 1, 2_395),
            Self::N20 => gscn_range(1_982, 1, 2_047),
            Self::N28 => gscn_range(1_901, 1, 2_002),
            Self::N78 => gscn_range(7_711, 1, 8_051),
        }
    }

    pub fn gscn_to_freq(self, gscn: u32) -> Option<f64> {
        let f = gscn_to_freq(gscn);
        let low = self.dl_low_mhz() * 1e6;
        let high = self.dl_high_mhz() * 1e6;
        if f >= low && f <= high { Some(f) } else { None }
    }

    pub fn from_freq(freq: f64) -> Option<(Self, u32)> {
        for &band in Self::all() {
            let low = band.dl_low_mhz() * 1e6;
            let high = band.dl_high_mhz() * 1e6;
            if freq >= low && freq <= high {
                return Some((band, freq_to_gscn(freq)));
            }
        }
        None
    }
}

fn gscn_range(first: u32, step: u32, last: u32) -> Vec<u32> {
    (first..=last).step_by(step as usize).collect()
}

/// GSCN → SSB center frequency in Hz.
///
/// FR1 range 1 (0–3_000 MHz):
///   SS_ref = N × 1,200,000 + M × 50,000  (Hz)
///   N = 1.., M ∈ {1, 3, 5}
///   GSCN = 3×N + (M−3)/2
///
/// FR1 range 2 (3_000–24_250 MHz):
///   SS_ref = (3,000,000 + N × 1,440) × 1,000  (Hz)
///   N = 1.., GSCN = 7,499 + N
pub fn gscn_to_freq(gscn: u32) -> f64 {
    if (2..7_499).contains(&gscn) {
        let m = find_m_for_gscn(gscn as i32);
        let n = ((gscn as i32) - (m - 3) / 2) / 3;
        n as f64 * 1_200_000.0 + m as f64 * 50_000.0
    } else if (7_499..=22_255).contains(&gscn) {
        let n = (gscn - 7_499) as f64;
        (3_000_000.0 + n * 1_440.0) * 1e3
    } else {
        0.0
    }
}

fn find_m_for_gscn(gscn: i32) -> i32 {
    for m in [1, 3, 5] {
        if (gscn - (m - 3) / 2) % 3 == 0 {
            return m;
        }
    }
    1
}

/// Frequency (Hz) → nearest GSCN.
pub fn freq_to_gscn(freq: f64) -> u32 {
    if freq <= 3e9 {
        let n = (freq / 1_200_000.0).round() as i32;
        let n = n.max(1);
        let base = n as f64 * 1_200_000.0;
        let rem = freq - base;
        let m = if rem < 100_000.0 {
            1
        } else if rem < 250_000.0 {
            3
        } else {
            5
        };
        (3 * n + (m - 3) / 2) as u32
    } else {
        let n = ((freq / 1e3 - 3_000_000.0) / 1_440.0).round() as i32;
        let n = n.max(1);
        7_499 + n as u32
    }
}

pub fn group_frequencies(freqs: &[f64], usable_bw_hz: f64) -> Vec<f64> {
    if freqs.is_empty() {
        return vec![];
    }
    let mut sorted: Vec<f64> = freqs.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let half_bw = usable_bw_hz / 2.0;
    let mut centers: Vec<f64> = Vec::new();
    let mut i = 0;

    while i < sorted.len() {
        let center = sorted[i] + half_bw;
        centers.push(center);
        while i < sorted.len() && sorted[i] <= center + half_bw {
            i += 1;
        }
    }

    centers
}

pub fn nearest_gscn(freq_hz: f64) -> Option<(Band, u32)> {
    let gscn = freq_to_gscn(freq_hz);
    for &band in Band::all() {
        if band.gscns().contains(&gscn) {
            return Some((band, gscn));
        }
    }
    Band::from_freq(freq_hz)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gscn_range1_low() {
        let f = gscn_to_freq(2);
        assert!(
            (f - 1_250_000.0).abs() < 1.0,
            "GSCN 2 should be 1250000 Hz, got {f}"
        );
    }

    #[test]
    fn test_gscn_range1_sequence() {
        let f2 = gscn_to_freq(2);
        let f3 = gscn_to_freq(3);
        let f4 = gscn_to_freq(4);
        let f5 = gscn_to_freq(5);
        assert!((f2 - 1_250_000.0).abs() < 1.0, "GSCN 2 = 1.25 MHz");
        assert!((f3 - 1_350_000.0).abs() < 1.0, "GSCN 3 = 1.35 MHz");
        assert!((f4 - 1_450_000.0).abs() < 1.0, "GSCN 4 = 1.45 MHz");
        assert!((f5 - 2_450_000.0).abs() < 1.0, "GSCN 5 = 2.45 MHz");
    }

    #[test]
    fn test_gscn_range2_low() {
        let f = gscn_to_freq(7_499);
        assert!((f - 3e9).abs() < 1.0, "GSCN 7499 should be 3 GHz, got {f}");
        let f2 = gscn_to_freq(7_500);
        assert!(
            (f2 - 3_001_440_000.0).abs() < 1.0,
            "GSCN 7500 should be 3001.44 MHz, got {f2}"
        );
    }

    #[test]
    fn test_gscn_roundtrip_range1() {
        for g in [2, 3, 4, 5, 100, 500, 1_000, 2_000, 7_498] {
            let f = gscn_to_freq(g);
            let g2 = freq_to_gscn(f);
            assert_eq!(g, g2, "GSCN roundtrip failed: {g} → {f} Hz → {g2}");
        }
    }

    #[test]
    fn test_gscn_roundtrip_range2() {
        for g in [7_499, 7_500, 8_000, 10_000, 15_000, 22_255] {
            let f = gscn_to_freq(g);
            let g2 = freq_to_gscn(f);
            assert_eq!(g, g2, "GSCN roundtrip failed: {g} → {f} Hz → {g2}");
        }
    }

    #[test]
    fn test_band_n8_gscns() {
        let gscns = Band::N8.gscns();
        assert!(!gscns.is_empty(), "n8 should have GSCN entries");
        for &g in &gscns {
            let f = gscn_to_freq(g);
            assert!(
                f >= Band::N8.dl_low_mhz() * 1e6 && f <= Band::N8.dl_high_mhz() * 1e6,
                "GSCN {g} → {f} Hz outside n8 range"
            );
        }
    }

    #[test]
    fn test_band_n28_gscns() {
        let gscns = Band::N28.gscns();
        assert!(!gscns.is_empty(), "n28 should have GSCN entries");
        for &g in &gscns {
            let f = gscn_to_freq(g);
            assert!(
                f >= Band::N28.dl_low_mhz() * 1e6 && f <= Band::N28.dl_high_mhz() * 1e6,
                "GSCN {g} → {f} Hz outside n28 range"
            );
        }
    }

    #[test]
    fn test_band_n78_gscns() {
        let gscns = Band::N78.gscns();
        assert!(!gscns.is_empty(), "n78 should have GSCN entries");
        assert!(
            gscns.len() > 20,
            "n78 should have many GSCN entries, got {}",
            gscns.len()
        );
        for &g in &gscns {
            let f = gscn_to_freq(g);
            assert!(
                f >= Band::N78.dl_low_mhz() * 1e6 && f <= Band::N78.dl_high_mhz() * 1e6,
                "GSCN {g} → {f} Hz outside n78 range"
            );
        }
    }

    #[test]
    fn test_band_from_freq() {
        let (band, _) = Band::from_freq(935.0e6).unwrap();
        assert_eq!(band, Band::N8);

        let (band, _) = Band::from_freq(3_500.0e6).unwrap();
        assert_eq!(band, Band::N78);
    }

    #[test]
    fn test_all_bands_have_gscns() {
        for &band in Band::all() {
            let gscns = band.gscns();
            assert!(!gscns.is_empty(), "{band:?} has no GSCN entries");
        }
    }

    #[test]
    fn test_gscn_values_match_ocudu() {
        assert!(
            Band::N8.gscns().contains(&2_318),
            "n8 should contain GSCN 2318"
        );
        assert!(
            Band::N28.gscns().contains(&1_901),
            "n28 should contain GSCN 1901"
        );
        assert!(
            Band::N78.gscns().contains(&7_711),
            "n78 should contain GSCN 7711"
        );
    }

    #[test]
    fn test_group_frequencies_empty() {
        let result = group_frequencies(&[], 24e6);
        assert!(result.is_empty());
    }

    #[test]
    fn test_group_frequencies_single() {
        let result = group_frequencies(&[935e6], 24e6);
        assert_eq!(result.len(), 1);
        assert!((result[0] - 947e6).abs() < 1.0);
    }

    #[test]
    fn test_group_frequencies_wide_band() {
        let freqs: Vec<f64> = Band::N1
            .gscns()
            .iter()
            .filter_map(|&g| Band::N1.gscn_to_freq(g))
            .collect();
        let groups = group_frequencies(&freqs, 24e6);
        assert!(
            groups.len() < freqs.len() / 5,
            "Should significantly reduce tuning steps: {} groups for {} frequencies",
            groups.len(),
            freqs.len(),
        );
        for &f in &freqs {
            let covered = groups.iter().any(|&c| (f - c).abs() <= 12e6);
            assert!(
                covered,
                "Frequency {:.3} MHz not covered by any group",
                f / 1e6
            );
        }
    }

    #[test]
    fn test_nearest_gscn() {
        let (band, gscn) = nearest_gscn(935.0e6).unwrap();
        assert_eq!(band, Band::N8);
        assert!(Band::N8.gscns().contains(&gscn));
    }
}
