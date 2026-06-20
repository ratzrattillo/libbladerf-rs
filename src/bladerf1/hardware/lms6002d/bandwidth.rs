//! LMS6002D LPF bandwidth setting and calibration.
//!
//! The LMS6002D digital LPF controls the modem bandwidth. Available settings
//! range from 1.5 MHz to 28 MHz in discrete steps. Calibration data from the
//! device's flash determines the appropriate filter coefficients for each
//! bandwidth and frequency band.

use crate::bladerf1::hardware::lms6002d::Lms6002d;
use crate::range::{Range, RangeItem};
use crate::{Channel, khz, mhz};

/// Band split frequency in Hz: frequencies below use low band, at or above use high band.
pub const BLADERF1_BAND_HIGH: u32 = 1_500_000_000;

/// Supported LPF bandwidths in Hz.
pub const UINT_BANDWIDTHS: [u32; 16] = [
    mhz(28),
    mhz(20),
    mhz(14),
    mhz(12),
    mhz(10),
    khz(8_750),
    mhz(7),
    mhz(6),
    khz(5_500),
    mhz(5),
    khz(3_840),
    mhz(3),
    khz(2_750),
    khz(2_500),
    khz(1_750),
    khz(1_500),
];

/// LMS6002D LPF bandwidth setting.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum LmsBandwidth {
    /// 28 MHz bandwidth.
    Bw28mhz = 0,
    /// 20 MHz bandwidth.
    Bw20mhz,
    /// 14 MHz bandwidth.
    Bw14mhz,
    /// 12 MHz bandwidth.
    Bw12mhz,
    /// 10 MHz bandwidth.
    Bw10mhz,
    /// 8.75 MHz bandwidth.
    Bw8p75mhz,
    /// 7 MHz bandwidth.
    Bw7mhz,
    /// 6 MHz bandwidth.
    Bw6mhz,
    /// 5.5 MHz bandwidth.
    Bw5p5mhz,
    /// 5 MHz bandwidth.
    Bw5mhz,
    /// 3.84 MHz bandwidth.
    Bw3p84mhz,
    /// 3 MHz bandwidth.
    Bw3mhz,
    /// 2.75 MHz bandwidth.
    Bw2p75mhz,
    /// 2.5 MHz bandwidth.
    Bw2p5mhz,
    /// 1.75 MHz bandwidth.
    Bw1p75mhz,
    /// 1.5 MHz bandwidth.
    Bw1p5mhz,
}

impl LmsBandwidth {
    fn from_index(index: u8) -> Self {
        Self::ALL
            .get(index as usize)
            .copied()
            .unwrap_or(Self::Bw28mhz)
    }

    fn to_index(self) -> u8 {
        self as u8
    }

    const ALL: [Self; 16] = [
        Self::Bw28mhz,
        Self::Bw20mhz,
        Self::Bw14mhz,
        Self::Bw12mhz,
        Self::Bw10mhz,
        Self::Bw8p75mhz,
        Self::Bw7mhz,
        Self::Bw6mhz,
        Self::Bw5p5mhz,
        Self::Bw5mhz,
        Self::Bw3p84mhz,
        Self::Bw3mhz,
        Self::Bw2p75mhz,
        Self::Bw2p5mhz,
        Self::Bw1p75mhz,
        Self::Bw1p5mhz,
    ];
}

impl From<LmsBandwidth> for u32 {
    fn from(value: LmsBandwidth) -> Self {
        UINT_BANDWIDTHS[value.to_index() as usize]
    }
}

impl From<u32> for LmsBandwidth {
    fn from(value: u32) -> Self {
        let mut result = Self::Bw28mhz;
        for (idx, &bw) in UINT_BANDWIDTHS.iter().enumerate() {
            if value <= bw {
                result = Self::from_index(idx as u8);
            } else {
                break;
            }
        }
        result
    }
}

/// Returns the range of supported LPF bandwidths.
pub fn get_bandwidth_range() -> Range {
    Range::new(
        UINT_BANDWIDTHS
            .iter()
            .rev()
            .map(|&bw| RangeItem::Value(bw as f64))
            .collect(),
    )
}

impl<'a> Lms6002d<'a> {
    pub(crate) fn set_bandwidth(
        &mut self,
        channel: Channel,
        bw: LmsBandwidth,
    ) -> crate::Result<()> {
        let addr = if channel == Channel::Rx { 0x54 } else { 0x34 };
        let mut data = self.read(addr)?;
        data &= !0x3c;
        data |= bw.to_index() << 2;
        self.write(addr, data)
    }

    pub(crate) fn get_bandwidth(&mut self, channel: Channel) -> crate::Result<LmsBandwidth> {
        let addr = if channel == Channel::Rx { 0x54 } else { 0x34 };
        let mut data = self.read(addr)?;
        data >>= 2;
        data &= 0xf;
        Ok(LmsBandwidth::from_index(data))
    }
}
