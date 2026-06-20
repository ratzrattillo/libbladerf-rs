//! Flash size decode, FPGA size enum, BINKV encode/decode, and calibration region builder.
//!
//! The BINKV format stores key-value pairs in binary form. Each field consists of
//! a length byte, the ASCII key, the ASCII value (concatenated, no separator), and a
//! two-byte CCITT CRC16 over the length+key+value. Unwritten flash cells read as 0xFF,
//! which serves as the terminator.

use crate::error::{Error, Result};

/// Minimum firmware image size for BladeRF flash.
pub use crate::bladerf1::board::firmware::BLADERF_FLASH_MIN_FW_SIZE;
/// FPGA bitstream size constants and validation for BladeRF flash.
pub use crate::bladerf1::board::fpga::{
    BLADERF_FLASH_FPGA_SIZE_40KLE, BLADERF_FLASH_FPGA_SIZE_115KLE, is_valid_fpga_size,
};
/// SPI flash address and size constants.
pub use crate::bladerf1::hardware::spi_flash::{
    BLADERF_FLASH_ADDR_CAL, BLADERF_FLASH_ADDR_FIRMWARE, BLADERF_FLASH_ADDR_FPGA,
    BLADERF_FLASH_BYTE_LEN_CAL, BLADERF_FLASH_BYTE_LEN_FIRMWARE, BLADERF_FLASH_ERASE_BLOCK_SIZE,
    BLADERF_FLASH_PAGE_SIZE,
};

/// FPGA size variants stored in calibration flash.
/// KLE40/KLE115 are bladeRF1, A4/A5/A9 are bladeRF2.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FpgaSize {
    KLE40,
    KLE115,
    A4,
    A5,
    A9,
}

impl FpgaSize {
    /// Returns the short identifier string (e.g. "40", "115", "A4").
    pub fn as_str(&self) -> &'static str {
        match self {
            FpgaSize::KLE40 => "40",
            FpgaSize::KLE115 => "115",
            FpgaSize::A4 => "A4",
            FpgaSize::A5 => "A5",
            FpgaSize::A9 => "A9",
        }
    }

    /// Returns the hosted variant label for FPGA bitstream naming.
    /// Returns an error for bladeRF2 variants (A4, A5, A9).
    pub fn variant_label(&self) -> Result<&'static str> {
        match self {
            FpgaSize::KLE40 => Ok("hostedx40"),
            FpgaSize::KLE115 => Ok("hostedx115"),
            _ => Err(Error::Unsupported("FPGA variant")),
        }
    }

    /// Parses an FPGA size from its short string identifier.
    /// Recognizes "40", "115", "A4", "A5", and "A9".
    pub fn parse(s: &str) -> Result<Self> {
        match s {
            "40" => Ok(FpgaSize::KLE40),
            "115" => Ok(FpgaSize::KLE115),
            "A4" => Ok(FpgaSize::A4),
            "A5" => Ok(FpgaSize::A5),
            "A9" => Ok(FpgaSize::A9),
            _ => Err(Error::Argument("unknown FPGA size".into())),
        }
    }
}

/// CCITT CRC16 checksum (polynomial 0x1021).
/// Computes a 16-bit CRC over the given byte slice.
/// Port of zcrc() from firmware_common/misc.h
pub fn zcrc(data: &[u8]) -> u16 {
    let mut crc: u16 = 0;
    for &byte in data {
        crc ^= (byte as u16) << 8;
        for _ in 0..8 {
            if (crc & 0x8000) != 0 {
                crc = (crc << 1) ^ 0x1021;
            } else {
                crc <<= 1;
            }
        }
    }
    crc
}

/// Encodes a single BINKV key-value field into the buffer at the given index.
/// Writes the length byte, key, value, and 2-byte CRC. Returns the index
/// immediately following the encoded field. Returns an error if the field
/// exceeds 255 bytes total or would overflow the buffer.
/// Port of binkv_encode_field() from flash.c:499-516
pub fn binkv_encode_field(buf: &mut [u8], idx: usize, field: &str, val: &str) -> Result<usize> {
    let flen = field.len();
    let vlen = val.len();
    let tlen = flen + vlen + 1; // +1 for length byte included in CRC

    if tlen >= 256 || idx + tlen + 2 > buf.len() {
        return Err(Error::BoardState(
            "binkv field too large or buffer overflow",
        ));
    }

    buf[idx] = (flen + vlen) as u8;
    buf[idx + 1..idx + 1 + flen].copy_from_slice(field.as_bytes());
    buf[idx + 1 + flen..idx + 1 + flen + vlen].copy_from_slice(val.as_bytes());

    let crc = zcrc(&buf[idx..idx + tlen]);
    buf[idx + tlen] = crc as u8;
    buf[idx + tlen + 1] = (crc >> 8) as u8;

    Ok(idx + tlen + 2)
}

