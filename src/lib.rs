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
//! ## Usage overview
//!
//! After a BladeRF is connected via USB (High or SuperSpeed USB port required) and fully booted,
//! an instance to a BladeRF can be opened using [`BladeRf1::from_first`]. A handle to a specific BladeRF
//! can also be obtained by its [`BladeRf1::from_bus_addr`] or its [`BladeRf1::from_serial`] or
//! [`BladeRf1::from_fd`] on Android.
//!
//! After obtaining an instance of a [`BladeRf1`], you can set basic parameters like Gain, Frequency
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
//! - Logging with adjustable levels (e.g. with log crate)
//! - DC calibration table support
//! - Board Data structure to retain current configuration and state.
//! - Usage from both async and blocking contexts (currently sync only)
//! - Structured and consistent error messages (e.g. with thiserror crate)
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

pub mod board;
pub mod hardware;
pub mod nios;

pub use board::bladerf1::*;

pub use bladerf_globals::*;

// #[derive(thiserror::Error, Debug)]
// pub enum BladeRfError {
//     /// Device not found.
//     #[error("NotFound")]
//     NotFound,
//     #[error("Unexpected")]
//     Unexpected,
//     #[error("Unsupported")]
//     Unsupported,
//     #[error("Invalid")]
//     Invalid,
// }

#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// I/O error occurred.
    #[error("io")]
    Io(#[from] std::io::Error),
    #[error("nusb")]
    Nusb(#[from] nusb::Error),
    /// USB transfer error.
    #[error("transfer")]
    Transfer(#[from] nusb::transfer::TransferError),
    /// Transfer truncated.
    #[error("transfer truncated")]
    TransferTruncated {
        /// Actual amount of bytes transferred.
        actual: usize,
        /// Expected number of bytes transferred.
        expected: usize,
    },
    /// An API call is not supported by your hardware.
    ///
    /// Try updating the firmware on your device.
    #[error("no api")]
    NoApi {
        /// Current device version.
        device: String,
        /// Minimum version required.
        min: String,
    },
    /// Invalid argument provided.
    #[error("{0}")]
    Argument(&'static str),
    /// BladeRF is in an invalid mode.
    // #[error("BladeRF in invalid mode. Required: {required:?}, actual: {actual:?}")]
    // WrongMode {
    //     /// The mode required for this operation.
    //     required: Mode,
    //     /// The actual mode of the device which differs from `required`.
    //     actual: Mode,
    // },
    /// Invalid value provided
    #[error("invalid")]
    Invalid,
    /// Device not found
    #[error("not found")]
    NotFound,
}
/// Result type for operations that may return an `Error`.
pub type Result<T> = std::result::Result<T, Error>;
