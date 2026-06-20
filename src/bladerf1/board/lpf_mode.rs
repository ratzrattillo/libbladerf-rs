//! LPF mode control for BladeRF1.
//!
//! Selects the operating mode of the LMS6002D's digital low-pass filter:
//! normal filtering, disabled, or bypassed through without filtering.

use crate::bladerf1::board::RfLinkSession;
use crate::bladerf1::hardware::lms6002d::LpfMode;
use crate::channel::Channel;
use crate::error::Result;

impl RfLinkSession<'_> {
    /// Sets the LPF operating mode for the given channel.
    ///
    /// Returns `Error::NotInitialized` if the board has not been initialized.
    pub fn set_lpf_mode(&mut self, channel: Channel, mode: LpfMode) -> Result<()> {
        self.require_initialized()?;
        self.lms().lpf_set_mode(channel, mode)
    }

    /// Returns the current LPF operating mode for the given channel.
    ///
    /// Returns `Error::NotInitialized` if the board has not been initialized.
    pub fn get_lpf_mode(&mut self, channel: Channel) -> Result<LpfMode> {
        self.require_initialized()?;
        self.lms().lpf_get_mode(channel)
    }
}
