use crate::BladeRf1;
use crate::{Error, Result};
use bladerf_globals::bladerf1::{
    BLADERF_GPIO_RX_MUX_MASK, BLADERF_GPIO_RX_MUX_SHIFT, BladeRf1RxMux,
};

impl BladeRf1 {
    /******************************************************************************/
    /* Sample RX FPGA Mux */
    /******************************************************************************/

    pub fn set_rx_mux(&self, mode: BladeRf1RxMux) -> Result<()> {
        // CHECK_BOARD_STATE(STATE_INITIALIZED);

        // Validate desired mux mode
        if mode != BladeRf1RxMux::MuxInvalid {
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

    pub fn get_rx_mux(&self) -> Result<BladeRf1RxMux> {
        // CHECK_BOARD_STATE(STATE_INITIALIZED);

        let mut config_gpio = self.config_gpio_read()?;

        // Extract RX mux bits
        config_gpio &= BLADERF_GPIO_RX_MUX_MASK as u32;
        config_gpio >>= BLADERF_GPIO_RX_MUX_SHIFT;
        let val = BladeRf1RxMux::from(config_gpio);

        // Ensure it's a valid/supported value
        if val == BladeRf1RxMux::MuxInvalid {
            log::error!("Invalid rx mux mode read from config gpio");
            Err(Error::Invalid)
        } else {
            Ok(val)
        }
    }
}
