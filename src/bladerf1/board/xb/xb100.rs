//! XB-100 LED expansion board GPIO control.
//!
//! The XB-100 provides 8 user LEDs (D1–D8) and one tri-color LED (TLED
//! with red, green, blue components), all accessible through the
//! expansion GPIO.

use crate::bladerf1::board::RfLinkSession;
use crate::error::Result;

macro_rules! bladerf_xb_gpio {
    ($n:expr) => {
        (1 << ($n - 1)) as u8
    };
}

const BLADERF_XB_GPIO_20: u8 = bladerf_xb_gpio!(20);
const BLADERF_XB_GPIO_21: u8 = bladerf_xb_gpio!(21);
const BLADERF_XB_GPIO_22: u8 = bladerf_xb_gpio!(22);
const BLADERF_XB_GPIO_23: u8 = bladerf_xb_gpio!(23);
const BLADERF_XB_GPIO_24: u8 = bladerf_xb_gpio!(24);
const BLADERF_XB_GPIO_25: u8 = bladerf_xb_gpio!(25);
const BLADERF_XB_GPIO_28: u8 = bladerf_xb_gpio!(28);
const BLADERF_XB_GPIO_29: u8 = bladerf_xb_gpio!(29);
const BLADERF_XB_GPIO_30: u8 = bladerf_xb_gpio!(30);
const BLADERF_XB_GPIO_31: u8 = bladerf_xb_gpio!(31);
const BLADERF_XB_GPIO_32: u8 = bladerf_xb_gpio!(32);

const BLADERF_XB100_LED_D1: u8 = BLADERF_XB_GPIO_24;
const BLADERF_XB100_LED_D2: u8 = BLADERF_XB_GPIO_32;
const BLADERF_XB100_LED_D3: u8 = BLADERF_XB_GPIO_30;
const BLADERF_XB100_LED_D4: u8 = BLADERF_XB_GPIO_28;
const BLADERF_XB100_LED_D5: u8 = BLADERF_XB_GPIO_23;
const BLADERF_XB100_LED_D6: u8 = BLADERF_XB_GPIO_25;
const BLADERF_XB100_LED_D7: u8 = BLADERF_XB_GPIO_31;
const BLADERF_XB100_LED_D8: u8 = BLADERF_XB_GPIO_29;
const BLADERF_XB100_TLED_RED: u8 = BLADERF_XB_GPIO_22;
const BLADERF_XB100_TLED_GREEN: u8 = BLADERF_XB_GPIO_21;
const BLADERF_XB100_TLED_BLUE: u8 = BLADERF_XB_GPIO_20;

/// Bitmask for all XB-100 GPIO pins, used for board detection.
pub(crate) const XB100_DETECT_MASK: u32 = (BLADERF_XB100_LED_D1
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

const XB100_LED_MASK: u32 = XB100_DETECT_MASK;

impl RfLinkSession<'_> {
    /// Prepares the XB-100 board. Currently a no-op placeholder.
    pub fn xb100_attach(&mut self) -> Result<()> {
        self.require_initialized()?;
        Ok(())
    }
    /// Enables the XB-100 board. When enabled, configures all LED GPIO pins
    /// as outputs and turns the LEDs on. Requires the board to be initialized.
    pub fn xb100_enable(&mut self, enable: bool) -> Result<()> {
        self.require_initialized()?;
        if enable {
            self.nios
                .nios_expansion_gpio_dir_write(XB100_LED_MASK, XB100_LED_MASK)?;
            self.nios
                .nios_expansion_gpio_write(XB100_LED_MASK, XB100_LED_MASK)?;
        }
        Ok(())
    }
    /// Initializes the XB-100 board. Currently a no-op placeholder.
    pub fn xb100_init(&mut self) -> Result<()> {
        self.require_initialized()?;
        Ok(())
    }
}
