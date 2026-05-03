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
pub mod transport;
pub mod version;
pub use channel::Channel;
pub use error::{Error, Result};
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
