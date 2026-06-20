//! Flash-stored calibration data access.
//!
//! Reads and writes factory calibration values from the SPI flash calibration
//! region. The calibration page stores the DAC trim value for VCTCXO tuning
//! and the FPGA package size indicator. These values are written during
//! manufacturing and persist across power cycles.

use crate::bladerf1::board::FlashSession;
use crate::bladerf1::board::fpga::{BLADERF_FLASH_FPGA_SIZE_40KLE, BLADERF_FLASH_FPGA_SIZE_115KLE};
use crate::bladerf1::hardware::spi_flash::{BLADERF_FLASH_ADDR_CAL, BLADERF_FLASH_PAGE_SIZE};
use crate::error::Error;
use crate::error::Result;
use crate::flash::{FpgaSize, binkv_decode_field, make_cal_region};

impl FlashSession<'_> {
    /// Reads the factory DAC trim value from the flash calibration region.
    ///
    /// Parses the binkv-encoded `"DAC"` field from the calibration page.
    /// Returns `Error::BoardState` if the field is missing or not a valid u16.
    pub fn read_flash_dac_trim(&mut self) -> Result<u16> {
        let mut buf = [0u8; BLADERF_FLASH_PAGE_SIZE];
        self.read_cal_cache(&mut buf)?;
        let dac_str = binkv_decode_field(&buf, "DAC")?;
        dac_str
            .parse::<u16>()
            .map_err(|_| Error::BoardState("failed to parse DAC trim from flash"))
    }

    /// Reads the FPGA package size indicator from the flash calibration region.
    ///
    /// Parses the binkv-encoded `"B"` field to determine whether the device
    /// uses a 40KLE or 115KLE FPGA. The value is used to select the correct
    /// bitstream size for flash operations.
    pub fn read_flash_fpga_size(&mut self) -> Result<FpgaSize> {
        let mut buf = [0u8; BLADERF_FLASH_PAGE_SIZE];
        self.read_cal_cache(&mut buf)?;
        let fpga_str = binkv_decode_field(&buf, "B")?;
        FpgaSize::parse(&fpga_str)
    }

    /// Writes a new DAC trim value to the flash calibration region.
    ///
    /// Reads the existing FPGA size indicator, constructs a full calibration
    /// page image, and erases/writes/verifies the calibration sector. Use
    /// this to update the factory trim after performing a new calibration
    /// measurement.
    pub fn write_flash_dac_trim(&mut self, dac_trim: u16) -> Result<()> {
        let fpga_size = self.read_flash_fpga_size()?;
        let cal_image = make_cal_region(fpga_size, dac_trim)?;
        let cal_page = BLADERF_FLASH_ADDR_CAL / BLADERF_FLASH_PAGE_SIZE as u32;
        self.erase_write_verify(cal_page, &cal_image)
    }

    /// Returns the expected FPGA bitstream size in bytes based on flash calibration data.
    ///
    /// Reads the FPGA size indicator and maps it to the corresponding
    /// bitstream byte count. Returns `Error::Argument` if the flash contains
    /// an unrecognized FPGA size.
    pub fn get_fpga_bytes(&mut self) -> Result<usize> {
        match self.read_flash_fpga_size()? {
            FpgaSize::KLE40 => Ok(BLADERF_FLASH_FPGA_SIZE_40KLE),
            FpgaSize::KLE115 => Ok(BLADERF_FLASH_FPGA_SIZE_115KLE),
            _ => Err(Error::Argument("unsupported FPGA size".into())),
        }
    }
}
