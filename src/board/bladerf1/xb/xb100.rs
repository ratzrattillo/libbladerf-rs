use crate::Result;
use crate::bladerf1::BladeRf1;
use crate::board::bladerf1::xb::{
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

        // The original libbladerf from Nuand saves the state of attached boards in a
        // separate structure. We try to determine the attached XB100 ONLY by reading
        // the NIOS_PKT_32X32_TARGET_EXP register. It seems like this register is
        // initialized to 0xffffffff when no board is attached at all. Thus, we return
        // "false", if the register is 0xffffffff.
        // TODO: Verify, if this is really the case, as for now it is an assumption.
        let xb_gpio = interface.lock().unwrap().nios_expansion_gpio_read()?;
        if xb_gpio == 0xffffffff {
            return Ok(false);
        }
        Ok((xb_gpio & mask) != 0)

        // Ok((interface.lock().unwrap().nios_expansion_gpio_read()? & mask) != 0)
    }

    /// This method does not do anything. Attach-operations are not required for XB100.
    pub fn xb100_attach(&self) -> Result<()> {
        Ok(())
    }

    /// This method does not do anything. Detach-operations are not required for XB100.
    pub fn xb100_detach(&self) -> Result<()> {
        Ok(())
    }

    /// Enable the XB100 expansion board
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

    /// This method does not do anything. Init-operations are not required for XB100.
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
