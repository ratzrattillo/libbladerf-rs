//! LMS6002D loopback configuration.
//!
//! The LMS6002D supports multiple loopback paths for testing: digital baseband
//! (TX→RX at various filter stages) and RF (antenna→LNAs). Loopback disables the
//! normal TX/RX RF paths and routes the signal internally. When exiting loopback,
//! the transceiver restores frequency and band settings.

use crate::bladerf1::Band;
use crate::bladerf1::hardware::lms6002d::Lms6002d;
use crate::bladerf1::hardware::lms6002d::LmsPowerAmplifier;
use crate::bladerf1::hardware::lms6002d::filters::LpfMode;
use crate::bladerf1::hardware::lms6002d::frequency::PLL_OUTPUT_SELECT_MASK;
use crate::bladerf1::hardware::lms6002d::gain::LmsLowNoiseAmplifier;
use crate::maybe_future::Op;
use crate::{Channel, Error};
use nusb::MaybeFuture;

bitflags::bitflags! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    pub(crate) struct LoopbackFlags: u8 {
        const OUTPUT = 1 << 4;
        const VGA2_INPUT = 1 << 5;
        const LPF_INPUT = 1 << 6;
        const BASEBAND = Self::OUTPUT.bits() | Self::VGA2_INPUT.bits() | Self::LPF_INPUT.bits();
        const _ = !0;
    }

    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    struct TopPowerFlags: u8 {
        const RF_LOOPBACK = 1 << 0;
        const _ = !0;
    }
}

const LBRFEN_SELECT_MASK: u8 = 0x07;

impl LoopbackFlags {
    pub(crate) fn is_enabled(self, tx_source: u8) -> bool {
        matches!(
            self.bits() & LBRFEN_SELECT_MASK,
            LBRFEN_LNA1 | LBRFEN_LNA2 | LBRFEN_LNA3
        ) || (self.intersects(Self::BASEBAND) && (tx_source & LOOBBBEN_MASK) != 0)
    }
}

/// LBRFEN register: LNA1 loopback.
pub const LBRFEN_LNA1: u8 = 1;
/// LBRFEN register: LNA2 loopback.
pub const LBRFEN_LNA2: u8 = 2;
/// LBRFEN register: LNA3 loopback.
pub const LBRFEN_LNA3: u8 = 3;
/// LBRFEN register: combined mask.
pub const LBRFEN_MASK: u8 = 0xf;
/// LOOPBBEN register: TX LPF loopback source.
pub const LOOPBBEN_TXLPF: u8 = 1 << 2;
/// LOOPBBEN register: TX VGA loopback source.
pub const LOOPBBEN_TXVGA: u8 = 2 << 2;
/// LOOPBBEN register: envelope peak detector loopback source.
pub const LOOPBBEN_ENVPK: u8 = 3 << 2;
/// LOOPBBEN register: combined mask.
pub const LOOBBBEN_MASK: u8 = 3 << 2;

/// High-level loopback path classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LmsLoopbackPath {
    /// Digital baseband loopback.
    LbpBb,
    /// RF loopback.
    LbpRf,
}

/// Supported loopback modes.
#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub enum Loopback {
    /// No loopback; normal RX/TX operation.
    None = 0,
    /// Firmware-level loopback (not implemented in hardware).
    Firmware,
    /// Digital: TX LPF → RX VGA2.
    BbTxlpfRxvga2,
    /// Digital: TX VGA1 → RX VGA2.
    BbTxvga1Rxvga2,
    /// Digital: TX LPF → RX LPF.
    BbTxlpfRxlpf,
    /// Digital: TX VGA1 → RX LPF.
    BbTxvga1Rxlpf,
    /// RF loopback through LNA1.
    Lna1,
    /// RF loopback through LNA2.
    Lna2,
    /// RF loopback through LNA3.
    Lna3,
    /// RFIC BIST mode (not implemented).
    RficBist,
}

