//! DC calibration table management — loading, saving, and looking up per-frequency
//! DC offset correction values.  Tables are JSON files named `<serial>_dc_rx.json`
//! and `<serial>_dc_tx.json`, auto-loaded at device open.

use crate::bladerf1::hardware::lms6002d::dc_calibration::DcCals;
use crate::bladerf1::hardware::lms6002d::dc_calibration::{AgcDcCorrection, DcPair};
use crate::error::Result;
use std::path::Path;

/// Single calibration entry with frequency, DC offset I/Q pair, and AGC sub-ranges.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DcCalEntry {
    pub freq: u32,
    pub dc: DcPair,
    pub max_dc: DcPair,
    pub mid_dc: DcPair,
    pub min_dc: DcPair,
}

impl DcCalEntry {
    /// Create an entry with the given frequency and DC offset, zeroed AGC sub-ranges.
    pub fn new(freq: u32, dc: DcPair) -> Self {
        Self {
            freq,
            dc,
            max_dc: DcPair::default(),
            mid_dc: DcPair::default(),
            min_dc: DcPair::default(),
        }
    }

    /// Set the AGC sub-range DC offsets (max, mid, min) and return the entry.
    pub fn with_agc(mut self, max_dc: DcPair, mid_dc: DcPair, min_dc: DcPair) -> Self {
        self.max_dc = max_dc;
        self.mid_dc = mid_dc;
        self.min_dc = min_dc;
        self
    }
}

impl From<&DcCalEntry> for AgcDcCorrection {
    /// Convert the entry's AGC sub-range values into an `AgcDcCorrection`.
    fn from(e: &DcCalEntry) -> Self {
        Self {
            max: e.max_dc,
            mid: e.mid_dc,
            min: e.min_dc,
        }
    }
}

/// Collection of calibration entries and associated register values.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DcCalTable {
    reg_vals: DcCals,
    entries: Vec<DcCalEntry>,
}

impl DcCalTable {
    /// Construct a new calibration table from register values and entries.
    pub fn new(reg_vals: DcCals, entries: Vec<DcCalEntry>) -> Self {
        Self { reg_vals, entries }
    }

    /// Returns a reference to the register values.
    pub fn reg_vals(&self) -> &DcCals {
        &self.reg_vals
    }

    /// Returns a slice of calibration entries.
    pub fn entries(&self) -> &[DcCalEntry] {
        &self.entries
    }

    /// Load the calibration table from a JSON file.
    pub fn load(path: &Path) -> Result<Self> {
        let buf = std::fs::read_to_string(path)?;
        Ok(serde_json::from_str(&buf)?)
    }

    /// Serialize the calibration table to a JSON file.
    pub fn save(&self, path: &Path) -> Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        Ok(std::fs::write(path, json)?)
    }

    fn lookup_index(&self, freq: u32) -> usize {
        if self.entries.is_empty() {
            return 0;
        }
        self.entries
            .partition_point(|e| e.freq <= freq)
            .saturating_sub(1)
    }

    /// Look up DC offset corrections for a frequency.  Returns an exact match, clamps
    /// at the nearest table boundary, or linearly interpolates between bracketing entries.
    pub fn lookup(&self, freq: u64) -> DcCalEntry {
        if self.entries.is_empty() {
            return DcCalEntry {
                freq: freq as u32,
                dc: DcPair::default(),
                max_dc: DcPair::default(),
                mid_dc: DcPair::default(),
                min_dc: DcPair::default(),
            };
        }
        let f = freq as u32;
        let idx = self.lookup_index(f);
        if self.entries[idx].freq == f {
            return self.entries[idx];
        }
        if idx == 0 && f < self.entries[0].freq {
            return self.entries[0];
        }
        if idx == self.entries.len() - 1 && f > self.entries[idx].freq {
            return self.entries[idx];
        }
        let (idx_low, idx_high) = if idx == self.entries.len() - 1 {
            (idx - 1, idx)
        } else {
            (idx, idx + 1)
        };
        let f_low = self.entries[idx_low].freq;
        let f_high = self.entries[idx_high].freq;
        DcCalEntry {
            freq: f,
            dc: DcPair::interp(
                f_low,
                self.entries[idx_low].dc,
                f_high,
                self.entries[idx_high].dc,
                f,
            ),
            max_dc: DcPair::interp(
                f_low,
                self.entries[idx_low].max_dc,
                f_high,
                self.entries[idx_high].max_dc,
                f,
            ),
            mid_dc: DcPair::interp(
                f_low,
                self.entries[idx_low].mid_dc,
                f_high,
                self.entries[idx_high].mid_dc,
                f,
            ),
            min_dc: DcPair::interp(
                f_low,
                self.entries[idx_low].min_dc,
                f_high,
                self.entries[idx_high].min_dc,
                f,
            ),
        }
    }
}
