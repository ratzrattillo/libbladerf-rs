//! Low-level SPI flash erase/write/verify operations.
//!
//! Provides the core flash programming primitive used by higher-level
//! firmware and FPGA flashing. The process operates in sector-sized
//! chunks: erases a sector, writes constituent pages, then verifies
//! the data. Each sector undergoes up to three retry attempts if
//! verification fails.

use crate::bladerf1::board::FlashSession;
use crate::bladerf1::hardware::spi_flash::{
    BLADERF_FLASH_ERASE_BLOCK_SIZE, BLADERF_FLASH_PAGE_SIZE,
};
use crate::error::{Error, Result};

const MAX_RETRIES: u8 = 3;

impl FlashSession<'_> {
    /// Erases, writes, and verifies data to the SPI flash starting at the given page.
    ///
    /// Validates that the page range and sector range are within flash bounds.
    /// For each sector required by the data: erases the sector, writes all
    /// constituent pages, then reads back and verifies. On verification
    /// failure retries the sector up to three times. Returns a detailed
    /// `Error::FlashVerificationFailed` if all retries are exhausted.
    ///
    /// Returns `Error::Argument` if the page or sector range exceeds flash
    /// capacity. Returns `Error::FlashVerificationFailed` if retries are
    /// exhausted.
    pub fn erase_write_verify(&mut self, page_start: u32, data: &[u8]) -> Result<()> {
        let total_pages = self.total_pages();
        let total_sectors = self.total_sectors();
        let pages_per_sector = (BLADERF_FLASH_ERASE_BLOCK_SIZE / BLADERF_FLASH_PAGE_SIZE) as u32;

        if page_start >= total_pages {
            return Err(Error::Argument(format!(
                "flash page {page_start} out of range (0..{total_pages})"
            )));
        }
        let page_count = data.len() / BLADERF_FLASH_PAGE_SIZE;
        if page_start + page_count as u32 > total_pages {
            return Err(Error::Argument(format!(
                "flash page range {page_start}..{} out of range (0..{total_pages})",
                page_start + page_count as u32,
            )));
        }
        let sector_start = page_start / pages_per_sector;
        let sector_count = (page_count as u32).div_ceil(pages_per_sector);
        if sector_start + sector_count > total_sectors {
            return Err(Error::Argument(format!(
                "flash sector range {sector_start}..{} out of range (0..{total_sectors})",
                sector_start + sector_count,
            )));
        }

        for (sec_idx, sector_data) in data.chunks(BLADERF_FLASH_ERASE_BLOCK_SIZE).enumerate() {
            let sector = sector_start + sec_idx as u32;

            for attempt in 0..=MAX_RETRIES {
                self.erase_sector(sector)?;

                let start_page = sector * pages_per_sector;
                for (page_idx, page_data) in sector_data
                    .chunks_exact(BLADERF_FLASH_PAGE_SIZE)
                    .enumerate()
                {
                    self.write_page(start_page + page_idx as u32, page_data)?;
                }

                match self.verify_pages(start_page, sector_data) {
                    Ok(()) => break,
                    Err(e) if attempt < MAX_RETRIES => {
                        log::warn!(
                            "Verification failed at sector {sector}, retry {}/{}: {e:#}",
                            attempt + 1,
                            MAX_RETRIES,
                        );
                    }
                    Err(_) => {
                        return Err(Error::FlashVerificationFailed {
                            byte_offset: sec_idx * BLADERF_FLASH_ERASE_BLOCK_SIZE,
                            expected: 0x00,
                            actual: 0xFF,
                        });
                    }
                }
            }
        }
        Ok(())
    }
}
