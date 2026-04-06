use crate::bladerf1::hardware::lms6002d::LMS6002D;
use crate::range::{Range, RangeItem};
use crate::{Channel, khz, mhz};
pub const BLADERF1_BAND_HIGH: u32 = 1500000000;
pub const UINT_BANDWIDTHS: [u32; 16] = [
    mhz!(28),
    mhz!(20),
    mhz!(14),
    mhz!(12),
    mhz!(10),
    khz!(8750),
    mhz!(7),
    mhz!(6),
    khz!(5500),
    mhz!(5),
    khz!(3840),
    mhz!(3),
    khz!(2750),
    khz!(2500),
    khz!(1750),
    khz!(1500),
];
pub enum LmsBandwidth {
    Bw28mhz,
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
        match index {
            1 => LmsBandwidth::Bw20mhz,
            2 => LmsBandwidth::Bw14mhz,
            3 => LmsBandwidth::Bw12mhz,
            4 => LmsBandwidth::Bw10mhz,
            5 => LmsBandwidth::Bw8p75mhz,
            6 => LmsBandwidth::Bw7mhz,
            7 => LmsBandwidth::Bw6mhz,
            8 => LmsBandwidth::Bw5p5mhz,
            9 => LmsBandwidth::Bw5mhz,
            10 => LmsBandwidth::Bw3p84mhz,
            11 => LmsBandwidth::Bw3mhz,
            12 => LmsBandwidth::Bw2p75mhz,
            13 => LmsBandwidth::Bw2p5mhz,
            14 => LmsBandwidth::Bw1p75mhz,
            15 => LmsBandwidth::Bw1p5mhz,
            _ => LmsBandwidth::Bw28mhz,
        }
    }
    fn to_index(&self) -> u8 {
        match self {
            LmsBandwidth::Bw28mhz => 0,
            LmsBandwidth::Bw20mhz => 1,
            LmsBandwidth::Bw14mhz => 2,
            LmsBandwidth::Bw12mhz => 3,
            LmsBandwidth::Bw10mhz => 4,
            LmsBandwidth::Bw8p75mhz => 5,
            LmsBandwidth::Bw7mhz => 6,
            LmsBandwidth::Bw6mhz => 7,
            LmsBandwidth::Bw5p5mhz => 8,
            LmsBandwidth::Bw5mhz => 9,
            LmsBandwidth::Bw3p84mhz => 10,
            LmsBandwidth::Bw3mhz => 11,
            LmsBandwidth::Bw2p75mhz => 12,
            LmsBandwidth::Bw2p5mhz => 13,
            LmsBandwidth::Bw1p75mhz => 14,
            LmsBandwidth::Bw1p5mhz => 15,
        }
    }
}
impl From<LmsBandwidth> for u32 {
    fn from(value: LmsBandwidth) -> Self {
        match value {
            LmsBandwidth::Bw28mhz => mhz!(28),
            LmsBandwidth::Bw20mhz => mhz!(20),
            LmsBandwidth::Bw14mhz => mhz!(14),
            LmsBandwidth::Bw12mhz => mhz!(12),
            LmsBandwidth::Bw10mhz => mhz!(10),
            LmsBandwidth::Bw8p75mhz => khz!(8750),
            LmsBandwidth::Bw7mhz => mhz!(7),
            LmsBandwidth::Bw6mhz => mhz!(6),
            LmsBandwidth::Bw5p5mhz => khz!(5500),
            LmsBandwidth::Bw5mhz => mhz!(5),
            LmsBandwidth::Bw3p84mhz => khz!(3840),
            LmsBandwidth::Bw3mhz => mhz!(3),
            LmsBandwidth::Bw2p75mhz => khz!(2750),
            LmsBandwidth::Bw2p5mhz => khz!(2500),
            LmsBandwidth::Bw1p75mhz => khz!(1750),
            LmsBandwidth::Bw1p5mhz => khz!(1500),
        }
    }
}
impl From<u32> for LmsBandwidth {
    fn from(value: u32) -> Self {
        if value <= khz!(1500) {
            LmsBandwidth::Bw1p5mhz
        } else if value <= khz!(1750) {
            LmsBandwidth::Bw1p75mhz
        } else if value <= khz!(2500) {
            LmsBandwidth::Bw2p5mhz
        } else if value <= khz!(2750) {
            LmsBandwidth::Bw2p75mhz
        } else if value <= mhz!(3) {
            LmsBandwidth::Bw3mhz
        } else if value <= khz!(3840) {
            LmsBandwidth::Bw3p84mhz
        } else if value <= mhz!(5) {
            LmsBandwidth::Bw5mhz
        } else if value <= khz!(5500) {
            LmsBandwidth::Bw5p5mhz
        } else if value <= mhz!(6) {
            LmsBandwidth::Bw6mhz
        } else if value <= mhz!(7) {
            LmsBandwidth::Bw7mhz
        } else if value <= khz!(8750) {
            LmsBandwidth::Bw8p75mhz
        } else if value <= mhz!(10) {
            LmsBandwidth::Bw10mhz
        } else if value <= mhz!(12) {
            LmsBandwidth::Bw12mhz
        } else if value <= mhz!(14) {
            LmsBandwidth::Bw14mhz
        } else if value <= mhz!(20) {
            LmsBandwidth::Bw20mhz
        } else {
            LmsBandwidth::Bw28mhz
        }
    }
}
impl LMS6002D {
    pub fn get_bandwidth_range() -> Range {
        Range {
            items: vec![
                RangeItem::Value(khz!(1500) as f64),
                RangeItem::Value(khz!(1750) as f64),
                RangeItem::Value(khz!(2500) as f64),
                RangeItem::Value(khz!(2750) as f64),
                RangeItem::Value(mhz!(3) as f64),
                RangeItem::Value(khz!(3840) as f64),
                RangeItem::Value(mhz!(5) as f64),
                RangeItem::Value(khz!(5500) as f64),
                RangeItem::Value(mhz!(6) as f64),
                RangeItem::Value(mhz!(7) as f64),
                RangeItem::Value(khz!(8750) as f64),
                RangeItem::Value(mhz!(10) as f64),
                RangeItem::Value(mhz!(12) as f64),
                RangeItem::Value(mhz!(14) as f64),
                RangeItem::Value(mhz!(20) as f64),
                RangeItem::Value(mhz!(28) as f64),
            ],
        }
    }
    pub fn set_bandwidth(&self, channel: Channel, bw: LmsBandwidth) -> crate::Result<()> {
        let addr = if channel == Channel::Rx { 0x54 } else { 0x34 };
        let mut data = self.read(addr)?;
        data &= !0x3c;
        data |= bw.to_index() << 2;
        self.write(addr, data)
    }
    pub fn get_bandwidth(&self, channel: Channel) -> crate::Result<LmsBandwidth> {
        let addr = if channel == Channel::Rx { 0x54 } else { 0x34 };
        let mut data = self.read(addr)?;
        data >>= 2;
        data &= 0xf;
        Ok(LmsBandwidth::from_index(data))
    }
}
