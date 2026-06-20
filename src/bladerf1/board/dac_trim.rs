//! DAC trim for VCTCXO frequency adjustment.
//!
//! The DAC161S055 is a 16-bit voltage-output DAC that generates the tuning
//! voltage for the on-board VCTCXO. Writing a new DAC code shifts the
//! VCTCXO frequency, allowing fine frequency correction. The value ranges
//! from 0x0000 (minimum voltage) to 0xFFFF (maximum voltage).

use crate::bladerf1::board::RfLinkSession;
use crate::error::Result;

impl RfLinkSession<'_> {
    /// Writes a 16-bit trim value to the DAC161S055 to adjust the VCTCXO frequency.
    ///
    /// The DAC output voltage shifts the VCTCXO oscillation frequency, enabling
    /// fine frequency calibration. The value 0x0000 produces the minimum output
    /// voltage and 0xFFFF produces the maximum.
    ///
    /// Returns `Error::BoardState` if the board is not initialized.
    pub fn set_dac_trim(&mut self, value: u16) -> Result<()> {
        self.require_initialized()?;
        self.dac().write(value)
    }

    /// Returns the current 16-bit DAC trim value.
    ///
    /// Reads the DAC161S055 output register to determine the active VCTCXO
    /// tuning setting.
    ///
    /// Returns `Error::BoardState` if the board is not initialized.
    pub fn get_dac_trim(&mut self) -> Result<u16> {
        self.require_initialized()?;
        self.dac().read()
    }
}
