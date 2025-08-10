// #![allow(private_interfaces, dead_code)]
mod bandwidth;
mod basic;
mod corrections;
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

use crate::hardware::dac161s055::DAC161S055;
use crate::hardware::lms6002d::LMS6002D;
use crate::hardware::si5338::SI5338;
use bladerf_globals::TuningMode;
use nusb::io::{EndpointRead, EndpointWrite};
use nusb::transfer::Bulk;
use nusb::{Device, Interface};
use std::sync::{Arc, Mutex};

// TODO: The tuning mode should be read from the board config
// In the packet captures, this is where the changes happen:
// -  Packet No. 317 in rx-BladeRFTest-unix-filtered.pcapng
// -  Packet No. 230 in rx-rusttool-filtered.pcapng
// This is maybe due to the tuning mode being FPGA and not Host
#[derive(Clone)]
struct BoardData {
    // speed: Speed,
    // TODO: Find out if we can determine Tuningmode from device to get rid of the board data...
    tuning_mode: TuningMode,
}

pub struct BladeRf1RxStreamer {
    dev: BladeRf1,
    reader: EndpointRead<Bulk>,
    buffer_size: usize,
}

pub struct BladeRf1TxStreamer {
    dev: BladeRf1,
    writer: EndpointWrite<Bulk>,
    buffer_size: usize,
}

/// Representation of a BladeRF1 device.
#[derive(Clone)]
pub struct BladeRf1 {
    device: Device,
    pub interface: Arc<Mutex<Interface>>,
    board_data: BoardData,
    lms: LMS6002D,
    si5338: SI5338,
    dac: DAC161S055,
}
