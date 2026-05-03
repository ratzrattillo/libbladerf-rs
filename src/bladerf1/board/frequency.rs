use crate::bladerf1::BladeRf1;
#[cfg(feature = "xb200")]
use crate::bladerf1::board::xb;
#[cfg(feature = "xb200")]
use crate::bladerf1::board::xb::xb200::Xb200Path;
use crate::bladerf1::hardware::lms6002d;
use crate::bladerf1::hardware::lms6002d::frequency::LmsFreq;
use crate::bladerf1::protocol::NiosPktRetuneRequest;
use crate::channel::Channel;
use crate::error::{Error, Result};
use crate::range::{Range, RangeItem};

#[derive(Clone, Copy, Debug)]
pub enum TuningMode {
    Host,
    Fpga,
}

impl BladeRf1 {
    pub fn set_frequency(
        &mut self,
        channel: Channel,
        #[allow(unused_mut)] mut frequency: u64,
        mode: TuningMode,
    ) -> Result<()> {
        log::trace!("Setting Frequency on channel {channel:?} to {frequency}Hz");
        #[cfg(feature = "xb200")]
        if xb::xb200_is_enabled(&mut self.nios)? {
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
                lms6002d::frequency::set_frequency(&mut self.nios, channel, frequency)?;
                let band = lms6002d::Band::from(frequency);
                self.band_select(channel, band)?;
            }
            TuningMode::Fpga => {
                self.schedule_retune(channel, NiosPktRetuneRequest::RETUNE_NOW, frequency, None)?;
            }
        }
        Ok(())
    }

    pub fn get_frequency(&mut self, channel: Channel) -> Result<u64> {
        let f = lms6002d::frequency::get_frequency(&mut self.nios, channel)?;
        if f.x == 0 {
            log::error!("LMSFreq.x was zero!");
            return Err(Error::HardwareState("LMSFreq.x was zero"));
        }
        #[allow(unused_mut)]
        let mut frequency_hz: u64 = (&f).into();
        log::trace!("Frequency Hz: {frequency_hz}");
        #[cfg(feature = "xb200")]
        if xb::xb200_is_enabled(&mut self.nios)? {
            let path = self.xb200_get_path(channel)?;
            log::trace!("XB200 path detected: {:?}", path);
            if path == Xb200Path::Mix {
                log::debug!("Mix path - converting: 1248000000 - {}", frequency_hz);
                frequency_hz = 1_248_000_000 - frequency_hz;
            }
        }
        Ok(frequency_hz)
    }

    pub fn get_frequency_range(&mut self) -> Result<Range> {
        #[cfg(feature = "xb200")]
        let freq_min = if xb::xb200_is_enabled(&mut self.nios)? {
            0.0
        } else {
            lms6002d::frequency::get_frequency_min() as f64
        };
        #[cfg(not(feature = "xb200"))]
        let freq_min = lms6002d::frequency::get_frequency_min() as f64;
        let freq_max = lms6002d::frequency::get_frequency_max() as f64;
        Ok(Range {
            items: vec![RangeItem::Step(freq_min, freq_max, 1f64, 1f64)],
        })
    }

    pub fn select_band(&mut self, channel: Channel, frequency: u32) -> Result<()> {
        let band = lms6002d::Band::from(frequency);
        self.band_select(channel, band)
    }

    pub fn schedule_retune(
        &mut self,
        channel: Channel,
        timestamp: u64,
        frequency: u64,
        quick_tune: Option<LmsFreq>,
    ) -> Result<LmsFreq> {
        let (lms_freq, _) =
            self.schedule_retune_with_duration(channel, timestamp, frequency, quick_tune)?;
        Ok(lms_freq)
    }

    pub fn schedule_retune_with_duration(
        &mut self,
        channel: Channel,
        timestamp: u64,
        frequency: u64,
        quick_tune: Option<LmsFreq>,
    ) -> Result<(LmsFreq, u64)> {
        let f = if let Some(qt) = quick_tune {
            qt
        } else {
            #[cfg(feature = "xb200")]
            if xb::xb200_is_enabled(&mut self.nios)? {
                log::info!(
                    "Consider supplying the quick_tune parameter to schedule_retune() when the XB-200 is enabled."
                );
            }
            frequency.try_into()?
        };
        log::trace!("{f:?}");
        let band = if f.flags & lms6002d::LMS_FREQ_FLAGS_LOW_BAND != 0 {
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
        Ok((f, result.duration))
    }

    pub fn cancel_scheduled_retunes(&mut self, channel: Channel) -> Result<()> {
        self.nios.nios_retune(
            channel,
            NiosPktRetuneRequest::CLEAR_QUEUE,
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
}
