use crate::BladeRf1;
use anyhow::{Result, anyhow};
use bladerf_globals::BladeRfDirection;
use bladerf_globals::bladerf1::{
    BLADERF_GPIO_AGC_ENABLE, BLADERF_LNA_GAIN_MAX_DB, BLADERF_LNA_GAIN_MID_DB,
    BLADERF_RXVGA1_GAIN_MAX, BLADERF_RXVGA1_GAIN_MIN, BLADERF_RXVGA2_GAIN_MAX,
    BLADERF_RXVGA2_GAIN_MIN, BLADERF_TXVGA1_GAIN_MAX, BLADERF_TXVGA1_GAIN_MIN,
    BLADERF_TXVGA2_GAIN_MAX, BLADERF_TXVGA2_GAIN_MIN, BLADERF1_RX_GAIN_OFFSET,
    BLADERF1_TX_GAIN_OFFSET, BladerfLnaGain,
};
use bladerf_globals::{
    BLADERF_MODULE_RX, BLADERF_MODULE_TX, BladerfGainMode, SdrRange, bladerf_channel_is_tx,
    bladerf_channel_rx, bladerf_channel_tx,
};

// pub fn __scale(r: &SdrRange, v: f32) -> f32 {
//     v / r.scale as f32
// }
//
// pub fn __scale_int(r: &SdrRange, v: f32) -> i8 {
//     __scale(r, v).round() as i8
// }
//
// pub fn __unscale(r: &SdrRange, v: f32) -> f32 {
//     v * r.scale as f32
// }
//
// pub fn __unscale_int(r: &SdrRange, v: f32) -> i8 {
//     __unscale(r, v).round() as i8
// }

impl BladeRf1 {
    /**
     * @brief      applies overall_gain to stage_gain, within the range max
     *
     * "Moves" gain from overall_gain to stage_gain, ensuring that overall_gain
     * doesn't go negative and stage_gain doesn't exceed range->max.
     *
     * @param[in]  range         The range for stage_gain
     * @param      stage_gain    The stage gain
     * @param      overall_gain  The overall gain
     */
    fn _apportion_gain(range: &SdrRange<i8>, stage_gain: i8, overall_gain: i8) -> (i8, i8) {
        //let headroom = __unscale_int(range, range.max as f32);
        let headroom = range.scale * range.max;
        let mut allotment = overall_gain.min(headroom);

        /* Enforce step size */
        while 0 != (allotment % range.step) {
            allotment -= 1;
        }

        (stage_gain + allotment, overall_gain - allotment)
    }

    fn _convert_gain_to_lna_gain(gain: i8) -> BladerfLnaGain {
        if gain >= BLADERF_LNA_GAIN_MAX_DB {
            BladerfLnaGain::Max
        } else if gain >= BLADERF_LNA_GAIN_MID_DB {
            BladerfLnaGain::Mid
        } else {
            BladerfLnaGain::Bypass
        }
    }

    fn _convert_lna_gain_to_gain(lna_gain: BladerfLnaGain) -> i8 {
        match lna_gain {
            BladerfLnaGain::Max => BLADERF_LNA_GAIN_MAX_DB,
            BladerfLnaGain::Mid => BLADERF_LNA_GAIN_MID_DB,
            BladerfLnaGain::Bypass => 0,
            _ => -1,
        }
    }

    pub fn get_gain_range(channel: u8) -> SdrRange<i8> {
        if bladerf_channel_is_tx!(channel) {
            /* Overall TX gain range */
            SdrRange {
                min: BLADERF_TXVGA1_GAIN_MIN
                    + BLADERF_TXVGA2_GAIN_MIN
                    + BLADERF1_TX_GAIN_OFFSET.round() as i8,
                max: BLADERF_TXVGA1_GAIN_MAX
                    + BLADERF_TXVGA2_GAIN_MAX
                    + BLADERF1_TX_GAIN_OFFSET.round() as i8,
                step: 1,
                scale: 1,
            }
        } else {
            /* Overall RX gain range */
            SdrRange {
                min: BLADERF_RXVGA1_GAIN_MIN
                    + BLADERF_RXVGA2_GAIN_MIN
                    + BLADERF1_RX_GAIN_OFFSET.round() as i8,
                max: BLADERF_LNA_GAIN_MAX_DB
                    + BLADERF_RXVGA1_GAIN_MAX
                    + BLADERF_RXVGA2_GAIN_MAX
                    + BLADERF1_RX_GAIN_OFFSET.round() as i8,
                step: 1,
                scale: 1,
            }
        }
    }

