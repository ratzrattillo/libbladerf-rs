mod bandwidth;
mod basic;
mod corrections;
mod flash;
mod frequency;
mod gain;
mod loopback;
mod rx_mux;
mod sample_rate;
mod stream;
pub mod xb;

pub use crate::hardware::lms6002d::{GainDb, GainMode, Loopback};
pub use corrections::Correction;
pub use rx_mux::RxMux;
pub use stream::SampleFormat;

use crate::bladerf1::frequency::TuningMode;
use crate::hardware::dac161s055::DAC161S055;
use crate::hardware::lms6002d::LMS6002D;
use crate::hardware::si5338::SI5338;
use nusb::io::{EndpointRead, EndpointWrite};
use nusb::transfer::Bulk;
use nusb::{Device, Interface};
use std::sync::{Arc, Mutex};

/// BladeRF1 USB vendor ID.
pub const BLADERF1_USB_VID: u16 = 0x2CF0;
/// BladeRF1 USB product ID.
pub const BLADERF1_USB_PID: u16 = 0x5246;

// /// Enable LMS receive
// ///
// /// @note This bit is set/cleared by bladerf_enable_module()
// pub const BLADERF_GPIO_LMS_RX_ENABLE: u8 = 1 << 1;
//
// /// Enable LMS transmit
// ///
// /// @note This bit is set/cleared by bladerf_enable_module()
// pub const BLADERF_GPIO_LMS_TX_ENABLE: u8 = 1 << 2;
//
// /// Switch to use TX low band (300MHz - 1.5GHz)
// ///
// /// @note This is set using bladerf_set_frequency().
// pub const BLADERF_GPIO_TX_LB_ENABLE: u8 = 2 << 3;
//
// /// Switch to use TX high band (1.5GHz - 3.8GHz)
// ///
// /// @note This is set using bladerf_set_frequency().
// pub const BLADERF_GPIO_TX_HB_ENABLE: u8 = 1 << 3;
//
// /// Counter mode enable
// ///
// /// Setting this bit to 1 instructs the FPGA to replace the (I, Q) pair in sample
// /// data with an incrementing, little-endian, 32-bit counter value. A 0 in bit
// /// specifies that sample data should be sent (as normally done).
// ///
// /// This feature is useful when debugging issues involving dropped samples.
// pub const BLADERF_GPIO_COUNTER_ENABLE: u16 = 1 << 9;
//
// /// Switch to use RX low band (300M - 1.5GHz)
// ///
// /// @note This is set using bladerf_set_frequency().
// pub const BLADERF_GPIO_RX_LB_ENABLE: u16 = 2 << 5;
//
// /// Switch to use RX high band (1.5GHz - 3.8GHz)
// ///
// /// @note This is set using bladerf_set_frequency().
// pub const BLADERF_GPIO_RX_HB_ENABLE: u16 = 1 << 5;

/// This GPIO bit configures the FPGA to use smaller DMA transfers (256 cycles
/// instead of 512). This is required when the device is not connected at Super
/// Speed (i.e., when it is connected at High Speed).
///
/// However, the caller need not set this in bladerf_config_gpio_write() calls.
/// The library will set this as needed; callers generally do not need to be
/// concerned with setting/clearing this bit.
pub const BLADERF_GPIO_FEATURE_SMALL_DMA_XFER: u16 = 1 << 7;

// /// Enable 8bit sample mode
// pub const BLADERF_GPIO_8BIT_MODE: u32 = 1 << 20;
//
// /// Packet capable core present bit.
// ///
// /// @note This is a read-only bit. The FPGA sets its value, and uses it to inform
// ///  host that there is a core capable of using packets in the FPGA.
// pub const BLADERF_GPIO_PACKET_CORE_PRESENT: u32 = 1 << 28;
//
// pub const BLADERF_DIRECTION_MASK: u8 = 0x1;

// TODO: The tuning mode should be read from the board config
// In the packet captures, this is where the changes happen:
// -  Packet No. 317 in rx-BladeRFTest-unix-filtered.pcapng
// -  Packet No. 230 in rx-rusttool-filtered.pcapng
// This is maybe due to the tuning mode being FPGA and not Host
/// BoardData struct contains information on the current state and settings of the
/// BladeRF. Data should only be stored in this structure if it cannot be determined during runtime.
#[derive(Clone)]
struct BoardData {
    // speed: Speed,
    // TODO: Find out if we can determine Tuningmode from device to get rid of the board data...
    tuning_mode: TuningMode,
}

/// TX Streamer to receive samples with the BladeRF1
pub struct BladeRf1RxStreamer {
    dev: BladeRf1,
    reader: EndpointRead<Bulk>,
    buffer_size: usize,
}

/// TX Streamer to transmit samples with the BladeRF1
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
