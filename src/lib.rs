#![deny(unsafe_code)]

//! # BladeRf API
//!
//! This crate provides a Rust interface to the [BladeRf](https://greatscottgadgets.com/bladerf/one/),
//! a popular software-defined radio (SDR) peripheral. It allows for transmitting and receiving
//! radio signals using the BladeRf device in pure Rust.
//!
/*//! ## Example
//!
//! ```rust,no_run
//! use anyhow::Result;
//! use seify_bladerf::{Config, BladeRf};
//!
//! fn main() -> Result<()> {
//!     let radio = BladeRf::open_first()?;
//!
//!     radio.start_rx(&Config {
//!         vga_db: 0,
//!         txvga_db: 0,
//!         lna_db: 0,
//!         amp_enable: false,
//!         antenna_enable: false,
//!         frequency_hz: 915_000_000,
//!         sample_rate_hz: 2_000_000,
//!         sample_rate_div: 1,
//!     })?;
//!
//!     let mut buf = vec![0u8; 32 * 1024];
//!     loop {
//!         radio.read(&mut buf)?;
//!         // Process samples...
//!     }
//! }
//! ```
//!
//! ## License
//!
//! This crate is licensed under the MIT License.*/

#![cfg_attr(docsrs, feature(doc_cfg), feature(doc_auto_cfg))]
// TODO(tjn): re-enable
// #![warn(missing_docs)]

// pub mod bladerf1;
// pub mod lms;
//pub mod nios;
//mod nios_legacy;
//pub mod backend;
pub mod bladerf;
pub mod board;
pub mod hardware;
pub mod nios;
mod types;
mod usb;
