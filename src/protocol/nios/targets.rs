#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NiosPkt8x8Target {
    Lms6 = 0x00,
    Si5338 = 0x01,
    VctcxoTamer = 0x02,
    TxTriggerCtl = 0x03,
    RxTriggerCtl = 0x04,
}
impl From<NiosPkt8x8Target> for u8 {
    fn from(t: NiosPkt8x8Target) -> Self {
        t as u8
    }
}
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NiosPkt8x16Target {
    VctcxoDac = 0x00,
    IqCorr = 0x01,
    AgcCorr = 0x02,
    Ad56x1Dac = 0x03,
    Ina219 = 0x04,
}
impl From<NiosPkt8x16Target> for u8 {
    fn from(t: NiosPkt8x16Target) -> Self {
        t as u8
    }
}
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NiosPkt8x16AddrIqCorr {
    RxGain = 0x00,
    RxPhase = 0x01,
    TxGain = 0x02,
    TxPhase = 0x03,
}
impl From<NiosPkt8x16AddrIqCorr> for u8 {
    fn from(t: NiosPkt8x16AddrIqCorr) -> Self {
        t as u8
    }
}
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NiosPkt8x32Target {
    Version = 0x00,
    Control = 0x01,
    Adf4351 = 0x02,
    RffeCsr = 0x03,
    Adf400x = 0x04,
    Fastlock = 0x05,
}
impl From<NiosPkt8x32Target> for u8 {
    fn from(t: NiosPkt8x32Target) -> Self {
        t as u8
    }
}
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NiosPkt32x32Target {
    Exp = 0x00,
    ExpDir = 0x01,
    AdiAxi = 0x02,
    WbMstr = 0x03,
}
impl From<NiosPkt32x32Target> for u8 {
    fn from(t: NiosPkt32x32Target) -> Self {
        t as u8
    }
}