/// BladeRF1 loopback mode definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BladeRf1LoopbackModes {
    /// Human-readable name of the loopback mode.
    _name: String,
    /// Corresponding hardware loopback mode.
    _mode: Loopback,
}
impl<'a> Lms6002d<'a> {
    pub(crate) fn set_loopback_mode(
        &mut self,
        mode: Loopback,
    ) -> impl MaybeFuture<Output = crate::Result<()>> {
        Op::new(async move {
            if !matches!(
                mode,
                Loopback::None
                    | Loopback::BbTxlpfRxvga2
                    | Loopback::BbTxvga1Rxvga2
                    | Loopback::BbTxlpfRxlpf
                    | Loopback::BbTxvga1Rxlpf
                    | Loopback::Lna1
                    | Loopback::Lna2
                    | Loopback::Lna3
            ) {
                return Err(Error::Unsupported("loopback mode"));
            }
            self.select_pa(LmsPowerAmplifier::PaNone).await?;
            self.select_lna(LmsLowNoiseAmplifier::LnaNone).await?;
            self.loopback_path(&Loopback::None).await?;
            self.loopback_rx(&mode).await?;
            self.loopback_tx(&mode).await?;
            self.loopback_path(&mode).await
        })
    }

    pub(crate) fn get_loopback_mode(
        &mut self,
    ) -> impl MaybeFuture<Output = crate::Result<Loopback>> {
        Op::new(async move {
            let lben_lbrfen = self.read(0x08).await?;
            let loopbben = self.read(0x46).await?;
            let mut loopback = Loopback::None;
            match lben_lbrfen & LBRFEN_SELECT_MASK {
                LBRFEN_LNA1 => {
                    loopback = Loopback::Lna1;
                }
                LBRFEN_LNA2 => {
                    loopback = Loopback::Lna2;
                }
                LBRFEN_LNA3 => {
                    loopback = Loopback::Lna3;
                }
                _ => {}
            }
            let baseband = LoopbackFlags::from_bits_retain(lben_lbrfen) & LoopbackFlags::BASEBAND;
            if baseband == LoopbackFlags::VGA2_INPUT {
                if (loopbben & LOOPBBEN_TXLPF) != 0 {
                    loopback = Loopback::BbTxlpfRxvga2;
                } else if (loopbben & LOOPBBEN_TXVGA) != 0 {
                    loopback = Loopback::BbTxvga1Rxvga2;
                }
            } else if baseband == LoopbackFlags::LPF_INPUT {
                if (loopbben & LOOPBBEN_TXLPF) != 0 {
                    loopback = Loopback::BbTxlpfRxlpf;
                } else if (loopbben & LOOPBBEN_TXVGA) != 0 {
                    loopback = Loopback::BbTxvga1Rxlpf;
                }
            }
            Ok(loopback)
        })
    }

    pub(crate) fn is_loopback_enabled(&mut self) -> impl MaybeFuture<Output = crate::Result<bool>> {
        Op::new(async move {
            let loopback = self.get_loopback_mode().await?;
            Ok(loopback != Loopback::None)
        })
    }

    fn loopback_path(&mut self, mode: &Loopback) -> impl MaybeFuture<Output = crate::Result<()>> {
        Op::new(async move {
            let mut loopbben = self.read(0x46).await?;
            let mut lben_lbrf = LoopbackFlags::from_bits_retain(self.read(0x08).await?);
            loopbben &= !LOOBBBEN_MASK;
            lben_lbrf
                .remove(LoopbackFlags::BASEBAND | LoopbackFlags::from_bits_retain(LBRFEN_MASK));
            match mode {
                Loopback::None => {}
                Loopback::BbTxlpfRxvga2 => {
                    loopbben |= LOOPBBEN_TXLPF;
                    lben_lbrf.insert(LoopbackFlags::VGA2_INPUT);
                }
                Loopback::BbTxvga1Rxvga2 => {
                    loopbben |= LOOPBBEN_TXVGA;
                    lben_lbrf.insert(LoopbackFlags::VGA2_INPUT);
                }
                Loopback::BbTxlpfRxlpf => {
                    loopbben |= LOOPBBEN_TXLPF;
                    lben_lbrf.insert(LoopbackFlags::LPF_INPUT);
                }
                Loopback::BbTxvga1Rxlpf => {
                    loopbben |= LOOPBBEN_TXVGA;
                    lben_lbrf.insert(LoopbackFlags::LPF_INPUT);
                }
                Loopback::Lna1 => {
                    lben_lbrf.insert(LoopbackFlags::from_bits_retain(LBRFEN_LNA1));
                }
                Loopback::Lna2 => {
                    lben_lbrf.insert(LoopbackFlags::from_bits_retain(LBRFEN_LNA2));
                }
                Loopback::Lna3 => {
                    lben_lbrf.insert(LoopbackFlags::from_bits_retain(LBRFEN_LNA3));
                }
                _ => Err(Error::Unsupported("loopback mode"))?,
            }
            self.write(0x46, loopbben).await?;
            self.write(0x08, lben_lbrf.bits()).await
        })
    }

