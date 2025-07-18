#![allow(dead_code)]

use crate::{BladeRf1, BladeRfError};
use anyhow::{Result, anyhow};
use bladerf_globals::bladerf1::BladerfXb;
use bladerf_globals::bladerf1::BladerfXb::{
    BladerfXb100, BladerfXb200, BladerfXb300, BladerfXbNone,
};

// /**
//  * @defgroup FN_EXP_IO Expansion I/O
//  *
//  * These definitions and functions provide high-level functionality for
//  * manipulating pins on the bladeRF1 U74 Expansion Header, and the associated
//  * mappings on expansion boards.
//  *
//  * These functions are thread-safe.
//  *
//  * @{
//  */
/** Expansion pin GPIO number to bitmask */
#[macro_export]
macro_rules! bladerf_xb_gpio {
    ($n:expr) => {
        (1 << ($n - 1)) as u8
    };
}

/** Specifies a pin to be an output */
#[macro_export]
macro_rules! bladerf_xb_dir_output {
    ($pin:expr) => {
        $pin as u8
    };
}

/** Specifies a pin to be an input */
#[macro_export]
macro_rules! bladerf_xb_dir_input {
    ($pin:expr) => {
        0 as u8
    };
}

/** Pin bitmask for Expansion GPIO 1 (U74 pin 11) */
pub const BLADERF_XB_GPIO_01: u8 = bladerf_xb_gpio!(1);

/** Pin bitmask for Expansion GPIO 2 (U74 pin 13) */
pub const BLADERF_XB_GPIO_02: u8 = bladerf_xb_gpio!(2);

/** Pin bitmask for Expansion GPIO 3 (U74 pin 17) */
pub const BLADERF_XB_GPIO_03: u8 = bladerf_xb_gpio!(3);

/** Pin bitmask for Expansion GPIO 4 (U74 pin 19) */
pub const BLADERF_XB_GPIO_04: u8 = bladerf_xb_gpio!(4);

/** Pin bitmask for Expansion GPIO 5 (U74 pin 23) */
pub const BLADERF_XB_GPIO_05: u8 = bladerf_xb_gpio!(5);

/** Pin bitmask for Expansion GPIO 6 (U74 pin 25) */
pub const BLADERF_XB_GPIO_06: u8 = bladerf_xb_gpio!(6);

/** Pin bitmask for Expansion GPIO 7 (U74 pin 29) */
pub const BLADERF_XB_GPIO_07: u8 = bladerf_xb_gpio!(7);

/** Pin bitmask for Expansion GPIO 8 (U74 pin 31) */
pub const BLADERF_XB_GPIO_08: u8 = bladerf_xb_gpio!(8);

/** Pin bitmask for Expansion GPIO 9 (U74 pin 35) */
pub const BLADERF_XB_GPIO_09: u8 = bladerf_xb_gpio!(9);

/** Pin bitmask for Expansion GPIO 10 (U74 pin 37) */
pub const BLADERF_XB_GPIO_10: u8 = bladerf_xb_gpio!(10);

/** Pin bitmask for Expansion GPIO 11 (U74 pin 41) */
pub const BLADERF_XB_GPIO_11: u8 = bladerf_xb_gpio!(11);

/** Pin bitmask for Expansion GPIO 12 (U74 pin 43) */
pub const BLADERF_XB_GPIO_12: u8 = bladerf_xb_gpio!(12);

/** Pin bitmask for Expansion GPIO 13 (U74 pin 47) */
pub const BLADERF_XB_GPIO_13: u8 = bladerf_xb_gpio!(13);

/** Pin bitmask for Expansion GPIO 14 (U74 pin 49) */
pub const BLADERF_XB_GPIO_14: u8 = bladerf_xb_gpio!(14);

/** Pin bitmask for Expansion GPIO 15 (U74 pin 53) */
pub const BLADERF_XB_GPIO_15: u8 = bladerf_xb_gpio!(15);

/** Pin bitmask for Expansion GPIO 16 (U74 pin 55) */
pub const BLADERF_XB_GPIO_16: u8 = bladerf_xb_gpio!(16);

/** Pin bitmask for Expansion GPIO 17 (U74 pin 12) */
pub const BLADERF_XB_GPIO_17: u8 = bladerf_xb_gpio!(17);

/** Pin bitmask for Expansion GPIO 18 (U74 pin 14) */
pub const BLADERF_XB_GPIO_18: u8 = bladerf_xb_gpio!(18);

/** Pin bitmask for Expansion GPIO 19 (U74 pin 18) */
pub const BLADERF_XB_GPIO_19: u8 = bladerf_xb_gpio!(19);

