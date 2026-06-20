//! Frequency tuning operations for BladeRF1.
//!
//! Supports both host-initiated tuning, where the LMS6002D is configured directly,
//! and FPGA-initiated tuning, where retune requests are queued through the NIOS
//! retune queue for precise, sample-aligned frequency changes during streaming.
//!
//! When the XB-200 expansion board is enabled, frequencies below the LMS6002D
//! minimum are translated via `1248 MHz - desired_freq` using the board's
//! upconverter path.

use crate::bladerf1::board::RfLinkSession;
#[cfg(feature = "xb200")]
use crate::bladerf1::board::xb::xb200::Xb200Path;
use crate::bladerf1::hardware::lms6002d;
use crate::bladerf1::hardware::lms6002d::dc_calibration::AgcDcCorrection;
use crate::bladerf1::hardware::lms6002d::frequency::LmsFreq;
/// Pre-computed LMS6002D tuning parameters for fast FPGA-initiated retune.
///
/// Use `get_quick_tune()` to query the current tuning state, then pass it
/// to `schedule_retune()` to skip the frequency-to-register conversion.
pub use crate::bladerf1::hardware::lms6002d::frequency::QuickTune;
use crate::bladerf1::protocol::RetuneTimestamp;
use crate::channel::Channel;
use crate::error::{Error, Result};
use crate::range::{Range, RangeItem};

/// Determines how frequency changes are applied to the LMS6002D.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TuningMode {
    /// Host-initiated tuning: the LMS6002D is reconfigured directly via SPI.
    /// Fast and synchronous, but may cause a brief RF disruption.
    Host,
    /// FPGA-initiated tuning: the retune request is enqueued to the NIOS
    /// retune queue, which applies the change at the specified timestamp.
    /// Provides sample-aligned, glitch-free frequency transitions during streaming.
    Fpga,
}

