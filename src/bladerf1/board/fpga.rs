//! FPGA loading and firmware log access.
//!
//! Supports two methods of loading the FPGA bitstream: host-based direct
//! programming through the USB bulk interface, or firmware-based autoloading
//! from an image stored in SPI flash. Host-based loading pushes the raw
//! bitstream via USB and polls until the FPGA reports configured. Flash-based
//! loading writes the bitstream (along with a length metadata page) to the
//! FPGA region of SPI flash, where the firmware can load it at boot or on
//! command.

use crate::bladerf1::board::{ConfigSession, FlashSession, RfLinkSession};
use crate::bladerf1::hardware::spi_flash::{
    BLADERF_FLASH_ADDR_FPGA, BLADERF_FLASH_ERASE_BLOCK_SIZE, BLADERF_FLASH_PAGE_SIZE,
};
use crate::error::{Error, Result};
use crate::flash::{binkv_encode_field, pad_to_page};
use crate::usb::{
    BladeRf1UsbInterfaceCommands, CONTROL_ENDPOINT_OUT, UsbInterfaceCommands, VendorRequest,
};
use std::fmt;
use std::thread;
use std::time::Duration;

/// Bitstream size for the 40KLE
pub const BLADERF_FLASH_FPGA_SIZE_40KLE: usize = 1_191_788;
/// Bitstream size for the 115KLE
pub const BLADERF_FLASH_FPGA_SIZE_115KLE: usize = 3_571_462;

/// Returns `true` if the given length matches a known FPGA bitstream size.
pub fn is_valid_fpga_size(len: usize) -> bool {
    len == BLADERF_FLASH_FPGA_SIZE_40KLE || len == BLADERF_FLASH_FPGA_SIZE_115KLE
}

const LOG_EOF: u32 = 0x00000000;
const LOG_ERR: u32 = 0xFFFFFFFF;

const FPGA_LOAD_TIMEOUT: Duration = Duration::from_secs(3);
const FPGA_STATUS_POLL_INTERVAL: Duration = Duration::from_millis(200);
const FPGA_STATUS_POLL_ATTEMPTS: u32 = 10;

/// Source file identifier encoded in a firmware log entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FwLogFile {
    /// No file or unrecognized identifier.
    None,
    /// bladeRF.c
    BladeRf,
    /// flash.c
    Flash,
    /// fpga.c
    Fpga,
    /// gpif.c
    Gpif,
    /// logger.c
    Logger,
    /// rf.c
    Rf,
    /// spi_flash_lib.c
    SpiFlash,
}

/// Single entry from the firmware log buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FwLogEntry {
    data: u16,
    line: u16,
    file: FwLogFile,
}

impl FwLogEntry {
    pub fn data(&self) -> u16 {
        self.data
    }

    pub fn line(&self) -> u16 {
        self.line
    }

    pub fn file(&self) -> FwLogFile {
        self.file
    }
}

impl FwLogFile {
    /// Converts a firmware file ID into an `FwLogFile` variant.
    fn from_id(id: u8) -> Self {
        match id {
            0 => Self::None,
            1 => Self::BladeRf,
            2 => Self::Flash,
            3 => Self::Fpga,
            4 => Self::Gpif,
            5 => Self::Logger,
            6 => Self::Rf,
            7 => Self::SpiFlash,
            _ => Self::None,
        }
    }

    /// Returns the C source file name corresponding to this log file ID.
    fn as_str(&self) -> &'static str {
        match self {
            Self::None => "<none>",
            Self::BladeRf => "bladeRF.c",
            Self::Flash => "flash.c",
            Self::Fpga => "fpga.c",
            Self::Gpif => "gpif.c",
            Self::Logger => "logger.c",
            Self::Rf => "rf.c",
            Self::SpiFlash => "spi_flash_lib.c",
        }
    }
}

impl FwLogEntry {
    /// Decodes a 32-bit log entry word into structured fields.
    fn from_u32(entry: u32) -> Self {
        Self {
            data: (entry & 0xFFFF) as u16,
            line: ((entry >> 16) & 0x7FF) as u16,
            file: FwLogFile::from_id(((entry >> 27) & 0x1F) as u8),
        }
    }
}

impl fmt::Display for FwLogEntry {
    /// Formats the entry as source file, line number, and hex data.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}, {}, 0x{:04X}",
            self.file.as_str(),
            self.line,
            self.data
        )
    }
}

