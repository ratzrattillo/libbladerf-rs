use crate::bladerf1::Band;
use crate::bladerf1::hardware::lms6002d::bandwidth::BLADERF1_BAND_HIGH;
use crate::bladerf1::hardware::lms6002d::filters::LpfMode;
use crate::bladerf1::hardware::lms6002d::gain::LmsLowNoiseAmplifier;
use crate::bladerf1::hardware::lms6002d::{LMS6002D, LmsPowerAmplifier};
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
impl LMS6002D {
    pub fn loopback_path(&self, mode: &Loopback) -> crate::Result<()> {
        let mut loopbben = self.read(0x46)?;
        let mut lben_lbrf = self.read(0x08)?;
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
            _ => Err(Error::Argument("Loopback mode not supported"))?,
        }
        self.write(0x46, loopbben)?;
        self.write(0x08, lben_lbrf)
    }
    pub fn enable_rf_loopback_switch(&self, enable: bool) -> crate::Result<()> {
        let mut regval = self.read(0x0b)?;
        if enable {
            regval |= 1;
        } else {
            regval &= !1;
        }
        self.write(0x0b, regval)
    }
    pub fn loopback_rx(&self, mode: &Loopback) -> crate::Result<()> {
        let lpf_mode = self.lpf_get_mode(Channel::Rx)?;
        match mode {
            Loopback::None => {
                self.rxvga1_enable(true)?;
                self.rxvga2_enable(true)?;
                self.enable_rf_loopback_switch(false)?;
                self.enable_lna_power(true)?;
                let f = &self.get_frequency(Channel::Rx)?;
                self.set_frequency(Channel::Rx, f.into())?;
                let f_hz: u64 = f.into();
                let band = if f_hz < BLADERF1_BAND_HIGH as u64 {
                    Band::Low
                } else {
                    Band::High
                };
                self.select_band(Channel::Rx, band)
            }
            Loopback::BbTxvga1Rxvga2 | Loopback::BbTxlpfRxvga2 => {
                self.rxvga2_enable(true)?;
                self.lpf_set_mode(Channel::Rx, LpfMode::Disabled)
            }
            Loopback::BbTxlpfRxlpf | Loopback::BbTxvga1Rxlpf => {
                self.rxvga1_enable(false)?;
                if lpf_mode == LpfMode::Disabled {
                    self.lpf_set_mode(Channel::Rx, LpfMode::Normal)?;
                }
                self.rxvga2_enable(true)
            }
            Loopback::Lna1 | Loopback::Lna2 | Loopback::Lna3 => {
                let lms_lna = match mode {
                    Loopback::Lna1 => LmsLowNoiseAmplifier::Lna1,
                    Loopback::Lna2 => LmsLowNoiseAmplifier::Lna2,
                    Loopback::Lna3 => LmsLowNoiseAmplifier::Lna3,
                    _ => return Err(Error::Argument("Could not convert LNA mode.")),
                };
                self.enable_lna_power(false)?;
                self.rxvga1_enable(true)?;
                if lpf_mode == LpfMode::Disabled {
                    self.lpf_set_mode(Channel::Rx, LpfMode::Normal)?;
                }
                self.rxvga2_enable(true)?;
                let mut regval = self.read(0x25)?;
                regval &= !0x03;
                regval |= u8::from(lms_lna);
                self.write(0x25, regval)?;
                self.select_lna(lms_lna)?;
                self.enable_rf_loopback_switch(true)
            }
            _ => Err(Error::Argument("Could not convert LNA mode.")),
        }
    }
    pub fn loopback_tx(&self, mode: &Loopback) -> crate::Result<()> {
        match mode {
            Loopback::None => {
                let f = &self.get_frequency(Channel::Tx)?;
                self.set_frequency(Channel::Tx, f.into())?;
                let f_hz: u64 = f.into();
                let band = if f_hz < BLADERF1_BAND_HIGH as u64 {
                    Band::Low
                } else {
                    Band::High
                };
                self.select_band(Channel::Tx, band)
            }
            Loopback::BbTxlpfRxvga2
            | Loopback::BbTxvga1Rxvga2
            | Loopback::BbTxlpfRxlpf
            | Loopback::BbTxvga1Rxlpf => Ok(()),
            Loopback::Lna1 | Loopback::Lna2 | Loopback::Lna3 => {
                self.select_pa(LmsPowerAmplifier::PaAux)
            }
            _ => Err(Error::Argument("Invalid loopback mode encountered")),
        }
    }
    pub fn set_loopback_mode(&self, mode: Loopback) -> crate::Result<()> {
        match mode {
            Loopback::None => {}
            Loopback::BbTxlpfRxvga2 => {}
            Loopback::BbTxvga1Rxvga2 => {}
            Loopback::BbTxlpfRxlpf => {}
            Loopback::BbTxvga1Rxlpf => {}
            Loopback::Lna1 => {}
            Loopback::Lna2 => {}
            Loopback::Lna3 => {}
            _ => return Err(Error::Argument("Unsupported loopback mode")),
        }
        self.select_pa(LmsPowerAmplifier::PaNone)?;
        self.select_lna(LmsLowNoiseAmplifier::LnaNone)?;
        self.loopback_path(&Loopback::None)?;
        self.loopback_rx(&mode)?;
        self.loopback_tx(&mode)?;
        self.loopback_path(&mode)
    }
    pub fn get_loopback_mode(&self) -> crate::Result<Loopback> {
        let mut loopback = Loopback::None;
        let lben_lbrfen = self.read(0x08)?;
        let loopbben = self.read(0x46)?;
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
    pub fn is_loopback_enabled(&self) -> crate::Result<bool> {
        let loopback = self.get_loopback_mode()?;
        Ok(loopback != Loopback::None)
    }
}
