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
use crate::maybe_future::Op;
use crate::range::{Range, RangeItem};
use nusb::MaybeFuture;

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
    pub fn set_gain_mode(
        &mut self,
        channel: Channel,
        mode: GainMode,
    ) -> impl MaybeFuture<Output = Result<()>> {
        Op::new(async move {
            self.require_initialized().await?;
            if channel.is_tx() {
                log::error!("Setting gain mode for TX is not supported");
                return Err(Error::Unsupported("TX gain modes"));
            }
            self.config_gpio_modify(|gpio| match mode {
                GainMode::Default => gpio | BLADERF_GPIO_AGC_ENABLE,
                GainMode::Mgc => gpio & !BLADERF_GPIO_AGC_ENABLE,
            })
            .await
        })
    }
    /// Returns the current RX gain mode by reading the AGC enable bit from config GPIO.
    ///
    /// Returns `Error::NotInitialized` if the board has not been initialized.
    pub fn get_gain_mode(&mut self) -> impl MaybeFuture<Output = Result<GainMode>> {
        Op::new(async move {
            self.require_initialized().await?;
            let data = self.config_gpio_read().await?;
            let gain_mode = if (data & BLADERF_GPIO_AGC_ENABLE) != 0 {
                GainMode::Default
            } else {
                GainMode::Mgc
            };
            Ok(gain_mode)
        })
    }
    /// Returns the current gain of an individual amplifier stage in dB.
    ///
    /// Returns `Error::NotInitialized` if the board has not been initialized.
    pub fn get_gain_stage(
        &mut self,
        stage: GainStage,
    ) -> impl MaybeFuture<Output = Result<GainDb>> {
        Op::new(async move {
            self.require_initialized().await?;
            match stage {
                GainStage::TxVga1 => self.lms().txvga1_get_gain().await,
                GainStage::TxVga2 => self.lms().txvga2_get_gain().await,
                GainStage::Lna => self.lms().lna_get_gain().await,
                GainStage::RxVga1 => self.lms().rxvga1_get_gain().await,
                GainStage::RxVga2 => self.lms().rxvga2_get_gain().await,
            }
        })
    }
    /// Sets the gain of an individual amplifier stage.
    ///
    /// Use `set_gain()` for automatic apportioning across stages.
    /// Direct stage control is available for fine tuning.
    ///
    /// Returns `Error::NotInitialized` if the board has not been initialized.
    pub fn set_gain_stage(
        &mut self,
        stage: GainStage,
        gain: GainDb,
    ) -> impl MaybeFuture<Output = Result<()>> {
        Op::new(async move {
            self.require_initialized().await?;
            match stage {
                GainStage::TxVga1 => self.lms().txvga1_set_gain(gain).await,
                GainStage::TxVga2 => self.lms().txvga2_set_gain(gain).await,
                GainStage::RxVga1 => self.lms().rxvga1_set_gain(gain).await,
                GainStage::RxVga2 => self.lms().rxvga2_set_gain(gain).await,
                GainStage::Lna => self.lms().lna_set_gain(gain).await,
            }
        })
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
    fn get_tx_gain(&mut self) -> impl MaybeFuture<Output = Result<GainDb>> {
        Op::new(async move {
            let txvga1 = self.lms().txvga1_get_gain().await?;
            let txvga2 = self.lms().txvga2_get_gain().await?;
            Ok((txvga1.db() + txvga2.db() + BLADERF1_TX_GAIN_OFFSET).into())
        })
    }
    fn get_rx_gain(&mut self) -> impl MaybeFuture<Output = Result<GainDb>> {
        Op::new(async move {
            let lna_gain_db = self.lms().lna_get_gain().await?;
            let rxvga1_gain_db = self.lms().rxvga1_get_gain().await?;
            let rxvga2_gain_db = self.lms().rxvga2_get_gain().await?;
            Ok((lna_gain_db.db()
                + rxvga1_gain_db.db()
                + rxvga2_gain_db.db()
                + BLADERF1_RX_GAIN_OFFSET)
                .into())
        })
    }
    /// Returns the current aggregate gain of the given channel in dB.
    ///
    /// Sums all amplifier stages (LNA + RXVGA1 + RXVGA2 for RX,
    /// TXVGA1 + TXVGA2 for TX) along with the board gain offset.
    ///
    /// Returns `Error::NotInitialized` if the board has not been initialized.
    pub fn get_gain(&mut self, channel: Channel) -> impl MaybeFuture<Output = Result<GainDb>> {
        Op::new(async move {
            self.require_initialized().await?;
            if channel.is_tx() {
                self.get_tx_gain().await
            } else {
                self.get_rx_gain().await
            }
        })
    }
    /// Sets the aggregate gain for the given channel.
    ///
    /// Distributes the requested gain across the available amplifier stages
    /// using an apportionment algorithm from the LMS6002D programming guide.
    /// Requests outside the channel's supported range are clamped to its bounds.
    ///
    /// Returns `Error::NotInitialized` if the board has not been initialized.
    pub fn set_gain(
        &mut self,
        channel: Channel,
        gain: GainDb,
    ) -> impl MaybeFuture<Output = Result<()>> {
        Op::new(async move {
            if channel.is_tx() {
                self.set_tx_gain(gain).await
            } else {
                self.set_rx_gain(gain).await
            }
        })
    }
    /// Sets the TX aggregate gain by apportioning across TXVGA1 and TXVGA2.
    ///
    /// Begins with both stages at minimum, then distributes remaining gain
    /// greedily in stage order (TXVGA2 first, then TXVGA1).
    ///
    /// Returns `Error::NotInitialized` if the board has not been initialized.
    pub fn set_tx_gain(&mut self, gain_db: GainDb) -> impl MaybeFuture<Output = Result<()>> {
        Op::new(async move {
            self.require_initialized().await?;
            let gain = TxGains::new(gain_db);
            self.lms().txvga1_set_gain(gain.vga1.into()).await?;
            self.lms().txvga2_set_gain(gain.vga2.into()).await
        })
    }
    /// Sets the RX aggregate gain by apportioning across LNA, RXVGA1, and RXVGA2.
    ///
    /// Begins with all stages at minimum, then distributes remaining gain using
    /// a multi-pass algorithm that clamps the LNA to half its maximum and
    /// adjusts RXVGA1 headroom for RXVGA2 as needed.
    ///
    /// Returns `Error::NotInitialized` if the board has not been initialized.
    pub fn set_rx_gain(&mut self, gain_db: GainDb) -> impl MaybeFuture<Output = Result<()>> {
        Op::new(async move {
            self.require_initialized().await?;
            let gain = RxGains::new(gain_db);
            self.lms().lna_set_gain(gain.lna.into()).await?;
            self.lms().rxvga1_set_gain(gain.vga1.into()).await?;
            self.lms().rxvga2_set_gain(gain.vga2.into()).await
        })
    }
}

