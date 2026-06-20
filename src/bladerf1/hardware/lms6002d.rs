//! LMS6002D RF transceiver driver.
//!
//! Core analog signal chain for BladeRF1 — a 0.3–3.8 GHz transceiver with
//! 12-bit ADC/DAC and programmable modulation bandwidth.

pub mod bandwidth;
pub mod dc_calibration;
pub mod filters;
pub mod frequency;
pub mod gain;
pub mod loopback;
use crate::channel::Channel;
use crate::error::{Error, Result};
use crate::nios_client::NiosCore;
use crate::protocol::nios::NiosPkt8x8Target;
pub use filters::LpfMode;
use gain::{LmsLowNoiseAmplifier, LmsPowerAmplifier};
/// Frequency band: Low (<1.5 GHz) or High (>=1.5 GHz).
#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub enum Band {
    /// Low band: frequencies below 1.5 GHz.
    Low = 0,
    /// High band: frequencies at or above 1.5 GHz.
    High = 1,
}

impl From<u64> for Band {
    fn from(freq: u64) -> Self {
        if freq < bandwidth::BLADERF1_BAND_HIGH as u64 {
            Band::Low
        } else {
            Band::High
        }
    }
}

impl From<u32> for Band {
    fn from(freq: u32) -> Self {
        Band::from(freq as u64)
    }
}
/// PLL tuning strategy.
#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub enum Tune {
    /// Full tuning with VCOCAP search.
    Normal = 0,
    /// Quick tuning using previously determined VCOCAP.
    Quick = 1,
}
/// LMS6002D register addresses used for dump/restore operations.
pub const LMS_REG_DUMPSET: [u8; 107] = [
    0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0E, 0x0F, 0x10, 0x11,
    0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x1B, 0x1C, 0x1D, 0x1E, 0x1F, 0x20, 0x21,
    0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28, 0x29, 0x2A, 0x2B, 0x2C, 0x2D, 0x2E, 0x2F, 0x30, 0x31,
    0x32, 0x33, 0x34, 0x35, 0x36, 0x40, 0x41, 0x42, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49, 0x4A,
    0x4B, 0x4C, 0x4D, 0x4E, 0x4F, 0x50, 0x51, 0x52, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5A,
    0x5B, 0x5C, 0x5D, 0x5E, 0x5F, 0x60, 0x61, 0x62, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x70, 0x71,
    0x72, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7A, 0x7B, 0x7C,
];
/// Maximum VCOCAP register value.
pub const VCOCAP_MAX_VALUE: u8 = 0x3f;
/// Minimum VCOCAP estimate value for interpolation.
pub const VCOCAP_EST_MIN: u8 = 15;
/// Maximum VCOCAP estimate value for interpolation.
pub const VCOCAP_EST_MAX: u8 = 55;
/// VCOCAP estimation range (EST_MAX - EST_MIN).
pub const VCOCAP_EST_RANGE: u8 = VCOCAP_EST_MAX - VCOCAP_EST_MIN;
/// VCOCAP estimation threshold for convergence.
pub const VCOCAP_EST_THRESH: u8 = 7;
/// Frequency flag indicating low band operation.
pub const LMS_FREQ_FLAGS_LOW_BAND: u8 = 1 << 0;
/// Frequency flag to force use of estimated VCOCAP without searching.
pub const LMS_FREQ_FLAGS_FORCE_VCOCAP: u8 = 1 << 1;
/// XB-200 expansion GPIO: enable bit.
pub const LMS_FREQ_XB_200_ENABLE: u8 = 1 << 7;
/// XB-200 expansion GPIO: RX module bit.
pub const LMS_FREQ_XB_200_MODULE_RX: u8 = 1 << 6;
/// XB-200 expansion GPIO: filter switch mask.
pub const LMS_FREQ_XB_200_FILTER_SW: u8 = 3 << 4;
/// XB-200 expansion GPIO: filter switch bit shift.
pub const LMS_FREQ_XB_200_FILTER_SW_SHIFT: u8 = 4;
/// XB-200 expansion GPIO: signal path mask.
pub const LMS_FREQ_XB_200_PATH: u8 = 3 << 2;
/// XB-200 expansion GPIO: signal path bit shift.
pub const LMS_FREQ_XB_200_PATH_SHIFT: u8 = 2;
/// VTUNE status delay for large VCOCAP steps (microseconds).
pub const VTUNE_DELAY_LARGE: u8 = 50;
/// VTUNE status delay for small VCOCAP steps (microseconds).
pub const VTUNE_DELAY_SMALL: u8 = 25;
/// Maximum iterations for VTUNE convergence loops.
pub const VTUNE_MAX_ITERATIONS: u8 = 20;
/// Maximum allowed VCOCAP distance between low and high limits.
pub const VCOCAP_MAX_LOW_HIGH: u8 = 12;

