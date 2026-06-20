//! DAC161S055 VCTCXO trim DAC driver.
//!
//! The DAC161S055 is a 16-bit rail-to-rail voltage-output DAC that controls the
//! VCTCXO oscillator frequency via its SPI interface.

use crate::error::Result;
use crate::nios_client::NiosCore;
use crate::protocol::nios::NiosPkt8x16Target;

/// DAC161S055 16-bit voltage-output DAC interface.
pub struct Dac161s055<'a> {
    pub(crate) nios: &'a mut NiosCore,
}

impl<'a> Dac161s055<'a> {
    /// Reads the current DAC register value.
    pub fn read(&mut self) -> Result<u16> {
        self.nios
            .nios_read::<u8, u16>(NiosPkt8x16Target::VctcxoDac, 0x98)
    }

    /// Writes a 16-bit value to the DAC to set the VCTCXO trim voltage.
    pub fn write(&mut self, value: u16) -> Result<()> {
        self.nios
            .nios_write::<u8, u16>(NiosPkt8x16Target::VctcxoDac, 0x28, 0x0u16)?;
        self.nios
            .nios_write::<u8, u16>(NiosPkt8x16Target::VctcxoDac, 0x8, value)
    }
}
