use crate::bladerf1::BladeRf1;
use crate::bladerf1::hardware::lms6002d;
use crate::bladerf1::hardware::lms6002d::gain::{
    BLADERF1_RX_GAIN_OFFSET, BLADERF1_TX_GAIN_OFFSET, GAIN_SPEC_LNA, GAIN_SPEC_RXVGA1,
    GAIN_SPEC_RXVGA2, GAIN_SPEC_TXVGA1, GAIN_SPEC_TXVGA2, GainDb, GainStage,
};
use crate::channel::Channel;
use crate::error::{Error, Result};
use crate::range::{Range, RangeItem};

pub const BLADERF_GPIO_AGC_ENABLE: u32 = 1 << 18;

#[derive(PartialEq, Debug, Clone, Copy)]
pub enum GainMode {
    Default,
    Mgc,
}

fn range_step(range: &Range) -> Result<f64> {
    range
        .step()
        .ok_or(Error::HardwareState("gain range missing step"))
}

fn range_scale(range: &Range) -> Result<f64> {
    range
        .scale()
        .ok_or(Error::HardwareState("gain range missing scale"))
}

fn range_min(range: &Range) -> Result<f64> {
    range
        .min()
        .ok_or(Error::HardwareState("gain range missing min"))
}

fn range_max(range: &Range) -> Result<f64> {
    range
        .max()
        .ok_or(Error::HardwareState("gain range missing max"))
}