/// VCO tuning (VTUNE) status read from LMS6002D.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VcoState {
    /// VCO is within the nominal tuning range.
    Norm = 0,
    /// VCO is below the nominal tuning range (increase VCOCAP).
    Low = 1,
    /// VCO is above the nominal tuning range (decrease VCOCAP).
    High = 2,
}

impl TryFrom<u8> for VcoState {
    type Error = Error;

    fn try_from(value: u8) -> std::result::Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Norm),
            1 => Ok(Self::Low),
            2 => Ok(Self::High),
            _ => Err(Error::BoardState("invalid VCO state")),
        }
    }
}
/// LMS6002D RF transceiver interface.
pub struct Lms6002d<'a> {
    pub(crate) nios: &'a mut NiosCore,
}

impl<'a> Lms6002d<'a> {
    pub(crate) fn read(&mut self, addr: u8) -> Result<u8> {
        self.nios.nios_read::<u8, u8>(NiosPkt8x8Target::Lms6, addr)
    }

    pub(crate) fn write(&mut self, addr: u8, data: u8) -> Result<()> {
        self.nios
            .nios_write::<u8, u8>(NiosPkt8x8Target::Lms6, addr, data)
    }

    pub(crate) fn set(&mut self, addr: u8, mask: u8) -> Result<()> {
        let data = self.read(addr)?;
        self.write(addr, data | mask)
    }

    pub(crate) fn clear(&mut self, addr: u8, mask: u8) -> Result<()> {
        let data = self.read(addr)?;
        self.write(addr, data & !mask)
    }

    #[allow(dead_code)]
    pub(crate) fn soft_reset(&mut self) -> Result<()> {
        self.write(0x05, 0x12)?;
        self.write(0x05, 0x32)
    }

    pub(crate) fn enable_rffe(&mut self, channel: Channel, enable: bool) -> Result<()> {
        let (addr, shift) = if channel == Channel::Tx {
            (0x40u8, 1u8)
        } else {
            (0x70u8, 0u8)
        };
        let mut data = self.read(addr)?;
        if enable {
            data |= 1 << shift;
        } else {
            data &= !(1 << shift);
        }
        self.write(addr, data)
    }

    pub(crate) fn select_band(&mut self, channel: Channel, band: Band) -> Result<()> {
        if self.is_loopback_enabled()? {
            log::debug!("Loopback enabled!");
            return Ok(());
        }
        match channel {
            Channel::Tx => {
                let lms_pa = if band == Band::Low {
                    LmsPowerAmplifier::Pa1
                } else {
                    LmsPowerAmplifier::Pa2
                };
                self.select_pa(lms_pa)
            }
            Channel::Rx => {
                let lms_lna = if band == Band::Low {
                    LmsLowNoiseAmplifier::Lna1
                } else {
                    LmsLowNoiseAmplifier::Lna2
                };
                self.select_lna(lms_lna)
            }
        }
    }

    pub(crate) fn read_expansion_gpio(&mut self) -> Result<u32> {
        self.nios.nios_expansion_gpio_read()
    }
}