    pub async fn get_gain_modes(&self, channel: u8) -> Result<Vec<BladerfGainMode>> {
        if bladerf_channel_is_tx!(channel) {
            Err(anyhow!("TX does not support gain modes"))
        } else {
            Ok(vec![BladerfGainMode::Mgc, BladerfGainMode::Default])
        }
    }

    pub async fn set_gain_mode(&self, channel: u8, mode: BladerfGainMode) -> Result<u32> {
        if bladerf_channel_is_tx!(channel) {
            return Err(anyhow!("Setting gain mode for TX is not supported"));
        }

        let mut config_gpio = self.config_gpio_read().await?;
        if mode == BladerfGainMode::Default {
            // Default mode is the same as Automatic mode
            // return Err(anyhow!("Todo: Implement AGC Table"));
            // if (!have_cap(board_data->capabilities, BLADERF_CAP_AGC_DC_LUT)) {
            //     log_warning("AGC not supported by FPGA. %s\n", MGC_WARN);
            //     log_info("To enable AGC, %s, then %s\n", FPGA_STR, DCCAL_STR);
            //     log_debug("%s: expected FPGA >= v0.7.0, got v%u.%u.%u\n",
            //               __FUNCTION__, board_data->fpga_version.major,
            //               board_data->fpga_version.minor,
            //               board_data->fpga_version.patch);
            //     return BLADERF_ERR_UNSUPPORTED;
            // }
            //
            // if (!board_data->cal.dc_rx) {
            //     log_warning("RX DC calibration table not found. %s\n", MGC_WARN);
            //     log_info("To enable AGC, %s\n", DCCAL_STR);
            //     return BLADERF_ERR_UNSUPPORTED;
            // }
            //
            // if (board_data->cal.dc_rx->version != TABLE_VERSION) {
            //     log_warning("RX DC calibration table is out-of-date. %s\n",
            //                 MGC_WARN);
            //     log_info("To enable AGC, %s\n", DCCAL_STR);
            //     log_debug("%s: expected version %u, got %u\n", __FUNCTION__,
            //               TABLE_VERSION, board_data->cal.dc_rx->version);
            //
            //     return BLADERF_ERR_UNSUPPORTED;
            // }
            config_gpio |= BLADERF_GPIO_AGC_ENABLE;
        } else if mode == BladerfGainMode::Mgc {
            config_gpio &= !BLADERF_GPIO_AGC_ENABLE;
        }

        self.config_gpio_write(config_gpio).await
    }

    pub async fn get_gain_mode(&self) -> Result<BladerfGainMode> {
        let data = self.config_gpio_read().await?;

        let gain_mode = if data & BLADERF_GPIO_AGC_ENABLE != 0 {
            BladerfGainMode::Default
        } else {
            BladerfGainMode::Mgc
        };
        Ok(gain_mode)
    }

    pub async fn get_gain_stage(&self, channel: u8, stage: &str) -> Result<i8> {
        // CHECK_BOARD_STATE(STATE_INITIALIZED);
        if channel == BLADERF_MODULE_TX {
            match stage {
                "txvga1" => self.lms.txvga1_get_gain().await,
                "txvga2" => self.lms.txvga2_get_gain().await,
                _ => Err(anyhow!("invalid stage {stage}")),
            }
        } else if channel == BLADERF_MODULE_RX {
            match stage {
                "lna" => {
                    let lna_gain = self.lms.lna_get_gain().await?;
                    Ok(Self::_convert_lna_gain_to_gain(lna_gain))
                }
                "rxvga1" => self.lms.rxvga1_get_gain().await,
                "rxvga2" => self.lms.rxvga2_get_gain().await,
                _ => Err(anyhow!("invalid stage {stage}")),
            }
        } else {
            Err(anyhow!("invalid channel {channel}"))
        }
    }

    pub async fn set_gain_stage(&self, channel: u8, stage: &str, gain: i8) -> Result<()> {
        // CHECK_BOARD_STATE(STATE_INITIALIZED);

        /* TODO implement gain clamping */
        match channel {
            BLADERF_MODULE_TX => match stage {
                "txvga1" => Ok(self.lms.txvga1_set_gain(gain).await?),
                "txvga2" => Ok(self.lms.txvga2_set_gain(gain).await?),
                _ => Err(anyhow!("invalid stage {stage}")),
            },
            BLADERF_MODULE_RX => match stage {
                "rxvga1" => Ok(self.lms.rxvga1_set_gain(gain).await?),
                "rxvga2" => Ok(self.lms.rxvga2_set_gain(gain).await?),
                "lna" => Ok(self
                    .lms
                    .lna_set_gain(Self::_convert_gain_to_lna_gain(gain))
                    .await?),
                _ => Err(anyhow!("invalid stage {stage}")),
            },
            _ => Err(anyhow!("Invalid channel {channel}")),
        }
    }

