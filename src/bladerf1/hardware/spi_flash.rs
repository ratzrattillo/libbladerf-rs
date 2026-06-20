//! SPI flash access model.
//!
//! Provides buffered page read/write, sector erase, multi-page operations,
//! and block verification through USB vendor commands routed through NIOS.

use crate::bladerf1::board::FlashSession;
use crate::error::{Error, Result};
use crate::usb::{UsbInterfaceCommands, VendorRequest};
use nusb::Speed;

/// Size of a single flash page in bytes.
pub const BLADERF_FLASH_PAGE_SIZE: usize = 256;
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

/// Metadata about the SPI flash device constructed from a USB vendor query.
pub(crate) struct FlashMeta {
    /// Total flash capacity in bytes.
    pub flash_size_bytes: u32,
    /// Total number of pages.
    pub total_pages: u32,
    /// Total number of erasable sectors.
    pub total_sectors: u32,
}

impl FlashSession<'_> {
    /// Checks the USB speed and returns the appropriate transfer chunk size.
    ///
    /// Returns `Error::UnsupportedSpeed` if the device is not at High or Super speed.
    fn chunk_size(&self) -> Result<usize> {
        match self.nios.transport().speed() {
            Speed::Super | Speed::SuperPlus => Ok(BLADERF_FLASH_PAGE_SIZE),
            Speed::High => Ok(64),
            _ => Err(Error::UnsupportedSpeed),
        }
    }

    fn read_page_buffer(&mut self, buf: &mut [u8]) -> Result<()> {
        let chunk_size = self.chunk_size()?;
        for (offset, chunk) in buf.chunks_exact_mut(chunk_size).enumerate() {
            self.nios.usb_vendor_cmd_in_w_index_data(
                VendorRequest::ReadPageBuffer,
                (offset * chunk_size) as u16,
                chunk,
            )?;
        }
        Ok(())
    }

    fn write_page_buffer(&mut self, buf: &[u8]) -> Result<()> {
        let chunk_size = self.chunk_size()?;
        for (offset, chunk) in buf.chunks_exact(chunk_size).enumerate() {
            self.nios.usb_vendor_cmd_out_w_index(
                VendorRequest::WritePageBuffer,
                (offset * chunk_size) as u16,
                chunk,
            )?;
        }
        Ok(())
    }

    /// Reads the on-device calibration cache into the provided buffer.
    pub(crate) fn read_cal_cache(&mut self, buf: &mut [u8]) -> Result<()> {
        let chunk_size = self.chunk_size()?;
        for (offset, chunk) in buf.chunks_exact_mut(chunk_size).enumerate() {
            self.nios.usb_vendor_cmd_in_w_index_data(
                VendorRequest::ReadCalCache,
                (offset * chunk_size) as u16,
                chunk,
            )?;
        }
        Ok(())
    }

    /// Reads a single page of flash into the provided buffer.
    ///
    /// Returns `Error::Argument` if the page number is out of range.
    pub fn read_page(&mut self, page: u32, buf: &mut [u8]) -> Result<()> {
        let total_pages = self.flash_meta.total_pages;
        if page >= total_pages {
            return Err(Error::Argument(format!(
                "flash page {page} out of range (0..{total_pages})"
            )));
        }
        self.nios
            .usb_vendor_cmd_int_w_index(VendorRequest::FlashRead, page as u16)?;
        self.read_page_buffer(buf)
    }

    /// Writes a single page of flash from the provided buffer.
    ///
    /// The corresponding sector must be erased before writing.
    /// Returns `Error::Argument` if the page number is out of range.
    pub fn write_page(&mut self, page: u32, buf: &[u8]) -> Result<()> {
        let total_pages = self.flash_meta.total_pages;
        if page >= total_pages {
            return Err(Error::Argument(format!(
                "flash page {page} out of range (0..{total_pages})"
            )));
        }
        self.write_page_buffer(buf)?;
        self.nios
            .usb_vendor_cmd_int_w_index(VendorRequest::FlashWrite, page as u16)?;
        Ok(())
    }

    /// Erases a 64 KB flash sector.
    ///
    /// Returns `Error::Argument` if the sector number is out of range.
    pub fn erase_sector(&mut self, sector: u32) -> Result<()> {
        let total_sectors = self.flash_meta.total_sectors;
        if sector >= total_sectors {
            return Err(Error::Argument(format!(
                "flash sector {sector} out of range (0..{total_sectors})"
            )));
        }
        self.nios
            .usb_vendor_cmd_int_w_index(VendorRequest::FlashErase, sector as u16)?;
        Ok(())
    }

    /// Reads contiguous pages of flash into the provided buffer.
    ///
    /// Returns `Error::Argument` if the buffer is too small for the requested page count.
    pub fn read_pages(&mut self, page_start: u32, page_count: usize, buf: &mut [u8]) -> Result<()> {
        let required = page_count * BLADERF_FLASH_PAGE_SIZE;
        if buf.len() < required {
            return Err(Error::Argument(format!(
                "buffer too small: {required} bytes required, {} provided",
                buf.len()
            )));
        }

        for page_idx in 0..page_count {
            let offset = page_idx * BLADERF_FLASH_PAGE_SIZE;
            self.read_page(
                page_start + page_idx as u32,
                &mut buf[offset..offset + BLADERF_FLASH_PAGE_SIZE],
            )?;
        }
        Ok(())
    }

    /// Writes contiguous pages of flash from the provided buffer.
    ///
    /// Corresponding sectors must be erased before writing.
    /// Returns `Error::Argument` if the buffer is too small for the requested page count.
    pub fn write_pages(&mut self, page_start: u32, page_count: usize, buf: &[u8]) -> Result<()> {
        let required = page_count * BLADERF_FLASH_PAGE_SIZE;
        if buf.len() < required {
            return Err(Error::Argument(format!(
                "buffer too small: {required} bytes required, {} provided",
                buf.len()
            )));
        }

        for page_idx in 0..page_count {
            let offset = page_idx * BLADERF_FLASH_PAGE_SIZE;
            self.write_page(
                page_start + page_idx as u32,
                &buf[offset..offset + BLADERF_FLASH_PAGE_SIZE],
            )?;
        }
        Ok(())
    }

    /// Erases a range of contiguous 64 KB flash sectors.
    pub fn erase_sectors(&mut self, start: u32, count: u32) -> Result<()> {
        for sector in start..start + count {
            self.erase_sector(sector)?;
        }
        Ok(())
    }

    /// Reads flash pages and verifies each against the expected data.
    ///
    /// Returns `Error::FlashVerificationFailed` on the first mismatch.
    pub fn verify_pages(&mut self, page_start: u32, expected: &[u8]) -> Result<()> {
        for (page_idx, expected_page) in expected.chunks_exact(BLADERF_FLASH_PAGE_SIZE).enumerate()
        {
            let mut actual = [0u8; BLADERF_FLASH_PAGE_SIZE];
            self.read_page(page_start + page_idx as u32, &mut actual)?;
            if expected_page != actual {
                let local = expected_page
                    .iter()
                    .zip(&actual)
                    .position(|(e, a)| e != a)
                    .unwrap();
                return Err(Error::FlashVerificationFailed {
                    byte_offset: page_idx * BLADERF_FLASH_PAGE_SIZE + local,
                    expected: expected_page[local],
                    actual: actual[local],
                });
            }
        }
        Ok(())
    }

    /// Returns the total flash capacity in bytes.
    pub fn size_bytes(&self) -> u32 {
        self.flash_meta.flash_size_bytes
    }

    /// Returns the total number of flash pages.
    pub fn total_pages(&self) -> u32 {
        self.flash_meta.total_pages
    }

    /// Returns the total number of erasable flash sectors.
    pub fn total_sectors(&self) -> u32 {
        self.flash_meta.total_sectors
    }

    /// Returns the number of erasable sectors available for FPGA bitstream storage.
    pub fn fpga_flash_sectors(&self) -> u32 {
        self.flash_meta.total_sectors
            - BLADERF_FLASH_ADDR_FPGA / BLADERF_FLASH_ERASE_BLOCK_SIZE as u32
    }

    /// Returns the total FPGA bitstream storage capacity in bytes.
    pub fn fpga_flash_bytes(&self) -> usize {
        self.fpga_flash_sectors() as usize * BLADERF_FLASH_ERASE_BLOCK_SIZE
    }
}
