//! Gain control for BladeRF1.
//!
//! Provides manual gain control with apportioning across the LMS6002D's
//! internal amplifier stages (LNA, RXVGA1, RXVGA2 for RX; TXVGA1, TXVGA2
//! for TX). The apportionment algorithm follows the LMS6002D programming
//! guide to distribute gain optimally across stages.
//!
//! Supports two RX gain modes: Default (AGC) and Mgc (manual gain control).
//! TX channel does not support gain modes.

use crate::bladerf1::board::RfLinkSession;
use crate::bladerf1::hardware::lms6002d::gain::{
    BLADERF1_RX_GAIN_OFFSET, BLADERF1_TX_GAIN_OFFSET, GAIN_SPEC_LNA, GAIN_SPEC_RXVGA1,
    GAIN_SPEC_RXVGA2, GAIN_SPEC_TXVGA1, GAIN_SPEC_TXVGA2, GainDb, GainStage,
};
use crate::channel::Channel;
use crate::error::{Error, Result};
use crate::range::{Range, RangeItem};

/// GPIO bit that enables automatic gain control on the RX channel.
pub const BLADERF_GPIO_AGC_ENABLE: u32 = 1 << 18;

/// Gain control mode for the RX channel.
#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub enum GainMode {
    /// Automatic gain control. The LMS6002D AGC adjusts gain based on
    /// the input signal level.
    Default,
    /// Manual gain control. Gain is set explicitly via `set_gain()` and
    /// remains fixed until changed.
    Mgc,
}