impl RfLinkSession<'_> {
    /// Returns `true` if the FPGA has completed configuration.
    pub fn is_fpga_configured(&mut self) -> Result<bool> {
        self.nios.usb_is_fpga_configured()
    }

    /// Reads all entries from the firmware log buffer.
    ///
    /// Iterates over the firmware's ring-buffer log via USB vendor requests
    /// until an end-of-log sentinel is received. Logs a warning and stops
    /// if a firmware error sentinel is encountered.
    pub fn read_fw_log(&mut self) -> Result<Vec<FwLogEntry>> {
        let mut entries = Vec::new();
        loop {
            let raw = self.nios.usb_vendor_cmd_int(VendorRequest::ReadLogEntry)?;
            if raw == LOG_EOF {
                break;
            }
            if raw == LOG_ERR {
                log::warn!("firmware log read error");
                break;
            }
            entries.push(FwLogEntry::from_u32(raw));
        }
        Ok(entries)
    }
}

impl ConfigSession<'_> {
    /// Loads an FPGA bitstream directly from the host over USB.
    ///
    /// Begins FPGA programming via USB, sends the raw bitstream over the
    /// bulk-out endpoint, and polls the configuration status until the FPGA
    /// reports success or a timeout is reached. Requires the device to be in
    /// config mode (USB alternate setting switched away from RfLink).
    ///
    /// Returns `Error::Argument` if the bitstream size is not 40KLE or
    /// 115KLE. Returns `Error::Timeout` if the FPGA does not complete
    /// configuration within the polling window.
    pub fn load_fpga(&mut self, bitstream: &[u8]) -> Result<()> {
        if !is_valid_fpga_size(bitstream.len()) {
            return Err(Error::Argument(format!(
                "invalid FPGA bitstream size: {} bytes (expected {} or {})",
                bitstream.len(),
                BLADERF_FLASH_FPGA_SIZE_40KLE,
                BLADERF_FLASH_FPGA_SIZE_115KLE,
            )));
        }

        self.nios.usb_begin_fpga_prog()?;
        self.nios
            .usb_bulk_out(CONTROL_ENDPOINT_OUT, bitstream, FPGA_LOAD_TIMEOUT)?;

        let configured = {
            let mut result = false;
            for _ in 0..FPGA_STATUS_POLL_ATTEMPTS {
                if self.nios.usb_is_fpga_configured()? {
                    result = true;
                    break;
                }
                thread::sleep(FPGA_STATUS_POLL_INTERVAL);
            }
            result
        };

        if !configured {
            return Err(Error::Timeout);
        }

        Ok(())
    }
}

impl FlashSession<'_> {
    /// Writes an FPGA bitstream to the SPI flash FPGA region.
    ///
    /// Prepends a metadata page containing the bitstream length in binkv
    /// format, pads the bitstream to page alignment, then erases, writes,
    /// and verifies the entire image. Once stored, the firmware can load
    /// the bitstream automatically at boot or on request.
    ///
    /// Returns `Error::Argument` if the bitstream size is not 40KLE or
    /// 115KLE. Returns `Error::FlashVerificationFailed` if verification
    /// fails after retries.
    pub fn flash_fpga(&mut self, bitstream: &[u8]) -> Result<()> {
        if !is_valid_fpga_size(bitstream.len()) {
            return Err(Error::Argument(format!(
                "invalid FPGA bitstream size: {} bytes (expected {} or {})",
                bitstream.len(),
                BLADERF_FLASH_FPGA_SIZE_40KLE,
                BLADERF_FLASH_FPGA_SIZE_115KLE,
            )));
        }

        let fpga_page = BLADERF_FLASH_ADDR_FPGA / BLADERF_FLASH_PAGE_SIZE as u32;

        let padded = pad_to_page(bitstream);

        let mut meta = [0xFFu8; BLADERF_FLASH_PAGE_SIZE];
        let len_str = bitstream.len().to_string();
        binkv_encode_field(&mut meta, 0, "LEN", &len_str)?;

        let mut all_data = Vec::with_capacity(BLADERF_FLASH_PAGE_SIZE + padded.len());
        all_data.extend_from_slice(&meta);
        all_data.extend_from_slice(&padded);

        self.erase_write_verify(fpga_page, &all_data)?;

        Ok(())
    }

    /// Erases the FPGA region of the SPI flash.
    ///
    /// Erases all sectors from the FPGA start address to the end of flash,
    /// effectively removing any stored bitstream so the firmware can no
    /// longer autoload an FPGA image.
    pub fn erase_stored_fpga(&mut self) -> Result<()> {
        let fpga_sector = BLADERF_FLASH_ADDR_FPGA / BLADERF_FLASH_ERASE_BLOCK_SIZE as u32;
        let count = self.total_sectors() - fpga_sector;
        self.erase_sectors(fpga_sector, count)
    }
}
