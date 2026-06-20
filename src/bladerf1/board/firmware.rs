//! Firmware flash operations.
//!
//! Erases, writes, and verifies the firmware region of the SPI flash.
//! The firmware image must meet minimum and maximum size constraints
//! to ensure it fits within the reserved flash partition.

use crate::bladerf1::board::FlashSession;
use crate::bladerf1::hardware::spi_flash::{
    BLADERF_FLASH_BYTE_LEN_FIRMWARE, BLADERF_FLASH_ERASE_BLOCK_SIZE, BLADERF_FLASH_PAGE_SIZE,
};
use crate::error::{Error, Result};
use crate::flash::pad_to_page;

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
    pub fn flash_firmware(&mut self, firmware: &[u8]) -> Result<()> {
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

        let padded = pad_to_page(firmware);

        let firmware_sectors =
            BLADERF_FLASH_BYTE_LEN_FIRMWARE / BLADERF_FLASH_ERASE_BLOCK_SIZE as u32;

        let page_count = padded.len() / BLADERF_FLASH_PAGE_SIZE;

        self.erase_sectors(0, firmware_sectors)?;
        self.write_pages(0, page_count, &padded)?;
        self.verify_pages(0, &padded)?;

        Ok(())
    }
}
