//! NIOS packet target and address constants.
//!
//! Defines the enumerated target identifiers and address offsets
//! for each address/data width family (8x8, 8x16, 8x32, 32x32, 8x64).
//! Each type implements `From<T>` for `u8` to allow seamless use
//! in packet encoding functions.

macro_rules! impl_from_for_u8 {
    ($t:ty) => {
        impl From<$t> for u8 {
            fn from(t: $t) -> Self {
                t as u8
            }
        }
    };
}

/// Target for 8-bit address / 8-bit data NIOS packets.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NiosPkt8x8Target {
    /// LMS6002D RF transceiver SPI bridge.
    Lms6 = 0x00,
    /// Si5338 clock generator I2C bridge.
    Si5338 = 0x01,
    /// VCTCXO trim DAC (DAC161S055).
    VctcxoTamer = 0x02,
    /// TX trigger control.
    TxTriggerCtl = 0x03,
    /// RX trigger control.
    RxTriggerCtl = 0x04,
}
impl_from_for_u8!(NiosPkt8x8Target);

/// Target for 8-bit address / 16-bit data NIOS packets.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NiosPkt8x16Target {
    /// VCTCXO DAC (DAC161S055) bridge.
    VctcxoDac = 0x00,
    /// IQ correction coefficients.
    IqCorr = 0x01,
    /// AGC DC correction coefficients.
    AgcCorr = 0x02,
    /// AD56x1 DAC bridge.
    Ad56x1Dac = 0x03,
    /// INA219 power monitor bridge.
    Ina219 = 0x04,
}
impl_from_for_u8!(NiosPkt8x16Target);

/// Address offsets within the `IqCorr` target (8x16 packet).
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NiosPkt8x16AddrIqCorr {
    /// RX gain correction coefficient.
    RxGain = 0x00,
    /// RX phase correction coefficient.
    RxPhase = 0x01,
    /// TX gain correction coefficient.
    TxGain = 0x02,
    /// TX phase correction coefficient.
    TxPhase = 0x03,
}
impl_from_for_u8!(NiosPkt8x16AddrIqCorr);

/// Address offsets within the `AgcCorr` target (8x16 packet).
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NiosPkt8x16AddrAgcCorr {
    /// DC correction Q value at maximum gain setting.
    DcQMax = 0x00,
    /// DC correction I value at maximum gain setting.
    DcIMax = 0x01,
    /// DC correction Q value at mid gain setting.
    DcQMid = 0x02,
    /// DC correction I value at mid gain setting.
    DcIMid = 0x03,
    /// DC correction Q value at minimum gain setting.
    DcQMin = 0x04,
    /// DC correction I value at minimum gain setting.
    DcIMin = 0x05,
}
impl_from_for_u8!(NiosPkt8x16AddrAgcCorr);

/// Target for 8-bit address / 32-bit data NIOS packets.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NiosPkt8x32Target {
    /// FPGA version register.
    Version = 0x00,
    /// Config GPIO / control register.
    Control = 0x01,
    /// ADF4351 synthesizer (XB-200).
    Adf4_351 = 0x02,
    /// RFFE CSR (radio frontend control).
    RffeCsr = 0x03,
    /// ADF400x synthesizer.
    Adf400x = 0x04,
    /// Fast-lock control.
    Fastlock = 0x05,
}
impl_from_for_u8!(NiosPkt8x32Target);

/// Target for 32-bit address / 32-bit data NIOS packets.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NiosPkt32x32Target {
    /// Expansion GPIO data register.
    Exp = 0x00,
    /// Expansion GPIO direction register.
    ExpDir = 0x01,
    /// ADI AXI bridge.
    AdiAxi = 0x02,
    /// Wishbone master port.
    WbMstr = 0x03,
}
impl_from_for_u8!(NiosPkt32x32Target);

/// Target for 8-bit address / 64-bit data NIOS packets.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NiosPkt8x64Target {
    /// Hardware timestamp counter.
    Timestamp = 0x00,
}
impl_from_for_u8!(NiosPkt8x64Target);

/// Address offsets within the `Timestamp` target (8x64 packet).
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NiosPkt8x64TimestampAddr {
    /// RX channel timestamp.
    Rx = 0x00,
    /// TX channel timestamp.
    Tx = 0x01,
}
impl_from_for_u8!(NiosPkt8x64TimestampAddr);
