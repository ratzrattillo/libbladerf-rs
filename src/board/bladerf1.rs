#![allow(dead_code)]

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
// use std::ops::{BitAnd, BitOr};
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
//
// /// @defgroup STREAMING_FORMAT_METADATA Metadata structure and flags
// /// Metadata status bits
// ///
// /// These are used in conjunction with the bladerf_metadata structure's `status`
// /// field.
// #[repr(u32)]
// #[derive(Default)]
// pub enum MetadataStatus {
//     /// For initialization purposes only.
//     #[default]
//     Invalid = 0,
//     /// A sample overrun has occurred.
//     ///
//     /// This indicates that either the host (more likely) or the FPGA is not keeping
//     /// up with the incoming samples.
//     Overrun = 1 << 0,
//
//     /// A sample underrun has occurred.
//     ///
//     /// This generally only occurs on the TX channel when the FPGA is starved of
//     /// samples.
//     ///
//     /// @note libbladeRF does not report this status. It is here for future use.
//     Underrun = 1 << 1,
// }
//
// /// Metadata flags
// ///
// /// These are used in conjunction with the bladerf_metadata structure's `flags`
// /// field.
// #[repr(u32)]
// #[derive(Default)]
// pub enum MetadataFlags {
//     /// For initialization purposes only.
//     #[default]
//     Invalid = 0,
//     /// Mark the associated buffer as the start of a burst transmission.
//     ///
//     /// @note This is only used for the bladerf_sync_tx() call.
//     ///
//     /// When using this flag, the bladerf_metadata::timestamp field should contain
//     /// the timestamp at which samples should be sent.
//     ///
//     /// Between specifying the BLADERF_META_FLAG_TX_BURST_START and
//     /// BLADERF_META_FLAG_TX_BURST_END flags, there is no need for the user to the
//     /// bladerf_metadata::timestamp field. The library will ensure the
//     /// correct value is used, based upon the timestamp initially provided and
//     /// the number of samples that have been sent.
//     TxBurstStart = 1 << 0,
//
//     /// Mark the associated buffer as the end of a burst transmission. This will
//     /// flush the remainder of the sync interface's current working buffer and
//     /// enqueue samples into the hardware's transmit FIFO.
//     ///
//     /// As of libbladeRF v1.3.0, it is no longer necessary for the API user to ensure
//     /// that the final 3 samples of a burst are \f$0 + 0 j\f$. libbladeRF now ensures
//     /// this hardware requirement is upheld.
//     ///
//     /// Specifying this flag and flushing the sync interface's working buffer implies
//     /// that the next timestamp that can be transmitted is the current timestamp plus
//     /// the duration of the burst that this flag is ending <b>and</b> the remaining
//     /// length of the remaining buffer that is flushed (The buffer size, in this
//     /// case, is the `buffer_size` value passed to the previous call of bladerf_sync_config()).
//     ///
//     /// Rather than attempting to keep track of the number of samples sent with
//     /// respect to buffer sizes, it is easiest to always assume 1 buffer's worth of
//     /// time is required between bursts. In this case "buffer" refers to the
//     /// `buffer_size` parameter provided to bladerf_sync_config().) If this is too
//     /// much time, consider using the BLADERF_META_FLAG_TX_UPDATE_TIMESTAMP
//     /// flag.
//     ///
//     /// @note This is only used for the bladerf_sync_tx() call. It is ignored by the
//     ///       bladerf_sync_rx() call.
//     TxBurstEnd = 1 << 1,
//
//     ///
//     /// Use this flag in conjunction with BLADERF_META_FLAG_TX_BURST_START to
//     /// indicate that the burst should be transmitted as soon as possible, as opposed
//     /// to waiting for a specific timestamp.
//     ///
//     /// When this flag is used, there is no need to set the
//     /// bladerf_metadata::timestamp field.
//     ///
//     TxNow = 1 << 2,
//
//     /// Use this flag within a burst (i.e., between the use of
//     /// BLADERF_META_FLAG_TX_BURST_START and BLADERF_META_FLAG_TX_BURST_END) to
//     /// specify that bladerf_sync_tx() should read the bladerf_metadata::timestamp
//     /// field and zero-pad samples up to the specified timestamp. The provided
//     /// samples will then be transmitted at that timestamp.
//     ///
//     /// Use this flag when potentially flushing an entire buffer via the
//     /// BLADERF_META_FLAG_TX_BURST_END would yield an unacceptably large gap in the
//     /// transmitted samples.
//     ///
//     /// In some applications where a transmitter is constantly transmitting with
//     /// tiny gaps (less than a buffer), users may end up using a single
//     /// BLADERF_META_FLAG_TX_BURST_START, and then many calls to
//     /// bladerf_sync_tx() with the BLADERF_META_FLAG_TX_UPDATE_TIMESTAMP flag set.
//     /// The BLADERF_META_FLAG_TX_BURST_END would only be used to end the stream
//     /// when shutting down.
//     ///
//     TxUpdateTimestamp = 1 << 3,
//
//     /// This flag indicates that calls to bladerf_sync_rx should return any available
//     /// samples, rather than wait until the timestamp indicated in the
//     /// bladerf_metadata timestamp field.
//     ///
//     RxNow = 1 << 31,
//     //
//     // ///
//     // /// This flag is asserted in bladerf_metadata.status by the hardware when an
//     // /// underflow is detected in the sample buffering system on the device.
//     // ///
//     // RxHwUnderflow = 1 << 0,
//     //
//     // ///
//     // /// This flag is asserted in bladerf_metadata.status by the hardware if mini
//     // /// expansion IO pin 1 is asserted.
//     // ///
//     // RxHwMiniexp1 = 1 << 16,
//     //
//     // ///
//     // /// This flag is asserted in bladerf_metadata.status by the hardware if mini
//     // /// expansion IO pin 2 is asserted.
//     // ///
//     // RxHwMiniexp2 = 1 << 17,
// }
//
// impl BitOr for MetadataFlags {
//     type Output = u32;
//
//     fn bitor(self, rhs: Self) -> Self::Output {
//         self as u32 | rhs as u32
//     }
// }
//
// impl BitOr<MetadataFlags> for u32 {
//     type Output = u32;
//
//     fn bitor(self, rhs: MetadataFlags) -> Self::Output {
//         self | rhs as u32
//     }
// }
//
// impl BitAnd for MetadataFlags {
//     type Output = u32;
//
//     fn bitand(self, rhs: Self) -> Self::Output {
//         self as u32 & rhs as u32
//     }
// }
//
// impl BitAnd<MetadataFlags> for u32 {
//     type Output = u32;
//
//     fn bitand(self, rhs: MetadataFlags) -> Self::Output {
//         self & rhs as u32
//     }
// }
//
// // impl Not for MetadataFlags {
// //     type Output = ();
// //
// //     fn not(self) -> Self::Output {
// //         todo!()
// //     }
// // }
//
// /// Sample metadata
// ///
// /// This structure is used in conjunction with the ::BLADERF_FORMAT_SC16_Q11_META
// /// format to TX scheduled bursts or retrieve timestamp information about
// /// received samples.
// #[derive(Default)]
// pub struct BladerfMetadata {
//     ///
//     /// Free-running FPGA counter that monotonically increases at the sample rate
//     /// of the associated channel.
//     ///
//     timestamp: u64,
//
//     /// Input bit field to control the behavior of the call that the metadata
//     /// structure is passed to. API calls read this field from the provided data
//     /// structure and do not modify it.
//     ///
//     /// Valid flags include
//     ///  BLADERF_META_FLAG_TX_BURST_START,
//     ///  BLADERF_META_FLAG_TX_BURST_END,
//     ///  BLADERF_META_FLAG_TX_NOW,
//     ///  BLADERF_META_FLAG_TX_UPDATE_TIMESTAMP, and
//     ///  BLADERF_META_FLAG_RX_NOW
//     ///
//     flags: u32,
//
//     /// Output bit field to denoting the status of transmissions/receptions. API
//     /// calls will write this field.
//     ///
//     /// Possible status flags include BLADERF_META_STATUS_OVERRUN and
//     /// BLADERF_META_STATUS_UNDERRUN.
//     ///
//     status: MetadataStatus,
//
//     /// This output parameter is updated to reflect the actual number of
//     /// contiguous samples that have been populated in an RX buffer during a
//     /// bladerf_sync_rx() call.
//     ///
//     /// This will not be equal to the requested count in the event of a
//     /// discontinuity (i.e., when the status field has the
//     /// BLADERF_META_STATUS_OVERRUN flag set). When an overrun occurs, it is
//     /// important not to read past the number of samples specified by this value,
//     /// as the remaining contents of the buffer are undefined.
//     ///
//     /// @note This parameter is not currently used by bladerf_sync_tx().
//     ///
//     actual_count: u32,
//
//     ///
//     /// Reserved for future use. This is not used by any functions. It is
//     /// recommended that users zero out this field.
//     ///
//     reserved: [u8; 32],
// }
