//! VCTCXO tamer mode control.
//!
//! The VCTCXO tamer provides an external reference oscillator input to
//! discipline the on-board VCTCXO. Selecting a tamer mode connects the
//! reference input (1 PPS or 10 MHz) so the firmware can lock the VCTCXO
//! frequency to the external reference for improved timing stability.

use crate::bladerf1::board::RfLinkSession;
use crate::error::{Error, Result};
use crate::protocol::nios::NiosPkt8x8Target;

/// VCTCXO tamer reference mode.
///
/// Selects the external reference signal used to discipline the VCTCXO
/// oscillator. When disabled the VCTCXO runs free with optional DAC trim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum VctcxoTamerMode {
    /// Tamer is disabled; VCTCXO runs free.
    Disabled = 0,
    /// 1 PPS (pulse-per-second) reference input.
    Pps1 = 1,
    /// 10 MHz continuous reference input.
    Mhz10 = 2,
}

impl TryFrom<u8> for VctcxoTamerMode {
    type Error = Error;
    /// Converts a raw tamer mode byte into a `VctcxoTamerMode` variant.
    ///
    /// Returns `Error::Unsupported` for any unrecognized value.
    fn try_from(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::Disabled),
            1 => Ok(Self::Pps1),
            2 => Ok(Self::Mhz10),
            _ => Err(Error::Unsupported("invalid VCTCXO tamer mode")),
        }
    }
}

const MODE_ADDR: u8 = 0xFF;

impl RfLinkSession<'_> {
    /// Sets the VCTCXO tamer to the specified reference mode.
    ///
    /// Write the mode value to the NIOS VCTCXO tamer register. Use
    /// `VctcxoTamerMode::Disabled` to return the VCTCXO to free-running
    /// operation.
    ///
    /// Returns `Error::BoardState` if the board is not initialized.
    pub fn set_vctcxo_tamer_mode(&mut self, mode: VctcxoTamerMode) -> Result<()> {
        self.require_initialized()?;
        self.nios
            .nios_write::<u8, u8>(NiosPkt8x8Target::VctcxoTamer, MODE_ADDR, mode as u8)
    }

    /// Returns the current VCTCXO tamer mode.
    ///
    /// Reads the NIOS VCTCXO tamer register and decodes the mode value.
    /// Returns `Error::BoardState` if the board is not initialized, or
    /// `Error::Unsupported` if the device reports an unrecognized mode.
    pub fn get_vctcxo_tamer_mode(&mut self) -> Result<VctcxoTamerMode> {
        self.require_initialized()?;
        let raw = self
            .nios
            .nios_read::<u8, u8>(NiosPkt8x8Target::VctcxoTamer, MODE_ADDR)?;
        VctcxoTamerMode::try_from(raw)
    }
}