impl RfLinkSession<'_> {
    fn _apportion_gain(stage_gain_range: &Range, stage_gain: i8, gain: i8) -> Result<(i8, i8)> {
        let stage_max_gain =
            (stage_gain_range.scale_checked()? * stage_gain_range.max_checked()?).round() as i8;
        let headroom = (stage_max_gain - stage_gain).abs();
        let mut allotment = gain.min(headroom);
        allotment -= allotment % (stage_gain_range.step_checked()? as i8);
        Ok((stage_gain + allotment, gain - allotment))
    }
    /// Returns the supported gain range for the given channel.
    ///
    /// RX range includes LNA + RXVGA1 + RXVGA2 stages. TX range includes
    /// TXVGA1 + TXVGA2 stages. The range accounts for the hardware-specific
    /// gain offset applied by the board.
    pub fn get_gain_range(channel: Channel) -> Range {
        if channel.is_tx() {
            Range::new(vec![RangeItem::Step(
                GAIN_SPEC_TXVGA1.min as f64
                    + GAIN_SPEC_TXVGA2.min as f64
                    + BLADERF1_TX_GAIN_OFFSET as f64,
                GAIN_SPEC_TXVGA1.max as f64
                    + GAIN_SPEC_TXVGA2.max as f64
                    + BLADERF1_TX_GAIN_OFFSET as f64,
                1f64,
                1f64,
            )])
        } else {
            Range::new(vec![RangeItem::Step(
                GAIN_SPEC_RXVGA1.min as f64
                    + GAIN_SPEC_RXVGA2.min as f64
                    + BLADERF1_RX_GAIN_OFFSET as f64,
                GAIN_SPEC_LNA.max as f64
                    + GAIN_SPEC_RXVGA1.max as f64
                    + GAIN_SPEC_RXVGA2.max as f64
                    + BLADERF1_RX_GAIN_OFFSET as f64,
                1f64,
                1f64,
            )])
        }
    }
    /// Returns the available gain modes for the given channel.
    ///
    /// Only the RX channel supports gain modes. Calling with TX returns
    /// `Error::Unsupported`.
    pub fn get_gain_modes(&self, channel: Channel) -> Result<Vec<GainMode>> {
        if channel.is_tx() {
            log::error!("TX does not support gain modes");
            Err(Error::Unsupported("TX gain modes"))
        } else {
            Ok(vec![GainMode::Mgc, GainMode::Default])
        }
    }
    /// Sets the gain mode for the given channel.
    ///
    /// Only the RX channel supports gain modes. Calling with TX returns
    /// `Error::Unsupported`. Toggles the AGC enable bit in the config GPIO.
    ///
    /// Returns `Error::NotInitialized` if the board has not been initialized.
    pub fn set_gain_mode(&mut self, channel: Channel, mode: GainMode) -> Result<()> {
        self.require_initialized()?;
        if channel.is_tx() {
            log::error!("Setting gain mode for TX is not supported");
            return Err(Error::Unsupported("TX gain modes"));
        }
        self.config_gpio_modify(|gpio| match mode {
            GainMode::Default => gpio | BLADERF_GPIO_AGC_ENABLE,
            GainMode::Mgc => gpio & !BLADERF_GPIO_AGC_ENABLE,
        })
    }
    /// Returns the current RX gain mode by reading the AGC enable bit from config GPIO.
    ///
    /// Returns `Error::NotInitialized` if the board has not been initialized.
    pub fn get_gain_mode(&mut self) -> Result<GainMode> {
        self.require_initialized()?;
        let data = self.config_gpio_read()?;
        let gain_mode = if (data & BLADERF_GPIO_AGC_ENABLE) != 0 {
            GainMode::Default
        } else {
            GainMode::Mgc
        };
        Ok(gain_mode)
    }
    /// Returns the current gain of an individual amplifier stage in dB.
    ///
    /// Returns `Error::NotInitialized` if the board has not been initialized.
    pub fn get_gain_stage(&mut self, stage: GainStage) -> Result<GainDb> {
        self.require_initialized()?;
        match stage {
            GainStage::TxVga1 => self.lms().txvga1_get_gain(),
            GainStage::TxVga2 => self.lms().txvga2_get_gain(),
            GainStage::Lna => self.lms().lna_get_gain(),
            GainStage::RxVga1 => self.lms().rxvga1_get_gain(),
            GainStage::RxVga2 => self.lms().rxvga2_get_gain(),
        }
    }
    /// Sets the gain of an individual amplifier stage.
    ///
    /// Use `set_gain()` for automatic apportioning across stages.
    /// Direct stage control is available for fine tuning.
    ///
    /// Returns `Error::NotInitialized` if the board has not been initialized.
    pub fn set_gain_stage(&mut self, stage: GainStage, gain: GainDb) -> Result<()> {
        self.require_initialized()?;
        match stage {
            GainStage::TxVga1 => self.lms().txvga1_set_gain(gain),
            GainStage::TxVga2 => self.lms().txvga2_set_gain(gain),
            GainStage::RxVga1 => self.lms().rxvga1_set_gain(gain),
            GainStage::RxVga2 => self.lms().rxvga2_set_gain(gain),
            GainStage::Lna => self.lms().lna_set_gain(gain),
        }
    }
    /// Returns the ordered list of amplifier stages for the given channel.
    ///
    /// RX: [LNA, RXVGA1, RXVGA2]. TX: [TXVGA1, TXVGA2].
    pub fn get_gain_stages(channel: Channel) -> &'static [GainStage] {
        if channel.is_tx() {
            &[GainStage::TxVga1, GainStage::TxVga2]
        } else {
            &[GainStage::Lna, GainStage::RxVga1, GainStage::RxVga2]
        }
    }
    /// Returns the supported gain range for an individual amplifier stage.
    pub fn get_gain_stage_range(stage: GainStage) -> Range {
        stage.gain_range()
    }
    fn get_tx_gain(&mut self) -> Result<GainDb> {
        self.require_initialized()?;
        let txvga1 = self.lms().txvga1_get_gain()?;
        let txvga2 = self.lms().txvga2_get_gain()?;
        Ok((txvga1.db() + txvga2.db() + BLADERF1_TX_GAIN_OFFSET as i8).into())
    }
    fn get_rx_gain(&mut self) -> Result<GainDb> {
        self.require_initialized()?;
        let lna_gain_db = self.lms().lna_get_gain()?;
        let rxvga1_gain_db = self.lms().rxvga1_get_gain()?;
        let rxvga2_gain_db = self.lms().rxvga2_get_gain()?;
        Ok((lna_gain_db.db()
            + rxvga1_gain_db.db()
            + rxvga2_gain_db.db()
            + BLADERF1_RX_GAIN_OFFSET as i8)
            .into())
    }
    /// Returns the current aggregate gain of the given channel in dB.
    ///
    /// Sums all amplifier stages (LNA + RXVGA1 + RXVGA2 for RX,
    /// TXVGA1 + TXVGA2 for TX) along with the board gain offset.
    ///
    /// Returns `Error::NotInitialized` if the board has not been initialized.
    pub fn get_gain(&mut self, channel: Channel) -> Result<GainDb> {
        self.require_initialized()?;
        if channel.is_tx() {
            self.get_tx_gain()
        } else {
            self.get_rx_gain()
        }
    }
    /// Sets the aggregate gain for the given channel.
    ///
    /// Distributes the requested gain across the available amplifier stages
    /// using an apportionment algorithm from the LMS6002D programming guide.
    /// If the exact gain cannot be achieved, the closest achievable value
    /// is set with a debug log message.
    ///
    /// Returns `Error::NotInitialized` if the board has not been initialized.
    pub fn set_gain(&mut self, channel: Channel, gain: GainDb) -> Result<()> {
        self.require_initialized()?;
        if channel.is_tx() {
            self.set_tx_gain(gain)
        } else {
            self.set_rx_gain(gain)
        }
    }
    /// Sets the TX aggregate gain by apportioning across TXVGA1 and TXVGA2.
    ///
    /// Begins with both stages at minimum, then distributes remaining gain
    /// greedily in stage order (TXVGA2 first, then TXVGA1).
    ///
    /// Returns `Error::NotInitialized` if the board has not been initialized.
    pub fn set_tx_gain(&mut self, gain_db: GainDb) -> Result<()> {
        self.require_initialized()?;
        let desired_gain = gain_db.db();
        let txvga1_range = Self::get_gain_stage_range(GainStage::TxVga1);
        let txvga2_range = Self::get_gain_stage_range(GainStage::TxVga2);
        let mut txvga1 =
            (txvga1_range.scale_checked()? * txvga1_range.min_checked()?).round() as i8;
        let mut txvga2 =
            (txvga2_range.scale_checked()? * txvga2_range.min_checked()?).round() as i8;
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
        self.lms().txvga1_set_gain(txvga1.into())?;
        self.lms().txvga2_set_gain(txvga2.into())
    }
    /// Sets the RX aggregate gain by apportioning across LNA, RXVGA1, and RXVGA2.
    ///
    /// Begins with all stages at minimum, then distributes remaining gain using
    /// a multi-pass algorithm that clamps the LNA to half its maximum and
    /// adjusts RXVGA1 headroom for RXVGA2 as needed.
    ///
    /// Returns `Error::NotInitialized` if the board has not been initialized.
    pub fn set_rx_gain(&mut self, gain_db: GainDb) -> Result<()> {
        self.require_initialized()?;
        let desired_gain = gain_db.db();
        let lna_range = Self::get_gain_stage_range(GainStage::Lna);
        let rxvga1_range = Self::get_gain_stage_range(GainStage::RxVga1);
        let rxvga2_range = Self::get_gain_stage_range(GainStage::RxVga2);
        let mut lna = (lna_range.scale_checked()? * lna_range.min_checked()?).round() as i8;
        let mut rxvga1 =
            (rxvga1_range.scale_checked()? * rxvga1_range.min_checked()?).round() as i8;
        let mut rxvga2 =
            (rxvga2_range.scale_checked()? * rxvga2_range.min_checked()?).round() as i8;
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
        let rxvga1_max =
            (rxvga1_range.scale_checked()? * rxvga1_range.max_checked()?).round() as i8;
        let rxvga2_step =
            (rxvga2_range.scale_checked()? * rxvga2_range.step_checked()?).round() as i8;
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
        self.lms().lna_set_gain(lna.into())?;
        self.lms().rxvga1_set_gain(rxvga1.into())?;
        self.lms().rxvga2_set_gain(rxvga2.into())
    }
}
