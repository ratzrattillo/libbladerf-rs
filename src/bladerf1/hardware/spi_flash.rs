//! SPI flash access model.
//!
//! Provides buffered page read/write, sector erase, multi-page operations,
//! and block verification through USB vendor commands routed through NIOS.

use crate::bladerf1::board::FlashSession;
use crate::error::{Error, Result};
use crate::maybe_future::Op;
use crate::usb::{UsbInterfaceCommands, VendorRequest};
use nusb::MaybeFuture;

pub(crate) mod layout;
pub(crate) use layout::FlashMeta;
use layout::{Page, PageIndex, SectorIndex, pages, pages_mut};

/// Size of a single flash page in bytes.
pub use crate::flash::BLADERF_FLASH_PAGE_SIZE;
/// Size of a flash erase block in bytes (64 KB).
pub const BLADERF_FLASH_ERASE_BLOCK_SIZE: usize = 64 * 1_024;
/// Flash address of the firmware region.
pub const BLADERF_FLASH_ADDR_FIRMWARE: u32 = 0x00000000;
/// Size of the firmware region in bytes.
pub const BLADERF_FLASH_BYTE_LEN_FIRMWARE: u32 = 0x00030000;
/// Flash address of the calibration data region.
pub const BLADERF_FLASH_ADDR_CAL: u32 = 0x00030000;
/// Size of the calibration data region in bytes.
pub const BLADERF_FLASH_BYTE_LEN_CAL: usize = 0x100;
/// Flash address of the FPGA bitstream region.
pub const BLADERF_FLASH_ADDR_FPGA: u32 = 0x00040000;

