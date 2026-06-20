//! BladeRF1 retune request and response packet builders.
//!
//! Provides `NiosPktRetuneRequest` for encoding LMS6002D retune commands
//! and `NiosPktRetuneResponse` for decoding the device's retune response.
//! The retune packet uses magic byte 0x54 and occupies the full 16-byte
//! NIOS packet buffer.

use crate::bladerf1::hardware::lms6002d::{Band, Tune};
use crate::channel::Channel;
use crate::error::Result;
use crate::protocol::nios::NiosPacketError;
use crate::protocol::nios::packet_generic::NiosPacket;

/// Magic byte identifying a retune packet.
pub const NIOS_PKT_RETUNE_MAGIC: u8 = 0x54;

/// Builder for a NIOS retune request packet.
///
/// Wraps a 16-byte buffer and provides `prepare()` to populate all
/// fields: timestamp, nint, nfrac, freqsel, vcocap, band, tune mode,
/// and expansion board GPIO. Also offers accessor methods to inspect
/// the encoded values.
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct NiosPktRetuneRequest<'a> {
    buf: &'a mut [u8],
}
impl<'a> NiosPacket for NiosPktRetuneRequest<'a> {
    fn as_slice(&self) -> &[u8] {
        self.buf
    }
    fn as_slice_mut(&mut self) -> &mut [u8] {
        self.buf
    }
}
impl<'a> NiosPktRetuneRequest<'a> {
    const NIOS_PKT_SIZE: usize = 16;
    const IDX_MAGIC: usize = 0;
    const IDX_TIMESTAMP: usize = 1;
    const IDX_INTFRAC: usize = 9;
    const IDX_FREQSEL: usize = 13;
    const IDX_BANDSEL: usize = 14;
    const IDX_XB_GPIO: usize = 15;
    const FLAG_RX: u8 = 1 << 6;
    const FLAG_TX: u8 = 1 << 7;
    const FLAG_QUICK_TUNE: u8 = 1 << 6;
    const FLAG_LOW_BAND: u8 = 1 << 7;
    const MASK_NFRAC: u32 = 0x7fffff;
    const MASK_FREQSEL: u8 = 0x3f;
    const MASK_VCOCAP: u8 = 0x3f;
    /// Creates a new retune request packet from a buffer.
    ///
    /// Requires `buf` to be at least 16 bytes. Returns an error if the
    /// buffer is too small.
    pub fn new(buf: &'a mut [u8]) -> Result<Self> {
        if buf.len() < Self::NIOS_PKT_SIZE {
            return Err(NiosPacketError::InvalidSize(buf.len()).into());
        }
        Ok(Self {
            buf: &mut buf[..Self::NIOS_PKT_SIZE],
        })
    }
    /// Populates the packet with all retune parameters.
    ///
    /// Encodes channel, timestamp, nint, nfrac, freqsel, vcocap, band,
    /// tune mode, and xb_gpio into the packet buffer. Returns an error
    /// if any value exceeds its field capacity.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare(
        &mut self,
        channel: Channel,
        timestamp: u64,
        nint: u16,
        nfrac: u32,
        freqsel: u8,
        vcocap: u8,
        band: Band,
        tune: Tune,
        xb_gpio: u8,
    ) -> Result<()> {
        self.set_magic();
        self.set_timestamp(timestamp);
        self.set_nint(nint);
        self.set_nfrac(nfrac)?;
        self.set_freqsel(freqsel, channel)?;
        self.set_vcocap(vcocap)?;
        self.set_band(band);
        self.set_tune(tune);
        self.set_xb_gpio(xb_gpio);
        Ok(())
    }
    fn set_magic(&mut self) {
        self.buf[Self::IDX_MAGIC] = NIOS_PKT_RETUNE_MAGIC;
    }
    fn set_timestamp(&mut self, timestamp: u64) {
        self.write_u64(Self::IDX_TIMESTAMP, timestamp);
    }
    fn set_nint(&mut self, nint: u16) {
        self.buf[Self::IDX_INTFRAC] = (nint >> 1) as u8;
        self.buf[Self::IDX_INTFRAC + 1] &= 0x7f;
        self.buf[Self::IDX_INTFRAC + 1] |= ((nint & 0x1) << 7) as u8;
    }
    fn set_nfrac(&mut self, nfrac: u32) -> Result<()> {
        if nfrac > Self::MASK_NFRAC {
            return Err(NiosPacketError::NfracOverflow(nfrac).into());
        }
        self.buf[Self::IDX_INTFRAC + 1] &= 0x80;
        self.buf[Self::IDX_INTFRAC + 1] |= ((nfrac >> 16) & 0x7f) as u8;
        self.buf[Self::IDX_INTFRAC + 2] = (nfrac >> 8) as u8;
        self.buf[Self::IDX_INTFRAC + 3] = nfrac as u8;
        Ok(())
    }
    fn set_freqsel(&mut self, freqsel: u8, channel: Channel) -> Result<()> {
        if freqsel > Self::MASK_FREQSEL {
            return Err(NiosPacketError::FreqselOverflow(freqsel, Self::MASK_FREQSEL).into());
        }
        self.buf[Self::IDX_FREQSEL] = freqsel
            | match channel {
                Channel::Rx => Self::FLAG_RX,
                Channel::Tx => Self::FLAG_TX,
            };
        Ok(())
    }
    fn set_vcocap(&mut self, vcocap: u8) -> Result<()> {
        if vcocap > Self::MASK_VCOCAP {
            return Err(NiosPacketError::VcocapOverflow(vcocap, Self::MASK_VCOCAP).into());
        }
        self.buf[Self::IDX_BANDSEL] &= !Self::MASK_VCOCAP;
        self.buf[Self::IDX_BANDSEL] |= vcocap & Self::MASK_VCOCAP;
        Ok(())
    }
    fn set_band(&mut self, band: Band) {
        match band {
            Band::Low => self.buf[Self::IDX_BANDSEL] |= Self::FLAG_LOW_BAND,
            Band::High => self.buf[Self::IDX_BANDSEL] &= !Self::FLAG_LOW_BAND,
        }
    }
    fn set_tune(&mut self, tune: Tune) {
        match tune {
            Tune::Quick => self.buf[Self::IDX_BANDSEL] |= Self::FLAG_QUICK_TUNE,
            Tune::Normal => self.buf[Self::IDX_BANDSEL] &= !Self::FLAG_QUICK_TUNE,
        }
    }
    fn set_xb_gpio(&mut self, xb_gpio: u8) {
        self.buf[Self::IDX_XB_GPIO] = xb_gpio;
    }
    /// Returns the timestamp field of the packet.
    pub fn timestamp(&self) -> u64 {
        self.read_u64(Self::IDX_TIMESTAMP)
    }
    /// Returns the nint (PLL integer divider) field of the packet.
    pub fn nint(&self) -> u16 {
        let mut nint = (self.buf[Self::IDX_INTFRAC] as u16) << 1;
        nint |= (self.buf[Self::IDX_INTFRAC + 1] as u16) >> 7;
        nint
    }
    /// Returns the nfrac (PLL fractional divider) field of the packet.
    pub fn nfrac(&self) -> u32 {
        let mut nfrac: u32 = ((self.buf[Self::IDX_INTFRAC + 1] & 0x7f) as u32) << 16;
        nfrac |= (self.buf[Self::IDX_INTFRAC + 2] as u32) << 8;
        nfrac |= self.buf[Self::IDX_INTFRAC + 3] as u32;
        nfrac
    }
    /// Returns the freqsel (frequency select) field of the packet.
    pub fn freqsel(&self) -> u8 {
        self.buf[Self::IDX_FREQSEL] & Self::MASK_FREQSEL
    }
    /// Returns the vcocap (VCO capacitor) field of the packet.
    pub fn vcocap(&self) -> u8 {
        self.buf[Self::IDX_BANDSEL] & Self::MASK_VCOCAP
    }
    /// Returns the band (high/low) selection of the packet.
    pub fn band(&self) -> Band {
        if (self.buf[Self::IDX_BANDSEL] & Self::FLAG_LOW_BAND) == 0 {
            Band::High
        } else {
            Band::Low
        }
    }
    /// Returns the tune mode (quick/normal) of the packet.
    pub fn tune(&self) -> Tune {
        if (self.buf[Self::IDX_BANDSEL] & Self::FLAG_QUICK_TUNE) == 0 {
            Tune::Normal
        } else {
            Tune::Quick
        }
    }
    /// Returns the expansion board GPIO value of the packet.
    pub fn xb_gpio(&self) -> u8 {
        self.buf[Self::IDX_XB_GPIO]
    }
}

