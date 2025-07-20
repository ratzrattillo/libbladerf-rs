use crate::BladeRf1;
use crate::Result;
use crate::board::bladerf1::expansion_boards::{
    BLADERF_XB100_LED_D1, BLADERF_XB100_LED_D2, BLADERF_XB100_LED_D3, BLADERF_XB100_LED_D4,
    BLADERF_XB100_LED_D5, BLADERF_XB100_LED_D6, BLADERF_XB100_LED_D7, BLADERF_XB100_LED_D8,
    BLADERF_XB100_TLED_BLUE, BLADERF_XB100_TLED_GREEN, BLADERF_XB100_TLED_RED,
};
use crate::nios::Nios;

pub struct Xb100 {}

impl BladeRf1 {
    pub fn xb100_attach(&mut self) -> Result<()> {
        self.xb100 = Some(Xb100 {});
        Ok(())
    }

    pub fn xb100_detach(&mut self) -> Result<()> {
        self.xb100 = None;
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

        let outputs: u32 = mask;
        let default_values: u32 = mask;

        if enable {
            self.interface.nios_expansion_gpio_write(mask, outputs)?;
            self.interface
                .nios_expansion_gpio_write(mask, default_values)?;
        }

        Ok(())
    }

    pub fn xb100_init(&self) -> Result<()> {
        Ok(())
    }

    pub fn xb100_gpio_read(&self) -> Result<u32> {
        self.interface.nios_expansion_gpio_read()
    }

    pub fn xb100_gpio_write(&self, val: u32) -> Result<()> {
        self.xb100_gpio_masked_write(0xffffffff, val)
    }

    pub fn xb100_gpio_masked_write(&self, mask: u32, val: u32) -> Result<()> {
        self.interface.nios_expansion_gpio_write(mask, val)
    }

    pub fn xb100_gpio_dir_read(&self) -> Result<u32> {
        self.interface.nios_expansion_gpio_dir_read()
    }

    pub fn xb100_gpio_dir_write(&self, val: u32) -> Result<()> {
        self.xb100_gpio_dir_masked_write(0xffffffff, val)
    }

    pub fn xb100_gpio_dir_masked_write(&self, mask: u32, val: u32) -> Result<()> {
        self.interface.nios_expansion_gpio_dir_write(mask, val)
    }
}
