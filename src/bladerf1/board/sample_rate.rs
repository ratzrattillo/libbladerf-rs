//! Sample rate control for BladeRF1.
//!
//! Configures the Si5338 MultiSynth clocks to produce the desired sample rate.
//! The Si5338 supports arbitrary rational rates via `RationalRate`, allowing
//! precise frequency synthesis beyond integer sample rates.

use crate::bladerf1::board::RfLinkSession;
use crate::bladerf1::hardware::si5338;
use crate::channel::Channel;
use crate::error::Result;
use crate::range::{Range, RangeItem};
impl RfLinkSession<'_> {
    /// Sets the sample rate for the given channel in samples per second.
    ///
    /// Programs the Si5338 MultiSynth clock to the desired integer rate.
    /// Returns the actual rate applied, which may differ if rounding is needed.
    ///
    /// Returns `Error::NotInitialized` if the board has not been initialized.
    pub fn set_sample_rate(&mut self, channel: Channel, rate: u32) -> Result<u32> {
        self.require_initialized()?;
        self.si().set_sample_rate(channel, rate)
    }
    /// Returns the current sample rate for the given channel in samples per second.
    ///
    /// Returns `Error::NotInitialized` if the board has not been initialized.
    pub fn get_sample_rate(&mut self, channel: Channel) -> Result<u32> {
        self.require_initialized()?;
        self.si().get_sample_rate(channel)
    }
    /// Returns the supported sample rate range in samples per second.
    pub fn get_sample_rate_range() -> Range {
        Range::new(vec![RangeItem::Step(
            si5338::BLADERF_SAMPLERATE_MIN as f64,
            si5338::BLADERF_SAMPLERATE_REC_MAX as f64,
            1f64,
            1f64,
        )])
    }
    /// Sets the sample rate for the given channel using a rational number.
    ///
    /// The `RationalRate` provides exact clock configuration via numerator,
    /// denominator, and post-divider for the Si5338 MultiSynth, enabling
    /// precise non-integer sample rates. The input rate is normalized and
    /// updated with the actual applied values.
    ///
    /// Returns the actual `RationalRate` applied by the hardware.
    ///
    /// Returns `Error::NotInitialized` if the board has not been initialized.
    pub fn set_rational_sample_rate(
        &mut self,
        channel: Channel,
        rate: &mut si5338::RationalRate,
    ) -> Result<si5338::RationalRate> {
        self.require_initialized()?;
        self.si().set_rational_sample_rate(channel, rate)
    }
    /// Returns the current rational sample rate configuration for the given channel.
    ///
    /// Reads the Si5338 MultiSynth registers and returns the actual rate as
    /// a `RationalRate` (numerator, denominator, post-divider).
    ///
    /// Returns `Error::NotInitialized` if the board has not been initialized.
    pub fn get_rational_sample_rate(&mut self, channel: Channel) -> Result<si5338::RationalRate> {
        self.require_initialized()?;
        self.si().get_rational_sample_rate(channel)
    }
}
