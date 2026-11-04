//! SPI flash access model.
//!
//! Page reads and calibration cache access through USB vendor commands.
//! Port of the read paths of `driver/spi_flash.c`.

use crate::bladerf2::board::FlashSession;
use crate::error::{Error, Result};
use crate::maybe_future::Op;
use nusb::MaybeFuture;

pub(crate) mod layout;
pub(crate) use layout::FlashMeta;
use layout::{Page, PageIndex};

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
}