impl BladeRf1 {
    fn _apportion_gain(stage_gain_range: &Range, stage_gain: i8, gain: i8) -> Result<(i8, i8)> {
        let stage_max_gain =
            (range_scale(stage_gain_range)? * range_max(stage_gain_range)?).round() as i8;
        let headroom = (stage_max_gain - stage_gain).abs();
        let mut allotment = gain.min(headroom);
        allotment -= allotment % (range_step(stage_gain_range)? as i8);
        Ok((stage_gain + allotment, gain - allotment))
    }
    pub fn get_gain_range(channel: Channel) -> Range {
        if channel.is_tx() {
            Range {
                items: vec![RangeItem::Step(
                    GAIN_SPEC_TXVGA1.min as f64
                        + GAIN_SPEC_TXVGA2.min as f64
                        + BLADERF1_TX_GAIN_OFFSET as f64,
                    GAIN_SPEC_TXVGA1.max as f64
                        + GAIN_SPEC_TXVGA2.max as f64
                        + BLADERF1_TX_GAIN_OFFSET as f64,
                    1f64,
                    1f64,
                )],
            }
        } else {
            Range {
                items: vec![RangeItem::Step(
                    GAIN_SPEC_RXVGA1.min as f64
                        + GAIN_SPEC_RXVGA2.min as f64
                        + BLADERF1_RX_GAIN_OFFSET as f64,
                    GAIN_SPEC_LNA.max as f64
                        + GAIN_SPEC_RXVGA1.max as f64
                        + GAIN_SPEC_RXVGA2.max as f64
                        + BLADERF1_RX_GAIN_OFFSET as f64,
                    1f64,
                    1f64,
                )],
            }
        }
    }
    pub fn get_gain_modes(&self, channel: Channel) -> Result<Vec<GainMode>> {
        if channel.is_tx() {
            log::error!("TX does not support gain modes");
            Err(Error::Unsupported("TX gain modes"))
        } else {
            Ok(vec![GainMode::Mgc, GainMode::Default])
        }
    }
    pub fn set_gain_mode(&mut self, channel: Channel, mode: GainMode) -> Result<()> {
        if channel.is_tx() {
            log::error!("Setting gain mode for TX is not supported");
            return Err(Error::Unsupported("TX gain modes"));
        }
        let mut config_gpio = self.config_gpio_read()?;
        if mode == GainMode::Default {
            config_gpio |= BLADERF_GPIO_AGC_ENABLE;
        } else if mode == GainMode::Mgc {
            config_gpio &= !BLADERF_GPIO_AGC_ENABLE;
        }
        self.config_gpio_write(config_gpio)
    }
    pub fn get_gain_mode(&mut self) -> Result<GainMode> {
        let data = self.config_gpio_read()?;
        let gain_mode = if data & BLADERF_GPIO_AGC_ENABLE != 0 {
            GainMode::Default
        } else {
            GainMode::Mgc
        };
        Ok(gain_mode)
    }
    pub fn get_gain_stage(&mut self, stage: GainStage) -> Result<GainDb> {
        match stage {
            GainStage::TxVga1 => lms6002d::gain::txvga1_get_gain(&mut self.nios),
            GainStage::TxVga2 => lms6002d::gain::txvga2_get_gain(&mut self.nios),
            GainStage::Lna => lms6002d::gain::lna_get_gain(&mut self.nios),
            GainStage::RxVga1 => lms6002d::gain::rxvga1_get_gain(&mut self.nios),
            GainStage::RxVga2 => lms6002d::gain::rxvga2_get_gain(&mut self.nios),
        }
    }
    pub fn set_gain_stage(&mut self, stage: GainStage, gain: GainDb) -> Result<()> {
        match stage {
            GainStage::TxVga1 => lms6002d::gain::txvga1_set_gain(&mut self.nios, gain),
            GainStage::TxVga2 => lms6002d::gain::txvga2_set_gain(&mut self.nios, gain),
            GainStage::RxVga1 => lms6002d::gain::rxvga1_set_gain(&mut self.nios, gain),
            GainStage::RxVga2 => lms6002d::gain::rxvga2_set_gain(&mut self.nios, gain),
            GainStage::Lna => lms6002d::gain::lna_set_gain(&mut self.nios, gain),
        }
    }
    pub fn get_gain_stages(channel: Channel) -> &'static [GainStage] {
        if channel.is_tx() {
            &[GainStage::TxVga1, GainStage::TxVga2]
        } else {
            &[GainStage::Lna, GainStage::RxVga1, GainStage::RxVga2]
        }
    }
    pub fn get_gain_stage_range(stage: GainStage) -> Range {
        lms6002d::gain::get_gain_stage_range(stage)
    }
    fn get_tx_gain(&mut self) -> Result<GainDb> {
        let txvga1 = lms6002d::gain::txvga1_get_gain(&mut self.nios)?;
        let txvga2 = lms6002d::gain::txvga2_get_gain(&mut self.nios)?;
        Ok(GainDb {
            db: txvga1.db + txvga2.db + BLADERF1_TX_GAIN_OFFSET as i8,
        })
    }
    fn get_rx_gain(&mut self) -> Result<GainDb> {
        let lna_gain_db = lms6002d::gain::lna_get_gain(&mut self.nios)?;
        let rxvga1_gain_db = lms6002d::gain::rxvga1_get_gain(&mut self.nios)?;
        let rxvga2_gain_db = lms6002d::gain::rxvga2_get_gain(&mut self.nios)?;
        Ok(GainDb {
            db: lna_gain_db.db
                + rxvga1_gain_db.db
                + rxvga2_gain_db.db
                + BLADERF1_RX_GAIN_OFFSET as i8,
        })
    }
    pub fn get_gain(&mut self, channel: Channel) -> Result<GainDb> {
        if channel.is_tx() {
            self.get_tx_gain()
        } else {
            self.get_rx_gain()
        }
    }
    pub fn set_gain(&mut self, channel: Channel, gain: GainDb) -> Result<()> {
        if channel.is_tx() {
            self.set_tx_gain(gain)
        } else {
            self.set_rx_gain(gain)
        }
    }
    pub fn set_tx_gain(&mut self, gain_db: GainDb) -> Result<()> {
        let desired_gain = gain_db.db;
        let txvga1_range = Self::get_gain_stage_range(GainStage::TxVga1);
        let txvga2_range = Self::get_gain_stage_range(GainStage::TxVga2);
        let mut txvga1 = (range_scale(&txvga1_range)? * range_min(&txvga1_range)?).round() as i8;
        let mut txvga2 = (range_scale(&txvga2_range)? * range_min(&txvga2_range)?).round() as i8;
        let mut gain = desired_gain - (BLADERF1_TX_GAIN_OFFSET as i8 + txvga1 + txvga2);
        log::trace!("gain={desired_gain} -> txvga2={txvga2} txvga1={txvga1} remainder={gain}");
        (txvga2, gain) = Self::_apportion_gain(&txvga2_range, txvga2, gain)?;
        log::trace!("gain={desired_gain} -> txvga2={txvga2} txvga1={txvga1} remainder={gain}");
        (txvga1, gain) = Self::_apportion_gain(&txvga1_range, txvga1, gain)?;
        log::trace!("gain={desired_gain} -> txvga2={txvga2} txvga1={txvga1} remainder={gain}");
        if gain != 0 {
            log::debug!("unable to achieve requested gain {desired_gain} (missed by {gain})");
            log::debug!("gain={desired_gain} -> txvga2={txvga2} txvga1={txvga1} remainder={gain}");
        }
        lms6002d::gain::txvga1_set_gain(&mut self.nios, txvga1.into())?;
        lms6002d::gain::txvga2_set_gain(&mut self.nios, txvga2.into())
    }
    pub fn set_rx_gain(&mut self, gain_db: GainDb) -> Result<()> {
        let desired_gain = gain_db.db;
        let lna_range = Self::get_gain_stage_range(GainStage::Lna);
        let rxvga1_range = Self::get_gain_stage_range(GainStage::RxVga1);
        let rxvga2_range = Self::get_gain_stage_range(GainStage::RxVga2);
        let mut lna = (range_scale(&lna_range)? * range_min(&lna_range)?).round() as i8;
        let mut rxvga1 = (range_scale(&rxvga1_range)? * range_min(&rxvga1_range)?).round() as i8;
        let mut rxvga2 = (range_scale(&rxvga2_range)? * range_min(&rxvga2_range)?).round() as i8;
        let mut gain = desired_gain - (BLADERF1_RX_GAIN_OFFSET as i8 + lna + rxvga1 + rxvga2);
        log::trace!(
            "gain={desired_gain} -> lna={lna} rxvga1={rxvga1} rxvga2={rxvga2} remainder={gain}"
        );
        (lna, gain) = Self::_apportion_gain(&lna_range, lna, gain)?;
        if lna > GAIN_SPEC_LNA.max / 2 {
            gain += lna - GAIN_SPEC_LNA.max / 2;
            lna = lna - (lna - GAIN_SPEC_LNA.max / 2);
        }
        log::trace!(
            "gain={desired_gain} -> lna={lna} rxvga1={rxvga1} rxvga2={rxvga2} remainder={gain}"
        );
        (rxvga1, gain) = Self::_apportion_gain(&rxvga1_range, rxvga1, gain)?;
        log::trace!(
            "gain={desired_gain} -> lna={lna} rxvga1={rxvga1} rxvga2={rxvga2} remainder={gain}"
        );
        (lna, gain) = Self::_apportion_gain(&lna_range, lna, gain)?;
        log::trace!(
            "gain={desired_gain} -> lna={lna} rxvga1={rxvga1} rxvga2={rxvga2} remainder={gain}"
        );
        (rxvga2, gain) = Self::_apportion_gain(&rxvga2_range, rxvga2, gain)?;
        log::trace!(
            "gain={desired_gain} -> lna={lna} rxvga1={rxvga1} rxvga2={rxvga2} remainder={gain}"
        );
        let rxvga1_max = (range_scale(&rxvga1_range)? * range_max(&rxvga1_range)?).round() as i8;
        let rxvga2_step = (range_scale(&rxvga2_range)? * range_step(&rxvga2_range)?).round() as i8;
        if gain > 0 && rxvga1 >= rxvga1_max {
            rxvga1 -= rxvga2_step;
            gain += rxvga2_step;
            (rxvga2, gain) = Self::_apportion_gain(&rxvga2_range, rxvga2, gain)?;
            (rxvga1, gain) = Self::_apportion_gain(&rxvga1_range, rxvga1, gain)?;
        }
        log::trace!(
            "gain={desired_gain} -> lna={lna} rxvga1={rxvga1} rxvga2={rxvga2} remainder={gain}"
        );
        if gain != 0 {
            log::debug!("unable to achieve requested gain {desired_gain} (missed by {gain})");
            log::debug!(
                "gain={desired_gain} -> lna={lna} rxvga1={rxvga1} rxvga2={rxvga2} remainder={gain}"
            );
        }
        lms6002d::gain::lna_set_gain(&mut self.nios, lna.into())?;
        lms6002d::gain::rxvga1_set_gain(&mut self.nios, rxvga1.into())?;
        lms6002d::gain::rxvga2_set_gain(&mut self.nios, rxvga2.into())
    }
}
