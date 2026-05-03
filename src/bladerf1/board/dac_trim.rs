use crate::bladerf1::BladeRf1;
use crate::bladerf1::hardware::dac161s055;
use crate::error::Result;

impl BladeRf1 {
    pub fn set_dac_trim(&mut self, value: u16) -> Result<()> {
        dac161s055::write(&mut self.nios, value)
    }

    pub fn get_dac_trim(&mut self) -> Result<u16> {
        dac161s055::read(&mut self.nios)
    }

    pub fn get_vctcxo_trim(&mut self) -> Result<u16> {
        self.read_flash_dac_trim()
    }
}
