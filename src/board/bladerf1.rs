// #![allow(private_interfaces, dead_code)]
mod bandwidth;
mod basic;
mod frequency;
mod gain;
mod loopback;
mod rx_mux;
mod sample_rate;
mod stream;
mod expansion_boards;
mod xb100;
mod xb200;
mod xb300;

use crate::hardware::dac161s055::DAC161S055;
use crate::hardware::lms6002d::LMS6002D;
use crate::hardware::si5338::SI5338;
use nusb::{Device, Interface, Speed};
use bladerf_globals::bladerf1::BladerfXb;

#[derive(thiserror::Error, Debug)]
pub enum BladeRfError {
    /// Device not found.
    #[error("NotFound")]
    NotFound,
    #[error("Unexpected")]
    Unexpected,
    #[error("Unsupported")]
    Unsupported,
}

struct BoardData {
    speed: Option<Speed>,
}

/// Representation of a BladeRF1 device.
pub struct BladeRf1 {
    device: Device,
    interface: Interface,
    board_data: BoardData,
    lms: LMS6002D,
    si5338: SI5338,
    dac: DAC161S055,
    xb: BladerfXb,
}
