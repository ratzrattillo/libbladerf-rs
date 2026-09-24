//! Sample layouts, format GPIO masks, and pure packed-SC16 conversions.
//!
//! Stock BladeRF1 streaming supports SC16, timestamped SC16, and packet
//! metadata. The other enum variants describe data layouts for conversion;
//! the stock FPGA cannot stream those formats.

use crate::{Error, Result};

/// I/Q sample representation used by a stream or conversion helper.
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum SampleFormat {
    /// Little-endian 16-bit I/Q words with 12 significant bits per component.
    Sc16Q11 = 0,
    /// SC16 samples with a 16-byte header in every FPGA message.
    Sc16Q11Meta = 1,
    /// Variable-length packet metadata, with payload length in 32-bit words.
    PacketMeta = 2,
    /// Signed 8-bit I/Q components; unsupported by the stock BladeRF1 FPGA.
    Sc8Q7 = 3,
    /// Timestamped SC8; unsupported by the stock BladeRF1 FPGA.
    Sc8Q7Meta = 4,
    /// Packed 12-bit components; conversion-only on stock BladeRF1.
    Sc16Q11Packed = 5,
}

/// GPIO bit enabling packet metadata.
pub const BLADERF_GPIO_PACKET: u32 = 1 << 19;
/// GPIO bit enabling timestamp metadata in each FPGA message.
pub const BLADERF_GPIO_TIMESTAMP: u32 = 1 << 16;
/// GPIO bit dividing the timestamp clock for complex sample counting.
pub const BLADERF_GPIO_TIMESTAMP_DIV2: u32 = 1 << 17;
/// GPIO bit enabling SC8 on FPGA designs implementing that mode.
pub const BLADERF_GPIO_8BIT_MODE: u32 = 1 << 20;
/// GPIO bit enabling highly packed SC16 on FPGA designs implementing that mode.
pub const BLADERF_GPIO_HIGHLY_PACKED_MODE: u32 = 1 << 21;

#[inline(always)]
const fn sign_extend_12(val: u16) -> i16 {
    ((val << 4) as i16) >> 4
}

impl SampleFormat {
    /// Returns bytes per complex sample, excluding any metadata headers.
    pub fn sample_size(self) -> usize {
        match self {
            Self::Sc16Q11 | Self::Sc16Q11Meta | Self::PacketMeta => 4,
            Self::Sc16Q11Packed => 3,
            Self::Sc8Q7 | Self::Sc8Q7Meta => 2,
        }
    }

    /// Unpacks pairs of packed 12-bit I/Q components into little-endian SC16.
    ///
    /// # Errors
    /// Rejects odd sample counts or buffers too small for `num_samples`.
    pub fn unpack_sc16q11_packed(src: &[u8], dst: &mut [u8], num_samples: usize) -> Result<()> {
        if !num_samples.is_multiple_of(2) {
            return Err(Error::Argument(
                "num_samples must be a multiple of 2".into(),
            ));
        }
        let src_needed = 3usize.saturating_mul(num_samples);
        let dst_needed = 4usize.saturating_mul(num_samples);
        if src.len() < src_needed {
            return Err(Error::Argument("source buffer too small".into()));
        }
        if dst.len() < dst_needed {
            return Err(Error::Argument("destination buffer too small".into()));
        }
        let pairs = num_samples / 2;
        let src_chunks = src[..src_needed].as_chunks::<6>().0;
        let dst_chunks = dst[..dst_needed].as_chunks_mut::<8>().0;
        for (s, d) in src_chunks.iter().zip(dst_chunks.iter_mut()).take(pairs) {
            let w0 = u16::from_le_bytes([s[0], s[1]]);
            let w1 = u16::from_le_bytes([s[2], s[3]]);
            let w2 = u16::from_le_bytes([s[4], s[5]]);
            let i0 = sign_extend_12(w0 & 0x0FFF);
            let q0 = sign_extend_12((w0 >> 12) | ((w1 & 0x00FF) << 4));
            let i1 = sign_extend_12((w1 >> 8) | ((w2 & 0x000F) << 8));
            let q1 = sign_extend_12(w2 >> 4);
            d[0] = i0 as u8;
            d[1] = (i0 >> 8) as u8;
            d[2] = q0 as u8;
            d[3] = (q0 >> 8) as u8;
            d[4] = i1 as u8;
            d[5] = (i1 >> 8) as u8;
            d[6] = q1 as u8;
            d[7] = (q1 >> 8) as u8;
        }
        Ok(())
    }

    /// Packs pairs of little-endian SC16 samples into packed 12-bit components.
    ///
    /// # Errors
    /// Rejects odd sample counts or buffers too small for `num_samples`.
    pub fn pack_sc16q11_packed(src: &[u8], dst: &mut [u8], num_samples: usize) -> Result<()> {
        if !num_samples.is_multiple_of(2) {
            return Err(Error::Argument(
                "num_samples must be a multiple of 2".into(),
            ));
        }
        let src_needed = 4usize.saturating_mul(num_samples);
        let dst_needed = 3usize.saturating_mul(num_samples);
        if src.len() < src_needed {
            return Err(Error::Argument("source buffer too small".into()));
        }
        if dst.len() < dst_needed {
            return Err(Error::Argument("destination buffer too small".into()));
        }
        let pairs = num_samples / 2;
        let src_chunks = src[..src_needed].as_chunks::<8>().0;
        let dst_chunks = dst[..dst_needed].as_chunks_mut::<6>().0;
        for (s, d) in src_chunks.iter().zip(dst_chunks.iter_mut()).take(pairs) {
            let v0 = i16::from_le_bytes([s[0], s[1]]) as u16;
            let v1 = i16::from_le_bytes([s[2], s[3]]) as u16;
            let v2 = i16::from_le_bytes([s[4], s[5]]) as u16;
            let v3 = i16::from_le_bytes([s[6], s[7]]) as u16;
            let w0 = (v0 & 0x0FFF) | ((v1 & 0x000F) << 12);
            let w1 = ((v1 >> 4) & 0x00FF) | ((v2 & 0x00FF) << 8);
            let w2 = ((v2 >> 8) & 0x000F) | ((v3 & 0x0FFF) << 4);
            d[0] = w0 as u8;
            d[1] = (w0 >> 8) as u8;
            d[2] = w1 as u8;
            d[3] = (w1 >> 8) as u8;
            d[4] = w2 as u8;
            d[5] = (w2 >> 8) as u8;
        }
        Ok(())
    }

    /// Returns whether this representation includes metadata headers.
    pub fn requires_timestamps(self) -> bool {
        matches!(self, Self::Sc16Q11Meta | Self::Sc8Q7Meta | Self::PacketMeta)
    }
}
