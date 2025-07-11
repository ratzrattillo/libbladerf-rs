// #![allow(private_interfaces, dead_code)]
mod bandwidth;
mod basic;
pub(crate) mod expansion_boards;
mod frequency;
mod gain;
mod loopback;
mod rx_mux;
mod sample_rate;
mod stream;
mod xb100;
pub mod xb200;
mod xb300;

use crate::board::bladerf1::xb100::Xb100;
use crate::board::bladerf1::xb300::Xb300;
use crate::hardware::dac161s055::DAC161S055;
use crate::hardware::lms6002d::LMS6002D;
use crate::hardware::si5338::SI5338;
use crate::xb200::Xb200;
use bladerf_globals::TuningMode;
use nusb::{Device, Interface, Speed};

#[derive(thiserror::Error, Debug)]
pub enum BladeRfError {
    /// Device not found.
    #[error("NotFound")]
    NotFound,
    #[error("Unexpected")]
    Unexpected,
    #[error("Unsupported")]
    Unsupported,
    #[error("Invalid")]
    Invalid,
}

// TODO: The tuning mode should be read from the board config
// In the packet captures, this is where the changes happen:
// -  Packet No. 317 in rx-BladeRFTest-unix-filtered.pcapng
// -  Packet No. 230 in rx-rusttool-filtered.pcapng
// This is maybe due to the tuning mode being FPGA and not Host
struct BoardData {
    speed: Speed,
    tuning_mode: TuningMode,
}

/// Representation of a BladeRF1 device.
pub struct BladeRf1 {
    device: Device,
    interface: Interface,
    board_data: BoardData,
    lms: LMS6002D,
    si5338: SI5338,
    dac: DAC161S055,
    xb100: Option<Xb100>,
    xb200: Option<Xb200>,
    xb300: Option<Xb300>,
}
