use crate::bladerf1::BladeRf1;
use crate::hardware::lms6002d::{
    BLADERF_LNA_GAIN_MAX_DB, BLADERF_LNA_GAIN_MID_DB, BLADERF_RXVGA1_GAIN_MAX,
    BLADERF_RXVGA1_GAIN_MIN, BLADERF_RXVGA2_GAIN_MAX, BLADERF_RXVGA2_GAIN_MIN,
    BLADERF_TXVGA1_GAIN_MAX, BLADERF_TXVGA1_GAIN_MIN, BLADERF_TXVGA2_GAIN_MAX,
    BLADERF_TXVGA2_GAIN_MIN, BLADERF1_RX_GAIN_OFFSET, BLADERF1_TX_GAIN_OFFSET, GainDb, GainMode,
};
use crate::range::{Range, RangeItem};
use crate::{Channel, Error, Result};

/// AGC enable control bit
///
/// @note This is set using bladerf_set_gain_mode().
pub const BLADERF_GPIO_AGC_ENABLE: u32 = 1 << 18;

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
    /// Returns the new suggested gain for a stage and the remaining gain, that is
    /// still left to be assigned to another stage / could not be assigned to this stage
    ///
    /// @param\[in\]  stage_gain_range   The range for stage_gain
    /// @param      stage_gain           The stage gain
    /// @param      gain                 The gain that should be assigned to the stage
    fn _apportion_gain(stage_gain_range: &Range, stage_gain: i8, gain: i8) -> (i8, i8) {
        // overall_gain is the gain that is left to be distributed on the selected stage
        // if not the whole overall_gain can be assigned to the stage, the remaining gain is returned
        let stage_max_gain =
            (stage_gain_range.scale().unwrap() * stage_gain_range.max().unwrap()).round() as i8;
        // headroom is the amount of gain space we have left to increase our gain
        let headroom = (stage_max_gain - stage_gain).abs();
        // allotment is the available gain amount that can be assigned to a stage
        let mut allotment = gain.min(headroom);
        // log::error!("headroom: {headroom}, allotment: {allotment}");

        // Enforce step size
        allotment -= allotment % (stage_gain_range.step().unwrap() as i8);
        // while 0 != allotment % (stage_gain_range.step().unwrap() as i8) {
        //     allotment -= 1;
        // }

        // Assign the allotment to the gain_stage and return
        // the remaining gain, that yet has to be apportioned!
        (stage_gain + allotment, gain - allotment)
    }

    /// Get the supported absolute gain range on a specific channel.
    ///The absolute gain is composed of the individual gains of specific stages, namely
    /// TXVGA1 and TXVGA2 for TX and RXVGA1, RXVGA2 and LNA for RX.
    pub fn get_gain_range(channel: Channel) -> Range {
        if channel.is_tx() {
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

    /// Get the supported gain modes of the selected channel.
    /// Supported are `GainMode::Mgc` and `GainMode::Default`.
    pub fn get_gain_modes(&self, channel: Channel) -> Result<Vec<GainMode>> {
        if channel.is_tx() {
            log::error!("TX does not support gain modes");
            Err(Error::Invalid)
        } else {
            Ok(vec![GainMode::Mgc, GainMode::Default])
        }
    }

    /// Set the desired gain mode to one of the options provided by `GainMode`.
    /// Supported are `GainMode::Mgc` and `GainMode::Default`.
    pub fn set_gain_mode(&self, channel: Channel, mode: GainMode) -> Result<()> {
        if channel.is_tx() {
            log::error!("Setting gain mode for TX is not supported");
            return Err(Error::Invalid);
        }

        let mut config_gpio = self.config_gpio_read()?;
        if mode == GainMode::Default {
            // TODO:
            // Default mode is the same as Automatic mode
            // return Err(anyhow!("Todo: Implement AGC Table"));
            // if (!have_cap(board_data->capabilities, BLADERF_CAP_AGC_DC_LUT)) {
            //     log_warning("AGC not supported by FPGA. %s", MGC_WARN);
            //     log_info("To enable AGC, %s, then %s", FPGA_STR, DCCAL_STR);
            //     log_debug("%s: expected FPGA >= v0.7.0, got v%u.%u.%u",
            //               __FUNCTION__, board_data->fpga_version.major,
            //               board_data->fpga_version.minor,
            //               board_data->fpga_version.patch);
            //     return BLADERF_ERR_UNSUPPORTED;
            // }
            //
            // if (!board_data->cal.dc_rx) {
            //     log_warning("RX DC calibration table not found. %s", MGC_WARN);
            //     log_info("To enable AGC, %s", DCCAL_STR);
            //     return BLADERF_ERR_UNSUPPORTED;
            // }
            //
            // if (board_data->cal.dc_rx->version != TABLE_VERSION) {
            //     log_warning("RX DC calibration table is out-of-date. %s",
            //                 MGC_WARN);
            //     log_info("To enable AGC, %s", DCCAL_STR);
            //     log_debug("%s: expected version %u, got %u", __FUNCTION__,
            //               TABLE_VERSION, board_data->cal.dc_rx->version);
            //
            //     return BLADERF_ERR_UNSUPPORTED;
            // }
            config_gpio |= BLADERF_GPIO_AGC_ENABLE;
        } else if mode == GainMode::Mgc {
            config_gpio &= !BLADERF_GPIO_AGC_ENABLE;
        }

        self.config_gpio_write(config_gpio)
    }

    /// Get the current gain mode of the BladeRF1.
    pub fn get_gain_mode(&self) -> Result<GainMode> {
        let data = self.config_gpio_read()?;

        let gain_mode = if data & BLADERF_GPIO_AGC_ENABLE != 0 {
            GainMode::Default
        } else {
            GainMode::Mgc
        };
        Ok(gain_mode)
    }

    // TODO: "stage" parameter could be enum instead of string.
    // TODO: "stage" indirectly determines the channel, thus we could scratch the channel parameter.
    /// Retrieve the gain of an individual stage. The absolute gain is composed of the gain of the individual stages.
    /// Supported stages are "txvga1" and "txvga2" for TX and "rxvga1", "rxvga2" and "lna" for RX.
    pub fn get_gain_stage(&self, channel: Channel, stage: &str) -> Result<GainDb> {
        // CHECK_BOARD_STATE(STATE_INITIALIZED);
        match channel {
            Channel::Tx => match stage {
                "txvga1" => self.lms.txvga1_get_gain(),
                "txvga2" => self.lms.txvga2_get_gain(),
                _ => {
                    log::error!("invalid stage {stage}");
                    Err(Error::Invalid)
                }
            },
            Channel::Rx => match stage {
                "lna" => self.lms.lna_get_gain(),
                "rxvga1" => self.lms.rxvga1_get_gain(),
                "rxvga2" => self.lms.rxvga2_get_gain(),
                _ => {
                    log::error!("invalid stage {stage}");
                    Err(Error::Invalid)
                }
            },
        }
    }

    /// Set the gain of a specific stage on a specific channel.
    /// Supported stages are "txvga1" and "txvga2" for TX and "rxvga1", "rxvga2" and "lna" for RX.
    pub fn set_gain_stage(&self, channel: Channel, stage: &str, gain: GainDb) -> Result<()> {
        // CHECK_BOARD_STATE(STATE_INITIALIZED);

        // TODO implement gain clamping
        match channel {
            Channel::Tx => match stage {
                "txvga1" => Ok(self.lms.txvga1_set_gain(gain)?),
                "txvga2" => Ok(self.lms.txvga2_set_gain(gain)?),
                _ => {
                    log::error!("invalid stage {stage}");
                    Err(Error::Invalid)
                }
            },
            Channel::Rx => match stage {
                "rxvga1" => Ok(self.lms.rxvga1_set_gain(gain)?),
                "rxvga2" => Ok(self.lms.rxvga2_set_gain(gain)?),
                "lna" => Ok(self.lms.lna_set_gain(gain)?), // Self::_convert_gain_to_lna_gain(gain)
                _ => {
                    log::error!("invalid stage {stage}");
                    Err(Error::Invalid)
                }
            },
        }
    }

    /// Returns the names of supported gain stages of a channel.
    pub fn get_gain_stages(channel: Channel) -> Vec<String> {
        if channel.is_tx() {
            vec!["txvga1".to_string(), "txvga2".to_string()]
        } else {
            vec![
                "lna".to_string(),
                "rxvga1".to_string(),
                "rxvga2".to_string(),
            ]
        }
    }

    /// Retrieve the supported gain values of a specific stage on a specific channel.
    /// Use `bladerf_get_gain_range()`, `bladerf_set_gain()`, and
    /// `bladerf_get_gain()` to control total system gain. For direct
    /// control of individual gain stages, use `bladerf_get_gain_stages()`,
    /// `bladerf_get_gain_stage_range()`, `bladerf_set_gain_stage()`, and
    /// `bladerf_get_gain_stage()`.
    pub fn get_gain_stage_range(channel: Channel, stage: &str) -> Result<Range> {
        match channel {
            Channel::Rx => match stage {
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
                        3f64,
                        1f64,
                    )],
                }),
                _ => {
                    log::error!("invalid stage {stage}");
                    Err(Error::Invalid)
                }
            },
            Channel::Tx => match stage {
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
            },
        }
    }

    /// Get the absolute gain of the TX channel.
    fn get_tx_gain(&self) -> Result<GainDb> {
        let txvga1 = self.lms.txvga1_get_gain()?;
        let txvga2 = self.lms.txvga2_get_gain()?;

        Ok(GainDb {
            db: txvga1.db + txvga2.db + BLADERF1_TX_GAIN_OFFSET as i8,
        })
    }

    /// Get the absolute gain of the RX channel.
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

    /// Get the absolute gain of the specified channel.
    pub fn get_gain(&self, channel: Channel) -> Result<GainDb> {
        // CHECK_BOARD_STATE(STATE_INITIALIZED);

        if channel.is_tx() {
            self.get_tx_gain()
        } else {
            self.get_rx_gain()
        }
    }

    /// Set the absolute gain on a specific channel.
    pub fn set_gain(&self, channel: Channel, gain: GainDb) -> Result<()> {
        // CHECK_BOARD_STATE(STATE_INITIALIZED);

        if channel.is_tx() {
            self.set_tx_gain(gain)
        } else {
            self.set_rx_gain(gain)
        }
    }

    /// Set the absolute gain on the TX channel.
    pub fn set_tx_gain(&self, gain_db: GainDb) -> Result<()> {
        let desired_gain = gain_db.db;

        let txvga1_range = Self::get_gain_stage_range(Channel::Tx, "txvga1")?;
        let txvga2_range = Self::get_gain_stage_range(Channel::Tx, "txvga2")?;

        let mut txvga1 =
            (txvga1_range.scale().unwrap() * txvga1_range.min().unwrap()).round() as i8;
        let mut txvga2 =
            (txvga2_range.scale().unwrap() * txvga2_range.min().unwrap()).round() as i8;

        // offset gain so that we can use it as a counter when apportioning gain
        // This is a relative gain value with the minimal possible gain substracted.
        let mut gain = desired_gain - (BLADERF1_TX_GAIN_OFFSET as i8 + txvga1 + txvga2);
        log::trace!("gain={desired_gain} -> txvga2={txvga2} txvga1={txvga1} remainder={gain}");

        // apportion gain to TXVGA2
        (txvga2, gain) = Self::_apportion_gain(&txvga2_range, txvga2, gain);
        log::trace!("gain={desired_gain} -> txvga2={txvga2} txvga1={txvga1} remainder={gain}");

        // apportion gain to TXVGA1
        (txvga1, gain) = Self::_apportion_gain(&txvga1_range, txvga1, gain);
        log::trace!("gain={desired_gain} -> txvga2={txvga2} txvga1={txvga1} remainder={gain}");

        // verification
        if gain != 0 {
            log::debug!("unable to achieve requested gain {desired_gain} (missed by {gain})");
            log::debug!("gain={desired_gain} -> txvga2={txvga2} txvga1={txvga1} remainder={gain}");
        }

        self.lms.txvga1_set_gain(GainDb { db: txvga1 })?;
        self.lms.txvga2_set_gain(GainDb { db: txvga2 })
    }

    /// Set the absolute gain on the RX channel.
    pub fn set_rx_gain(&self, gain_db: GainDb) -> Result<()> {
        let desired_gain = gain_db.db;

        let lna_range = Self::get_gain_stage_range(Channel::Rx, "lna")?;
        let rxvga1_range = Self::get_gain_stage_range(Channel::Rx, "rxvga1")?;
        let rxvga2_range = Self::get_gain_stage_range(Channel::Rx, "rxvga2")?;

        // Start with the minimum gain for each stage.
        let mut lna = (lna_range.scale().unwrap() * lna_range.min().unwrap()).round() as i8;
        let mut rxvga1 =
            (rxvga1_range.scale().unwrap() * rxvga1_range.min().unwrap()).round() as i8;
        let mut rxvga2 =
            (rxvga2_range.scale().unwrap() * rxvga2_range.min().unwrap()).round() as i8;

        // offset gain so that we can use it as a counter when apportioning gain
        let mut gain = desired_gain - (BLADERF1_RX_GAIN_OFFSET as i8 + lna + rxvga1 + rxvga2);
        log::trace!(
            "gain={desired_gain} -> lna={lna} rxvga1={rxvga1} rxvga2={rxvga2} remainder={gain}"
        );

        // apportion some gain to RXLNA (but only half of it for now)
        (lna, gain) = Self::_apportion_gain(&lna_range, lna, gain);
        if lna > BLADERF_LNA_GAIN_MID_DB {
            gain += lna - BLADERF_LNA_GAIN_MID_DB;
            lna = lna - (lna - BLADERF_LNA_GAIN_MID_DB);
        }
        log::trace!(
            "gain={desired_gain} -> lna={lna} rxvga1={rxvga1} rxvga2={rxvga2} remainder={gain}"
        );

        // apportion gain to RXVGA1
        (rxvga1, gain) = Self::_apportion_gain(&rxvga1_range, rxvga1, gain);
        log::trace!(
            "gain={desired_gain} -> lna={lna} rxvga1={rxvga1} rxvga2={rxvga2} remainder={gain}"
        );

        // apportion more gain to RXLNA
        (lna, gain) = Self::_apportion_gain(&lna_range, lna, gain);
        log::trace!(
            "gain={desired_gain} -> lna={lna} rxvga1={rxvga1} rxvga2={rxvga2} remainder={gain}"
        );

        // apportion gain to RXVGA2
        (rxvga2, gain) = Self::_apportion_gain(&rxvga2_range, rxvga2, gain);
        log::trace!(
            "gain={desired_gain} -> lna={lna} rxvga1={rxvga1} rxvga2={rxvga2} remainder={gain}"
        );

        // if we still have remaining gain, it's because rxvga2 has a step size of
        // 3 dB. Steal a few dB from rxvga1...
        let rxvga1_max =
            (rxvga1_range.scale().unwrap() * rxvga1_range.max().unwrap()).round() as i8;
        let rxvga2_step =
            (rxvga2_range.scale().unwrap() * rxvga2_range.step().unwrap()).round() as i8;

        if gain > 0 && rxvga1 >= rxvga1_max {
            rxvga1 -= rxvga2_step;
            gain += rxvga2_step;

            (rxvga2, gain) = Self::_apportion_gain(&rxvga2_range, rxvga2, gain);
            (rxvga1, gain) = Self::_apportion_gain(&rxvga1_range, rxvga1, gain);
        }
        log::trace!(
            "gain={desired_gain} -> lna={lna} rxvga1={rxvga1} rxvga2={rxvga2} remainder={gain}"
        );

        // verification
        if gain != 0 {
            log::debug!("unable to achieve requested gain {desired_gain} (missed by {gain})");
            log::debug!(
                "gain={desired_gain} -> lna={lna} rxvga1={rxvga1} rxvga2={rxvga2} remainder={gain}"
            );
        }

        // that should do it. actually apply the changes:
        self.lms.lna_set_gain(GainDb { db: lna })?;
        self.lms.rxvga1_set_gain(GainDb { db: rxvga1 })?;
        self.lms.rxvga2_set_gain(GainDb { db: rxvga2 })
    }
}