    fn enable_rf_loopback_switch(
        &mut self,
        enable: bool,
    ) -> impl MaybeFuture<Output = crate::Result<()>> {
        Op::new(async move {
            let mut regval = TopPowerFlags::from_bits_retain(self.read(0x0b).await?);
            regval.set(TopPowerFlags::RF_LOOPBACK, enable);
            self.write(0x0b, regval.bits()).await
        })
    }

    fn loopback_rx(&mut self, mode: &Loopback) -> impl MaybeFuture<Output = crate::Result<()>> {
        Op::new(async move {
            let lpf_mode = self.lpf_get_mode(Channel::Rx).await?;
            match mode {
                Loopback::None => {
                    self.rxvga1_enable(true).await?;
                    self.rxvga2_enable(true).await?;
                    self.enable_rf_loopback_switch(false).await?;
                    self.enable_lna_power(true).await?;
                    let f = self.get_frequency(Channel::Rx).await?;
                    self.set_frequency(Channel::Rx, (&f).into()).await?;
                    let f_hz: u64 = (&f).into();
                    let band = Band::from(f_hz);
                    self.select_band(Channel::Rx, band).await
                }
                Loopback::BbTxvga1Rxvga2 | Loopback::BbTxlpfRxvga2 => {
                    self.rxvga2_enable(true).await?;
                    self.lpf_set_mode(Channel::Rx, LpfMode::Disabled).await
                }
                Loopback::BbTxlpfRxlpf | Loopback::BbTxvga1Rxlpf => {
                    self.rxvga1_enable(false).await?;
                    if lpf_mode == LpfMode::Disabled {
                        self.lpf_set_mode(Channel::Rx, LpfMode::Normal).await?;
                    }
                    self.rxvga2_enable(true).await
                }
                Loopback::Lna1 | Loopback::Lna2 | Loopback::Lna3 => {
                    let lms_lna = match mode {
                        Loopback::Lna1 => LmsLowNoiseAmplifier::Lna1,
                        Loopback::Lna2 => LmsLowNoiseAmplifier::Lna2,
                        Loopback::Lna3 => LmsLowNoiseAmplifier::Lna3,
                        _ => unreachable!(),
                    };
                    self.enable_lna_power(false).await?;
                    self.rxvga1_enable(true).await?;
                    if lpf_mode == LpfMode::Disabled {
                        self.lpf_set_mode(Channel::Rx, LpfMode::Normal).await?;
                    }
                    self.rxvga2_enable(true).await?;
                    let mut regval = self.read(0x25).await?;
                    regval &= !PLL_OUTPUT_SELECT_MASK;
                    regval |= u8::from(lms_lna);
                    self.write(0x25, regval).await?;
                    self.select_lna(lms_lna).await?;
                    self.enable_rf_loopback_switch(true).await
                }
                _ => Err(Error::Unsupported("loopback mode")),
            }
        })
    }

    fn loopback_tx(&mut self, mode: &Loopback) -> impl MaybeFuture<Output = crate::Result<()>> {
        Op::new(async move {
            match mode {
                Loopback::None => {
                    let f = self.get_frequency(Channel::Tx).await?;
                    self.set_frequency(Channel::Tx, (&f).into()).await?;
                    let f_hz: u64 = (&f).into();
                    let band = Band::from(f_hz);
                    self.select_band(Channel::Tx, band).await
                }
                Loopback::BbTxlpfRxvga2
                | Loopback::BbTxvga1Rxvga2
                | Loopback::BbTxlpfRxlpf
                | Loopback::BbTxvga1Rxlpf => Ok(()),
                Loopback::Lna1 | Loopback::Lna2 | Loopback::Lna3 => {
                    self.select_pa(LmsPowerAmplifier::PaAux).await
                }
                _ => Err(Error::Unsupported("loopback mode")),
            }
        })
    }
}
