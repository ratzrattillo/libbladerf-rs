use crate::BladeRf1;
use anyhow::{Result, anyhow};
use bladerf_globals::bladerf1::{
    BLADERF_GPIO_RX_MUX_MASK, BLADERF_GPIO_RX_MUX_SHIFT, BladerfRxMux,
};

impl BladeRf1 {
    /******************************************************************************/
    /* Sample RX FPGA Mux */
    /******************************************************************************/

    pub fn set_rx_mux(&self, mode: BladerfRxMux) -> Result<()> {
        // CHECK_BOARD_STATE(STATE_INITIALIZED);

        /* Validate desired mux mode */
        if mode != BladerfRxMux::MuxInvalid {
            let rx_mux_val = (mode as u32) << BLADERF_GPIO_RX_MUX_SHIFT;

            let mut config_gpio = self.config_gpio_read()?;

            /* Clear out and assign the associated RX mux bits */
            config_gpio &= !(BLADERF_GPIO_RX_MUX_MASK as u32);
            config_gpio |= rx_mux_val;

            self.config_gpio_write(config_gpio)
        } else {
            Err(anyhow!("Invalid RX mux mode setting passed"))
        }
    }

    pub fn get_rx_mux(&self) -> Result<BladerfRxMux> {
        //CHECK_BOARD_STATE(STATE_INITIALIZED);

        let mut config_gpio = self.config_gpio_read()?;

        /* Extract RX mux bits */
        config_gpio &= BLADERF_GPIO_RX_MUX_MASK as u32;
        config_gpio >>= BLADERF_GPIO_RX_MUX_SHIFT;
        let val = BladerfRxMux::from(config_gpio);

        /* Ensure it's a valid/supported value */
        if val == BladerfRxMux::MuxBaseband {
            Err(anyhow!("Invalid rx mux mode read from config gpio"))
        } else {
            Ok(val)
        }
    }
}