/** Pin bitmask for Expansion GPIO 20 (U74 pin 20) */
pub const BLADERF_XB_GPIO_20: u8 = bladerf_xb_gpio!(20);

/** Pin bitmask for Expansion GPIO 21 (U74 pin 24) */
pub const BLADERF_XB_GPIO_21: u8 = bladerf_xb_gpio!(21);

/** Pin bitmask for Expansion GPIO 22 (U74 pin 26) */
pub const BLADERF_XB_GPIO_22: u8 = bladerf_xb_gpio!(22);

/** Pin bitmask for Expansion GPIO 23 (U74 pin 30) */
pub const BLADERF_XB_GPIO_23: u8 = bladerf_xb_gpio!(23);

/** Pin bitmask for Expansion GPIO 24 (U74 pin 32) */
pub const BLADERF_XB_GPIO_24: u8 = bladerf_xb_gpio!(24);

/** Pin bitmask for Expansion GPIO 25 (U74 pin 36) */
pub const BLADERF_XB_GPIO_25: u8 = bladerf_xb_gpio!(25);

/** Pin bitmask for Expansion GPIO 26 (U74 pin 38) */
pub const BLADERF_XB_GPIO_26: u8 = bladerf_xb_gpio!(26);

/** Pin bitmask for Expansion GPIO 27 (U74 pin 42) */
pub const BLADERF_XB_GPIO_27: u8 = bladerf_xb_gpio!(27);

/** Pin bitmask for Expansion GPIO 28 (U74 pin 44) */
pub const BLADERF_XB_GPIO_28: u8 = bladerf_xb_gpio!(28);

/** Pin bitmask for Expansion GPIO 29 (U74 pin 48) */
pub const BLADERF_XB_GPIO_29: u8 = bladerf_xb_gpio!(29);

/** Pin bitmask for Expansion GPIO 30 (U74 pin 50) */
pub const BLADERF_XB_GPIO_30: u8 = bladerf_xb_gpio!(30);

/** Pin bitmask for Expansion GPIO 31 (U74 pin 54) */
pub const BLADERF_XB_GPIO_31: u8 = bladerf_xb_gpio!(31);

/** Pin bitmask for Expansion GPIO 32 (U74 pin 56) */
pub const BLADERF_XB_GPIO_32: u8 = bladerf_xb_gpio!(32);

/** Bitmask for XB-200 header J7, pin 1 */
pub const BLADERF_XB200_PIN_J7_1: u8 = BLADERF_XB_GPIO_10;

/** Bitmask for XB-200 header J7, pin 2 */
pub const BLADERF_XB200_PIN_J7_2: u8 = BLADERF_XB_GPIO_11;

/** Bitmask for XB-200 header J7, pin 5 */
pub const BLADERF_XB200_PIN_J7_5: u8 = BLADERF_XB_GPIO_08;

/** Bitmask for XB-200 header J7, pin 6 */
pub const BLADERF_XB200_PIN_J7_6: u8 = BLADERF_XB_GPIO_09;

/** Bitmask for XB-200 header J13, pin 1 */
pub const BLADERF_XB200_PIN_J13_1: u8 = BLADERF_XB_GPIO_17;

/** Bitmask for XB-200 header J13, pin 2 */
pub const BLADERF_XB200_PIN_J13_2: u8 = BLADERF_XB_GPIO_18;

/* XB-200 J13 Pin 6 is actually reserved for SPI */

/** Bitmask for XB-200 header J16, pin 1 */
pub const BLADERF_XB200_PIN_J16_1: u8 = BLADERF_XB_GPIO_31;

/** Bitmask for XB-200 header J16, pin 2 */
pub const BLADERF_XB200_PIN_J16_2: u8 = BLADERF_XB_GPIO_32;

/** Bitmask for XB-200 header J16, pin 3 */
pub const BLADERF_XB200_PIN_J16_3: u8 = BLADERF_XB_GPIO_19;

/** Bitmask for XB-200 header J16, pin 4 */
pub const BLADERF_XB200_PIN_J16_4: u8 = BLADERF_XB_GPIO_20;

/** Bitmask for XB-200 header J16, pin 5 */
pub const BLADERF_XB200_PIN_J16_5: u8 = BLADERF_XB_GPIO_21;

/** Bitmask for XB-200 header J16, pin 6 */
pub const BLADERF_XB200_PIN_J16_6: u8 = BLADERF_XB_GPIO_24;

/** Bitmask for XB-100 header J2, pin 3 */
pub const BLADERF_XB100_PIN_J2_3: u8 = BLADERF_XB_GPIO_07;

