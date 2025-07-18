use crate::nios::Nios;
use anyhow::Result;
use bladerf_nios::NIOS_PKT_8X16_TARGET_VCTCXO_DAC;
use nusb::Interface;

pub struct DAC161S055 {
    interface: Interface,
}

impl DAC161S055 {
    pub fn new(interface: Interface) -> Self {
        Self { interface }
    }

    pub fn write(&self, value: u16) -> Result<()> {
        /* Ensure the device is in write-through mode */
        self.interface
            .nios_write::<u8, u16>(NIOS_PKT_8X16_TARGET_VCTCXO_DAC, 0x28, 0x0)?;

        /* Write DAC value to channel 0 */
        self.interface
            .nios_write::<u8, u16>(NIOS_PKT_8X16_TARGET_VCTCXO_DAC, 0x8, value)
    }
}
