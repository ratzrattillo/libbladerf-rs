//! Pure Rust driver for the Nuand BladeRF1 SDR.
//!
//! This crate provides a complete, native-Rust driver for the BladeRF1 (x40/x115)
//! software-defined radio platform, with no dependency on the C libbladeRF library.
//! USB transport is handled via [nusb].
//!
//! # Feature flags
//!
//! | Flag        | Default | Effect                                  |
//! |-------------|---------|-----------------------------------------|
//! | `bladerf1`  | yes*    | BladeRF1 support (x40/x115)            |
//! | `bladerf2`  | no      | BladeRF2 support — **stub only**       |
//! | `xb100`     | yes     | XB-100 expansion board support         |
//! | `xb200`     | yes     | XB-200 transverter board support       |
//! | `xb300`     | yes     | XB-300 amplifier board support         |
//!
//! \* Enabled implicitly by the `xb100`, `xb200`, or `xb300` features.
//!
//! # Session-based USB model
//!
//! The BladeRF1 FX3 firmware exposes three USB alternate settings, each providing
//! access to a different functional mode of the device:
//!
//! - **RfLink** — normal RF operation (streaming, tuning, gain, etc.)
//! - **SpiFlash** — SPI flash read/write/erase
//! - **Config** — FPGA loading and device configuration
//!
//! [`bladerf1::BladeRf1`] owns the device and grants access through session types that
//! borrow `&mut NiosCore`. The Rust borrow checker enforces that only one
//! session is active at a time, serializing all register I/O at compile time.
//!
//! # Entry point
//!
//! Open a device and obtain an [`bladerf1::RfLinkSession`] to begin RF operations:
//!
//! ```ignore
//! let mut dev = BladeRf1::from_first()?;
//! let mut sess = dev.rf_link_session()?;
//! sess.initialize(false)?;
//! ```
//!
//! [nusb]: https://github.com/kevinmehall/nusb

#[cfg(feature = "bladerf1")]
pub mod bladerf1;
#[cfg(feature = "bladerf2")]
pub mod bladerf2;
pub mod channel;
pub mod error;
pub mod flash;
pub mod nios_client;
pub mod protocol;
pub mod range;
pub mod usb;
pub mod version;
pub use channel::Channel;
pub use error::{Error, Result};
pub use nusb::transfer::Buffer;
pub use version::SemanticVersion;
pub(crate) const fn khz(value: u32) -> u32 {
    value * 1_000
}
pub(crate) const fn mhz(value: u32) -> u32 {
    value * 1_000_000
}
#[allow(dead_code)]
pub(crate) const fn ghz(value: u32) -> u32 {
    value * 1_000_000_000
}
