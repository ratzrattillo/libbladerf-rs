//! Flash-stored calibration data access.
//!
//! The calibration page stores the factory VCTCXO trim DAC value (`"DAC"`)
//! and the FPGA package size (`"B"`) in BINKV format. Port of
//! `spi_flash_read_vctcxo_trim()` and `spi_flash_read_fpga_size()` from
//! `board/bladerf1/flash.c`, which libbladeRF shares with the BladeRF2.

use crate::bladerf2::board::FlashSession;
use crate::bladerf2::hardware::spi_flash::BLADERF_FLASH_PAGE_SIZE;
use crate::error::{Error, Result};
use crate::flash::{FpgaSize, binkv_decode_field};
use crate::maybe_future::Op;
use nusb::MaybeFuture;

/// VCTCXO trim value libbladeRF falls back to when the flash holds none.
pub const DEFAULT_VCTCXO_TRIM: u16 = 0x1FFC;

pub(crate) fn fpga_size_from_cal(cal: &[u8]) -> Result<FpgaSize> {
    FpgaSize::parse(&binkv_decode_field(cal, "B")?)
}

fn dac_trim_from_cal(cal: &[u8]) -> Result<u16> {
    let value = binkv_decode_field(cal, "DAC")?;
    let parsed = if let Some(hex) = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
    {
        u16::from_str_radix(hex, 16)
    } else if value.len() > 1 && value.starts_with('0') {
        u16::from_str_radix(&value[1..], 8)
    } else {
        value.parse()
    };
    parsed.map_err(|_| Error::FlashData("failed to parse DAC trim from flash"))
}

impl FlashSession<'_> {
    /// Reads the factory VCTCXO trim DAC value from the flash calibration region.
    ///
    /// Parses the BINKV `"DAC"` field (decimal, `0x` hex, or `0` octal, as
    /// `strtoul` base 0 accepts). libbladeRF substitutes
    /// [`DEFAULT_VCTCXO_TRIM`] when this fails.
    ///
    /// # Errors
    /// Returns [`Error::FlashData`] if the field is missing, corrupt, or not
    /// a valid `u16`.
    pub fn read_flash_dac_trim(&mut self) -> impl MaybeFuture<Output = Result<u16>> {
        Op::new(async move {
            let mut buf = [0u8; BLADERF_FLASH_PAGE_SIZE];
            self.read_cal_cache(&mut buf).await?;
            dac_trim_from_cal(&buf)
        })
    }

    /// Reads the FPGA package size (`A4`, `A5`, or `A9`) from the flash
    /// calibration region.
    ///
    /// # Errors
    /// Returns [`Error::FlashData`] if the `"B"` field is missing or corrupt,
    /// or [`Error::Argument`] for an unknown size string.
    pub fn read_flash_fpga_size(&mut self) -> impl MaybeFuture<Output = Result<FpgaSize>> {
        Op::new(async move {
            let mut buf = [0u8; BLADERF_FLASH_PAGE_SIZE];
            self.read_cal_cache(&mut buf).await?;
            fpga_size_from_cal(&buf)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flash::binkv_add_field;

    fn cal(fields: &[(&str, &str)]) -> [u8; BLADERF_FLASH_PAGE_SIZE] {
        let mut buf = [0xFF; BLADERF_FLASH_PAGE_SIZE];
        for (key, value) in fields {
            binkv_add_field(&mut buf, key, value).unwrap();
        }
        buf
    }

    #[test]
    fn reads_fpga_size_and_dac_trim() {
        let page = cal(&[("B", "A9"), ("DAC", "8000")]);
        assert_eq!(fpga_size_from_cal(&page).unwrap(), FpgaSize::A9);
        assert_eq!(dac_trim_from_cal(&page).unwrap(), 8000);
    }

    #[test]
    fn dac_trim_accepts_strtoul_base_zero_forms() {
        for (text, value) in [("0x1ffc", 0x1FFC), ("0X1FFC", 0x1FFC), ("010", 8), ("0", 0)] {
            assert_eq!(dac_trim_from_cal(&cal(&[("DAC", text)])).unwrap(), value);
        }
        for text in ["", "70000", "0x", "abc"] {
            assert!(dac_trim_from_cal(&cal(&[("DAC", text)])).is_err());
        }
    }

    #[test]
    fn missing_fields_are_flash_data_errors() {
        let page = [0xFF; BLADERF_FLASH_PAGE_SIZE];
        assert!(matches!(
            fpga_size_from_cal(&page),
            Err(Error::FlashData(_))
        ));
        assert!(matches!(dac_trim_from_cal(&page), Err(Error::FlashData(_))));
    }
}
