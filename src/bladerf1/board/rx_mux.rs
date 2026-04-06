use crate::bladerf1::BladeRf1;
use crate::{Error, Result};
#[derive(PartialEq, Debug, Clone)]
pub enum RxMux {
    MuxInvalid = -1,
    MuxBaseband = 0,
    Mux12BitCounter = 1,
    Mux32BitCounter = 2,
    MuxDigitalLoopback = 4,
}
impl From<u32> for RxMux {
    fn from(value: u32) -> Self {
        match value {
            0 => RxMux::MuxBaseband,
            1 => RxMux::Mux12BitCounter,
            2 => RxMux::Mux32BitCounter,
            4 => RxMux::MuxDigitalLoopback,
            _ => RxMux::MuxInvalid,
        }
    }
}
pub const BLADERF_GPIO_RX_MUX_MASK: u16 = 7 << BLADERF_GPIO_RX_MUX_SHIFT;
pub const BLADERF_GPIO_RX_MUX_SHIFT: u16 = 8;
impl BladeRf1 {
    pub fn set_rx_mux(&self, mode: RxMux) -> Result<()> {
        if mode != RxMux::MuxInvalid {
            let rx_mux_val = (mode as u32) << BLADERF_GPIO_RX_MUX_SHIFT;
            let mut config_gpio = self.config_gpio_read()?;
            config_gpio &= !(BLADERF_GPIO_RX_MUX_MASK as u32);
            config_gpio |= rx_mux_val;
            self.config_gpio_write(config_gpio)
        } else {
            log::error!("Invalid RX mux mode setting passed");
            Err(Error::Invalid)
        }
    }
    pub fn get_rx_mux(&self) -> Result<RxMux> {
        let mut config_gpio = self.config_gpio_read()?;
        config_gpio &= BLADERF_GPIO_RX_MUX_MASK as u32;
        config_gpio >>= BLADERF_GPIO_RX_MUX_SHIFT;
        let val = RxMux::from(config_gpio);
        if val == RxMux::MuxInvalid {
            log::error!("Invalid rx mux mode read from config gpio");
            Err(Error::Invalid)
        } else {
            Ok(val)
        }
    }
}
