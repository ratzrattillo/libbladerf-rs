use crate::BladeRf1;
use crate::hardware::lms6002d::{
    BLADERF_RXVGA1_GAIN_MAX, BLADERF_RXVGA1_GAIN_MIN, BLADERF_RXVGA2_GAIN_MAX,
    BLADERF_RXVGA2_GAIN_MIN, BLADERF_TXVGA1_GAIN_MAX, BLADERF_TXVGA1_GAIN_MIN,
    BLADERF_TXVGA2_GAIN_MAX, BLADERF_TXVGA2_GAIN_MIN, BLADERF1_RX_GAIN_OFFSET,
    BLADERF1_TX_GAIN_OFFSET,
};
use crate::{Error, Result};
use bladerf_globals::bladerf1::BLADERF_GPIO_AGC_ENABLE;
use bladerf_globals::range::{Range, RangeItem};
use bladerf_globals::{
    BLADERF_LNA_GAIN_MAX_DB, BLADERF_LNA_GAIN_MID_DB, BLADERF_MODULE_RX, BLADERF_MODULE_TX,
    BladeRf1GainMode, bladerf_channel_is_tx, bladerf_channel_rx, bladerf_channel_tx,
};
use bladerf_globals::{BladeRf1Direction, GainDb};
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
    /// @brief      applies overall_gain to stage_gain, within the range max
    ///
    /// "Moves" gain from overall_gain to stage_gain, ensuring that overall_gain
    /// doesn't go negative and stage_gain doesn't exceed range->max.
    ///
    /// @param\[in\]  range         The range for stage_gain
    /// @param      stage_gain    The stage gain
    /// @param      overall_gain  The overall gain
    fn _apportion_gain(range: &Range, stage_gain: i8, overall_gain: i8) -> (i8, i8) {
        // let headroom = __unscale_int(range, range.max as f32);
        let headroom = (range.scale().unwrap() * range.max().unwrap()).round() as i8;
        let mut allotment = overall_gain.min(headroom);

        // Enforce step size
        while 0 != (allotment % (range.step().unwrap() as i8)) {
            allotment -= 1;
        }

        (stage_gain + allotment, overall_gain - allotment)
    }

    pub fn get_gain_range(channel: u8) -> Range {
        if bladerf_channel_is_tx!(channel) {
            // Overall TX gain range
            Range {
                items: vec![RangeItem::Step(
                    BLADERF_TXVGA1_GAIN_MIN as f64
                        + BLADERF_TXVGA2_GAIN_MIN as f64
                        + BLADERF1_TX_GAIN_OFFSET as f64,
                    BLADERF_TXVGA1_GAIN_MAX as f64
                        + BLADERF_TXVGA2_GAIN_MAX as f64
                        + BLADERF1_TX_GAIN_OFFSET as f64,
                    1f64,
                    1f64,
                )],
            }
        } else {
            // Overall RX gain range
            Range {
                items: vec![RangeItem::Step(
                    BLADERF_RXVGA1_GAIN_MIN as f64
                        + BLADERF_RXVGA2_GAIN_MIN as f64
                        + BLADERF1_RX_GAIN_OFFSET as f64,
                    BLADERF_LNA_GAIN_MAX_DB as f64
                        + BLADERF_RXVGA1_GAIN_MAX as f64
                        + BLADERF_RXVGA2_GAIN_MAX as f64
                        + BLADERF1_RX_GAIN_OFFSET as f64,
                    1f64,
                    1f64,
                )],
            }
        }
    }

    pub fn get_gain_modes(&self, channel: u8) -> Result<Vec<BladeRf1GainMode>> {
        if bladerf_channel_is_tx!(channel) {
            log::error!("TX does not support gain modes");
            Err(Error::Invalid)
        } else {
            Ok(vec![BladeRf1GainMode::Mgc, BladeRf1GainMode::Default])
        }
    }

    pub fn set_gain_mode(&self, channel: u8, mode: BladeRf1GainMode) -> Result<()> {
        if bladerf_channel_is_tx!(channel) {
            log::error!("Setting gain mode for TX is not supported");
            return Err(Error::Invalid);
        }

        let mut config_gpio = self.config_gpio_read()?;
        if mode == BladeRf1GainMode::Default {
            // TODO:
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
        } else if mode == BladeRf1GainMode::Mgc {
            config_gpio &= !BLADERF_GPIO_AGC_ENABLE;
        }

        self.config_gpio_write(config_gpio)
    }

    pub fn get_gain_mode(&self) -> Result<BladeRf1GainMode> {
        let data = self.config_gpio_read()?;

        let gain_mode = if data & BLADERF_GPIO_AGC_ENABLE != 0 {
            BladeRf1GainMode::Default
        } else {
            BladeRf1GainMode::Mgc
        };
        Ok(gain_mode)
    }

    pub fn get_gain_stage(&self, channel: u8, stage: &str) -> Result<GainDb> {
        // CHECK_BOARD_STATE(STATE_INITIALIZED);
        if channel == BLADERF_MODULE_TX {
            match stage {
                "txvga1" => self.lms.txvga1_get_gain(),
                "txvga2" => self.lms.txvga2_get_gain(),
                _ => {
                    log::error!("invalid stage {stage}");
                    Err(Error::Invalid)
                }
            }
        } else if channel == BLADERF_MODULE_RX {
            match stage {
                "lna" => self.lms.lna_get_gain(),
                "rxvga1" => self.lms.rxvga1_get_gain(),
                "rxvga2" => self.lms.rxvga2_get_gain(),
                _ => {
                    log::error!("invalid stage {stage}");
                    Err(Error::Invalid)
                }
            }
        } else {
            log::error!("invalid channel {channel}");
            Err(Error::Invalid)
        }
    }

    pub fn set_gain_stage(&self, channel: u8, stage: &str, gain: GainDb) -> Result<()> {
        // CHECK_BOARD_STATE(STATE_INITIALIZED);

        // TODO implement gain clamping
        match channel {
            BLADERF_MODULE_TX => match stage {
                "txvga1" => Ok(self.lms.txvga1_set_gain(gain)?),
                "txvga2" => Ok(self.lms.txvga2_set_gain(gain)?),
                _ => {
                    log::error!("invalid stage {stage}");
                    Err(Error::Invalid)
                }
            },
            BLADERF_MODULE_RX => match stage {
                "rxvga1" => Ok(self.lms.rxvga1_set_gain(gain)?),
                "rxvga2" => Ok(self.lms.rxvga2_set_gain(gain)?),
                "lna" => Ok(self.lms.lna_set_gain(gain)?), // Self::_convert_gain_to_lna_gain(gain)
                _ => {
                    log::error!("invalid stage {stage}");
                    Err(Error::Invalid)
                }
            },
            _ => {
                log::error!("invalid channel {channel}");
                Err(Error::Invalid)
            }
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

    /// Use bladerf_get_gain_range(), bladerf_set_gain(), and
    /// bladerf_get_gain() to control total system gain. For direct
    /// control of individual gain stages, use bladerf_get_gain_stages(),
    /// bladerf_get_gain_stage_range(), bladerf_set_gain_stage(), and
    /// bladerf_get_gain_stage().
    pub fn get_gain_stage_range(channel: u8, stage: &str) -> Result<Range> {
        if channel == BLADERF_MODULE_RX {
            match stage {
                "lna" => Ok(Range {
                    items: vec![RangeItem::Step(
                        0f64,
                        BLADERF_LNA_GAIN_MAX_DB as f64,
                        3f64,
                        1f64,
                    )],
                }),
                "rxvga1" => Ok(Range {
                    items: vec![RangeItem::Step(
                        BLADERF_RXVGA1_GAIN_MIN as f64,
                        BLADERF_RXVGA1_GAIN_MAX as f64,
                        1f64,
                        1f64,
                    )],
                }),
                "rxvga2" => Ok(Range {
                    items: vec![RangeItem::Step(
                        BLADERF_RXVGA2_GAIN_MIN as f64,
                        BLADERF_RXVGA2_GAIN_MAX as f64,
                        1f64,
                        1f64,
                    )],
                }),
                _ => {
                    log::error!("invalid stage {stage}");
                    Err(Error::Invalid)
                }
            }
        } else {
            match stage {
                "txvga1" => Ok(Range {
                    items: vec![RangeItem::Step(
                        BLADERF_TXVGA1_GAIN_MIN as f64,
                        BLADERF_TXVGA1_GAIN_MAX as f64,
                        1f64,
                        1f64,
                    )],
                }),
                "txvga2" => Ok(Range {
                    items: vec![RangeItem::Step(
                        BLADERF_TXVGA2_GAIN_MIN as f64,
                        BLADERF_TXVGA2_GAIN_MAX as f64,
                        1f64,
                        1f64,
                    )],
                }),
                _ => {
                    log::error!("invalid stage {stage}");
                    Err(Error::Invalid)
                }
            }
        }
    }

    fn get_tx_gain(&self) -> Result<GainDb> {
        let txvga1 = self.lms.txvga1_get_gain()?;
        let txvga2 = self.lms.txvga2_get_gain()?;

        Ok(GainDb {
            db: txvga1.db + txvga2.db + BLADERF1_TX_GAIN_OFFSET.round() as i8,
        })
    }

    fn get_rx_gain(&self) -> Result<GainDb> {
        let lna_gain_db = self.lms.lna_get_gain()?;
        let rxvga1_gain_db = self.lms.rxvga1_get_gain()?;
        let rxvga2_gain_db = self.lms.rxvga2_get_gain()?;

        Ok(GainDb {
            db: lna_gain_db.db
                + rxvga1_gain_db.db
                + rxvga2_gain_db.db
                + BLADERF1_RX_GAIN_OFFSET as i8,
        })
    }

    pub fn get_gain(&self, channel: u8) -> Result<GainDb> {
        // CHECK_BOARD_STATE(STATE_INITIALIZED);

        if bladerf_channel_is_tx!(channel) {
            self.get_tx_gain()
        } else {
            self.get_rx_gain()
        }
    }

    pub fn set_gain(&self, channel: u8, gain: GainDb) -> Result<()> {
        // CHECK_BOARD_STATE(STATE_INITIALIZED);

        if bladerf_channel_is_tx!(channel) {
            self.set_tx_gain(gain)
        } else {
            self.set_rx_gain(gain)
        }
    }

    pub fn set_tx_gain(&self, gain_db: GainDb) -> Result<()> {
        let mut gain = gain_db.db;
        let orig_gain = gain;

        let txvga1_range = Self::get_gain_stage_range(bladerf_channel_tx!(0), "txvga1")?;
        let txvga2_range = Self::get_gain_stage_range(bladerf_channel_tx!(0), "txvga2")?;

        // __unscale_int // as we use i8 type here, rounding is not necessary
        let mut txvga1 =
            (txvga1_range.scale().unwrap() * txvga1_range.min().unwrap()).round() as i8;
        let mut txvga2 =
            (txvga2_range.scale().unwrap() * txvga2_range.min().unwrap()).round() as i8;

        // offset gain so that we can use it as a counter when apportioning gain
        gain -= BLADERF1_TX_GAIN_OFFSET.round() as i8 + txvga1 + txvga2;

        // apportion gain to TXVGA2
        (txvga2, gain) = Self::_apportion_gain(&txvga2_range, txvga2, gain);

        // apportion gain to TXVGA1
        (txvga1, gain) = Self::_apportion_gain(&txvga1_range, txvga1, gain);

        // verification
        if gain != 0 {
            log::debug!("unable to achieve requested gain {orig_gain} (missed by {gain})\n");
            log::debug!("gain={orig_gain} -> txvga2={txvga2} txvga1={txvga1} remainder={gain}\n");
        }

        // self.lms.txvga1_set_gain(txvga1)?;
        // self.lms.txvga2_set_gain(txvga2)
        self.lms.txvga1_set_gain(GainDb { db: txvga1 })?;
        self.lms.txvga2_set_gain(GainDb { db: txvga2 })
    }

    pub fn set_rx_gain(&self, gain_db: GainDb) -> Result<()> {
        let mut gain = gain_db.db;
        let orig_gain = gain;
        const CHANNEL: u8 = bladerf_channel_rx!(0);

        let lna_range = Self::get_gain_stage_range(CHANNEL, "lna")?;
        let rxvga1_range = Self::get_gain_stage_range(CHANNEL, "rxvga1")?;
        let rxvga2_range = Self::get_gain_stage_range(CHANNEL, "rxvga2")?;

        // __unscale_int // as we use i8 type here, rounding is not necessary
        let mut lna = (lna_range.scale().unwrap() * lna_range.min().unwrap()).round() as i8;
        let mut rxvga1 =
            (rxvga1_range.scale().unwrap() * rxvga1_range.min().unwrap()).round() as i8;
        let mut rxvga2 =
            (rxvga2_range.scale().unwrap() * rxvga2_range.min().unwrap()).round() as i8;

        // offset gain so that we can use it as a counter when apportioning gain
        gain -= BLADERF1_RX_GAIN_OFFSET as i8 + lna + rxvga1 + rxvga2;

        // apportion some gain to RXLNA (but only half of it for now)
        (lna, gain) = Self::_apportion_gain(&lna_range, lna, gain);
        if lna > BLADERF_LNA_GAIN_MID_DB {
            gain += lna - BLADERF_LNA_GAIN_MID_DB;
            lna = lna - (lna - BLADERF_LNA_GAIN_MID_DB);
        }

        // apportion gain to RXVGA1
        (rxvga1, gain) = Self::_apportion_gain(&rxvga1_range, rxvga1, gain);

        // apportion more gain to RXLNA
        (lna, gain) = Self::_apportion_gain(&lna_range, lna, gain);

        // apportion gain to RXVGA2
        (rxvga2, gain) = Self::_apportion_gain(&rxvga2_range, rxvga2, gain);

        // if we still have remaining gain, it's because rxvga2 has a step size of
        // 3 dB. Steal a few dB from rxvga1...
        // In the original driver __unscale_int replaced by normal multiplications without rounding
        if gain > 0
            && rxvga1 >= (rxvga1_range.scale().unwrap() * rxvga1_range.max().unwrap()).round() as i8
        {
            // __unscale_int replaced by normal multiplications without rounding
            rxvga1 -= (rxvga2_range.scale().unwrap() * rxvga2_range.step().unwrap()).round() as i8;
            gain += (rxvga2_range.scale().unwrap() * rxvga2_range.step().unwrap()).round() as i8;

            (rxvga2, gain) = Self::_apportion_gain(&rxvga2_range, rxvga2, gain);
            (rxvga1, gain) = Self::_apportion_gain(&rxvga1_range, rxvga1, gain);
        }

        // verification
        if gain != 0 {
            log::debug!("unable to achieve requested gain {orig_gain} (missed by {gain})\n");
            log::debug!(
                "gain={orig_gain} -> 1xvga1={rxvga1} lna={lna} rxvga2={rxvga2} remainder={gain}\n"
            );
        }

        // that should do it. actually apply the changes:
        self.lms.lna_set_gain(GainDb { db: lna })?; // Self::_convert_gain_to_lna_gain(lna)
        // __scale_int(&rxvga1_range, rxvga1 as f32)
        self.lms.rxvga1_set_gain(GainDb {
            db: rxvga1 / rxvga1_range.scale().unwrap() as i8,
        })?;
        // __scale_int(&rxvga2_range, rxvga2 as f32)
        self.lms.rxvga2_set_gain(GainDb {
            db: rxvga2 / rxvga2_range.scale().unwrap() as i8,
        })
    }
}
