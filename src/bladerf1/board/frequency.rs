use crate::bladerf1::BladeRf1;
use crate::bladerf1::board::xb::xb200::Xb200Path;
use crate::bladerf1::hardware::lms6002d::bandwidth::BLADERF1_BAND_HIGH;
use crate::bladerf1::hardware::lms6002d::frequency::LmsFreq;
use crate::bladerf1::hardware::lms6002d::{
    Band, LMS_FREQ_FLAGS_FORCE_VCOCAP, LMS_FREQ_FLAGS_LOW_BAND, LMS6002D, Tune,
};
use crate::bladerf1::protocol::NiosPktRetuneRequest;
use crate::channel::Channel;
use crate::error::{Error, Result};
use crate::range::{Range, RangeItem};
#[derive(Clone)]
pub enum TuningMode {
    #[allow(dead_code)]
    Host,
    Fpga,
}
impl BladeRf1 {
    pub fn set_frequency(&self, channel: Channel, mut frequency: u64) -> Result<()> {
        log::trace!("Setting Frequency on channel {channel:?} to {frequency}Hz");
        let freq_min = LMS6002D::get_frequency_min() as u64;
        if BladeRf1::xb200_is_enabled(&self.interface)? {
            if frequency < freq_min {
                log::debug!("Setting path to Mix");
                self.xb200_set_path(channel, Xb200Path::Mix)?;
                self.xb200_auto_filter_selection(channel, frequency)?;
                frequency = 1248000000 - frequency;
            } else {
                log::debug!("Setting path to Bypass");
                self.xb200_set_path(channel, Xb200Path::Bypass)?;
            }
        }
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
    pub fn get_frequency(&self, channel: Channel) -> Result<u64> {
        let f = self.lms.get_frequency(channel)?;
        if f.x == 0 {
            log::error!("LMSFreq.x was zero!");
            return Err(Error::Invalid);
        }
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
    pub fn get_frequency_range(&self) -> Result<Range> {
        let freq_min = if BladeRf1::xb200_is_enabled(&self.interface)? {
            0.0
        } else {
            LMS6002D::get_frequency_min() as f64
        };
        let freq_max = LMS6002D::get_frequency_max() as f64;
        Ok(Range {
            items: vec![RangeItem::Step(freq_min, freq_max, 1f64, 1f64)],
        })
    }
    pub fn select_band(&self, channel: Channel, frequency: u32) -> Result<()> {
        let band = if frequency < BLADERF1_BAND_HIGH {
            Band::Low
        } else {
            Band::High
        };
        self.band_select(channel, band)
    }
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
    pub fn cancel_scheduled_retunes(&self, channel: Channel) -> Result<()> {
        self.interface.lock().unwrap().nios_retune(
            channel,
            NiosPktRetuneRequest::CLEAR_QUEUE,
            0u16,
            0u32,
            0,
            0,
            Band::Low,
            Tune::Normal,
            0,
        )
    }
}
