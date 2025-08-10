use crate::BladeRf1;
use crate::Result;
use crate::board::bladerf1::expansion_boards::{
    BLADERF_XB100_LED_D1, BLADERF_XB100_LED_D2, BLADERF_XB100_LED_D3, BLADERF_XB100_LED_D4,
    BLADERF_XB100_LED_D5, BLADERF_XB100_LED_D6, BLADERF_XB100_LED_D7, BLADERF_XB100_LED_D8,
    BLADERF_XB100_TLED_BLUE, BLADERF_XB100_TLED_GREEN, BLADERF_XB100_TLED_RED,
};
use crate::nios::Nios;
use nusb::Interface;
use std::sync::{Arc, Mutex};

impl BladeRf1 {
    /// Trying to detect if XB100 is enabled by reading the BLADERF_XB100* gpio Flags,
    /// which is set in xb100_enable(). Might be not the best, or correct way.
    pub fn xb100_is_enabled(interface: &Arc<Mutex<Interface>>) -> Result<bool> {
        let mask: u32 = (BLADERF_XB100_LED_D1
            | BLADERF_XB100_LED_D2
            | BLADERF_XB100_LED_D3
            | BLADERF_XB100_LED_D4
            | BLADERF_XB100_LED_D5
            | BLADERF_XB100_LED_D6
            | BLADERF_XB100_LED_D7
            | BLADERF_XB100_LED_D8
            | BLADERF_XB100_TLED_RED
            | BLADERF_XB100_TLED_GREEN
            | BLADERF_XB100_TLED_BLUE) as u32;

        Ok((interface.lock().unwrap().nios_expansion_gpio_read()? & mask) != 0)
    }

    pub fn xb100_attach(&self) -> Result<()> {
        Ok(())
    }

    pub fn xb100_detach(&self) -> Result<()> {
        Ok(())
    }

    pub fn xb100_enable(&self, enable: bool) -> Result<()> {
        let mask: u32 = (BLADERF_XB100_LED_D1
            | BLADERF_XB100_LED_D2
            | BLADERF_XB100_LED_D3
            | BLADERF_XB100_LED_D4
            | BLADERF_XB100_LED_D5
            | BLADERF_XB100_LED_D6
            | BLADERF_XB100_LED_D7
            | BLADERF_XB100_LED_D8
            | BLADERF_XB100_TLED_RED
            | BLADERF_XB100_TLED_GREEN
            | BLADERF_XB100_TLED_BLUE) as u32;

        if enable {
            let interface = self.interface.lock().unwrap();

            interface.nios_expansion_gpio_dir_write(mask, mask)?;
            interface.nios_expansion_gpio_write(mask, mask)?;
        }

        Ok(())
    }

    pub fn xb100_init(&self) -> Result<()> {
        Ok(())
    }

    // pub fn xb100_gpio_read(&self) -> Result<u32> {
    //     self.interface.nios_expansion_gpio_read()
    // }

    // pub fn xb100_gpio_write(&self, val: u32) -> Result<()> {
    //     self.xb100_gpio_masked_write(0xffffffff, val)
    // }

    // pub fn xb100_gpio_masked_write(&self, mask: u32, val: u32) -> Result<()> {
    //     self.interface.nios_expansion_gpio_write(mask, val)
    // }
    //
    // pub fn xb100_gpio_dir_read(&self) -> Result<u32> {
    //     self.interface.nios_expansion_gpio_dir_read()
    // }
    //
    // pub fn xb100_gpio_dir_write(&self, val: u32) -> Result<()> {
    //     self.xb100_gpio_dir_masked_write(0xffffffff, val)
    // }
    //
    // pub fn xb100_gpio_dir_masked_write(&self, mask: u32, val: u32) -> Result<()> {
    //     self.interface.nios_expansion_gpio_dir_write(mask, val)
    // }
}
