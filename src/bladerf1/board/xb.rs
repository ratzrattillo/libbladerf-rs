pub(crate) mod xb100;
pub(crate) mod xb200;
pub(crate) mod xb300;
use crate::bladerf1::BladeRf1;
use crate::bladerf1::nios_client::NiosInterface;
use crate::error::{Error, Result};
use std::sync::{Arc, Mutex};
fn detect_xb_board(interface: &Arc<Mutex<NiosInterface>>, check_mask: u32) -> Result<bool> {
    let gpio = interface.lock().unwrap().nios_expansion_gpio_read()?;
    if gpio == 0xffffffff {
        return Ok(false);
    }
    Ok((gpio & check_mask) != 0)
}
fn detect_xb_board_by_dir(interface: &Arc<Mutex<NiosInterface>>, check_mask: u32) -> Result<bool> {
    let gpio_dir = interface.lock().unwrap().nios_expansion_gpio_dir_read()?;
    Ok((gpio_dir & check_mask) != 0)
}
#[derive(Clone, PartialEq, Debug)]
pub enum ExpansionBoard {
    XbNone = 0,
    Xb100,
    Xb200,
    Xb300,
}
macro_rules! bladerf_xb_gpio {
    ($n:expr) => {
        (1 << ($n - 1)) as u8
    };
}
pub(crate) const BLADERF_XB_GPIO_20: u8 = bladerf_xb_gpio!(20);
pub(crate) const BLADERF_XB_GPIO_21: u8 = bladerf_xb_gpio!(21);
pub(crate) const BLADERF_XB_GPIO_22: u8 = bladerf_xb_gpio!(22);
pub(crate) const BLADERF_XB_GPIO_23: u8 = bladerf_xb_gpio!(23);
pub(crate) const BLADERF_XB_GPIO_24: u8 = bladerf_xb_gpio!(24);
pub(crate) const BLADERF_XB_GPIO_25: u8 = bladerf_xb_gpio!(25);
pub(crate) const BLADERF_XB_GPIO_28: u8 = bladerf_xb_gpio!(28);
pub(crate) const BLADERF_XB_GPIO_29: u8 = bladerf_xb_gpio!(29);
pub(crate) const BLADERF_XB_GPIO_30: u8 = bladerf_xb_gpio!(30);
pub(crate) const BLADERF_XB_GPIO_31: u8 = bladerf_xb_gpio!(31);
pub(crate) const BLADERF_XB_GPIO_32: u8 = bladerf_xb_gpio!(32);
pub(crate) const BLADERF_XB100_LED_D1: u8 = BLADERF_XB_GPIO_24;
pub(crate) const BLADERF_XB100_LED_D2: u8 = BLADERF_XB_GPIO_32;
pub(crate) const BLADERF_XB100_LED_D3: u8 = BLADERF_XB_GPIO_30;
pub(crate) const BLADERF_XB100_LED_D4: u8 = BLADERF_XB_GPIO_28;
pub(crate) const BLADERF_XB100_LED_D5: u8 = BLADERF_XB_GPIO_23;
pub(crate) const BLADERF_XB100_LED_D6: u8 = BLADERF_XB_GPIO_25;
pub(crate) const BLADERF_XB100_LED_D7: u8 = BLADERF_XB_GPIO_31;
pub(crate) const BLADERF_XB100_LED_D8: u8 = BLADERF_XB_GPIO_29;
pub(crate) const BLADERF_XB100_TLED_RED: u8 = BLADERF_XB_GPIO_22;
pub(crate) const BLADERF_XB100_TLED_GREEN: u8 = BLADERF_XB_GPIO_21;
pub(crate) const BLADERF_XB100_TLED_BLUE: u8 = BLADERF_XB_GPIO_20;
impl BladeRf1 {
    pub fn expansion_get_attached(&self) -> Result<ExpansionBoard> {
        if self.interface.lock().unwrap().nios_expansion_gpio_read()? == 0xffffffff {
            return Ok(ExpansionBoard::XbNone);
        }
        if BladeRf1::xb100_is_enabled(&self.interface)? {
            Ok(ExpansionBoard::Xb100)
        } else if BladeRf1::xb200_is_enabled(&self.interface)? {
            Ok(ExpansionBoard::Xb200)
        } else if BladeRf1::xb300_is_enabled(&self.interface)? {
            Ok(ExpansionBoard::Xb300)
        } else {
            Ok(ExpansionBoard::XbNone)
        }
    }
    pub fn expansion_attach(&self, xb: ExpansionBoard) -> Result<()> {
        let attached = self.expansion_get_attached()?;
        if xb != attached && attached != ExpansionBoard::XbNone {
            log::error!("Switching XB types is not supported.");
            return Err(Error::Invalid);
        }
        if xb == ExpansionBoard::Xb100 {
            log::debug!("Attaching XB100");
            self.xb100_attach()?;
            log::debug!("Enabling XB100");
            self.xb100_enable(true)?;
            log::debug!("Initializing XB100");
            self.xb100_init()?;
        } else if xb == ExpansionBoard::Xb200 {
            log::trace!("Attaching XB200");
            self.xb200_attach()?;
            log::trace!("Enabling XB200");
            self.xb200_enable(true)?;
            log::trace!("Initializing XB200");
            self.xb200_init()?;
        } else if xb == ExpansionBoard::Xb300 {
            log::trace!("Attaching XB300");
            self.xb300_attach()?;
            log::trace!("Enabling XB300");
            self.xb300_enable(true)?;
            log::trace!("Initializing XB300");
            self.xb300_init()?;
        } else if xb == ExpansionBoard::XbNone {
            log::error!("Disabling an attached XB is not supported.");
            return Err(Error::Invalid);
        } else {
            log::error!("Unknown xb type: {xb:?}");
            return Err(Error::Invalid);
        }
        Ok(())
    }
}
