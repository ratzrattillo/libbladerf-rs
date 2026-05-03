#[cfg(feature = "xb100")]
mod xb100;
#[cfg(feature = "xb200")]
pub mod xb200;
#[cfg(feature = "xb300")]
mod xb300;

use crate::bladerf1::BladeRf1;
#[cfg(any(feature = "xb100", feature = "xb200", feature = "xb300"))]
use crate::bladerf1::nios_client::NiosClient;
use crate::error::{Error, Result};

#[derive(Clone, PartialEq, Debug)]
pub enum ExpansionBoard {
    XbNone = 0,
    #[cfg(feature = "xb100")]
    Xb100,
    #[cfg(feature = "xb200")]
    Xb200,
    #[cfg(feature = "xb300")]
    Xb300,
}

#[cfg(any(feature = "xb100", feature = "xb200"))]
pub(crate) fn detect_xb_board(nios: &mut NiosClient, check_mask: u32) -> Result<bool> {
    let gpio = nios.nios_expansion_gpio_read()?;
    if gpio == 0xffffffff {
        return Ok(false);
    }
    Ok((gpio & check_mask) != 0)
}

#[cfg(feature = "xb300")]
pub(crate) fn detect_xb_board_by_dir(nios: &mut NiosClient, check_mask: u32) -> Result<bool> {
    let gpio_dir = nios.nios_expansion_gpio_dir_read()?;
    Ok((gpio_dir & check_mask) != 0)
}

#[cfg(feature = "xb200")]
pub(crate) fn xb200_is_enabled(nios: &mut NiosClient) -> Result<bool> {
    detect_xb_board(nios, xb200::BLADERF_XB_RF_ON)
}

impl BladeRf1 {
    pub fn expansion_get_attached(&mut self) -> Result<ExpansionBoard> {
        if self.nios.nios_expansion_gpio_read()? == 0xffffffff {
            return Ok(ExpansionBoard::XbNone);
        }
        #[cfg(feature = "xb100")]
        if detect_xb_board(&mut self.nios, xb100::XB100_DETECT_MASK)? {
            return Ok(ExpansionBoard::Xb100);
        }
        #[cfg(feature = "xb200")]
        if detect_xb_board(&mut self.nios, xb200::BLADERF_XB_RF_ON)? {
            return Ok(ExpansionBoard::Xb200);
        }
        #[cfg(feature = "xb300")]
        if detect_xb_board_by_dir(&mut self.nios, xb300::XB300_DETECT_MASK)? {
            return Ok(ExpansionBoard::Xb300);
        }
        Ok(ExpansionBoard::XbNone)
    }

    pub fn expansion_attach(&mut self, xb: ExpansionBoard) -> Result<()> {
        let attached = self.expansion_get_attached()?;
        if xb != attached && attached != ExpansionBoard::XbNone {
            log::error!("Switching XB types is not supported.");
            return Err(Error::Unsupported("switching XB types"));
        }
        #[cfg(feature = "xb100")]
        if xb == ExpansionBoard::Xb100 {
            self.xb100_attach()?;
            self.xb100_enable(true)?;
            self.xb100_init()?;
            return Ok(());
        }
        #[cfg(feature = "xb200")]
        if xb == ExpansionBoard::Xb200 {
            self.xb200_attach()?;
            self.xb200_enable(true)?;
            self.xb200_init()?;
            return Ok(());
        }
        #[cfg(feature = "xb300")]
        if xb == ExpansionBoard::Xb300 {
            self.xb300_attach()?;
            self.xb300_enable(true)?;
            self.xb300_init()?;
            return Ok(());
        }
        if xb == ExpansionBoard::XbNone {
            log::error!("Disabling an attached XB is not supported.");
            return Err(Error::Unsupported("disabling attached XB"));
        }
        log::error!("Unknown xb type: {xb:?}");
        Err(Error::Unsupported("unknown XB type"))
    }
}
