//! A reimplementation of basic libbladeRF functions in Rust, based on [nusb].
//!
//! [nusb]: https://github.com/kevinmehall/nusb
//!
//! Use [libbladerf-rs] to control your bladeRF from your Rust application.
//! [libbladerf-rs] shall currently not be considered as a replacement for the official [libbladeRF]
//! due to several features not being available.
//!
//! [libbladeRF]: https://github.com/Nuand/bladeRF
//! [libbladerf-rs]: https://github.com/ratzrattillo/libbladerf-rs
//!
//!
//! ## Usage overview
//!
//! After a BladeRF is connected via USB (High or SuperSpeed USB port required) and fully booted,
//! an instance to a BladeRF can be opened using [`bladerf1::BladeRf1::from_first`]. A handle to a specific BladeRF
//! can also be obtained by its [`bladerf1::BladeRf1::from_bus_addr`] or its [`bladerf1::BladeRf1::from_serial`] or
//! [`bladerf1::BladeRf1::from_fd`] on Android.
//!
//! After obtaining an instance of a [`bladerf1::BladeRf1`], you can set basic parameters like Gain, Frequency
//! and Sample Rate or Bandwidth.
//!
//! ## Examples
//! An example exists to demonstrate the current functionality of [libbladerf-rs]:
//! ```bash
//! cargo run --package info
//! ```
//!
//! ## Limitations
//!
//! [libbladerf-rs] currently only supports the BladeRF1. Support for BladeRF2 is currently not
//! possible, as I am not in the possession of named SDR.
//!
//! ### Implemented Features
//! - Getting/Setting gain levels of individual stages like rxvga1, rxvga2, lna, txvga1 and txvga2.
//! - Getting/Setting RX/TX frequency
//! - Getting/Setting Bandwidth
//! - Getting/Setting Sample Rate
//! - Support for BladeRF1 Expansion boards (XB100, XB200, XB300)
//! - Interface for sending and receiving I/Q samples
//!
//! ### Missing Features
//! - Support for BladeRF2
//! - Support for Firmware and FPGA flashing/validation
//! - Support for different I/Q sample formats and timestamps
//! - DC calibration table support
//! - Usage from both async and blocking contexts (currently sync only)
//! - Extensive documentation
//! - AGC enablement
//!
//! ## Developers
//! Contributions of any kind are welcome!
//!
//! If possible, method names should adhere to the documented methods in [libbladeRF-doc]
//!
//! [libbladeRF-doc]: https://www.nuand.com/libbladeRF-doc/v2.5.0/modules.html
//! [Wireshark]: https://www.wireshark.org/download.html
//!
//! For debugging purposes, it is useful to compare the communication between the SDR and
//! the original [libbladeRF] with the communication of [libbladerf-rs].
//! Hand tooling for this purpose is [Wireshark]. Allow wireshark to monitor USB traffic:
//!
//! ```bash
//! sudo usermod -a -G wireshark <your_user>
//! sudo modprobe usbmon
//! sudo setfacl -m u:<your_user>:r /dev/usbmon*
//! ```
//!
//! Filter out unwanted traffic by using a Wireshark filter like e.g.
//!
//! ```wireshark
//! usb.bus_id == 1 and usb.device_address == 2
//! ```
//!
//! Datasheets for the BladeRF1 hardware are available at the following resources:
//! ### SI5338
//! [SI5338 Datasheet](https://www.skyworksinc.com/-/media/Skyworks/SL/documents/public/data-sheets/Si5338.pdf)
//!
//! [SI5338 Reference Manual](https://www.skyworksinc.com/-/media/Skyworks/SL/documents/public/reference-manuals/Si5338-RM.pdf)
//!
//! ### LMS6002D
//! [LMS6002D Datasheet](https://cdn.sanity.io/files/yv2p7ubm/production/47449c61cd388c058561bfd3121b8a10b3d2c987.pdf)
//!
//! [LMS6002D Programming and Calibration Guide](https://cdn.sanity.io/files/yv2p7ubm/production/d20182c51057add570a74bd51d9c1336e814ea90.pdf)
//!
//! ### DAC161S055
//! [DAC Datasheet](https://www.ti.com/lit/ds/symlink/dac161s055.pdf?ts=1739140548819&ref_url=https%253A%252F%252Fwww.ti.com%252Fproduct%252Fde-de%252FDAC161S055)

mod bladerf;
mod board;
pub mod hardware;
pub mod nios_client;
pub mod protocol;
pub mod range;
pub mod transport;

// Re-export nios2 types for backward compatibility
pub mod nios2 {
    //! NIOS II communication types.
    //!
    //! This module re-exports types from `nios2_client` and `protocol::nios`.

    pub use crate::nios_client::{Nios, NiosClient, NiosInterface};
    pub use crate::protocol::nios::packet_generic::{NiosNum, NiosPacket};
    pub use crate::protocol::nios::{
        NiosPkt, NiosPkt8x8Target, NiosPkt8x16AddrIqCorr, NiosPkt8x16Target, NiosPkt8x32Target,
        NiosPkt32x32Target, NiosPktFlags, NiosPktRetuneRequest, NiosPktRetuneResponse,
        NiosPktStatus, NiosProtocol,
    };
}

pub use bladerf::{Channel, Direction};
pub use board::bladerf1;
pub use hardware::lms6002d::{Band, Tune};

use std::fmt::{Display, Formatter};

/// Version structure for FPGA, firmware, libbladeRF, and associated utilities
#[derive(Debug)]
pub struct SemanticVersion {
    /// Major version
    pub major: u16,
    /// Minor version
    pub minor: u16,
    /// Patch version
    pub patch: u16,
}

impl Display for SemanticVersion {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{}.{}.{}", self.major, self.minor, self.patch))
    }
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("I/O error")]
    Io(#[from] std::io::Error),
    #[error("USB error")]
    Nusb(#[from] nusb::Error),
    #[error("USB transfer error")]
    Transfer(#[from] nusb::transfer::TransferError),
    #[error("{0}")]
    Argument(&'static str),
    #[error("invalid state or value")]
    Invalid,
    #[error("device not found")]
    NotFound,
    #[error("operation timed out")]
    Timeout,
    #[error("NIOS packet error: {0}")]
    NiosPacket(#[from] NiosPacketError),
    #[error("endpoint busy")]
    EndpointBusy,
    #[error("FPGA tuning failed")]
    TuningFailed,
    #[error("FPGA retune queue is full")]
    RetuneQueueFull,
    #[error("NIOS write failed")]
    NiosWriteFailed,
}

#[derive(thiserror::Error, Debug)]
pub enum NiosPacketError {
    #[error("nfrac value {0} exceeds maximum 0x7FFFFF")]
    NfracOverflow(u32),
    #[error("freqsel value {0} exceeds maximum {1}")]
    FreqselOverflow(u8, u8),
    #[error("vcocap value {0} exceeds maximum {1}")]
    VcocapOverflow(u8, u8),
    #[error("invalid packet size: expected 16 bytes, got {0}")]
    InvalidSize(usize),
}
/// Result type for operations that may return an `Error`.
pub type Result<T> = std::result::Result<T, Error>;