/// Decodes a BINKV field by name from a buffer of concatenated fields.
/// Iterates through fields, verifying CRC for each, until the matching key is found.
/// Stops on 0xFF (unwritten flash) or buffer boundary. Returns an error if the
/// field is not found or a CRC mismatch occurs.
/// Port of binkv_decode_field() from flash.c:462-497
pub fn binkv_decode_field(buf: &[u8], field: &str) -> Result<String> {
    let flen = field.len();
    let mut pos = 0;

    while pos < buf.len() {
        let c = buf[pos] as usize;

        // 0xFF means unwritten flash
        if c == 0xFF {
            break;
        }

        // Check we have enough room for length byte + data + 2-byte CRC
        if pos + c + 3 > buf.len() {
            break;
        }

        let stored_crc = u16::from_le_bytes([buf[pos + c + 1], buf[pos + c + 2]]);
        let calc_crc = zcrc(&buf[pos..pos + c + 1]);

        if stored_crc != calc_crc {
            return Err(Error::BoardState("binkv CRC mismatch"));
        }

        // Check if field name matches (starts right after length byte)
        if flen <= c && &buf[pos + 1..pos + 1 + flen] == field.as_bytes() {
            let val_start = pos + 1 + flen;
            let val_end = pos + 1 + c;
            let val = String::from_utf8_lossy(&buf[val_start..val_end]).into_owned();
            return Ok(val);
        }

        pos += c + 3;
    }

    Err(Error::BoardState("binkv field not found"))
}

/// Appends a BINKV field at the end of existing fields in the buffer.
/// Scans forward to find the first 0xFF terminator or buffer end, then encodes
/// the new field at that position.
/// Port of binkv_add_field() from flash.c:518-542
pub fn binkv_add_field(buf: &mut [u8], field: &str, val: &str) -> Result<()> {
    let mut i = 0;
    while i < buf.len() {
        let field_len = buf[i] as usize;
        if field_len == 0xFF {
            break;
        }
        i += field_len + 3;
    }
    binkv_encode_field(buf, i, field, val)?;
    Ok(())
}

/// Pads `data` to a multiple of flash page size with 0xFF bytes.
/// Returns a new vector containing the original data followed by padding.
pub fn pad_to_page(data: &[u8]) -> Vec<u8> {
    let padding =
        (BLADERF_FLASH_PAGE_SIZE - data.len() % BLADERF_FLASH_PAGE_SIZE) % BLADERF_FLASH_PAGE_SIZE;
    let mut out = Vec::with_capacity(data.len() + padding);
    out.extend_from_slice(data);
    out.extend(std::iter::repeat_n(0xFF, padding));
    out
}

/// Decodes the flash memory size in bytes from the JEDEC manufacturer and device ID.
/// Supports Macronix (0xC2), Winbond (0xEF), and Renesas (0x1F) devices.
/// Returns `Error::Unsupported` for unknown manufacturer or device ID combinations.
/// Port of spi_flash_decode_flash_architecture() from bladerf1/flash.c
pub fn decode_flash_size(manufacturer_id: u8, device_id: u8) -> Result<u32> {
    match manufacturer_id {
        0xC2 => match device_id {
            0x36 => Ok(32 << 17),
            _ => Err(Error::Unsupported("unknown Macronix flash device")),
        },
        0xEF => match device_id {
            0x15 => Ok(32 << 17),
            0x16 => Ok(64 << 17),
            0x17 => Ok(128 << 17),
            _ => Err(Error::Unsupported("unknown Winbond flash device")),
        },
        0x1F => match device_id {
            0x47 => Ok(32 << 17),
            _ => Err(Error::Unsupported("unknown Renesas flash device")),
        },
        _ => Err(Error::Unsupported("unknown flash manufacturer")),
    }
}

/// Builds a 256-byte calibration region image in BINKV format.
/// Populates the buffer with the FPGA size field ("B") and DAC trim value field ("DAC"),
/// padded with 0xFF bytes for unwritten flash.
/// Port of make_cal_region() from image.c:513-557
pub fn make_cal_region(fpga_size: FpgaSize, dac_trim: u16) -> Result<[u8; 256]> {
    let mut buf = [0xFFu8; 256];
    binkv_add_field(&mut buf, "B", fpga_size.as_str())?;
    let dac_str = format!("{}", dac_trim);
    binkv_add_field(&mut buf, "DAC", &dac_str)?;
    Ok(buf)
}
