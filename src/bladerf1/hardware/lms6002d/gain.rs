//! LMS6002D gain control.
//!
//! RX gain chain: LNA + VGA1 + VGA2.
//! TX gain chain: VGA1 + VGA2.
//! Each stage has its own programmable gain range and step size.

use crate::Error;
use crate::bladerf1::hardware::lms6002d::Lms6002d;
use crate::range::{Range, RangeItem};

/// RX gain offset applied when converting between dB FS and dBm.
pub const BLADERF1_RX_GAIN_OFFSET: f32 = -6.0;
/// TX gain offset applied when converting between dB FS and dBm.
pub const BLADERF1_TX_GAIN_OFFSET: f32 = 52.0;

/// LMS6002D power amplifier selection.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LmsPowerAmplifier {
    /// Auxiliary amplifier.
    PaAux,
    /// PA for low band (<1.5 GHz).
    Pa1,
    /// PA for high band (>=1.5 GHz).
    Pa2,
    /// No power amplifier selected.
    PaNone,
}
/// LMS6002D low-noise amplifier selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LmsLowNoiseAmplifier {
    /// No LNA selected.
    LnaNone,
    /// LNA for low band and general use.
    Lna1,
    /// LNA for high band.
    Lna2,
    /// LNA for XB-200 expansion.
    Lna3,
}
impl From<LmsLowNoiseAmplifier> for u8 {
    fn from(value: LmsLowNoiseAmplifier) -> Self {
        match value {
            LmsLowNoiseAmplifier::LnaNone => 0,
            LmsLowNoiseAmplifier::Lna1 => 1,
            LmsLowNoiseAmplifier::Lna2 => 2,
            LmsLowNoiseAmplifier::Lna3 => 3,
        }
    }
}
impl TryFrom<u8> for LmsLowNoiseAmplifier {
    type Error = Error;
    fn try_from(value: u8) -> crate::Result<Self> {
        match value {
            0 => Ok(LmsLowNoiseAmplifier::LnaNone),
            1 => Ok(LmsLowNoiseAmplifier::Lna1),
            2 => Ok(LmsLowNoiseAmplifier::Lna2),
            3 => Ok(LmsLowNoiseAmplifier::Lna3),
            _ => Err(Error::Argument("invalid LNA value".into())),
        }
    }
}
impl TryFrom<u8> for LmsPowerAmplifier {
    type Error = Error;
    fn try_from(pa_en: u8) -> crate::Result<Self> {
        match pa_en & 7 {
            0 => Ok(LmsPowerAmplifier::PaNone),
            2 => Ok(LmsPowerAmplifier::Pa1),
            4 => Ok(LmsPowerAmplifier::Pa2),
            _ => Err(Error::Argument("invalid PA value".into())),
        }
    }
}
/// Gain stage specification: minimum, maximum, and step in dB.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GainSpec {
    /// Minimum gain in dB.
    pub(crate) min: i8,
    /// Maximum gain in dB.
    pub(crate) max: i8,
    /// Gain step in dB.
    pub(crate) step: i8,
}
impl GainSpec {
    pub const fn new(min: i8, max: i8, step: i8) -> Self {
        Self { min, max, step }
    }
}