impl RfLinkSession<'_> {
    /// Sets the RF frequency for the given channel.
    ///
    /// With `TuningMode::Host`, the LMS6002D is tuned immediately via SPI,
    /// band selection is applied, and any DC calibration table entries for
    /// the frequency are loaded. With `TuningMode::Fpga`, a retune request
    /// is enqueued to the NIOS retune queue (DC calibration table is not
    /// applied in this path).
    ///
    /// When the XB-200 is enabled and the frequency is below the LMS6002D
    /// minimum, the signal is routed through the XB-200 upconverter path
    /// using `1248 MHz - desired_freq` translation.
    ///
    /// Returns `Error::NotInitialized` if the board has not been initialized.
    pub fn set_frequency(
        &mut self,
        channel: Channel,
        #[allow(unused_mut)] mut frequency: u64,
        mode: TuningMode,
    ) -> Result<()> {
        self.require_initialized()?;
        log::trace!("Setting Frequency on channel {channel:?} to {frequency}Hz");
        #[cfg(feature = "xb200")]
        if self.nios.xb200_is_enabled()? {
            let freq_min = lms6002d::frequency::get_frequency_min() as u64;
            if frequency < freq_min {
                log::debug!(
                    "Setting path to Mix (freq {} < min {})",
                    frequency,
                    freq_min
                );
                self.xb200_set_path(channel, Xb200Path::Mix)?;
                self.xb200_auto_filter_selection(channel, frequency)?;
                log::debug!(
                    "Converting frequency: 1248000000 - {} = {}",
                    frequency,
                    1_248_000_000 - frequency
                );
                frequency = 1_248_000_000 - frequency;
            } else {
                log::debug!(
                    "Setting path to Bypass (freq {} >= min {})",
                    frequency,
                    freq_min
                );
                self.xb200_set_path(channel, Xb200Path::Bypass)?;
            }
        }
        match mode {
            TuningMode::Host => {
                self.lms().set_frequency(channel, frequency)?;
                let band = lms6002d::Band::from(frequency);
                self.band_select(channel, band)?;
            }
            TuningMode::Fpga => {
                self.schedule_retune(channel, RetuneTimestamp::Now, frequency, None)?;
            }
        }
        let table = match channel {
            Channel::Rx => self.dc_rx_table,
            Channel::Tx => self.dc_tx_table,
        };
        if let Some(table) = table {
            let entry = table.lookup(frequency);
            self.lms().set_dc_offset_i(channel, entry.dc.i)?;
            self.lms().set_dc_offset_q(channel, entry.dc.q)?;
            if channel == Channel::Rx {
                self.nios
                    .nios_set_agc_dc_correction(&AgcDcCorrection::from(&entry))?;
            }
        }
        Ok(())
    }

    /// Returns the current RF frequency of the given channel in Hz.
    ///
    /// Reads the raw frequency from the LMS6002D and, when the XB-200 is
    /// enabled in Mix path, applies the inverse translation
    /// `1248 MHz - raw_freq` to report the actual user-facing frequency.
    ///
    /// Returns `Error::BoardState` if the LMS6002D register read yields
    /// an invalid value.
    pub fn get_frequency(&mut self, channel: Channel) -> Result<u64> {
        self.require_initialized()?;
        let f = self.lms().get_frequency(channel)?;
        if f.x == 0 {
            log::error!("LMSFreq.x was zero!");
            return Err(Error::BoardState("LMSFreq.x was zero"));
        }
        #[allow(unused_mut)]
        let mut frequency_hz: u64 = (&f).into();
        log::trace!("Frequency Hz: {frequency_hz}");
        #[cfg(feature = "xb200")]
        if self.nios.xb200_is_enabled()? {
            let path = self.xb200_get_path(channel)?;
            log::trace!("XB200 path detected: {:?}", path);
            if path == Xb200Path::Mix {
                log::debug!("Mix path - converting: 1248000000 - {}", frequency_hz);
                frequency_hz = 1_248_000_000 - frequency_hz;
            }
        }
        Ok(frequency_hz)
    }

    /// Returns the supported RF frequency range in Hz.
    ///
    /// When the XB-200 is enabled, the minimum is extended to 0 Hz since the
    /// upconverter path can reach below the LMS6002D's native minimum.
    ///
    /// Returns `Error::NotInitialized` if the board has not been initialized.
    pub fn get_frequency_range(&mut self) -> Result<Range> {
        self.require_initialized()?;
        #[cfg(feature = "xb200")]
        let freq_min = if self.nios.xb200_is_enabled()? {
            0.0
        } else {
            lms6002d::frequency::get_frequency_min() as f64
        };
        #[cfg(not(feature = "xb200"))]
        let freq_min = lms6002d::frequency::get_frequency_min() as f64;
        let freq_max = lms6002d::frequency::get_frequency_max() as f64;
        Ok(Range::new(vec![RangeItem::Step(
            freq_min, freq_max, 1f64, 1f64,
        )]))
    }

    /// Selects the LMS6002D band (low or high) for the given channel based on frequency.
    pub fn select_band(&mut self, channel: Channel, frequency: u32) -> Result<()> {
        let band = lms6002d::Band::from(frequency);
        self.band_select(channel, band)
    }

    /// Schedules an FPGA-initiated frequency retune via the NIOS retune queue.
    ///
    /// Retains only the `LmsFreq` from the full response; use
    /// `schedule_retune_with_duration` to also retrieve the retune duration.
    ///
    /// Returns `Error::NotInitialized` if the board has not been initialized.
    pub fn schedule_retune(
        &mut self,
        channel: Channel,
        timestamp: RetuneTimestamp,
        frequency: u64,
        quick_tune: Option<QuickTune>,
    ) -> Result<LmsFreq> {
        self.require_initialized()?;
        let (lms_freq, _) =
            self.schedule_retune_with_duration(channel, timestamp, frequency, quick_tune)?;
        Ok(lms_freq)
    }

    /// Schedules an FPGA-initiated frequency retune via the NIOS retune queue.
    ///
    /// Converts the frequency to LMS6002D register values and sends the retune
    /// command through the FPGA's NIOS retune interface. Returns the computed
    /// `LmsFreq` register values along with the retune duration in nanoseconds.
    ///
    /// If `quick_tune` is provided, it is converted directly to register values,
    /// bypassing the frequency-to-register conversion (useful for rapid hopping).
    ///
    /// Returns `Error::NotInitialized` if the board has not been initialized.
    pub fn schedule_retune_with_duration(
        &mut self,
        channel: Channel,
        timestamp: RetuneTimestamp,
        frequency: u64,
        quick_tune: Option<QuickTune>,
    ) -> Result<(LmsFreq, u64)> {
        self.require_initialized()?;
        let f: LmsFreq = if let Some(qt) = quick_tune {
            qt.into()
        } else {
            #[cfg(feature = "xb200")]
            if self.nios.xb200_is_enabled()? {
                log::info!(
                    "Consider supplying the quick_tune parameter to schedule_retune() when the XB-200 is enabled."
                );
            }
            frequency.try_into()?
        };
        log::trace!("{f:?}");
        let band = if (f.flags & lms6002d::LMS_FREQ_FLAGS_LOW_BAND) != 0 {
            lms6002d::Band::Low
        } else {
            lms6002d::Band::High
        };
        let tune = if (f.flags & lms6002d::LMS_FREQ_FLAGS_FORCE_VCOCAP) != 0 {
            lms6002d::Tune::Quick
        } else {
            lms6002d::Tune::Normal
        };
        let result = self.nios.nios_retune(
            channel, timestamp, f.nint, f.nfrac, f.freqsel, f.vcocap, band, tune, f.xb_gpio,
        )?;
        Ok((f, result.duration()))
    }

    /// Cancels all pending FPGA-initiated retune requests for a channel.
    ///
    /// Sends a `RetuneTimestamp::ClearQueue` command through the NIOS retune
    /// interface to flush the queue without applying any frequency change.
    ///
    /// Returns `Error::NotInitialized` if the board has not been initialized.
    pub fn cancel_scheduled_retunes(&mut self, channel: Channel) -> Result<()> {
        self.require_initialized()?;
        self.nios.nios_retune(
            channel,
            RetuneTimestamp::ClearQueue,
            0u16,
            0u32,
            0,
            0,
            lms6002d::Band::Low,
            lms6002d::Tune::Normal,
            0,
        )?;
        Ok(())
    }

    /// Returns the current LMS6002D tuning parameters as a `QuickTune`.
    ///
    /// The returned value can be passed to `schedule_retune()` to bypass
    /// the frequency-to-register conversion for fast retune operations.
    /// When the XB-200 is enabled, the parameters account for the
    /// upconverter path.
    ///
    /// Returns `Error::NotInitialized` if the board has not been initialized.
    pub fn get_quick_tune(&mut self, channel: Channel) -> Result<QuickTune> {
        self.require_initialized()?;
        #[cfg(feature = "xb200")]
        let xb200 = self.nios.xb200_is_enabled()?;
        #[cfg(not(feature = "xb200"))]
        let xb200 = false;
        self.lms().get_quick_tune(channel, xb200)
    }
}
