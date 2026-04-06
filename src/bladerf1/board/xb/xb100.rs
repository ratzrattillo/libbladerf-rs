use crate::bladerf1::BladeRf1;
use crate::bladerf1::board::xb::{
    BLADERF_XB100_LED_D1, BLADERF_XB100_LED_D2, BLADERF_XB100_LED_D3, BLADERF_XB100_LED_D4,
    BLADERF_XB100_LED_D5, BLADERF_XB100_LED_D6, BLADERF_XB100_LED_D7, BLADERF_XB100_LED_D8,
    BLADERF_XB100_TLED_BLUE, BLADERF_XB100_TLED_GREEN, BLADERF_XB100_TLED_RED, detect_xb_board,
};
use crate::bladerf1::nios_client::NiosInterface;
use crate::error::Result;
use std::sync::{Arc, Mutex};
const XB100_LED_MASK: u32 = (BLADERF_XB100_LED_D1
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
impl BladeRf1 {
    pub fn xb100_is_enabled(interface: &Arc<Mutex<NiosInterface>>) -> Result<bool> {
        detect_xb_board(interface, XB100_LED_MASK)
    }
    pub fn xb100_attach(&self) -> Result<()> {
        Ok(())
    }
    pub fn xb100_enable(&self, enable: bool) -> Result<()> {
        if enable {
            let mut interface = self.interface.lock().unwrap();
            interface.nios_expansion_gpio_dir_write(XB100_LED_MASK, XB100_LED_MASK)?;
            interface.nios_expansion_gpio_write(XB100_LED_MASK, XB100_LED_MASK)?;
        }
        Ok(())
    }
    pub fn xb100_init(&self) -> Result<()> {
        Ok(())
    }
}
