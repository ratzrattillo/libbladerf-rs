//! RX mux selection for the BladeRF1.
//!
//! Controls the input source routed to the RX data path via config GPIO bits.
//! Options include baseband samples from the LMS6002D, internal test counters,
//! or digital loopback.

use crate::bladerf1::board::RfLinkSession;
use crate::{Error, Result};

/// RX input select.
///
/// Determines the source routed into the RX data pipeline. The baseband
/// variant passes samples directly from the LMS6002D. The counter variants
/// inject internal test patterns (12-bit or 32-bit counters) for diagnostics.
/// Digital loopback routes TX data directly to the RX path without the RF chain.
#[derive(PartialEq, Eq, Debug, Clone, Copy)]
#[repr(u8)]
pub enum RxMux {
    /// Normal RX path: baseband samples from the LMS6002D.
    MuxBaseband = 0,
    /// Internal 12-bit counter test pattern.
    Mux12BitCounter = 1,
    /// Internal 32-bit counter test pattern.
    Mux32BitCounter = 2,
    /// Digital loopback: TX data routed directly to RX.
    MuxDigitalLoopback = 4,
}

impl TryFrom<u32> for RxMux {
    type Error = Error;
    /// Converts a raw GPIO mux value into an `RxMux` variant.
    ///
    /// Returns `Error::Argument` for any value not matching a known mux setting.
    fn try_from(value: u32) -> Result<Self> {
        match value {
            0 => Ok(RxMux::MuxBaseband),
            1 => Ok(RxMux::Mux12BitCounter),
            2 => Ok(RxMux::Mux32BitCounter),
            4 => Ok(RxMux::MuxDigitalLoopback),
            _ => Err(Error::Argument("invalid RX mux value".into())),
        }
    }
}

/// Bit mask for the RX mux field in the config GPIO register.
pub const BLADERF_GPIO_RX_MUX_MASK: u16 = 7 << BLADERF_GPIO_RX_MUX_SHIFT;
/// Bit shift for the RX mux field in the config GPIO register.
pub const BLADERF_GPIO_RX_MUX_SHIFT: u16 = 8;

impl RfLinkSession<'_> {
    /// Sets the RX input mux to the specified source.
    ///
    /// The board must be initialized. The mux is configured via the config GPIO
    /// register using an atomic read-modify-write.
    ///
    /// Returns `Error::BoardState` if the board is not initialized.
    pub fn set_rx_mux(&mut self, mode: RxMux) -> Result<()> {
        self.require_initialized()?;
        let rx_mux_val = (mode as u32) << BLADERF_GPIO_RX_MUX_SHIFT;
        self.config_gpio_modify(|gpio| (gpio & !(BLADERF_GPIO_RX_MUX_MASK as u32)) | rx_mux_val)
    }

    /// Returns the currently selected RX input mux.
    ///
    /// Reads the config GPIO register and extracts the RX mux field.
    /// Returns `Error::BoardState` if the board is not initialized.
    pub fn get_rx_mux(&mut self) -> Result<RxMux> {
        self.require_initialized()?;
        let mut config_gpio = self.config_gpio_read()?;
        config_gpio &= BLADERF_GPIO_RX_MUX_MASK as u32;
        config_gpio >>= BLADERF_GPIO_RX_MUX_SHIFT;
        RxMux::try_from(config_gpio)
    }
}