/** Bitmask for XB-100 header J2, pin 4 */
pub const BLADERF_XB100_PIN_J2_4: u8 = BLADERF_XB_GPIO_08;

/** Bitmask for XB-100 header J3, pin 3 */
pub const BLADERF_XB100_PIN_J3_3: u8 = BLADERF_XB_GPIO_09;

/** Bitmask for XB-100 header J3, pin 4 */
pub const BLADERF_XB100_PIN_J3_4: u8 = BLADERF_XB_GPIO_10;

/** Bitmask for XB-100 header J4, pin 3 */
pub const BLADERF_XB100_PIN_J4_3: u8 = BLADERF_XB_GPIO_11;

/** Bitmask for XB-100 header J4, pin 4 */
pub const BLADERF_XB100_PIN_J4_4: u8 = BLADERF_XB_GPIO_12;

/** Bitmask for XB-100 header J5, pin 3 */
pub const BLADERF_XB100_PIN_J5_3: u8 = BLADERF_XB_GPIO_13;

/** Bitmask for XB-100 header J5, pin 4 */
pub const BLADERF_XB100_PIN_J5_4: u8 = BLADERF_XB_GPIO_14;

/** Bitmask for XB-100 header J11, pin 2 */
pub const BLADERF_XB100_PIN_J11_2: u8 = BLADERF_XB_GPIO_05;

/** Bitmask for XB-100 header J11, pin 3 */
pub const BLADERF_XB100_PIN_J11_3: u8 = BLADERF_XB_GPIO_04;

/** Bitmask for XB-100 header J11, pin 4 */
pub const BLADERF_XB100_PIN_J11_4: u8 = BLADERF_XB_GPIO_03;

/** Bitmask for XB-100 header J11, pin 5 */
pub const BLADERF_XB100_PIN_J11_5: u8 = BLADERF_XB_GPIO_06;

/** Bitmask for XB-100 header J12, pin 2 */
pub const BLADERF_XB100_PIN_J12_2: u8 = BLADERF_XB_GPIO_01;

/*  XB-100 header J12, pins 3 and 4 are reserved for SPI */

/** Bitmask for XB-100 header J12, pin 5 */
pub const BLADERF_XB100_PIN_J12_5: u8 = BLADERF_XB_GPIO_02;

/** Bitmask for XB-100 LED_D1 (blue) */
pub const BLADERF_XB100_LED_D1: u8 = BLADERF_XB_GPIO_24;

/** Bitmask for XB-100 LED_D2 (blue) */
pub const BLADERF_XB100_LED_D2: u8 = BLADERF_XB_GPIO_32;

/** Bitmask for XB-100 LED_D3 (blue) */
pub const BLADERF_XB100_LED_D3: u8 = BLADERF_XB_GPIO_30;

/** Bitmask for XB-100 LED_D4 (red) */
pub const BLADERF_XB100_LED_D4: u8 = BLADERF_XB_GPIO_28;

/** Bitmask for XB-100 LED_D5 (red) */
pub const BLADERF_XB100_LED_D5: u8 = BLADERF_XB_GPIO_23;

/** Bitmask for XB-100 LED_D6 (red) */
pub const BLADERF_XB100_LED_D6: u8 = BLADERF_XB_GPIO_25;

/** Bitmask for XB-100 LED_D7 (green) */
pub const BLADERF_XB100_LED_D7: u8 = BLADERF_XB_GPIO_31;

/** Bitmask for XB-100 LED_D8 (green) */
pub const BLADERF_XB100_LED_D8: u8 = BLADERF_XB_GPIO_29;

/** Bitmask for XB-100 tricolor LED, red cathode */
pub const BLADERF_XB100_TLED_RED: u8 = BLADERF_XB_GPIO_22;

/** Bitmask for XB-100 tricolor LED, green cathode */
pub const BLADERF_XB100_TLED_GREEN: u8 = BLADERF_XB_GPIO_21;

/** Bitmask for XB-100 tricolor LED, blue cathode */
pub const BLADERF_XB100_TLED_BLUE: u8 = BLADERF_XB_GPIO_20;

/** Bitmask for XB-100 DIP switch 1 */
pub const BLADERF_XB100_DIP_SW1: u8 = BLADERF_XB_GPIO_27;

/** Bitmask for XB-100 DIP switch 2 */
pub const BLADERF_XB100_DIP_SW2: u8 = BLADERF_XB_GPIO_26;

/** Bitmask for XB-100 DIP switch 3 */
pub const BLADERF_XB100_DIP_SW3: u8 = BLADERF_XB_GPIO_16;

