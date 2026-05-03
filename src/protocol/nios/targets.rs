macro_rules! impl_from_for_u8 {
    ($t:ty) => {
        impl From<$t> for u8 {
            fn from(t: $t) -> Self {
                t as u8
            }
        }
    };
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NiosPkt8x8Target {
    Lms6 = 0x00,
    Si5_338 = 0x01,
    VctcxoTamer = 0x02,
    TxTriggerCtl = 0x03,
    RxTriggerCtl = 0x04,
}
impl_from_for_u8!(NiosPkt8x8Target);

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NiosPkt8x16Target {
    VctcxoDac = 0x00,
    IqCorr = 0x01,
    AgcCorr = 0x02,
    Ad56x1Dac = 0x03,
    Ina219 = 0x04,
}
impl_from_for_u8!(NiosPkt8x16Target);

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NiosPkt8x16AddrIqCorr {
    RxGain = 0x00,
    RxPhase = 0x01,
    TxGain = 0x02,
    TxPhase = 0x03,
}
impl_from_for_u8!(NiosPkt8x16AddrIqCorr);

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NiosPkt8x32Target {
    Version = 0x00,
    Control = 0x01,
    Adf4_351 = 0x02,
    RffeCsr = 0x03,
    Adf400x = 0x04,
    Fastlock = 0x05,
}
impl_from_for_u8!(NiosPkt8x32Target);

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NiosPkt32x32Target {
    Exp = 0x00,
    ExpDir = 0x01,
    AdiAxi = 0x02,
    WbMstr = 0x03,
}
impl_from_for_u8!(NiosPkt32x32Target);
