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
//! | `bladerf1`  | yes     | BladeRF1 support (x40/x115)            |
//! | `bladerf2`  | no      | BladeRF2 support — **early, device identification only** |
//! | `xb100`     | yes     | XB-100 expansion board (requires `bladerf1`) |
//! | `xb200`     | yes     | XB-200 transverter board (requires `bladerf1`) |
//! | `xb300`     | yes     | XB-300 amplifier board (requires `bladerf1`) |
//! | `smol`      | no      | nusb `smol` runtime integration         |
//! | `tokio`     | no      | nusb `tokio` runtime integration        |
//!
//! The expansion boards exist only for the BladeRF1: enabling `xb100`, `xb200`
//! or `xb300` without `bladerf1` is a compile error.
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
//! # Sync and async
//!
//! Every I/O method returns a [`MaybeFuture`]: call `.wait()` to block the
//! current thread (native targets only) or `.await` it from async code. The
//! crate mirrors nusb's semantics: on native targets `.wait()` needs no async
//! runtime, while `.await` requires one of the opt-in `smol` or `tokio`
//! features, selected through the features of the same name. Streaming
//! timeouts apply to the blocking path only; awaited stream futures consume
//! one USB completion per await and are cancel-safe.
//!
//! On `wasm32-unknown-unknown` (WebUSB) there is no `.wait()`; open the
//! device with [`bladerf1::BladeRf1::from_device`] and shut it down with
//! [`bladerf1::BladeRf1::close`].
//!
//! # Entry point
//!
//! Open a device and obtain an [`bladerf1::RfLinkSession`] to begin RF operations:
//!
//! ```ignore
//! use libbladerf_rs::MaybeFuture;
//!
//! let mut dev = BladeRf1::from_first().wait()?;
//! let mut sess = dev.rf_link_session().wait()?;
//! sess.initialize(false).wait()?;
//! ```
//!
//! [nusb]: https://github.com/kevinmehall/nusb

#![deny(missing_docs, missing_debug_implementations)]

#[cfg(all(
    any(feature = "xb100", feature = "xb200", feature = "xb300"),
    not(feature = "bladerf1")
))]
compile_error!(
    "xb100/xb200/xb300 are BladeRF1 expansion boards and require the `bladerf1` feature"
);
#[cfg(feature = "bladerf1")]
pub mod bladerf1;
#[cfg(feature = "bladerf2")]
pub mod bladerf2;
/// RX/TX channel selector.
pub mod channel;
pub mod error;
pub mod flash;
#[cfg(any(feature = "bladerf1", feature = "bladerf2"))]
#[cfg_attr(not(feature = "bladerf1"), allow(dead_code))]
pub(crate) mod maybe_future;
#[cfg(any(feature = "bladerf1", feature = "bladerf2"))]
#[cfg_attr(not(feature = "bladerf1"), allow(dead_code))]
pub(crate) mod nios_client;
pub mod protocol;
pub mod range;
#[cfg(any(feature = "bladerf1", feature = "bladerf2"))]
#[cfg_attr(not(feature = "bladerf1"), allow(dead_code))]
pub(crate) mod usb;
/// Semantic version triple reported by firmware and FPGA.
pub mod version;
pub use channel::Channel;
pub use error::{Error, ErrorKind, Result};
pub use nusb;
pub use nusb::MaybeFuture;
pub use nusb::transfer::Buffer;
pub use version::SemanticVersion;
#[allow(dead_code)]
pub(crate) const fn khz(value: u32) -> u32 {
    value * 1_000
}
#[allow(dead_code)]
pub(crate) const fn mhz(value: u32) -> u32 {
    value * 1_000_000
}
#[allow(dead_code)]
pub(crate) const fn ghz(value: u32) -> u32 {
    value * 1_000_000_000
}
