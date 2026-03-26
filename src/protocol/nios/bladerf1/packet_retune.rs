use crate::protocol::nios::packet_generic::NiosPacket;
use crate::{Band, Channel, Error, NiosPacketError, Result, Tune};

pub const NIOS_PKT_RETUNE_MAGIC: u8 = 0x54;

pub struct NiosPktRetuneRequest {
    buf: Vec<u8>,
}

impl NiosPacket for NiosPktRetuneRequest {
    fn as_slice(&self) -> &[u8] {
        &self.buf
    }

    fn as_slice_mut(&mut self) -> &mut [u8] {
        &mut self.buf
    }

    fn into_inner(self) -> Vec<u8> {
        self.buf
    }

    fn into_packet(self) -> Vec<u8> {
        let mut buf = self.buf;
        buf.truncate(16);
        buf
    }
}

impl NiosPktRetuneRequest {
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
    pub(crate) const RETUNE_NOW: u64 = 0x00;
    pub(crate) const CLEAR_QUEUE: u64 = u64::MAX;

    #[allow(clippy::too_many_arguments)]
    pub fn prepare(
        self,
        channel: Channel,
        timestamp: u64,
        nint: u16,
        nfrac: u32,
        freqsel: u8,
        vcocap: u8,
        band: Band,
        tune: Tune,
        xb_gpio: u8,
    ) -> Result<Self> {
        Ok(self
            .set_magic()
            .set_timestamp(timestamp)
            .set_nint(nint)
            .set_nfrac(nfrac)?
            .set_freqsel(freqsel, channel)?
            .set_vcocap(vcocap)?
            .set_band(band)
            .set_tune(tune)
            .set_xb_gpio(xb_gpio))
    }

    fn set_magic(mut self) -> Self {
        self.buf[Self::IDX_MAGIC] = NIOS_PKT_RETUNE_MAGIC;
        self
    }

    fn set_timestamp(mut self, timestamp: u64) -> Self {
        self.write_u64(Self::IDX_TIMESTAMP, timestamp);
        self
    }

    fn set_nint(mut self, nint: u16) -> Self {
        self.buf[Self::IDX_INTFRAC] = (nint >> 1) as u8;
        self.buf[Self::IDX_INTFRAC + 1] &= 0x7f; // clear the nint bit
        self.buf[Self::IDX_INTFRAC + 1] |= ((nint & 0x1) << 7) as u8;
        self
    }

    fn set_nfrac(mut self, nfrac: u32) -> Result<Self> {
        if nfrac > Self::MASK_NFRAC {
            return Err(NiosPacketError::NfracOverflow(nfrac).into());
        }
        self.buf[Self::IDX_INTFRAC + 1] &= 0x80; // clear the nfrac bits, keep nint bit
        self.buf[Self::IDX_INTFRAC + 1] |= ((nfrac >> 16) & 0x7f) as u8;
        self.buf[Self::IDX_INTFRAC + 2] = (nfrac >> 8) as u8;
        self.buf[Self::IDX_INTFRAC + 3] = nfrac as u8;
        Ok(self)
    }

    fn set_freqsel(mut self, freqsel: u8, channel: Channel) -> Result<Self> {
        if freqsel > Self::MASK_FREQSEL {
            return Err(NiosPacketError::FreqselOverflow(freqsel, Self::MASK_FREQSEL).into());
        }
        self.buf[Self::IDX_FREQSEL] = freqsel
            | match channel {
                Channel::Rx => Self::FLAG_RX,
                Channel::Tx => Self::FLAG_TX,
            };
        Ok(self)
    }

    fn set_vcocap(mut self, vcocap: u8) -> Result<Self> {
        if vcocap > Self::MASK_VCOCAP {
            return Err(NiosPacketError::VcocapOverflow(vcocap, Self::MASK_VCOCAP).into());
        }
        self.buf[Self::IDX_BANDSEL] &= !Self::MASK_VCOCAP; // clear vcocap bits
        self.buf[Self::IDX_BANDSEL] |= vcocap & Self::MASK_VCOCAP;
        Ok(self)
    }

    fn set_band(mut self, band: Band) -> Self {
        match band {
            Band::Low => self.buf[Self::IDX_BANDSEL] |= Self::FLAG_LOW_BAND,
            Band::High => self.buf[Self::IDX_BANDSEL] &= !Self::FLAG_LOW_BAND,
        }
        self
    }

