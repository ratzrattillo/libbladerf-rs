// TODO: find a place where to put generic checks for e.g. type sizes
// assert_eq!(size_of::<u64>(), 8);

#[macro_export]
macro_rules! bladerf_channel_rx {
    ($ch:expr) => {
        ((($ch) << 1) | 0x0) as u8
    };
}
#[macro_export]
macro_rules! bladerf_channel_tx {
    ($ch:expr) => {
        ((($ch) << 1) | 0x1) as u8
    };
}

#[repr(u8)]
pub enum BladerfDirection {
    RX = 0,
    TX = 1,
}
/**
 * Convenience macro: true if argument is a TX channel
 */
#[macro_export]
macro_rules! bladerf_channel_is_tx {
    ($ch:expr) => {
        (($ch) & BladerfDirection::TX as u8) as u8
    };
}

#[allow(dead_code)]
pub const BLADERF_MODULE_RX: u8 = bladerf_channel_rx!(0);
#[allow(dead_code)]
pub const BLADERF_MODULE_TX: u8 = bladerf_channel_tx!(0);

pub const ENDPOINT_OUT: u8 = 0x02;
pub const ENDPOINT_IN: u8 = 0x82;
