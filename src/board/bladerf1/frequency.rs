use crate::BladeRf1;
use crate::hardware::lms6002d::{
    BLADERF1_BAND_HIGH, LMS_FREQ_FLAGS_FORCE_VCOCAP, LMS_FREQ_FLAGS_LOW_BAND,
    LMS_FREQ_XB_200_ENABLE, LMS_FREQ_XB_200_FILTER_SW_SHIFT, LMS_FREQ_XB_200_MODULE_RX,
    LMS_FREQ_XB_200_PATH_SHIFT, LMS6002D,
};
use crate::nios::Nios;
use crate::xb200::BladerfXb200Path;
use anyhow::Result;
use anyhow::anyhow;
use bladerf_globals::bladerf1::{BLADERF_FREQUENCY_MAX, BLADERF_FREQUENCY_MIN, BladeRf1QuickTune};
use bladerf_globals::{SdrRange, TuningMode, bladerf_channel_rx};
use bladerf_nios::packet_retune::{Band, NiosPktRetuneRequest};

impl BladeRf1 {
    pub async fn set_frequency(&mut self, channel: u8, mut frequency: u64) -> Result<()> {
        //let dc_cal = if channel == bladerf_channel_rx!(0) { cal_dc.rx } else { cal.dc_tx };

        log::debug!("Setting Frequency on channel {channel} to {frequency}Hz");

        if self.xb200.is_some() {
            if frequency < BLADERF_FREQUENCY_MIN as u64 {
                log::debug!("Setting path to Mix");
                self.xb200_set_path(channel, &BladerfXb200Path::Mix).await?;

                self.xb200_auto_filter_selection(channel, frequency as u32)
                    .await?;

                frequency = 1248000000 - frequency;
            } else {
                log::debug!("Setting path to Bypass");
                self.xb200_set_path(channel, &BladerfXb200Path::Bypass)
                    .await?;
            }
        }

        // For tuning HOST Tuning Mode:
        match &self.board_data.tuning_mode {
            TuningMode::Host => {
                self.lms.set_frequency(channel, frequency as u32).await?;
                let band = if frequency < BLADERF1_BAND_HIGH as u64 {
                    Band::Low
                } else {
                    Band::High
                };
                self.band_select(channel, band).await?;
            }
            TuningMode::Fpga => {
                self.lms
                    .schedule_retune(
                        channel,
                        NiosPktRetuneRequest::RETUNE_NOW,
                        frequency as u32,
                        None,
                    )
                    .await?;
            }
        }

        Ok(())
    }

    pub async fn get_frequency(&self, channel: u8) -> Result<u32> {
        let f = self.lms.get_frequency(channel).await?;
        if f.x == 0 {
            /* If we see this, it's most often an indication that communication
             * with the LMS6002D is not occuring correctly */
            return Err(anyhow!("LMSFreq.x was zero!"));
        }
        let mut frequency_hz = LMS6002D::frequency_to_hz(&f);
        log::debug!("Frequency Hz: {frequency_hz}");

        if self.xb200.is_some() {
            let path = self.xb200_get_path(channel).await?;

            if path == BladerfXb200Path::Mix {
                log::debug!("Bypass Frequency Hz: 1248000000 - {frequency_hz}");
                frequency_hz = 1248000000 - frequency_hz;
            }
        }

        Ok(frequency_hz)
    }

    pub fn get_frequency_range(&self) -> SdrRange<u32> {
        if self.xb200.is_some() {
            SdrRange {
                min: 0,
                max: BLADERF_FREQUENCY_MAX,
                step: 1,
                scale: 1,
            }
        } else {
            SdrRange {
                min: BLADERF_FREQUENCY_MIN,
                max: BLADERF_FREQUENCY_MAX,
                step: 1,
                scale: 1,
            }
        }
    }

    pub async fn select_band(&self, channel: u8, frequency: u32) -> Result<()> {
        // CHECK_BOARD_STATE(STATE_INITIALIZED);

        let band = if frequency < BLADERF1_BAND_HIGH {
            Band::Low
        } else {
            Band::High
        };

        self.band_select(channel, band).await
    }

    pub async fn lms_get_quick_tune(&self, module: u8) -> Result<BladeRf1QuickTune> {
        let f = self.lms.get_frequency(module).await?;

        let mut quick_tune = BladeRf1QuickTune {
            freqsel: f.freqsel,
            vcocap: f.vcocap,
            nint: f.nint,
            nfrac: f.nfrac,
            flags: 0,
            xb_gpio: 0,
        };

        let val = self.interface.nios_expansion_gpio_read().await?;

        if self.xb200.is_some() {
            quick_tune.xb_gpio |= LMS_FREQ_XB_200_ENABLE;
            if module == bladerf_channel_rx!(0) {
                quick_tune.xb_gpio |= LMS_FREQ_XB_200_MODULE_RX;
                /* BLADERF_XB_CONFIG_RX_BYPASS_MASK */
                quick_tune.xb_gpio |= (((val & 0x30) >> 4) << LMS_FREQ_XB_200_PATH_SHIFT) as u8;
                /* BLADERF_XB_RX_MASK */
                quick_tune.xb_gpio |=
                    (((val & 0x30000000) >> 28) << LMS_FREQ_XB_200_FILTER_SW_SHIFT) as u8;
            } else {
                /* BLADERF_XB_CONFIG_TX_BYPASS_MASK */
                quick_tune.xb_gpio |=
                    (((val & 0x0C) >> 2) << LMS_FREQ_XB_200_FILTER_SW_SHIFT) as u8;
                /* BLADERF_XB_TX_MASK */
                quick_tune.xb_gpio |=
                    (((val & 0x0C000000) >> 26) << LMS_FREQ_XB_200_PATH_SHIFT) as u8;
            }

            quick_tune.flags = LMS_FREQ_FLAGS_FORCE_VCOCAP;

            if LMS6002D::frequency_to_hz(&f) < BLADERF1_BAND_HIGH {
                quick_tune.flags |= LMS_FREQ_FLAGS_LOW_BAND;
            }
        }
        Ok(quick_tune)
    }
}