    fn set_tune(mut self, tune: Tune) -> Self {
        match tune {
            Tune::Quick => self.buf[Self::IDX_BANDSEL] |= Self::FLAG_QUICK_TUNE,
            Tune::Normal => self.buf[Self::IDX_BANDSEL] &= !Self::FLAG_QUICK_TUNE,
        }
        self
    }

    fn set_xb_gpio(mut self, xb_gpio: u8) -> Self {
        self.buf[Self::IDX_XB_GPIO] = xb_gpio;
        self
    }

    pub fn timestamp(&self) -> u64 {
        self.read_u64(Self::IDX_TIMESTAMP)
    }

    pub fn nint(&self) -> u16 {
        let mut nint = (self.buf[Self::IDX_INTFRAC] as u16) << 1;
        nint |= (self.buf[Self::IDX_INTFRAC + 1] as u16) >> 7;
        nint
    }

    pub fn nfrac(&self) -> u32 {
        let mut nfrac: u32 = ((self.buf[Self::IDX_INTFRAC + 1] & 0x7f) as u32) << 16;
        nfrac |= (self.buf[Self::IDX_INTFRAC + 2] as u32) << 8;
        nfrac |= self.buf[Self::IDX_INTFRAC + 3] as u32;
        nfrac
    }

    pub fn freqsel(&self) -> u8 {
        self.buf[Self::IDX_FREQSEL] & Self::MASK_FREQSEL
    }

    pub fn vcocap(&self) -> u8 {
        self.buf[Self::IDX_BANDSEL] & Self::MASK_VCOCAP
    }

    pub fn band(&self) -> Band {
        if self.buf[Self::IDX_BANDSEL] & Self::FLAG_LOW_BAND == 0 {
            Band::High
        } else {
            Band::Low
        }
    }

    pub fn tune(&self) -> Tune {
        if self.buf[Self::IDX_BANDSEL] & Self::FLAG_QUICK_TUNE == 0 {
            Tune::Normal
        } else {
            Tune::Quick
        }
    }

    pub fn xb_gpio(&self) -> u8 {
        self.buf[Self::IDX_XB_GPIO]
    }
}

impl TryFrom<Vec<u8>> for NiosPktRetuneRequest {
    type Error = Error;

    fn try_from(buf: Vec<u8>) -> Result<Self> {
        Ok(Self { buf })
    }
}

pub struct NiosPktRetuneResponse {
    buf: Vec<u8>,
}

impl NiosPacket for NiosPktRetuneResponse {
    fn as_slice(&self) -> &[u8] {
        &self.buf
    }

    fn as_slice_mut(&mut self) -> &mut [u8] {
        &mut self.buf
    }

    fn into_inner(self) -> Vec<u8> {
        self.buf
    }

    fn into_packet(self) -> Vec<u8> {
        self.buf
    }
}

impl NiosPktRetuneResponse {
    const IDX_TIMESTAMP: usize = 1;
    const IDX_VCOCAP: usize = 9;
    const IDX_FLAGS: usize = 10;

    const MASK_VCOCAP: u8 = 0x3f;
    const FLAG_DURATION_VCOCAP_VALID: u8 = 0x1;
    const FLAG_SUCCESS: u8 = 0x2;

    pub fn duration(&self) -> u64 {
        self.read_u64(Self::IDX_TIMESTAMP)
    }

    pub fn vcocap_valid(&self) -> bool {
        self.buf[Self::IDX_FLAGS] & Self::FLAG_DURATION_VCOCAP_VALID != 0
    }

    pub fn vcocap(&self) -> u8 {
        self.buf[Self::IDX_VCOCAP] & Self::MASK_VCOCAP
    }

    pub fn is_success(&self) -> bool {
        self.buf[Self::IDX_FLAGS] & Self::FLAG_SUCCESS != 0
    }
}

impl TryFrom<Vec<u8>> for NiosPktRetuneResponse {
    type Error = Error;

    fn try_from(mut buf: Vec<u8>) -> Result<Self> {
        if buf.len() < 16 {
            return Err(NiosPacketError::InvalidSize(buf.len()).into());
        }
        buf.truncate(16);
        Ok(Self { buf })
    }
}