/// LNA gain specification: 0–6 dB in 3 dB steps.
pub const GAIN_SPEC_LNA: GainSpec = GainSpec::new(0, 6, 3);
/// RX VGA1 gain specification: 5–30 dB in 1 dB steps.
pub const GAIN_SPEC_RXVGA1: GainSpec = GainSpec::new(5, 30, 1);
/// RX VGA2 gain specification: 0–30 dB in 3 dB steps.
pub const GAIN_SPEC_RXVGA2: GainSpec = GainSpec::new(0, 30, 3);
/// TX VGA1 gain specification: -35 to -4 dB in 1 dB steps.
pub const GAIN_SPEC_TXVGA1: GainSpec = GainSpec::new(-35, -4, 1);
/// TX VGA2 gain specification: 0–25 dB in 1 dB steps.
pub const GAIN_SPEC_TXVGA2: GainSpec = GainSpec::new(0, 25, 1);
/// Gain value in decibels.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct GainDb {
    db: i8,
}
impl GainDb {
    /// Returns the gain value in dB.
    pub fn db(&self) -> i8 {
        self.db
    }
}
impl From<i8> for GainDb {
    fn from(db: i8) -> Self {
        Self { db }
    }
}
/// LNA gain code used to program the LNA gain register.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum LnaGainCode {
    /// Bypass LNA1 and LNA2 (0 dB gain).
    BypassLna1Lna2 = 0x1,
    /// Mid-level gain for all LNAs (3 dB).
    MidAllLnas,
    /// Maximum gain for all LNAs (6 dB).
    MaxAllLnas,
}
impl From<LnaGainCode> for u8 {
    fn from(value: LnaGainCode) -> Self {
        match value {
            LnaGainCode::BypassLna1Lna2 => 1,
            LnaGainCode::MidAllLnas => 2,
            LnaGainCode::MaxAllLnas => 3,
        }
    }
}
impl TryFrom<u8> for LnaGainCode {
    type Error = ();
    fn try_from(value: u8) -> std::result::Result<Self, Self::Error> {
        match value {
            1 => Ok(LnaGainCode::BypassLna1Lna2),
            2 => Ok(LnaGainCode::MidAllLnas),
            3 => Ok(LnaGainCode::MaxAllLnas),
            _ => {
                log::error!("Unsupported Gain Code {value}");
                Err(())
            }
        }
    }
}
impl From<LnaGainCode> for GainDb {
    fn from(value: LnaGainCode) -> Self {
        GainDb {
            db: match value {
                LnaGainCode::MaxAllLnas => GAIN_SPEC_LNA.max,
                LnaGainCode::MidAllLnas => GAIN_SPEC_LNA.max / 2,
                LnaGainCode::BypassLna1Lna2 => 0i8,
            },
        }
    }
}
impl From<GainDb> for LnaGainCode {
    fn from(value: GainDb) -> Self {
        if value.db() >= GAIN_SPEC_LNA.max {
            LnaGainCode::MaxAllLnas
        } else if value.db() >= GAIN_SPEC_LNA.max / 2 {
            LnaGainCode::MidAllLnas
        } else {
            LnaGainCode::BypassLna1Lna2
        }
    }
}
/// RX VGA1 hardware gain code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Rxvga1GainCode {
    /// Raw register value.
    pub(crate) code: u8,
}
impl From<u8> for Rxvga1GainCode {
    fn from(code: u8) -> Self {
        Self { code }
    }
}
impl From<Rxvga1GainCode> for GainDb {
    fn from(value: Rxvga1GainCode) -> Self {
        let gain_db = (GAIN_SPEC_RXVGA1.min as f32
            + (20.0 * (127.0 / (127.0 - value.code as f32)).log10()))
        .round() as i8;
        GainDb {
            db: gain_db.clamp(GAIN_SPEC_RXVGA1.min, GAIN_SPEC_RXVGA1.max),
        }
    }
}
impl From<GainDb> for Rxvga1GainCode {
    fn from(value: GainDb) -> Self {
        let gain_db = value.db().clamp(GAIN_SPEC_RXVGA1.min, GAIN_SPEC_RXVGA1.max);
        Rxvga1GainCode {
            code: (127.0 - 127.0 / (10.0f32.powf((gain_db - GAIN_SPEC_RXVGA1.min) as f32 / 20.0)))
                .round() as u8,
        }
    }
}
/// RX VGA2 hardware gain code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Rxvga2GainCode {
    /// Raw register value.
    pub(crate) code: u8,
}
impl From<u8> for Rxvga2GainCode {
    fn from(code: u8) -> Self {
        Self { code }
    }
}
impl From<Rxvga2GainCode> for GainDb {
    fn from(value: Rxvga2GainCode) -> Self {
        let gain_db = (value.code * GAIN_SPEC_RXVGA2.step as u8) as i8;
        GainDb {
            db: gain_db.clamp(GAIN_SPEC_RXVGA2.min, GAIN_SPEC_RXVGA2.max),
        }
    }
}
impl From<GainDb> for Rxvga2GainCode {
    fn from(value: GainDb) -> Self {
        let gain_db = value.db().clamp(GAIN_SPEC_RXVGA2.min, GAIN_SPEC_RXVGA2.max);
        Rxvga2GainCode {
            code: (gain_db as f32 / GAIN_SPEC_RXVGA2.step as f32).round() as u8,
        }
    }
}
/// TX VGA1 hardware gain code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Txvga1GainCode {
    /// Raw register value.
    pub(crate) code: u8,
}
impl From<u8> for Txvga1GainCode {
    fn from(code: u8) -> Self {
        Self { code }
    }
}
impl From<Txvga1GainCode> for GainDb {
    fn from(value: Txvga1GainCode) -> Self {
        let clamped = value.code & 0x1f;
        GainDb {
            db: clamped as i8 + GAIN_SPEC_TXVGA1.min,
        }
    }
}
impl From<GainDb> for Txvga1GainCode {
    fn from(value: GainDb) -> Self {
        let clamped = value.db().clamp(GAIN_SPEC_TXVGA1.min, GAIN_SPEC_TXVGA1.max);
        Txvga1GainCode {
            code: (clamped - GAIN_SPEC_TXVGA1.min) as u8,
        }
    }
}
/// TX VGA2 hardware gain code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Txvga2GainCode {
    /// Raw register value.
    pub(crate) code: u8,
}
impl From<u8> for Txvga2GainCode {
    fn from(code: u8) -> Self {
        Self { code }
    }
}
impl From<Txvga2GainCode> for GainDb {
    fn from(value: Txvga2GainCode) -> Self {
        let clamped = (value.code >> 3) & 0x1f;
        GainDb {
            db: clamped.min(GAIN_SPEC_TXVGA2.max as u8) as i8,
        }
    }
}
impl From<GainDb> for Txvga2GainCode {
    fn from(value: GainDb) -> Self {
        let clamped = value.db().clamp(GAIN_SPEC_TXVGA2.min, GAIN_SPEC_TXVGA2.max);
        Txvga2GainCode {
            code: ((clamped & 0x1f) << 3) as u8,
        }
    }
}
/// Identifies a specific gain stage in the LMS6002D signal chain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GainStage {
    /// RX low-noise amplifier (0–6 dB, step 3).
    Lna,
    /// RX variable gain amplifier stage 1 (5–30 dB, step 1).
    RxVga1,
    /// RX variable gain amplifier stage 2 (0–30 dB, step 3).
    RxVga2,
    /// TX variable gain amplifier stage 1 (-35 to -4 dB, step 1).
    TxVga1,
    /// TX variable gain amplifier stage 2 (0–25 dB, step 1).
    TxVga2,
}
impl GainStage {
    /// Returns `true` if this stage belongs to the RX path.
    pub const fn is_rx(&self) -> bool {
        matches!(self, GainStage::Lna | GainStage::RxVga1 | GainStage::RxVga2)
    }
    /// Returns `true` if this stage belongs to the TX path.
    pub const fn is_tx(&self) -> bool {
        matches!(self, GainStage::TxVga1 | GainStage::TxVga2)
    }
    /// Returns the gain range (min, max, step) for this stage.
    pub fn gain_range(self) -> Range {
        let spec = match self {
            Self::Lna => GAIN_SPEC_LNA,
            Self::RxVga1 => GAIN_SPEC_RXVGA1,
            Self::RxVga2 => GAIN_SPEC_RXVGA2,
            Self::TxVga1 => GAIN_SPEC_TXVGA1,
            Self::TxVga2 => GAIN_SPEC_TXVGA2,
        };
        Range::new(vec![RangeItem::Step(
            spec.min as f64,
            spec.max as f64,
            spec.step as f64,
            1.0,
        )])
    }
}
impl From<GainStage> for &'static str {
    fn from(stage: GainStage) -> Self {
        match stage {
            GainStage::Lna => "lna",
            GainStage::RxVga1 => "rxvga1",
            GainStage::RxVga2 => "rxvga2",
            GainStage::TxVga1 => "txvga1",
            GainStage::TxVga2 => "txvga2",
        }
    }
}
impl TryFrom<&str> for GainStage {
    type Error = crate::error::Error;
    fn try_from(name: &str) -> crate::error::Result<Self> {
        match name.to_lowercase().as_str() {
            "lna" => Ok(GainStage::Lna),
            "rxvga1" => Ok(GainStage::RxVga1),
            "rxvga2" => Ok(GainStage::RxVga2),
            "txvga1" => Ok(GainStage::TxVga1),
            "txvga2" => Ok(GainStage::TxVga2),
            _ => Err(Error::Argument("unknown gain stage".into())),
        }
    }
}
impl<'a> Lms6002d<'a> {
    pub(crate) fn lna_set_gain(&mut self, gain_db: GainDb) -> crate::Result<()> {
        let mut data = self.read(0x75)?;
        data &= !(3 << 6);
        let lna_gain_code: LnaGainCode = gain_db.into();
        let lna_gain_code_u8: u8 = lna_gain_code.into();
        data |= (lna_gain_code_u8 & 3) << 6;
        self.write(0x75, data)
    }

