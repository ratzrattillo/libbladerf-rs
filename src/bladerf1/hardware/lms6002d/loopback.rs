use crate::bladerf1::Band;
use crate::bladerf1::hardware::lms6002d::LmsPowerAmplifier;
use crate::bladerf1::hardware::lms6002d::filters::LpfMode;
use crate::bladerf1::hardware::lms6002d::gain::LmsLowNoiseAmplifier;
use crate::bladerf1::nios_client::NiosClient;
use crate::{Channel, Error};
pub const LBEN_OPIN: u8 = 1 << 4;
pub const LBEN_VGA2IN: u8 = 1 << 5;
pub const LBEN_LPFIN: u8 = 1 << 6;
pub const LBEN_MASK: u8 = LBEN_OPIN | LBEN_VGA2IN | LBEN_LPFIN;
pub const LBRFEN_LNA1: u8 = 1;
pub const LBRFEN_LNA2: u8 = 2;
pub const LBRFEN_LNA3: u8 = 3;
pub const LBRFEN_MASK: u8 = 0xf;
pub const LOOPBBEN_TXLPF: u8 = 1 << 2;
pub const LOOPBBEN_TXVGA: u8 = 2 << 2;
pub const LOOPBBEN_ENVPK: u8 = 3 << 2;
pub const LOOBBBEN_MASK: u8 = 3 << 2;
pub enum LmsLoopbackPath {
    LbpBb,
    LbpRf,
}
#[derive(PartialEq, Debug, Clone)]
#[repr(u8)]
pub enum Loopback {
    None = 0,
    Firmware,
    BbTxlpfRxvga2,
    BbTxvga1Rxvga2,
    BbTxlpfRxlpf,
    BbTxvga1Rxlpf,
    Lna1,
    Lna2,
    Lna3,
    RficBist,
}
pub struct BladeRf1LoopbackModes {
    _name: String,
    _mode: Loopback,
}
pub fn loopback_path(nios: &mut NiosClient, mode: &Loopback) -> crate::Result<()> {
    let mut loopbben = super::read(nios, 0x46)?;
    let mut lben_lbrf = super::read(nios, 0x08)?;
    loopbben &= !LOOBBBEN_MASK;
    lben_lbrf &= !(LBRFEN_MASK | LBEN_MASK);
    match mode {
        Loopback::None => {}
        Loopback::BbTxlpfRxvga2 => {
            loopbben |= LOOPBBEN_TXLPF;
            lben_lbrf |= LBEN_VGA2IN;
        }
        Loopback::BbTxvga1Rxvga2 => {
            loopbben |= LOOPBBEN_TXVGA;
            lben_lbrf |= LBEN_VGA2IN;
        }
        Loopback::BbTxlpfRxlpf => {
            loopbben |= LOOPBBEN_TXLPF;
            lben_lbrf |= LBEN_LPFIN;
        }
        Loopback::BbTxvga1Rxlpf => {
            loopbben |= LOOPBBEN_TXVGA;
            lben_lbrf |= LBEN_LPFIN;
        }
        Loopback::Lna1 => {
            lben_lbrf |= LBRFEN_LNA1;
        }
        Loopback::Lna2 => {
            lben_lbrf |= LBRFEN_LNA2;
        }
        Loopback::Lna3 => {
            lben_lbrf |= LBRFEN_LNA3;
        }
        _ => Err(Error::Unsupported("loopback mode"))?,
    }
    super::write(nios, 0x46, loopbben)?;
    super::write(nios, 0x08, lben_lbrf)
}
pub fn enable_rf_loopback_switch(nios: &mut NiosClient, enable: bool) -> crate::Result<()> {
    let mut regval = super::read(nios, 0x0b)?;
    if enable {
        regval |= 1;
    } else {
        regval &= !1;
    }
    super::write(nios, 0x0b, regval)
}
pub fn loopback_rx(nios: &mut NiosClient, mode: &Loopback) -> crate::Result<()> {
    let lpf_mode = super::filters::lpf_get_mode(nios, Channel::Rx)?;
    match mode {
        Loopback::None => {
            super::gain::rxvga1_enable(nios, true)?;
            super::gain::rxvga2_enable(nios, true)?;
            enable_rf_loopback_switch(nios, false)?;
            super::gain::enable_lna_power(nios, true)?;
            let f = super::frequency::get_frequency(nios, Channel::Rx)?;
            super::frequency::set_frequency(nios, Channel::Rx, (&f).into())?;
            let f_hz: u64 = (&f).into();
            let band = Band::from(f_hz);
            super::select_band(nios, Channel::Rx, band)
        }
        Loopback::BbTxvga1Rxvga2 | Loopback::BbTxlpfRxvga2 => {
            super::gain::rxvga2_enable(nios, true)?;
            super::filters::lpf_set_mode(nios, Channel::Rx, LpfMode::Disabled)
        }
        Loopback::BbTxlpfRxlpf | Loopback::BbTxvga1Rxlpf => {
            super::gain::rxvga1_enable(nios, false)?;
            if lpf_mode == LpfMode::Disabled {
                super::filters::lpf_set_mode(nios, Channel::Rx, LpfMode::Normal)?;
            }
            super::gain::rxvga2_enable(nios, true)
        }
        Loopback::Lna1 | Loopback::Lna2 | Loopback::Lna3 => {
            let lms_lna = match mode {
                Loopback::Lna1 => LmsLowNoiseAmplifier::Lna1,
                Loopback::Lna2 => LmsLowNoiseAmplifier::Lna2,
                Loopback::Lna3 => LmsLowNoiseAmplifier::Lna3,
                _ => unreachable!(),
            };
            super::gain::enable_lna_power(nios, false)?;
            super::gain::rxvga1_enable(nios, true)?;
            if lpf_mode == LpfMode::Disabled {
                super::filters::lpf_set_mode(nios, Channel::Rx, LpfMode::Normal)?;
            }
            super::gain::rxvga2_enable(nios, true)?;
            let mut regval = super::read(nios, 0x25)?;
            regval &= !0x03;
            regval |= u8::from(lms_lna);
            super::write(nios, 0x25, regval)?;
            super::gain::select_lna(nios, lms_lna)?;
            enable_rf_loopback_switch(nios, true)
        }
        _ => Err(Error::Unsupported("loopback mode")),
    }
}
pub fn loopback_tx(nios: &mut NiosClient, mode: &Loopback) -> crate::Result<()> {
    match mode {
        Loopback::None => {
            let f = super::frequency::get_frequency(nios, Channel::Tx)?;
            super::frequency::set_frequency(nios, Channel::Tx, (&f).into())?;
            let f_hz: u64 = (&f).into();
            let band = Band::from(f_hz);
            super::select_band(nios, Channel::Tx, band)
        }
        Loopback::BbTxlpfRxvga2
        | Loopback::BbTxvga1Rxvga2
        | Loopback::BbTxlpfRxlpf
        | Loopback::BbTxvga1Rxlpf => Ok(()),
        Loopback::Lna1 | Loopback::Lna2 | Loopback::Lna3 => {
            super::gain::select_pa(nios, LmsPowerAmplifier::PaAux)
        }
        _ => Err(Error::Unsupported("loopback mode")),
    }
}
pub fn set_loopback_mode(nios: &mut NiosClient, mode: Loopback) -> crate::Result<()> {
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
    super::gain::select_pa(nios, LmsPowerAmplifier::PaNone)?;
    super::gain::select_lna(nios, LmsLowNoiseAmplifier::LnaNone)?;
    loopback_path(nios, &Loopback::None)?;
    loopback_rx(nios, &mode)?;
    loopback_tx(nios, &mode)?;
    loopback_path(nios, &mode)
}
pub fn get_loopback_mode(nios: &mut NiosClient) -> crate::Result<Loopback> {
    let mut loopback = Loopback::None;
    let lben_lbrfen = super::read(nios, 0x08)?;
    let loopbben = super::read(nios, 0x46)?;
    match lben_lbrfen & 0x7 {
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
    match lben_lbrfen & LBEN_MASK {
        LBEN_VGA2IN => {
            if (loopbben & LOOPBBEN_TXLPF) != 0 {
                loopback = Loopback::BbTxlpfRxvga2;
            } else if (loopbben & LOOPBBEN_TXVGA) != 0 {
                loopback = Loopback::BbTxvga1Rxvga2;
            }
        }
        LBEN_LPFIN => {
            if (loopbben & LOOPBBEN_TXLPF) != 0 {
                loopback = Loopback::BbTxlpfRxlpf;
            } else if (loopbben & LOOPBBEN_TXVGA) != 0 {
                loopback = Loopback::BbTxvga1Rxlpf;
            }
        }
        _ => {}
    }
    Ok(loopback)
}
pub fn is_loopback_enabled(nios: &mut NiosClient) -> crate::Result<bool> {
    let loopback = get_loopback_mode(nios)?;
    Ok(loopback != Loopback::None)
}
