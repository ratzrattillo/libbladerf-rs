//! ARFCN stands for Absolute Radio Frequency Channel Number — it's a GSM standard numbering scheme that maps an integer to a specific RF frequency.
//!
//!   In GSM, each channel (ARFCN) corresponds to a fixed frequency. For GSM-900 (the band kalibrate targets), the formula is:
//!
//!   f_uplink   = 890.0 + 0.2 * arfcn   (MHz)
//!   f_downlink = 935.0 + 0.2 * arfcn   (MHz)
//!
//!   So ARFCN 0 → 890.0 MHz uplink / 935.0 MHz downlink, ARFCN 1 → 890.2 / 935.2, etc. The kalibrate tool uses the downlink frequencies since it's receiving (not transmitting) to detect GSM base stations.
//!
//!   It's essentially an indirect way to specify frequency — the user passes an ARFCN number, and the code converts it to the actual frequency the SDR should tune to.

/// GSM band indicator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Band {
    Gsm850,
    Gsm900,
    EGsm900,
    Dcs1_800,
    Pcs1_900,
}

#[allow(dead_code)]
impl Band {
    /// First ARFCN in the band.
    pub fn first_arfcn(self) -> i32 {
        match self {
            Self::Gsm850 => 128,
            Self::Gsm900 => 1,
            Self::EGsm900 => 0,
            Self::Dcs1_800 => 512,
            Self::Pcs1_900 => 512,
        }
    }

    /// Next ARFCN after `chan`, or `None` if past the end.
    pub fn next_arfcn(self, chan: i32) -> Option<i32> {
        match self {
            Self::Gsm850 => (128..251).contains(&chan).then_some(chan + 1),
            Self::Gsm900 => (1..124).contains(&chan).then_some(chan + 1),
            Self::EGsm900 => match chan {
                0..=123 => Some(chan + 1),
                124 => Some(975),
                975..=1_022 => Some(chan + 1),
                _ => None,
            },
            Self::Dcs1_800 => (512..885).contains(&chan).then_some(chan + 1),
            Self::Pcs1_900 => (512..810).contains(&chan).then_some(chan + 1),
        }
    }

    /// Iterator over all ARFCNs in this band.
    pub fn arfcns(self) -> BandArfcnIter {
        BandArfcnIter {
            band: self,
            current: self.first_arfcn(),
            started: false,
        }
    }

    /// Convert ARFCN to downlink frequency in Hz.
    /// Returns `None` for invalid ARFCN/band combinations.
    /// For ARFCNs 512..810 (ambiguous between DCS-1800 and PCS-1900), this band is used.
    pub fn arfcn_to_freq(self, arfcn: i32) -> Option<f64> {
        match arfcn {
            128..=251 => Some(824.2e6 + 0.2e6 * (arfcn - 128) as f64 + 45.0e6),
            1..=124 => Some(890.0e6 + 0.2e6 * arfcn as f64 + 45.0e6),
            0 if self == Self::EGsm900 => Some(935.0e6),
            975..=1_023 if self == Self::EGsm900 => {
                Some(890.0e6 + 0.2e6 * (arfcn - 1_024) as f64 + 45.0e6)
            }
            512..=810 => match self {
                Self::Dcs1_800 => Some(1_710.2e6 + 0.2e6 * (arfcn - 512) as f64 + 95.0e6),
                Self::Pcs1_900 => Some(1_850.2e6 + 0.2e6 * (arfcn - 512) as f64 + 80.0e6),
                _ => None,
            },
            811..=885 => Some(1_710.2e6 + 0.2e6 * (arfcn - 512) as f64 + 95.0e6),
            _ => None,
        }
    }

    /// Convert downlink frequency in Hz to ARFCN.
    /// Returns the band and ARFCN, or `None` if frequency doesn't map to a known band.
    pub fn from_freq(freq: f64) -> Option<(Self, i32)> {
        if (869.2e6..=893.8e6).contains(&freq) {
            return Some((Self::Gsm850, ((freq - 869.2e6) / 0.2e6) as i32 + 128));
        }
        if freq == 935.0e6 {
            return Some((Self::EGsm900, 0));
        }
        if (925.2e6..=934.8e6).contains(&freq) {
            return Some((Self::EGsm900, ((freq - 935.0e6) / 0.2e6) as i32 + 1_024));
        }
        if (935.2e6..=959.8e6).contains(&freq) {
            return Some((Self::Gsm900, ((freq - 935.0e6) / 0.2e6) as i32));
        }
        if (1_805.2e6..=1_879.8e6).contains(&freq) {
            return Some((Self::Dcs1_800, ((freq - 1_805.2e6) / 0.2e6) as i32 + 512));
        }
        if (1_930.2e6..=1_989.8e6).contains(&freq) {
            return Some((Self::Pcs1_900, ((freq - 1_930.2e6) / 0.2e6) as i32 + 512));
        }
        None
    }

