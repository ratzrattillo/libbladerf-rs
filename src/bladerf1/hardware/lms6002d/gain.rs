use crate::Error;
use crate::bladerf1::hardware::lms6002d::LMS6002D;
use crate::range::{Range, RangeItem};
pub const BLADERF1_RX_GAIN_OFFSET: f32 = -6.0;
pub const BLADERF1_TX_GAIN_OFFSET: f32 = 52.0;
pub enum LmsPowerAmplifier {
    PaAux,
    Pa1,
    Pa2,
    PaNone,
}
#[derive(Clone, Copy)]
pub enum LmsLowNoiseAmplifier {
    LnaNone,
    Lna1,
    Lna2,
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
            _ => Err(Error::Invalid),
        }
    }
}
#[derive(Debug, Clone, Copy)]
pub struct GainSpec {
    pub min: i8,
    pub max: i8,
    pub step: i8,
}
pub const GAIN_SPEC_LNA: GainSpec = GainSpec {
    min: 0,
    max: 6,
    step: 3,
};
pub const GAIN_SPEC_RXVGA1: GainSpec = GainSpec {
    min: 5,
    max: 30,
    step: 1,
};
pub const GAIN_SPEC_RXVGA2: GainSpec = GainSpec {
    min: 0,
    max: 30,
    step: 3,
};
pub const GAIN_SPEC_TXVGA1: GainSpec = GainSpec {
    min: -35,
    max: -4,
    step: 1,
};
pub const GAIN_SPEC_TXVGA2: GainSpec = GainSpec {
    min: 0,
    max: 25,
    step: 1,
};
pub struct GainDb {
    pub db: i8,
}
#[derive(PartialEq, Clone, Copy)]
pub enum LnaGainCode {
    BypassLna1Lna2 = 0x1,
    MidAllLnas,
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
        if value.db >= GAIN_SPEC_LNA.max {
            LnaGainCode::MaxAllLnas
        } else if value.db >= GAIN_SPEC_LNA.max / 2 {
            LnaGainCode::MidAllLnas
        } else {
            LnaGainCode::BypassLna1Lna2
        }
    }
}
pub struct Rxvga1GainCode {
    pub code: u8,
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
        let gain_db = value.db.clamp(GAIN_SPEC_RXVGA1.min, GAIN_SPEC_RXVGA1.max);
        Rxvga1GainCode {
            code: (127.0 - 127.0 / (10.0f32.powf((gain_db - GAIN_SPEC_RXVGA1.min) as f32 / 20.0)))
                .round() as u8,
        }
    }
}
pub struct Rxvga2GainCode {
    pub code: u8,
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
        let gain_db = value.db.clamp(GAIN_SPEC_RXVGA2.min, GAIN_SPEC_RXVGA2.max);
        Rxvga2GainCode {
            code: (gain_db as f32 / GAIN_SPEC_RXVGA2.step as f32).round() as u8,
        }
    }
}
pub struct Txvga1GainCode {
    pub code: u8,
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
        let clamped = value.db.clamp(GAIN_SPEC_TXVGA1.min, GAIN_SPEC_TXVGA1.max);
        Txvga1GainCode {
            code: (clamped - GAIN_SPEC_TXVGA1.min) as u8,
        }
    }
}
pub struct Txvga2GainCode {
    pub code: u8,
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
        let clamped = value.db.clamp(GAIN_SPEC_TXVGA2.min, GAIN_SPEC_TXVGA2.max);
        Txvga2GainCode {
            code: ((clamped & 0x1f) << 3) as u8,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GainStage {
    Lna,
    RxVga1,
    RxVga2,
    TxVga1,
    TxVga2,
}
impl GainStage {
    pub const fn is_rx(&self) -> bool {
        matches!(self, GainStage::Lna | GainStage::RxVga1 | GainStage::RxVga2)
    }
    pub const fn is_tx(&self) -> bool {
        matches!(self, GainStage::TxVga1 | GainStage::TxVga2)
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
            _ => Err(crate::error::Error::Invalid),
        }
    }
}
impl LMS6002D {
    fn gain_spec_to_range(spec: GainSpec) -> Range {
        Range {
            items: vec![RangeItem::Step(
                spec.min as f64,
                spec.max as f64,
                spec.step as f64,
                1.0,
            )],
        }
    }
    pub fn get_lna_gain_range() -> Range {
        Self::gain_spec_to_range(GAIN_SPEC_LNA)
    }
    pub fn get_rxvga1_gain_range() -> Range {
        Self::gain_spec_to_range(GAIN_SPEC_RXVGA1)
    }
    pub fn get_rxvga2_gain_range() -> Range {
        Self::gain_spec_to_range(GAIN_SPEC_RXVGA2)
    }
    pub fn get_txvga1_gain_range() -> Range {
        Self::gain_spec_to_range(GAIN_SPEC_TXVGA1)
    }
    pub fn get_txvga2_gain_range() -> Range {
        Self::gain_spec_to_range(GAIN_SPEC_TXVGA2)
    }
    pub fn get_gain_stage_range(stage: GainStage) -> Range {
        match stage {
            GainStage::Lna => Self::get_lna_gain_range(),
            GainStage::RxVga1 => Self::get_rxvga1_gain_range(),
            GainStage::RxVga2 => Self::get_rxvga2_gain_range(),
            GainStage::TxVga1 => Self::get_txvga1_gain_range(),
            GainStage::TxVga2 => Self::get_txvga2_gain_range(),
        }
    }
    pub fn lna_set_gain(&self, gain: GainDb) -> crate::Result<()> {
        let mut data = self.read(0x75)?;
        data &= !(3 << 6);
        let lna_gain_code: LnaGainCode = gain.into();
        let lna_gain_code_u8: u8 = lna_gain_code.into();
        data |= (lna_gain_code_u8 & 3) << 6;
        self.write(0x75, data)
    }
    pub fn lna_get_gain(&self) -> crate::Result<GainDb> {
        let mut data = self.read(0x75)?;
        data >>= 6;
        data &= 3;
        let lna_gain_code: LnaGainCode = data.try_into().map_err(|_| Error::Invalid)?;
        Ok(lna_gain_code.into())
    }
    pub fn get_lna(&self) -> crate::Result<LmsLowNoiseAmplifier> {
        let data = self.read(0x75)?;
        LmsLowNoiseAmplifier::try_from((data >> 4) & 0x3)
    }
    pub fn rxvga1_enable(&self, enable: bool) -> crate::Result<()> {
        let mut data = self.read(0x7d)?;
        if enable {
            data &= !(1 << 3);
        } else {
            data |= 1 << 3;
        }
        self.write(0x7d, data)
    }
    pub fn rxvga1_set_gain(&self, gain_db: GainDb) -> crate::Result<()> {
        let code: Rxvga1GainCode = gain_db.into();
        self.write(0x76, code.code)
    }
    pub fn rxvga1_get_gain(&self) -> crate::Result<GainDb> {
        let mut data = self.read(0x76)?;
        data &= 0x7f;
        let rxvga1_gain_code = Rxvga1GainCode {
            code: data.clamp(0, 120),
        };
        Ok(rxvga1_gain_code.into())
    }
    pub fn rxvga2_enable(&self, enable: bool) -> crate::Result<()> {
        let mut data = self.read(0x64)?;
        if enable {
            data |= 1 << 1;
        } else {
            data &= !(1 << 1);
        }
        self.write(0x64, data)
    }
    pub fn rxvga2_set_gain(&self, gain_db: GainDb) -> crate::Result<()> {
        let code: Rxvga2GainCode = gain_db.into();
        self.write(0x65, code.code)
    }
    pub fn rxvga2_get_gain(&self) -> crate::Result<GainDb> {
        let rxvga2_gain_code = Rxvga2GainCode {
            code: self.read(0x65)?,
        };
        Ok(rxvga2_gain_code.into())
    }
    pub fn txvga1_get_gain(&self) -> crate::Result<GainDb> {
        let txvga1_gain_code = Txvga1GainCode {
            code: self.read(0x41)?,
        };
        Ok(txvga1_gain_code.into())
    }
    pub fn txvga2_get_gain(&self) -> crate::Result<GainDb> {
        let txvga2_gain_code = Txvga2GainCode {
            code: self.read(0x45)?,
        };
        Ok(txvga2_gain_code.into())
    }
    pub fn txvga1_set_gain(&self, gain: GainDb) -> crate::Result<()> {
        let txvga1_gain_code: Txvga1GainCode = gain.into();
        self.write(0x41, txvga1_gain_code.code)
    }
    pub fn txvga2_set_gain(&self, gain: GainDb) -> crate::Result<()> {
        let mut data = self.read(0x45)?;
        data &= !(0x1f << 3);
        let txvga2_gain_code: Txvga2GainCode = gain.into();
        data |= txvga2_gain_code.code;
        self.write(0x45, data)
    }
    pub fn enable_lna_power(&self, enable: bool) -> crate::Result<()> {
        let mut regval = self.read(0x7d)?;
        if enable {
            regval &= !(1 << 0);
        } else {
            regval |= 1 << 0;
        }
        self.write(0x7d, regval)?;
        regval = self.read(0x70)?;
        if enable {
            regval &= !(1 << 1);
        } else {
            regval |= 1 << 1;
        }
        self.write(0x70, regval)
    }
    pub fn select_pa(&self, pa: LmsPowerAmplifier) -> crate::Result<()> {
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
    pub fn select_lna(&self, lna: LmsLowNoiseAmplifier) -> crate::Result<()> {
        let mut data = self.read(0x75)?;
        data &= !(3 << 4);
        data |= (u8::from(lna) & 3) << 4;
        self.write(0x75, data)
    }
}
