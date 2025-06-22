// #![allow(private_interfaces, dead_code)]
mod bandwidth;
mod basic;
mod frequency;
mod gain;
mod sample_rate;
mod stream;

use crate::hardware::dac161s055::DAC161S055;
use crate::hardware::lms6002d::LMS6002D;
use crate::hardware::si5338::SI5338;
use nusb::{Device, Interface};

#[derive(thiserror::Error, Debug)]
pub enum BladeRfError {
    /// Device not found.
    #[error("NotFound")]
    NotFound,
    #[error("Unexpected")]
    Unexpected,
}

/// Representation of a BladeRF1 device.
pub struct BladeRf1 {
    device: Device,
    interface: Interface,
    lms: LMS6002D,
    si5338: SI5338,
    dac: DAC161S055,
    // xb200: Option<XB200>,
}