    /// Determine band from ARFCN alone.
    /// Returns `None` for ambiguous ARFCNs (512-810) which need explicit band context.
    pub fn from_arfcn(arfcn: i32) -> Option<Self> {
        match arfcn {
            128..=251 => Some(Self::Gsm850),
            1..=124 => Some(Self::Gsm900),
            0 | 975..=1_023 => Some(Self::EGsm900),
            811..=885 => Some(Self::Dcs1_800),
            // 512-810 is ambiguous between DCS-1800 and PCS-1900
            _ => None,
        }
    }

    /// All bands in scan order (lowest to highest frequency).
    pub fn all() -> &'static [Band] {
        &[
            Band::Gsm850,
            Band::Gsm900,
            Band::EGsm900,
            Band::Dcs1_800,
            Band::Pcs1_900,
        ]
    }

    /// Parse a band name string (e.g. "GSM-850", "850", "DCS").
    /// Returns `None` if the string doesn't match any known band.
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "GSM850" | "GSM-850" | "850" => Some(Self::Gsm850),
            "GSM900" | "GSM-900" | "900" => Some(Self::Gsm900),
            "EGSM" | "E-GSM" | "EGSM900" | "E-GSM900" | "E-GSM-900" => Some(Self::EGsm900),
            "DCS" | "DCS1800" | "DCS-1800" | "1800" => Some(Self::Dcs1_800),
            "PCS" | "PCS1900" | "PCS-1900" | "1900" => Some(Self::Pcs1_900),
            _ => None,
        }
    }
}

/// Iterator over ARFCNs in a GSM band.
pub struct BandArfcnIter {
    band: Band,
    current: i32,
    started: bool,
}

impl Iterator for BandArfcnIter {
    type Item = i32;

    fn next(&mut self) -> Option<Self::Item> {
        if !self.started {
            self.started = true;
            return Some(self.current);
        }
        match self.band.next_arfcn(self.current) {
            Some(next) => {
                self.current = next;
                Some(next)
            }
            None => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gsm850_range() {
        let arfcns: Vec<i32> = Band::Gsm850.arfcns().collect();
        assert_eq!(arfcns.len(), 124); // 128..=251
        assert_eq!(arfcns[0], 128);
        assert_eq!(*arfcns.last().unwrap(), 251);
    }

    #[test]
    fn test_egsm900_discontinuity() {
        let arfcns: Vec<i32> = Band::EGsm900.arfcns().collect();
        assert_eq!(arfcns[0], 0);
        assert_eq!(arfcns[125], 975); // jumps from 124 -> 975
        assert_eq!(*arfcns.last().unwrap(), 1_023);
    }

    #[test]
    fn test_arfcn_to_freq_gsm850() {
        let freq = Band::Gsm850.arfcn_to_freq(128).unwrap();
        assert!((freq - 869.2e6).abs() < 1.0);
    }

    #[test]
    fn test_arfcn_to_freq_ambiguous() {
        // ARFCN 600 is ambiguous — needs band context
        assert!(Band::Gsm850.arfcn_to_freq(600).is_none());
        let dcs = Band::Dcs1_800.arfcn_to_freq(600).unwrap();
        let pcs = Band::Pcs1_900.arfcn_to_freq(600).unwrap();
        assert!(dcs != pcs);
    }

    #[test]
    fn test_from_freq_roundtrip() {
        let freq = Band::Gsm900.arfcn_to_freq(50).unwrap();
        let (band, arfcn) = Band::from_freq(freq).unwrap();
        assert_eq!(band, Band::Gsm900);
        assert_eq!(arfcn, 50);
    }

    #[test]
    fn test_band_from_str() {
        assert_eq!(Band::from_str("850"), Some(Band::Gsm850));
        assert_eq!(Band::from_str("DCS-1800"), Some(Band::Dcs1_800));
        assert_eq!(Band::from_str("unknown"), None);
    }
}
