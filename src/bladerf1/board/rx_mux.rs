use crate::bladerf1::BladeRf1;
use crate::{Error, Result};
#[derive(PartialEq, Debug, Clone, Copy)]
pub enum RxMux {
    MuxBaseband = 0,
    Mux12BitCounter = 1,
    Mux32BitCounter = 2,
    MuxDigitalLoopback = 4,
}
impl TryFrom<u32> for RxMux {
    type Error = Error;
    fn try_from(value: u32) -> Result<Self> {
        match value {
            0 => Ok(RxMux::MuxBaseband),
            1 => Ok(RxMux::Mux12BitCounter),
            2 => Ok(RxMux::Mux32BitCounter),
            4 => Ok(RxMux::MuxDigitalLoopback),
            _ => Err(Error::Argument("invalid RX mux value".into())),
        }
    }
}
pub const BLADERF_GPIO_RX_MUX_MASK: u16 = 7 << BLADERF_GPIO_RX_MUX_SHIFT;
pub const BLADERF_GPIO_RX_MUX_SHIFT: u16 = 8;
impl BladeRf1 {
    pub fn set_rx_mux(&mut self, mode: RxMux) -> Result<()> {
        let rx_mux_val = (mode as u32) << BLADERF_GPIO_RX_MUX_SHIFT;
        let mut config_gpio = self.config_gpio_read()?;
        config_gpio &= !(BLADERF_GPIO_RX_MUX_MASK as u32);
        config_gpio |= rx_mux_val;
        self.config_gpio_write(config_gpio)
    }
    pub fn get_rx_mux(&mut self) -> Result<RxMux> {
        let mut config_gpio = self.config_gpio_read()?;
        config_gpio &= BLADERF_GPIO_RX_MUX_MASK as u32;
        config_gpio >>= BLADERF_GPIO_RX_MUX_SHIFT;
        RxMux::try_from(config_gpio)
    }
}
