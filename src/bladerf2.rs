//! BladeRF2 driver module.
//!
//! Re-exports the primary API surface for the BladeRF2 (xA4/xA5/xA9) SDR:
//! the [`BladeRf2`] device handle, its session types, and device
//! identification data (FPGA size and source, flash geometry, calibration).

pub mod board;
pub mod hardware;

pub use board::{
    BLADERF_FLASH_FPGA_SIZE_A4, BLADERF_FLASH_FPGA_SIZE_A5, BLADERF_FLASH_FPGA_SIZE_A9,
    BLADERF2_USB_PID, BLADERF2_USB_VID, BladeRf2, DEFAULT_VCTCXO_TRIM, FlashSession, FpgaSource,
    RfLinkSession, fpga_bytes,
};
