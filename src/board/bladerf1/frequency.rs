use crate::bladerf::Channel;
use crate::bladerf1::BladeRf1;
use crate::board::bladerf1::xb::xb200::Xb200Path;
use crate::hardware::lms6002d::{
    BLADERF_FREQUENCY_MAX, BLADERF_FREQUENCY_MIN, BLADERF1_BAND_HIGH, Band,
    LMS_FREQ_FLAGS_FORCE_VCOCAP, LMS_FREQ_FLAGS_LOW_BAND, LmsFreq, Tune,
};
use crate::nios::Nios;
use crate::nios::packet_retune::NiosPktRetuneRequest;
use crate::range::{Range, RangeItem};
use crate::{Error, Result};

/// BladeRF allows for two tuning modes in which commands to tune to a specific frequency are sent to the BladeRF:
/// - Host: Commands sent via USB.
/// - FPGA: Commands sent from the FPGA.
///
/// FPGA Tuning might allow for more accurate tuning at specific timestamps (No USB delays).
#[derive(Clone)]
pub enum TuningMode {
    #[allow(dead_code)]
    Host,
    Fpga,
}

impl BladeRf1 {
    /// Set the freqeuncy on a specific channel. The frequency must lie in the supported
    /// frequency range of the BladeRF1. If an XB200 Expansion Board is attached, the lower
    /// frequencies provided by that board are automatically supported.
    /// Both Host- and FPGA-TuningModes are supported depending on the config option set in the
    /// BladeRFs BoardData.
    pub fn set_frequency(&self, channel: Channel, mut frequency: u64) -> Result<()> {
        // let dc_cal = if channel == bladerf_channel_rx!(0) { cal_dc.rx } else { cal.dc_tx };

        log::trace!("Setting Frequency on channel {channel:?} to {frequency}Hz");

        if BladeRf1::xb200_is_enabled(&self.interface)? {
            if frequency < BLADERF_FREQUENCY_MIN as u64 {
                log::debug!("Setting path to Mix");
                self.xb200_set_path(channel, Xb200Path::Mix)?;

                self.xb200_auto_filter_selection(channel, frequency)?;

                frequency = 1248000000 - frequency;
            } else {
                log::debug!("Setting path to Bypass");
                self.xb200_set_path(channel, Xb200Path::Bypass)?;
            }
        }

        // For tuning HOST Tuning Mode:
        match &self.board_data.tuning_mode {
            TuningMode::Host => {
                self.lms.set_frequency(channel, frequency)?;
                let band = if frequency < BLADERF1_BAND_HIGH as u64 {
                    Band::Low
                } else {
                    Band::High
                };
                self.band_select(channel, band)?;
            }
            TuningMode::Fpga => {
                self.schedule_retune(channel, NiosPktRetuneRequest::RETUNE_NOW, frequency, None)?;
            }
        }

        Ok(())
    }

    /// Get the frequency that the BladeRF1 is tuned to on a specific channel.
    pub fn get_frequency(&self, channel: Channel) -> Result<u64> {
        let f = self.lms.get_frequency(channel)?;
        if f.x == 0 {
            // If we see this, it's most often an indication that communication
            // with the LMS6002D is not occurring correctly
            log::error!("LMSFreq.x was zero!");
            return Err(Error::Invalid);
        }
        // let mut frequency_hz = LMS6002D::frequency_to_hz(&f);
        let mut frequency_hz: u64 = (&f).into();
        log::trace!("Frequency Hz: {frequency_hz}");

        if BladeRf1::xb200_is_enabled(&self.interface)? {
            let path = self.xb200_get_path(channel)?;

            if path == Xb200Path::Mix {
                log::debug!("Bypass Frequency Hz: 1248000000 - {frequency_hz}");
                frequency_hz = 1248000000 - frequency_hz;
            }
        }

        Ok(frequency_hz)
    }

    /// Get the supported frequency range of the BladeRFF1
    /// A wider frequency range is returned if the XB200 is being attached and enabled.
    pub fn get_frequency_range(&self) -> Result<Range> {
        if BladeRf1::xb200_is_enabled(&self.interface)? {
            Ok(Range {
                items: vec![RangeItem::Step(
                    0.0,
                    BLADERF_FREQUENCY_MAX as f64,
                    1f64,
                    1f64,
                )],
            })
        } else {
            Ok(Range {
                items: vec![RangeItem::Step(
                    BLADERF_FREQUENCY_MIN as f64,
                    BLADERF_FREQUENCY_MAX as f64,
                    1f64,
                    1f64,
                )],
            })
        }
    }

    // TODO: Does this method have to be exposed externally?
    /// Select the High Band for Frequencies above 1.5GHz, otehrwise Low Band
    pub fn select_band(&self, channel: Channel, frequency: u32) -> Result<()> {
        // CHECK_BOARD_STATE(STATE_INITIALIZED);

        let band = if frequency < BLADERF1_BAND_HIGH {
            Band::Low
        } else {
            Band::High
        };

        self.band_select(channel, band)
    }

    // TODO: Better Express semantics by using "Either" for frequency and quick_tune.
    /// Schedule a retune at a specific point in time. the retune operation is handled by the FPGA
    /// Allows to provide a quick_tune parameter with precalculated tuning parameters for increased speed.
    pub fn schedule_retune(
        &self,
        channel: Channel,
        timestamp: u64,
        frequency: u64,
        quick_tune: Option<LmsFreq>,
    ) -> Result<LmsFreq> {
        let f = if let Some(qt) = quick_tune {
            qt
        } else {
            if BladeRf1::xb200_is_enabled(&self.interface)? {
                log::info!(
                    "Consider supplying the quick_tune parameter to schedule_retune() when the XB-200 is enabled."
                );
            }
            frequency.try_into()?
        };

        log::trace!("{f:?}");

        let band = if f.flags & LMS_FREQ_FLAGS_LOW_BAND != 0 {
            Band::Low
        } else {
            Band::High
        };

        let tune = if (f.flags & LMS_FREQ_FLAGS_FORCE_VCOCAP) != 0 {
            Tune::Quick
        } else {
            Tune::Normal
        };

        self.interface.lock().unwrap().nios_retune(
            channel, timestamp, f.nint, f.nfrac, f.freqsel, f.vcocap, band, tune, f.xb_gpio,
        )?;
        Ok(f)
    }

    /// Cancel currently outstanding scheduled retunes
    pub fn cancel_scheduled_retunes(&self, channel: Channel) -> Result<()> {
        self.interface.lock().unwrap().nios_retune(
            channel,
            NiosPktRetuneRequest::CLEAR_QUEUE,
            0,
            0,
            0,
            0,
            Band::Low,
            Tune::Normal,
            0,
        )
    }
}
