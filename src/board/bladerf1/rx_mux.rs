use crate::bladerf1::BladeRf1;
use crate::{Error, Result};

/// RX mux modes
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

/// Bit mask representing the rx mux selection
///
/// @note These bits are set using bladerf_set_rx_mux()
pub const BLADERF_GPIO_RX_MUX_MASK: u16 = 7 << BLADERF_GPIO_RX_MUX_SHIFT;

/// Starting bit index of the RX mux values in FX3 <-> FPGA GPIO bank
pub const BLADERF_GPIO_RX_MUX_SHIFT: u16 = 8;

impl BladeRf1 {
    /******************************************************************************/
    /* Sample RX FPGA Mux */
    /******************************************************************************/

    pub fn set_rx_mux(&self, mode: RxMux) -> Result<()> {
        // CHECK_BOARD_STATE(STATE_INITIALIZED);

        // Validate desired mux mode
        if mode != RxMux::MuxInvalid {
            let rx_mux_val = (mode as u32) << BLADERF_GPIO_RX_MUX_SHIFT;

            let mut config_gpio = self.config_gpio_read()?;

            // Clear out and assign the associated RX mux bits
            config_gpio &= !(BLADERF_GPIO_RX_MUX_MASK as u32);
            config_gpio |= rx_mux_val;

            self.config_gpio_write(config_gpio)
        } else {
            log::error!("Invalid RX mux mode setting passed");
            Err(Error::Invalid)
        }
    }

    pub fn get_rx_mux(&self) -> Result<RxMux> {
        // CHECK_BOARD_STATE(STATE_INITIALIZED);

        let mut config_gpio = self.config_gpio_read()?;

        // Extract RX mux bits
        config_gpio &= BLADERF_GPIO_RX_MUX_MASK as u32;
        config_gpio >>= BLADERF_GPIO_RX_MUX_SHIFT;
        let val = RxMux::from(config_gpio);

        // Ensure it's a valid/supported value
        if val == RxMux::MuxInvalid {
            log::error!("Invalid rx mux mode read from config gpio");
            Err(Error::Invalid)
        } else {
            Ok(val)
        }
    }
}