    pub(crate) fn lna_get_gain(&mut self) -> crate::Result<GainDb> {
        let mut data = self.read(0x75)?;
        data >>= 6;
        data &= 3;
        let lna_gain_code: LnaGainCode = data
            .try_into()
            .map_err(|_| Error::BoardState("invalid LNA gain code from hardware"))?;
        Ok(lna_gain_code.into())
    }

    pub(crate) fn get_lna(&mut self) -> crate::Result<LmsLowNoiseAmplifier> {
        let data = self.read(0x75)?;
        LmsLowNoiseAmplifier::try_from((data >> 4) & 0x3)
    }

    pub(crate) fn get_pa(&mut self) -> crate::Result<LmsPowerAmplifier> {
        let data = self.read(0x44)?;
        if (data & (1 << 1)) == 0 {
            return Ok(LmsPowerAmplifier::PaAux);
        }
        LmsPowerAmplifier::try_from((data >> 2) & 7)
    }

    pub(crate) fn rxvga1_enable(&mut self, enable: bool) -> crate::Result<()> {
        let mut data = self.read(0x7d)?;
        if enable {
            data &= !(1 << 3);
        } else {
            data |= 1 << 3;
        }
        self.write(0x7d, data)
    }

    pub(crate) fn rxvga1_set_gain(&mut self, gain_db: GainDb) -> crate::Result<()> {
        let code: Rxvga1GainCode = gain_db.into();
        self.write(0x76, code.code)
    }