/// Decoder for a NIOS retune response packet.
///
/// Provides access to the retune duration, VCO capacitor value,
/// validity flags, and success status from the 16-byte response buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NiosPktRetuneResponse<'a> {
    buf: &'a [u8],
}
impl<'a> NiosPktRetuneResponse<'a> {
    const NIOS_PKT_SIZE: usize = 16;
    const IDX_TIMESTAMP: usize = 1;
    const IDX_VCOCAP: usize = 9;
    const IDX_FLAGS: usize = 10;
    const MASK_VCOCAP: u8 = 0x3f;
    const FLAG_DURATION_VCOCAP_VALID: u8 = 0x1;
    const FLAG_SUCCESS: u8 = 0x2;
    /// Creates a new retune response decoder from a buffer.
    ///
    /// Requires `buf` to be at least 16 bytes.
    pub fn new(buf: &'a [u8]) -> Result<Self> {
        if buf.len() < Self::NIOS_PKT_SIZE {
            return Err(NiosPacketError::InvalidSize(buf.len()).into());
        }
        Ok(Self {
            buf: &buf[..Self::NIOS_PKT_SIZE],
        })
    }
    /// Returns the retune duration in clock ticks.
    pub fn duration(&self) -> u64 {
        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(&self.buf[Self::IDX_TIMESTAMP..Self::IDX_TIMESTAMP + 8]);
        u64::from_le_bytes(bytes)
    }
    /// Returns `true` if the duration and vcocap fields are valid.
    pub fn vcocap_valid(&self) -> bool {
        (self.buf[Self::IDX_FLAGS] & Self::FLAG_DURATION_VCOCAP_VALID) != 0
    }
    /// Returns the VCO capacitor value from the retune response.
    pub fn vcocap(&self) -> u8 {
        self.buf[Self::IDX_VCOCAP] & Self::MASK_VCOCAP
    }
    /// Returns `true` if the retune operation succeeded.
    pub fn is_success(&self) -> bool {
        (self.buf[Self::IDX_FLAGS] & Self::FLAG_SUCCESS) != 0
    }
}