#[derive(Debug, PartialEq, Eq)]
struct TxGains {
    vga1: i8,
    vga2: i8,
}

impl TxGains {
    fn new(requested: GainDb) -> Self {
        let min = GAIN_SPEC_TXVGA1.min + GAIN_SPEC_TXVGA2.min + BLADERF1_TX_GAIN_OFFSET;
        let max = GAIN_SPEC_TXVGA1.max + GAIN_SPEC_TXVGA2.max + BLADERF1_TX_GAIN_OFFSET;
        let remaining = i16::from(requested.db().clamp(min, max)) - i16::from(min);
        let (vga2, remaining) = GAIN_SPEC_TXVGA2.apportion(GAIN_SPEC_TXVGA2.min, remaining);
        let (vga1, _) = GAIN_SPEC_TXVGA1.apportion(GAIN_SPEC_TXVGA1.min, remaining);
        Self { vga1, vga2 }
    }
}

#[derive(Debug, PartialEq, Eq)]
struct RxGains {
    lna: i8,
    vga1: i8,
    vga2: i8,
}

impl RxGains {
    fn new(requested: GainDb) -> Self {
        let min = GAIN_SPEC_LNA.min
            + GAIN_SPEC_RXVGA1.min
            + GAIN_SPEC_RXVGA2.min
            + BLADERF1_RX_GAIN_OFFSET;
        let max = GAIN_SPEC_LNA.max
            + GAIN_SPEC_RXVGA1.max
            + GAIN_SPEC_RXVGA2.max
            + BLADERF1_RX_GAIN_OFFSET;
        let mut remaining = i16::from(requested.db().clamp(min, max)) - i16::from(min);
        let (mut lna, rest) = GAIN_SPEC_LNA.apportion(GAIN_SPEC_LNA.min, remaining);
        remaining = rest;
        let mid = GAIN_SPEC_LNA.max / 2;
        if lna > mid {
            remaining += i16::from(lna - mid);
            lna = mid;
        }
        let (mut vga1, rest) = GAIN_SPEC_RXVGA1.apportion(GAIN_SPEC_RXVGA1.min, remaining);
        (lna, remaining) = GAIN_SPEC_LNA.apportion(lna, rest);
        let (mut vga2, rest) = GAIN_SPEC_RXVGA2.apportion(GAIN_SPEC_RXVGA2.min, remaining);
        remaining = rest;
        if remaining > 0 && vga1 >= GAIN_SPEC_RXVGA1.max {
            vga1 -= GAIN_SPEC_RXVGA2.step;
            remaining += i16::from(GAIN_SPEC_RXVGA2.step);
            (vga2, remaining) = GAIN_SPEC_RXVGA2.apportion(vga2, remaining);
            (vga1, _) = GAIN_SPEC_RXVGA1.apportion(vga1, remaining);
        }
        Self { lna, vga1, vga2 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bladerf1::hardware::lms6002d::gain::Rxvga2GainCode;

    #[test]
    fn every_gain_input_stays_in_range_and_conserves_the_clamped_total() {
        for requested in i8::MIN..=i8::MAX {
            let tx = TxGains::new(requested.into());
            assert!((-35..=-4).contains(&tx.vga1));
            assert!((0..=25).contains(&tx.vga2));
            assert_eq!(tx.vga1 + tx.vga2 + 52, requested.clamp(17, 73));
            let rx = RxGains::new(requested.into());
            assert!([0, 3, 6].contains(&rx.lna));
            assert!((5..=30).contains(&rx.vga1));
            assert!((0..=30).contains(&rx.vga2) && rx.vga2 % 3 == 0);
            assert_eq!(rx.lna + rx.vga1 + rx.vga2 - 6, requested.clamp(-1, 60));
            let code = Rxvga2GainCode::from(GainDb::from(requested));
            let expected = (f32::from(requested.clamp(0, 30)) / 3.0).round() as u8;
            assert_eq!(code.code, expected);
        }
        for raw in u8::MIN..=u8::MAX {
            let gain = GainDb::from(Rxvga2GainCode::from(raw)).db();
            assert_eq!(gain, (u16::from(raw) * 3).min(30) as i8);
        }
    }

    #[test]
    fn c_stage_priority_and_rxvga2_rounding_boundaries_are_preserved() {
        for (requested, lna, vga1, vga2) in [
            (-1, 0, 5, 0),
            (0, 0, 6, 0),
            (2, 3, 5, 0),
            (27, 3, 30, 0),
            (28, 3, 28, 3),
            (29, 3, 29, 3),
            (30, 6, 30, 0),
            (31, 6, 28, 3),
            (32, 6, 29, 3),
            (60, 6, 30, 30),
        ] {
            assert_eq!(RxGains::new(requested.into()), RxGains { lna, vga1, vga2 });
        }
        for (requested, vga1, vga2) in [(17, -35, 0), (42, -35, 25), (43, -34, 25), (73, -4, 25)] {
            assert_eq!(TxGains::new(requested.into()), TxGains { vga1, vga2 });
        }
    }
}
