#[cfg(feature = "bladerf1")]
pub mod bladerf1;
#[cfg(feature = "bladerf2")]
pub mod bladerf2;
pub mod channel;
pub mod error;
pub mod nios_client;
pub mod protocol;
pub mod range;
pub mod transport;
pub mod version;
pub use channel::Channel;
pub use error::{Error, Result};
pub use version::SemanticVersion;
macro_rules! khz {
    ($value:expr) => {
        ($value * 1000u32)
    };
}
pub(crate) use khz;
macro_rules! mhz {
    ($value:expr) => {
        ($value * 1000000u32)
    };
}
pub(crate) use mhz;
