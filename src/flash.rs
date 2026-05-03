use crate::error::{Error, Result};

// Flash constants
pub const BLADERF_FLASH_PAGE_SIZE: usize = 256;
pub const BLADERF_FLASH_ERASE_BLOCK_SIZE: usize = 64 * 1_024;
pub const BLADERF_FLASH_ADDR_CAL: u32 = 0x00030000;
pub const BLADERF_FLASH_BYTE_LEN_CAL: usize = 0x100;
pub const BLADERF_FLASH_TOTAL_PAGES: u16 = 4096;
pub const BLADERF_FLASH_TOTAL_SECTORS: u16 = 16;
pub const BLADERF_FLASH_CAL_PAGE_START: u16 =
    (BLADERF_FLASH_ADDR_CAL / BLADERF_FLASH_PAGE_SIZE as u32) as u16;
pub const BLADERF_FLASH_CAL_PAGE_END: u16 =
    BLADERF_FLASH_CAL_PAGE_START + (BLADERF_FLASH_BYTE_LEN_CAL / BLADERF_FLASH_PAGE_SIZE) as u16;

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
    pub fn as_str(&self) -> &'static str {
        match self {
            FpgaSize::KLE40 => "40",
            FpgaSize::KLE115 => "115",
            FpgaSize::A4 => "A4",
            FpgaSize::A5 => "A5",
            FpgaSize::A9 => "A9",
        }
    }

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

/// CCITT CRC16 (polynomial 0x1021).
/// Port of zcrc() from firmware_common/misc.h
pub fn zcrc(data: &[u8]) -> u16 {
    let mut crc: u16 = 0;
    for &byte in data {
        crc ^= (byte as u16) << 8;
        for _ in 0..8 {
            if crc & 0x8000 != 0 {
                crc = (crc << 1) ^ 0x1021;
            } else {
                crc <<= 1;
            }
        }
    }
    crc
}

/// Encode a single binkv field into buf starting at idx.
/// Returns the new index after encoding.
/// Port of binkv_encode_field() from flash.c:499-516
pub fn binkv_encode_field(buf: &mut [u8], idx: usize, field: &str, val: &str) -> Result<usize> {
    let flen = field.len();
    let vlen = val.len();
    let tlen = flen + vlen + 1; // +1 for length byte included in CRC

    if tlen >= 256 || idx + tlen + 2 > buf.len() {
        return Err(Error::HardwareState(
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

/// Decode a binkv field by name from buf.
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
            return Err(Error::HardwareState("binkv CRC mismatch"));
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

    Err(Error::HardwareState("binkv field not found"))
}

/// Append a binkv field at the end of existing fields in buf.
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

/// Build a 256-byte calibration image in binkv format.
/// Port of make_cal_region() from image.c:513-557
pub fn make_cal_region(fpga_size: FpgaSize, dac_trim: u16) -> Result<[u8; 256]> {
    let mut buf = [0xFFu8; 256];
    binkv_add_field(&mut buf, "B", fpga_size.as_str())?;
    let dac_str = format!("{}", dac_trim);
    binkv_add_field(&mut buf, "DAC", &dac_str)?;
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zcrc_empty() {
        assert_eq!(zcrc(&[]), 0);
    }

    #[test]
    fn test_zcrc_known() {
        // CRC16-CCITT with init=0 (matching C zcrc()) of "123456789" is 0x31C3
        // Note: 0x29B1 is CRC-CCITT-FALSE (init=0xFFFF), which is different
        assert_eq!(zcrc(b"123456789"), 0x31C3);
    }

    #[test]
    fn test_binkv_roundtrip() {
        let mut buf = [0xFFu8; 256];
        binkv_add_field(&mut buf, "B", "40").unwrap();
        binkv_add_field(&mut buf, "DAC", "32768").unwrap();

        assert_eq!(binkv_decode_field(&buf, "B").unwrap(), "40");
        assert_eq!(binkv_decode_field(&buf, "DAC").unwrap(), "32768");
    }

    #[test]
    fn test_binkv_all_ff() {
        let buf = [0xFFu8; 256];
        assert!(binkv_decode_field(&buf, "B").is_err());
    }

    #[test]
    fn test_binkv_encode_decode_single() {
        let mut buf = [0xFFu8; 256];
        let idx = binkv_encode_field(&mut buf, 0, "DAC", "1000").unwrap();
        assert!(idx > 0);
        assert_eq!(binkv_decode_field(&buf, "DAC").unwrap(), "1000");
    }

    #[test]
    fn test_make_cal_region() {
        let cal = make_cal_region(FpgaSize::KLE40, 0x8000).unwrap();
        assert_eq!(binkv_decode_field(&cal, "B").unwrap(), "40");
        assert_eq!(binkv_decode_field(&cal, "DAC").unwrap(), "32768");
    }

    #[test]
    fn test_fpga_size_roundtrip() {
        for (variant, s) in [
            (FpgaSize::KLE40, "40"),
            (FpgaSize::KLE115, "115"),
            (FpgaSize::A4, "A4"),
            (FpgaSize::A5, "A5"),
            (FpgaSize::A9, "A9"),
        ] {
            assert_eq!(variant.as_str(), s);
            assert_eq!(FpgaSize::parse(s).unwrap(), variant);
        }
        assert!(FpgaSize::parse("unknown").is_err());
    }
}
