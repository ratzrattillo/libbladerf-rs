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

#[derive(Clone)]
pub enum TuningMode {
    #[allow(dead_code)]
    Host,
    Fpga,
}

impl BladeRf1 {
    pub fn set_frequency(&self, channel: u8, mut frequency: u64) -> Result<()> {
        // let dc_cal = if channel == bladerf_channel_rx!(0) { cal_dc.rx } else { cal.dc_tx };

        log::trace!("Setting Frequency on channel {channel} to {frequency}Hz");

        if BladeRf1::xb200_is_enabled(&self.interface)? {
            if frequency < BLADERF_FREQUENCY_MIN as u64 {
                log::debug!("Setting path to Mix");
                self.xb200_set_path(channel, &Xb200Path::Mix)?;

                self.xb200_auto_filter_selection(channel, frequency)?;

                frequency = 1248000000 - frequency;
            } else {
                log::debug!("Setting path to Bypass");
                self.xb200_set_path(channel, &Xb200Path::Bypass)?;
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

    pub fn get_frequency(&self, channel: u8) -> Result<u64> {
        let f = self.lms.get_frequency(channel)?;
        if f.x == 0 {
            // If we see this, it's most often an indication that communication
            // with the LMS6002D is not occuring correctly
            log::error!("LMSFreq.x was zero!");
            return Err(Error::Invalid);
        }
        // let mut frequency_hz = LMS6002D::frequency_to_hz(&f);
        let mut frequency_hz: u64 = (&f).into();
        log::trace!("Frequency Hz: {frequency_hz}");

        // if self.xb200.is_some() {
        if BladeRf1::xb200_is_enabled(&self.interface)? {
            let path = self.xb200_get_path(channel)?;

            if path == Xb200Path::Mix {
                log::debug!("Bypass Frequency Hz: 1248000000 - {frequency_hz}");
                frequency_hz = 1248000000 - frequency_hz;
            }
        }

        Ok(frequency_hz)
    }

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

    pub fn select_band(&self, channel: u8, frequency: u32) -> Result<()> {
        // CHECK_BOARD_STATE(STATE_INITIALIZED);

        let band = if frequency < BLADERF1_BAND_HIGH {
            Band::Low
        } else {
            Band::High
        };

        self.band_select(channel, band)
    }

    pub fn schedule_retune(
        &self,
        channel: u8,
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

    pub fn cancel_scheduled_retunes(&self, channel: u8) -> Result<()> {
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