    pub fn get_gain_stages(channel: u8) -> Vec<String> {
        if bladerf_channel_is_tx!(channel) {
            vec!["txvga1".to_string(), "txvga2".to_string()]
        } else {
            vec![
                "lna".to_string(),
                "rxvga1".to_string(),
                "rxvga2".to_string(),
            ]
        }
    }

    /** Use bladerf_get_gain_range(), bladerf_set_gain(), and
     *             bladerf_get_gain() to control total system gain. For direct
     *             control of individual gain stages, use bladerf_get_gain_stages(),
     *             bladerf_get_gain_stage_range(), bladerf_set_gain_stage(), and
     *             bladerf_get_gain_stage().
     **/
    pub fn get_gain_stage_range(channel: u8, stage: &str) -> Result<SdrRange<i8>> {
        if channel == BLADERF_MODULE_RX {
            match stage {
                "lna" => Ok(SdrRange {
                    min: 0,
                    max: BLADERF_LNA_GAIN_MAX_DB,
                    step: 3,
                    scale: 1,
                }),
                "rxvga1" => Ok(SdrRange {
                    min: BLADERF_RXVGA1_GAIN_MIN,
                    max: BLADERF_RXVGA1_GAIN_MAX,
                    step: 1,
                    scale: 1,
                }),
                "rxvga2" => Ok(SdrRange {
                    min: BLADERF_RXVGA2_GAIN_MIN,
                    max: BLADERF_RXVGA2_GAIN_MAX,
                    step: 3,
                    scale: 1,
                }),
                _ => Err(anyhow!("Invalid stage: {stage}")),
            }
        } else {
            match stage {
                "txvga1" => Ok(SdrRange {
                    min: BLADERF_TXVGA1_GAIN_MIN,
                    max: BLADERF_TXVGA1_GAIN_MAX,
                    step: 1,
                    scale: 1,
                }),
                "txvga2" => Ok(SdrRange {
                    min: BLADERF_TXVGA2_GAIN_MIN,
                    max: BLADERF_TXVGA2_GAIN_MAX,
                    step: 3,
                    scale: 1,
                }),
                _ => Err(anyhow!("Invalid stage: {stage}")),
            }
        }
    }

    pub async fn get_tx_gain(&self) -> Result<i8> {
        let txvga1 = self.lms.txvga1_get_gain().await?;
        let txvga2 = self.lms.txvga2_get_gain().await?;

        Ok(txvga1 + txvga2 + BLADERF1_TX_GAIN_OFFSET.round() as i8)
    }

    pub async fn get_rx_gain(&self) -> Result<i8> {
        let lna_gain = self.lms.lna_get_gain().await?;
        let rxvga1_gain = self.lms.rxvga1_get_gain().await?;
        let rxvga2_gain = self.lms.rxvga2_get_gain().await?;

        let lna_gain_db = match lna_gain {
            // BladerfLnaGain::Bypass => 0,
            BladerfLnaGain::Mid => BLADERF_LNA_GAIN_MID_DB,
            BladerfLnaGain::Max => BLADERF_LNA_GAIN_MAX_DB,
            _ => 0,
        };

        Ok(lna_gain_db + rxvga1_gain + rxvga2_gain + BLADERF1_RX_GAIN_OFFSET.round() as i8)
    }

    pub async fn get_gain(&self, channel: u8) -> Result<i8> {
        // CHECK_BOARD_STATE(STATE_INITIALIZED);

        if bladerf_channel_is_tx!(channel) {
            self.get_tx_gain().await
        } else {
            self.get_rx_gain().await
        }
    }

    pub async fn set_gain(&self, channel: u8, gain: i8) -> Result<()> {
        // CHECK_BOARD_STATE(STATE_INITIALIZED);

        if bladerf_channel_is_tx!(channel) {
            self.set_tx_gain(gain).await
        } else {
            self.set_rx_gain(gain).await
        }
    }

