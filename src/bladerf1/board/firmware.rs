//! Firmware flash operations.
//!
//! Erases, writes, and verifies the firmware region of the SPI flash.
//! The firmware image must meet minimum and maximum size constraints
//! to ensure it fits within the reserved flash partition.

use crate::bladerf1::board::FlashSession;
use crate::bladerf1::hardware::spi_flash::BLADERF_FLASH_BYTE_LEN_FIRMWARE;
use crate::error::{Error, Result};
use crate::maybe_future::Op;
use nusb::MaybeFuture;

/// Minimum firmware image size in bytes.
pub const BLADERF_FLASH_MIN_FW_SIZE: usize = 50 * 1024;

impl FlashSession<'_> {
    /// Flash a new BLADERF_FIRMWARE image to the device.
    ///
    /// Validates the image size, pads to page alignment, erases the firmware
    /// sectors, writes the padded data page-by-page, and verifies the write
    /// by reading back and comparing.
    ///
    /// Returns `Error::Argument` if the image is too small or exceeds the
    /// firmware partition. Returns `Error::FlashVerificationFailed` if the
    /// read-back verification does not match the written data.
    pub fn flash_firmware(&mut self, firmware: &[u8]) -> impl MaybeFuture<Output = Result<()>> {
        Op::new(async move {
            if firmware.len() < BLADERF_FLASH_MIN_FW_SIZE {
                return Err(Error::Argument(format!(
                    "firmware size {} bytes is below minimum {} bytes",
                    firmware.len(),
                    BLADERF_FLASH_MIN_FW_SIZE
                )));
            }
            if firmware.len() > BLADERF_FLASH_BYTE_LEN_FIRMWARE as usize {
                return Err(Error::Argument(format!(
                    "firmware size {} bytes exceeds firmware region {} bytes",
                    firmware.len(),
                    BLADERF_FLASH_BYTE_LEN_FIRMWARE
                )));
            }

            let mut image = vec![0xff; BLADERF_FLASH_BYTE_LEN_FIRMWARE as usize];
            image[..firmware.len()].copy_from_slice(firmware);
            self.erase_write_verify(0, &image).await
        })
    }
}