impl FlashSession<'_> {
    /// Reads the on-device calibration cache into the provided buffer.
    pub(crate) fn read_cal_cache(
        &mut self,
        buf: &mut Page,
    ) -> impl MaybeFuture<Output = Result<()>> {
        Op::new(async move {
            *buf = self.nios.control().await?.read_calibration_page().await?;
            Ok(())
        })
    }

    /// Reads one complete 256-byte flash page.
    ///
    /// # Errors
    /// Returns an argument error if `buf` is not exactly 256 bytes or `page`
    /// is out of range. USB and firmware failures are propagated.
    pub fn read_page(
        &mut self,
        page: u32,
        buf: &mut [u8],
    ) -> impl MaybeFuture<Output = Result<()>> {
        Op::new(async move {
            let buf = <&mut Page>::try_from(buf).map_err(|_| {
                Error::Argument("flash page buffer must be exactly 256 bytes".into())
            })?;
            let page = self.flash_meta.page(page)?;
            self.read_page_index(page, buf).await
        })
    }

    /// Writes one complete 256-byte flash page.
    ///
    /// The corresponding sector must be erased before writing.
    /// Cancellation retains the whole page transaction for completion before
    /// subsequent I/O. Completed writes are not rolled back.
    ///
    /// # Errors
    /// Returns an argument error if `buf` is not exactly 256 bytes or `page`
    /// is out of range. USB and firmware failures are propagated.
    pub fn write_page(&mut self, page: u32, buf: &[u8]) -> impl MaybeFuture<Output = Result<()>> {
        Op::new(async move {
            let buf = <&Page>::try_from(buf)
                .map_err(|_| Error::Argument("flash page data must be exactly 256 bytes".into()))?;
            let page = self.flash_meta.page(page)?;
            self.write_page_index(page, buf).await
        })
    }

    /// Erases a complete 64-KiB flash sector.
    ///
    /// # Errors
    /// Returns an argument error for an out-of-range sector, or the underlying
    /// USB/firmware error. Cancellation does not undo an erase.
    pub fn erase_sector(&mut self, sector: u32) -> impl MaybeFuture<Output = Result<()>> {
        Op::new(async move {
            let sector = self.flash_meta.sector(sector)?;
            self.erase_sector_index(sector).await
        })
    }

    /// Reads contiguous pages of flash into the provided buffer.
    ///
    /// Empty ranges are no-ops and may start one page past the flash.
    ///
    /// # Errors
    /// Returns an argument error unless `buf` contains exactly `page_count`
    /// whole pages and the entire range fits in flash. USB/firmware failures
    /// are propagated; earlier pages may already have been read.
    pub fn read_pages(
        &mut self,
        page_start: u32,
        page_count: usize,
        buf: &mut [u8],
    ) -> impl MaybeFuture<Output = Result<()>> {
        Op::new(async move {
            let pages = pages_mut(buf)?;
            require_page_count(page_count, pages.len())?;
            let indices = self.flash_meta.pages(page_start, pages.len())?;
            for (index, page) in indices.zip(pages) {
                self.read_page_index(index, page).await?;
            }
            Ok(())
        })
    }

    /// Writes contiguous pages of flash from the provided buffer.
    ///
    /// Corresponding sectors must be erased before writing. Empty ranges are
    /// no-ops and may start one page past the flash. Completed writes persist
    /// after an error or cancellation; unfinished page transactions are settled
    /// before the next I/O operation.
    ///
    /// # Errors
    /// Returns an argument error unless `buf` contains exactly `page_count`
    /// whole pages and the entire range fits in flash. Propagates USB/firmware
    /// failures without automatically replaying failed writes.
    pub fn write_pages(
        &mut self,
        page_start: u32,
        page_count: usize,
        buf: &[u8],
    ) -> impl MaybeFuture<Output = Result<()>> {
        Op::new(async move {
            let pages = pages(buf)?;
            require_page_count(page_count, pages.len())?;
            let indices = self.flash_meta.pages(page_start, pages.len())?;
            for (index, page) in indices.zip(pages) {
                self.write_page_index(index, page).await?;
            }
            Ok(())
        })
    }

    /// Erases a range of contiguous 64-KiB flash sectors.
    ///
    /// Empty ranges are no-ops and may start one sector past the flash.
    /// Completed erases persist after cancellation or a later error.
    ///
    /// # Errors
    /// Rejects an invalid complete range before any erase. Propagates
    /// USB/firmware failures without automatically replaying failed erases.
    pub fn erase_sectors(
        &mut self,
        start: u32,
        count: u32,
    ) -> impl MaybeFuture<Output = Result<()>> {
        Op::new(async move {
            let indices = self.flash_meta.sectors(start, count as usize)?;
            for sector in indices {
                self.erase_sector_index(sector).await?;
            }
            Ok(())
        })
    }

    /// Reads flash pages and verifies each against the expected data.
    ///
    /// Empty ranges are no-ops and may start one page past the flash.
    ///
    /// # Errors
    /// Rejects partial pages and out-of-range requests before I/O. Returns
    /// [`Error::FlashVerificationFailed`] on the first mismatch, with a byte
    /// offset relative to `page_start`, or the underlying USB/firmware error.
    pub fn verify_pages(
        &mut self,
        page_start: u32,
        expected: &[u8],
    ) -> impl MaybeFuture<Output = Result<()>> {
        Op::new(async move {
            let pages = pages(expected)?;
            let indices = self.flash_meta.pages(page_start, pages.len())?;
            for (page_idx, (index, expected_page)) in indices.zip(pages).enumerate() {
                let mut actual = [0u8; BLADERF_FLASH_PAGE_SIZE];
                self.read_page_index(index, &mut actual).await?;
                verify_page(expected_page, &actual, page_idx * BLADERF_FLASH_PAGE_SIZE)?;
            }
            Ok(())
        })
    }

    /// Returns the total flash capacity in bytes.
    pub fn size_bytes(&self) -> u32 {
        self.flash_meta.size_bytes()
    }

    /// Returns the total number of flash pages.
    pub fn total_pages(&self) -> u32 {
        self.flash_meta.total_pages()
    }

    /// Returns the total number of erasable flash sectors.
    pub fn total_sectors(&self) -> u32 {
        self.flash_meta.total_sectors()
    }

    /// Returns the number of erasable sectors available for FPGA bitstream storage.
    pub fn fpga_flash_sectors(&self) -> u32 {
        self.flash_meta.total_sectors()
            - BLADERF_FLASH_ADDR_FPGA / BLADERF_FLASH_ERASE_BLOCK_SIZE as u32
    }

    /// Returns the total FPGA bitstream storage capacity in bytes.
    pub fn fpga_flash_bytes(&self) -> usize {
        self.fpga_flash_sectors() as usize * BLADERF_FLASH_ERASE_BLOCK_SIZE
    }

    pub(crate) fn read_page_index(
        &mut self,
        index: PageIndex,
        page: &mut Page,
    ) -> impl MaybeFuture<Output = Result<()>> {
        Op::new(async move {
            *page = self
                .nios
                .control()
                .await?
                .read_flash_page(index.get())
                .await?;
            Ok(())
        })
    }

    pub(crate) fn write_page_index(
        &mut self,
        index: PageIndex,
        page: &Page,
    ) -> impl MaybeFuture<Output = Result<()>> {
        Op::new(async move {
            self.nios
                .control()
                .await?
                .write_flash_page(index.get(), page)
                .await
        })
    }

    pub(crate) fn erase_sector_index(
        &mut self,
        index: SectorIndex,
    ) -> impl MaybeFuture<Output = Result<()>> {
        Op::new(async move {
            self.nios
                .control()
                .await?
                .usb_vendor_cmd_int_w_index(VendorRequest::FlashErase, index.get())
                .await?;
            Ok(())
        })
    }
}

fn require_page_count(requested: usize, actual: usize) -> Result<()> {
    if requested != actual {
        return Err(Error::Argument(
            "flash page count must match the complete buffer".into(),
        ));
    }
    Ok(())
}

fn verify_page(expected: &Page, actual: &Page, byte_offset: usize) -> Result<()> {
    if let Some(local) = expected.iter().zip(actual).position(|(e, a)| e != a) {
        return Err(Error::FlashVerificationFailed {
            byte_offset: byte_offset + local,
            expected: expected[local],
            actual: actual[local],
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verification_preserves_later_page_byte_offsets_and_values() {
        let expected = [0xab; 256];
        let mut actual = expected;
        actual[237] = 0xcd;
        assert!(matches!(
            verify_page(&expected, &actual, 7 * 256),
            Err(Error::FlashVerificationFailed {
                byte_offset: 2029,
                expected: 0xab,
                actual: 0xcd
            })
        ));
        assert!(require_page_count(usize::MAX, 0).is_err());
        assert!(require_page_count(0, 1).is_err());
        assert!(require_page_count(0, 0).is_ok());
    }
}