    pub async fn set_tx_gain(&self, mut gain: i8) -> Result<()> {
        let orig_gain = gain;

        let txvga1_range = Self::get_gain_stage_range(bladerf_channel_tx!(0), "txvga1")?;
        let txvga2_range = Self::get_gain_stage_range(bladerf_channel_tx!(0), "txvga2")?;

        // __unscale_int // as we use i8 type here, rounding is not necessary
        let mut txvga1 = txvga1_range.scale * txvga1_range.min;
        let mut txvga2 = txvga2_range.scale * txvga2_range.min;

        // offset gain so that we can use it as a counter when apportioning gain
        gain -= BLADERF1_TX_GAIN_OFFSET.round() as i8 + txvga1 + txvga2;

        // apportion gain to TXVGA2
        (txvga2, gain) = Self::_apportion_gain(&txvga2_range, txvga2, gain);

        // apportion gain to TXVGA1
        (txvga1, gain) = Self::_apportion_gain(&txvga1_range, txvga1, gain);

        // verification
        if gain != 0 {
            println!(
                "unable to achieve requested gain {} (missed by {})\n",
                orig_gain, gain
            );
            println!(
                "gain={} -> txvga2={} txvga1={} remainder={}\n",
                orig_gain, txvga2, txvga1, gain
            );
        }

        self.lms.txvga1_set_gain(txvga1).await?;
        self.lms.txvga2_set_gain(txvga2).await?;
        Ok(())
    }

    pub async fn set_rx_gain(&self, mut gain: i8) -> Result<()> {
        let orig_gain = gain;

        let lna_range = Self::get_gain_stage_range(bladerf_channel_rx!(0), "lna")?;
        let rxvga1_range = Self::get_gain_stage_range(bladerf_channel_rx!(0), "rxvga1")?;
        let rxvga2_range = Self::get_gain_stage_range(bladerf_channel_rx!(0), "rxvga2")?;

        // __unscale_int // as we use i8 type here, rounding is not necessary
        let mut lna = lna_range.scale * lna_range.min;
        let mut rxvga1 = rxvga1_range.scale * rxvga1_range.min;
        let mut rxvga2 = rxvga2_range.scale * rxvga2_range.min;

        // offset gain so that we can use it as a counter when apportioning gain
        gain -= BLADERF1_RX_GAIN_OFFSET.round() as i8 + lna + rxvga1 + rxvga2;

        // apportion some gain to RXLNA (but only half of it for now)
        (lna, gain) = Self::_apportion_gain(&lna_range, lna, gain);
        if lna > BLADERF_LNA_GAIN_MID_DB {
            gain += lna - BLADERF_LNA_GAIN_MID_DB;
            lna -= lna - BLADERF_LNA_GAIN_MID_DB;
        }

        // apportion gain to RXVGA1
        (rxvga1, gain) = Self::_apportion_gain(&rxvga1_range, rxvga1, gain);

        // apportion more gain to RXLNA
        (lna, gain) = Self::_apportion_gain(&lna_range, lna, gain);

        // apportion gain to RXVGA2
        (rxvga2, gain) = Self::_apportion_gain(&rxvga2_range, rxvga2, gain);

        // if we still have remaining gain, it's because rxvga2 has a step size of
        // 3 dB. Steal a few dB from rxvga1...
        // __unscale_int replaced by normal multiplications without rounding
        if gain > 0 && rxvga1 >= (rxvga1_range.scale * rxvga1_range.max) {
            // __unscale_int replaced by normal multiplications without rounding
            rxvga1 -= rxvga2_range.scale * rxvga2_range.step;
            gain += rxvga2_range.scale * rxvga2_range.step;

            (rxvga2, gain) = Self::_apportion_gain(&rxvga2_range, rxvga2, gain);
            (rxvga1, gain) = Self::_apportion_gain(&rxvga1_range, rxvga1, gain);
        }

        // verification
        if gain != 0 {
            println!(
                "unable to achieve requested gain {} (missed by {})\n",
                orig_gain, gain
            );
            println!(
                "gain={} -> 1xvga1={} lna={} rxvga2={} remainder={}\n",
                orig_gain, rxvga1, lna, rxvga2, gain
            );
        }

        // that should do it. actually apply the changes:
        self.lms
            .lna_set_gain(Self::_convert_gain_to_lna_gain(lna))
            .await?;
        // __scale_int(&rxvga1_range, rxvga1 as f32)
        self.lms
            .rxvga1_set_gain(rxvga1 / rxvga1_range.scale)
            .await?;
        // __scale_int(&rxvga2_range, rxvga2 as f32)
        self.lms
            .rxvga2_set_gain(rxvga2 / rxvga2_range.scale)
            .await?;

        Ok(())
    }
}