    pub(crate) fn rxvga1_get_gain(&mut self) -> crate::Result<GainDb> {
        let mut data = self.read(0x76)?;
        data &= 0x7f;
        let rxvga1_gain_code = Rxvga1GainCode::from(data.clamp(0, 120));
        Ok(rxvga1_gain_code.into())
    }

    pub(crate) fn rxvga2_enable(&mut self, enable: bool) -> crate::Result<()> {
        let mut data = self.read(0x64)?;
        if enable {
            data |= 1 << 1;
        } else {
            data &= !(1 << 1);
        }
        self.write(0x64, data)
    }

    pub(crate) fn rxvga2_set_gain(&mut self, gain_db: GainDb) -> crate::Result<()> {
        let code: Rxvga2GainCode = gain_db.into();
        self.write(0x65, code.code)
    }

    pub(crate) fn rxvga2_get_gain(&mut self) -> crate::Result<GainDb> {
        let rxvga2_gain_code = Rxvga2GainCode::from(self.read(0x65)?);
        Ok(rxvga2_gain_code.into())
    }

    pub(crate) fn txvga1_get_gain(&mut self) -> crate::Result<GainDb> {
        let txvga1_gain_code = Txvga1GainCode::from(self.read(0x41)?);
        Ok(txvga1_gain_code.into())
    }

    pub(crate) fn txvga2_get_gain(&mut self) -> crate::Result<GainDb> {
        let txvga2_gain_code = Txvga2GainCode::from(self.read(0x45)?);
        Ok(txvga2_gain_code.into())
    }

    pub(crate) fn txvga1_set_gain(&mut self, gain_db: GainDb) -> crate::Result<()> {
        let txvga1_gain_code: Txvga1GainCode = gain_db.into();
        self.write(0x41, txvga1_gain_code.code)
    }

    pub(crate) fn txvga2_set_gain(&mut self, gain_db: GainDb) -> crate::Result<()> {
        let mut data = self.read(0x45)?;
        data &= !(0x1f << 3);
        let txvga2_gain_code: Txvga2GainCode = gain_db.into();
        data |= txvga2_gain_code.code;
        self.write(0x45, data)
    }

    pub(crate) fn enable_lna_power(&mut self, enable: bool) -> crate::Result<()> {
        let mut regval = self.read(0x7d)?;
        if enable {
            regval &= !(1 << 0);
        } else {
            regval |= 1 << 0;
        }
        self.write(0x7d, regval)?;
        let mut regval = self.read(0x70)?;
        if enable {
            regval &= !(1 << 1);
        } else {
            regval |= 1 << 1;
        }
        self.write(0x70, regval)
    }

    pub(crate) fn select_pa(&mut self, pa: LmsPowerAmplifier) -> crate::Result<()> {
        let mut data = self.read(0x44)?;
        data &= !0x1C;
        data |= 1 << 1;
        match pa {
            LmsPowerAmplifier::PaAux => {
                data &= !(1 << 1);
            }
            LmsPowerAmplifier::Pa1 => {
                data |= 2 << 2;
            }
            LmsPowerAmplifier::Pa2 => {
                data |= 4 << 2;
            }
            LmsPowerAmplifier::PaNone => {}
        }
        self.write(0x44, data)
    }

    pub(crate) fn select_lna(&mut self, lna: LmsLowNoiseAmplifier) -> crate::Result<()> {
        let mut data = self.read(0x75)?;
        data &= !(3 << 4);
        data |= (u8::from(lna) & 3) << 4;
        self.write(0x75, data)
    }
}
