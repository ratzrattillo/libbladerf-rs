use crate::bladerf1::nios_client::NiosClient;
use crate::range::{Range, RangeItem};
use crate::{Channel, khz, mhz};

pub const BLADERF1_BAND_HIGH: u32 = 1_500_000_000;

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

#[derive(Clone, Copy, PartialEq, Debug)]
#[repr(u8)]
pub enum LmsBandwidth {
    Bw28mhz = 0,
    Bw20mhz,
    Bw14mhz,
    Bw12mhz,
    Bw10mhz,
    Bw8p75mhz,
    Bw7mhz,
    Bw6mhz,
    Bw5p5mhz,
    Bw5mhz,
    Bw3p84mhz,
    Bw3mhz,
    Bw2p75mhz,
    Bw2p5mhz,
    Bw1p75mhz,
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

pub fn get_bandwidth_range() -> Range {
    Range {
        items: UINT_BANDWIDTHS
            .iter()
            .rev()
            .map(|&bw| RangeItem::Value(bw as f64))
            .collect(),
    }
}

pub fn set_bandwidth(
    nios: &mut NiosClient,
    channel: Channel,
    bw: LmsBandwidth,
) -> crate::Result<()> {
    let addr = if channel == Channel::Rx { 0x54 } else { 0x34 };
    let mut data = super::read(nios, addr)?;
    data &= !0x3c;
    data |= bw.to_index() << 2;
    super::write(nios, addr, data)
}

pub fn get_bandwidth(nios: &mut NiosClient, channel: Channel) -> crate::Result<LmsBandwidth> {
    let addr = if channel == Channel::Rx { 0x54 } else { 0x34 };
    let mut data = super::read(nios, addr)?;
    data >>= 2;
    data &= 0xf;
    Ok(LmsBandwidth::from_index(data))
}