/** Bitmask for XB-100 DIP switch 4 */
pub const BLADERF_XB100_DIP_SW4: u8 = BLADERF_XB_GPIO_15;

/** Bitmask for XB-100 button J6 */
pub const BLADERF_XB100_BTN_J6: u8 = BLADERF_XB_GPIO_19;

/** Bitmask for XB-100 button J7 */
pub const BLADERF_XB100_BTN_J7: u8 = BLADERF_XB_GPIO_18;

/** Bitmask for XB-100 button J8 */
pub const BLADERF_XB100_BTN_J8: u8 = BLADERF_XB_GPIO_17;

// struct bladerf1_board_data {
//     /* Board state */
//     enum {
//     STATE_UNINITIALIZED,
//     STATE_FIRMWARE_LOADED,
//     STATE_FPGA_LOADED,
//     STATE_INITIALIZED,
//     } state;
//
//     /* Bitmask of capabilities determined by version numbers */
//     uint64_t capabilities;
//
//     /* Format currently being used with a module, or -1 if module is not used */
//     bladerf_format module_format[NUM_MODULES];
//
//     /* Which mode of operation we use for tuning */
//     bladerf_tuning_mode tuning_mode;
//
//     /* Calibration data */
//     struct calibrations {
//     struct dc_cal_tbl *dc_rx;
//     struct dc_cal_tbl *dc_tx;
//     } cal;
//     uint16_t dac_trim;
//
//     /* Board properties */
//     bladerf_fpga_size fpga_size;
//     /* Data message size */
//     size_t msg_size;
//
//     /* Version information */
//     struct bladerf_version fpga_version;
//     struct bladerf_version fw_version;
//     char fpga_version_str[BLADERF_VERSION_STR_MAX+1];
//     char fw_version_str[BLADERF_VERSION_STR_MAX+1];
//
//     /* Synchronous interface handles */
//     struct bladerf_sync sync[NUM_MODULES];
// };

impl BladeRf1 {
    /******************************************************************************/
    /* Expansion support */
    /******************************************************************************/
    // Maybe directly access self.xb instead of cloning it...
    pub fn expansion_get_attached(&self) -> BladerfXb {
        // CHECK_BOARD_STATE(STATE_FPGA_LOADED);
        if self.xb100.is_some() {
            BladerfXb100
        } else if self.xb200.is_some() {
            BladerfXb200
        } else if self.xb300.is_some() {
            BladerfXb300
        } else {
            BladerfXbNone
        }
    }

    pub fn expansion_attach(&mut self, xb: BladerfXb) -> Result<()> {
        // struct bladerf1_board_data *board_data = dev->board_data;

        // CHECK_BOARD_STATE(STATE_INITIALIZED);

        let attached = self.expansion_get_attached();

        if xb != attached && attached != BladerfXbNone {
            return Err(anyhow!("Switching XB types is not supported."));
        }

        if xb == BladerfXb100 {
            // if (!have_cap(board_data->capabilities, BLADERF_CAP_MASKED_XBIO_WRITE)) {
            //   log::debug!("%s: XB100 support requires FPGA v0.4.1 or later.\n", __FUNCTION__);
            //   return BLADERF_ERR_UNSUPPORTED;
            // }

            log::debug!("Attaching XB100");
            self.xb100_attach()?;

            log::debug!("Enabling XB100");
            self.xb100_enable(true)?;

            log::debug!("Initializing XB100");
            self.xb100_init()?;
        } else if xb == BladerfXb200 {
            // if (!have_cap(board_data->capabilities, BLADERF_CAP_XB200)) {
            //   log::debug!("%s: XB200 support requires FPGA v0.0.5 or later\n", __FUNCTION__);
            //   return BLADERF_ERR_UPDATE_FPGA;
            // }

            log::debug!("Attaching XB200");
            self.xb200_attach()?;

            log::debug!("Enabling XB200");
            self.xb200_enable(true)?;

            log::debug!("Initializing XB200");
            self.xb200_init()?;
        } else if xb == BladerfXb300 {
            log::debug!("Attaching XB300");
            self.xb300_attach()?;

            log::debug!("Enabling XB300");
            self.xb300_enable(true)?;

            log::debug!("Initializing XB300");
            self.xb300_init()?;
        } else if xb == BladerfXbNone {
            log::debug!("Disabling an attached XB is not supported.");
            return Err(BladeRfError::Unsupported.into());
        } else {
            log::debug!("Unknown xb type: {xb:?}");
            return Err(BladeRfError::Unsupported.into());
        }

        /* Cache what we have attached in our device handle to alleviate the
         * need to go read the device state */
        // self.xb = xb;

        Ok(())
    }
}
