//! LMS6002D PLL frequency tuning.
//!
//! Computes and applies the register values for the LMS6002D fractional-N synthesizer.
//! The band split occurs at 1.5 GHz; frequencies below use the low-band RF path,
//! frequencies at or above use the high-band RF path.
//! The PLL parameters are: NINT (integer divider), NFRAC (fractional divider),
//! FREQSEL (VCO selection and post-divider), and VCOCAP (tuning capacitor trim).
//! See the LMS6002D programming guide for register-level detail.

use crate::bladerf1::hardware::lms6002d::Band;
use crate::bladerf1::hardware::lms6002d::{
    LMS_FREQ_FLAGS_FORCE_VCOCAP, LMS_FREQ_FLAGS_LOW_BAND, LMS_FREQ_XB_200_ENABLE,
    LMS_FREQ_XB_200_FILTER_SW_SHIFT, LMS_FREQ_XB_200_MODULE_RX, LMS_FREQ_XB_200_PATH_SHIFT,
    VCOCAP_EST_MIN, VCOCAP_EST_RANGE, VCOCAP_MAX_LOW_HIGH, VCOCAP_MAX_VALUE, VTUNE_DELAY_LARGE,
    VTUNE_DELAY_SMALL, VTUNE_MAX_ITERATIONS, VcoState,
};
use crate::channel::Channel;
use crate::error::Error;
use std::thread::sleep;
use std::time::Duration;
/// Minimum frequency with XB-200 expansion board enabled.
pub const BLADERF_FREQUENCY_MIN_XB200: u32 = 0;
/// Minimum supported frequency in Hz.
pub const BLADERF_FREQUENCY_MIN: u32 = 237_500_000;
/// Maximum supported frequency in Hz.
pub const BLADERF_FREQUENCY_MAX: u32 = 3_800_000_000;
const LMS_REFERENCE_HZ: u32 = 38_400_000;
/// Pre-calculated PLL tuning parameters for fast frequency retuning.
///
/// Contains the NINT/NFRAC divider values, VCOCAP estimate, and XB-200 GPIO settings
/// captured from a previous full PLL configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuickTune {
    /// VCO selection and post-divider encoded value.
    pub(crate) freqsel: u8,
    /// VCOCAP tuning capacitor trim value.
    pub(crate) vcocap: u8,
    /// Integer portion of the fractional-N divider.
    pub(crate) nint: u16,
    /// Fractional portion of the fractional-N divider (23-bit resolution).
    pub(crate) nfrac: u32,
    /// Tuning flags (low band, force VCOCAP).
    pub(crate) flags: u8,
    /// XB-200 expansion GPIO configuration for filter and path routing.
    pub(crate) xb_gpio: u8,
}

impl From<&LmsFreq> for QuickTune {
    fn from(f: &LmsFreq) -> Self {
        Self {
            freqsel: f.freqsel,
            vcocap: f.vcocap,
            nint: f.nint,
            nfrac: f.nfrac,
            flags: f.flags,
            xb_gpio: f.xb_gpio,
        }
    }
}

impl From<QuickTune> for LmsFreq {
    fn from(qt: QuickTune) -> Self {
        Self {
            freqsel: qt.freqsel,
            vcocap: qt.vcocap,
            nint: qt.nint,
            nfrac: qt.nfrac,
            flags: qt.flags,
            xb_gpio: qt.xb_gpio,
            x: 0,
            vcocap_result: 0,
        }
    }
}
/// VCO4 lower frequency boundary in Hz.
pub const VCO4_LOW: u64 = 3_800_000_000;
/// VCO4 upper frequency boundary in Hz.
pub const VCO4_HIGH: u64 = 4_535_000_000;
/// VCO3 lower frequency boundary in Hz.
pub const VCO3_LOW: u64 = VCO4_HIGH;
/// VCO3 upper frequency boundary in Hz.
pub const VCO3_HIGH: u64 = 5_408_000_000;
/// VCO2 lower frequency boundary in Hz.
pub const VCO2_LOW: u64 = VCO3_HIGH;
/// VCO2 upper frequency boundary in Hz.
pub const VCO2_HIGH: u64 = 6_480_000_000;
/// VCO1 lower frequency boundary in Hz.
pub const VCO1_LOW: u64 = VCO2_HIGH;
/// VCO1 upper frequency boundary in Hz.
pub const VCO1_HIGH: u64 = 7_600_000_000;

/// FREQSEL encoding for VCO4.
pub const VCO4: u8 = 4 << 3;
/// FREQSEL encoding for VCO3.
pub const VCO3: u8 = 5 << 3;
/// FREQSEL encoding for VCO2.
pub const VCO2: u8 = 6 << 3;
/// FREQSEL encoding for VCO1.
pub const VCO1: u8 = 7 << 3;
/// Post-divider: divide by 2.
pub const DIV2: u8 = 0x4;
/// Post-divider: divide by 4.
pub const DIV4: u8 = 0x5;
/// Post-divider: divide by 8.
pub const DIV8: u8 = 0x6;
/// Post-divider: divide by 16.
pub const DIV16: u8 = 0x7;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FreqRange {
    low: u64,
    high: u64,
    value: u8,
}
pub const BANDS: [FreqRange; 16] = [
    FreqRange {
        low: BLADERF_FREQUENCY_MIN as u64,
        high: VCO4_HIGH / 16,
        value: VCO4 | DIV16,
    },
    FreqRange {
        low: VCO3_LOW / 16,
        high: VCO3_HIGH / 16,
        value: VCO3 | DIV16,
    },
    FreqRange {
        low: VCO2_LOW / 16,
        high: VCO2_HIGH / 16,
        value: VCO2 | DIV16,
    },
    FreqRange {
        low: VCO1_LOW / 16,
        high: VCO1_HIGH / 16,
        value: VCO1 | DIV16,
    },
    FreqRange {
        low: VCO4_LOW / 8,
        high: VCO4_HIGH / 8,
        value: VCO4 | DIV8,
    },
    FreqRange {
        low: VCO3_LOW / 8,
        high: VCO3_HIGH / 8,
        value: VCO3 | DIV8,
    },
    FreqRange {
        low: VCO2_LOW / 8,
        high: VCO2_HIGH / 8,
        value: VCO2 | DIV8,
    },
    FreqRange {
        low: VCO1_LOW / 8,
        high: VCO1_HIGH / 8,
        value: VCO1 | DIV8,
    },
    FreqRange {
        low: VCO4_LOW / 4,
        high: VCO4_HIGH / 4,
        value: VCO4 | DIV4,
    },
    FreqRange {
        low: VCO3_LOW / 4,
        high: VCO3_HIGH / 4,
        value: VCO3 | DIV4,
    },
    FreqRange {
        low: VCO2_LOW / 4,
        high: VCO2_HIGH / 4,
        value: VCO2 | DIV4,
    },
    FreqRange {
        low: VCO1_LOW / 4,
        high: VCO1_HIGH / 4,
        value: VCO1 | DIV4,
    },
    FreqRange {
        low: VCO4_LOW / 2,
        high: VCO4_HIGH / 2,
        value: VCO4 | DIV2,
    },
    FreqRange {
        low: VCO3_LOW / 2,
        high: VCO3_HIGH / 2,
        value: VCO3 | DIV2,
    },
    FreqRange {
        low: VCO2_LOW / 2,
        high: VCO2_HIGH / 2,
        value: VCO2 | DIV2,
    },
    FreqRange {
        low: VCO1_LOW / 2,
        high: BLADERF_FREQUENCY_MAX as u64,
        value: VCO1 | DIV2,
    },
];
/// Full LMS6002D PLL frequency parameters.
///
/// Computed from a target frequency and written to the synthesizer registers.
/// The VCO multiplies the 38.4 MHz reference by (NINT + NFRAC/2^23), then
/// divides by X to produce the RF output. VCOCAP adjusts the VCO tuning varactor.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct LmsFreq {
    /// Frequency selector: VCO choice and post-divider.
    pub(crate) freqsel: u8,
    /// VCOCAP tuning capacitor trim value.
    pub(crate) vcocap: u8,
    /// Integer portion of the fractional-N PLL divider.
    pub(crate) nint: u16,
    /// Fractional portion of the PLL divider (23-bit resolution).
    pub(crate) nfrac: u32,
    /// Tuning flags (low band, force VCOCAP).
    pub(crate) flags: u8,
    /// XB-200 expansion GPIO configuration for filter and path routing.
    pub(crate) xb_gpio: u8,
    /// VCO multiplication factor (power of 2).
    pub(crate) x: u8,
    /// Final VCOCAP value after VTUNE convergence search.
    pub(crate) vcocap_result: u8,
}
impl From<&LmsFreq> for u64 {
    fn from(value: &LmsFreq) -> Self {
        let pll_coeff = ((value.nint as u64) << 23) + value.nfrac as u64;
        let div = (value.x as u64) << 23;
        let numerator =
            (LMS_REFERENCE_HZ as u128 * pll_coeff as u128 + (div as u128 >> 1)) / div as u128;
        numerator as u64
    }
}
impl TryFrom<u64> for LmsFreq {
    type Error = Error;
    fn try_from(value: u64) -> Result<Self, Self::Error> {
        fn estimate_vcocap(f_target: u32, f_low: u32, f_high: u32) -> u8 {
            let denom: f32 = (f_high - f_low) as f32;
            let num: f32 = VCOCAP_EST_RANGE as f32;
            let f_diff: f32 = (f_target - f_low) as f32;
            let vcocap = (num / denom * f_diff) + 0.5 + VCOCAP_EST_MIN as f32;
            if vcocap > VCOCAP_MAX_VALUE as f32 {
                log::debug!("Clamping VCOCAP estimate from {vcocap} to {VCOCAP_MAX_VALUE}");
                VCOCAP_MAX_VALUE
            } else {
                log::debug!("VCOCAP estimate: {vcocap}");
                vcocap as u8
            }
        }
        let freq = value.clamp(BLADERF_FREQUENCY_MIN as u64, BLADERF_FREQUENCY_MAX as u64);
        let freq_range = BANDS
            .iter()
            .find(|freq_range| (freq >= freq_range.low) && (freq <= freq_range.high))
            .ok_or(Error::Argument(
                "Could not determine frequency range".into(),
            ))?;
        let freqsel = freq_range.value;
        log::trace!("freqsel: {freqsel}");
        let vcocap = estimate_vcocap(freq as u32, freq_range.low as u32, freq_range.high as u32);
        log::trace!("vcocap: {vcocap}");
        let vco_x = 1u64 << ((freqsel & 7) - 3);
        log::trace!("vco_x: {vco_x}");
        if vco_x > u8::MAX as u64 {
            return Err(Error::BoardState("VCO divider out of u8 range"));
        }
        let x = vco_x as u8;
        log::trace!("x: {x}");
        let mut temp = (vco_x * freq) / LMS_REFERENCE_HZ as u64;
        if temp > u16::MAX as u64 {
            return Err(Error::Argument(
                "frequency results in nint exceeding u16 range".into(),
            ));
        }
        let nint = temp as u16;
        log::trace!("nint: {nint}");
        let nfrac_num = (1u64 << 23) * (vco_x * freq - nint as u64 * LMS_REFERENCE_HZ as u64);
        temp = (nfrac_num + LMS_REFERENCE_HZ as u64 / 2) / LMS_REFERENCE_HZ as u64;
        if temp > u32::MAX as u64 {
            return Err(Error::BoardState("nfrac exceeds u32 range"));
        }
        let nfrac = temp as u32;
        log::trace!("nfrac: {nfrac}");
        let flags = if Band::from(freq) == Band::Low {
            LMS_FREQ_FLAGS_LOW_BAND
        } else {
            0
        };
        log::trace!("flags: {flags}");
        Ok(LmsFreq {
            freqsel,
            vcocap,
            nint,
            nfrac,
            flags,
            xb_gpio: 0,
            x,
            vcocap_result: 0,
        })
    }
}

/// Returns the minimum supported frequency in Hz.
pub const fn get_frequency_min() -> u32 {
    BLADERF_FREQUENCY_MIN
}

/// Returns the maximum supported frequency in Hz.
pub const fn get_frequency_max() -> u32 {
    BLADERF_FREQUENCY_MAX
}

use super::Lms6002d;
impl<'a> Lms6002d<'a> {
    pub(crate) fn config_charge_pumps(&mut self, channel: Channel) -> crate::Result<()> {
        let base: u8 = if channel == Channel::Rx { 0x20 } else { 0x10 };
        let mut data = self.read(base + 6)?;
        data &= !0x1f;
        data |= 0x0c;
        self.write(base + 6, data)?;
        let mut data = self.read(base + 7)?;
        data &= !0x1f;
        data |= 0x03;
        self.write(base + 7, data)?;
        let mut data = self.read(base + 8)?;
        data &= !0x1f;
        data |= 0x03;
        self.write(base + 8, data)
    }

    fn write_vcocap(&mut self, base: u8, vcocap: u8, vcocap_reg_state: u8) -> crate::Result<()> {
        if vcocap > VCOCAP_MAX_VALUE {
            return Err(Error::Argument("vcocap exceeds maximum value".into()));
        }
        log::trace!("Writing VCOCAP={vcocap}");
        self.write(base + 9, vcocap | vcocap_reg_state)
    }

    fn get_vtune(&mut self, base: u8, delay: u8) -> crate::Result<VcoState> {
        if delay != 0 {
            sleep(Duration::from_micros(delay as u64));
        }
        let vtune = self.read(base + 10)?;
        VcoState::try_from(vtune >> 6)
    }

    fn set_precalculated_frequency(
        &mut self,
        channel: Channel,
        f: &mut LmsFreq,
    ) -> crate::Result<()> {
        let base: u8 = if channel == Channel::Rx { 0x20 } else { 0x10 };
        let pll_base: u8 = base | 0x80;
        f.vcocap_result = 0xff;
        let mut data = self.read(0x09)?;
        data |= 0x05;
        self.write(0x09, data)?;
        let vcocap_reg_state = match self.read(base + 9) {
            Ok(v) => v,
            Err(e) => {
                self.turn_off_dsms()?;
                log::error!(
                    "Failed to read vcocap regstate! Device requires re-initialization (call initialize()) to restore DSM state."
                );
                return Err(e);
            }
        };
        let vcocap_reg_state = vcocap_reg_state & !0x3f;
        if let Err(e) = self.write_vcocap(base, f.vcocap, vcocap_reg_state) {
            self.turn_off_dsms()?;
            log::error!(
                "Failed to write vcocap_reg_state! Device requires re-initialization (call initialize()) to restore DSM state."
            );
            return Err(e);
        }
        let low_band = (f.flags & LMS_FREQ_FLAGS_LOW_BAND) != 0;
        let lben_lbrfen = self.read(0x08)?;
        let loopbben = self.read(0x46)?;
        let lb_enabled = matches!(lben_lbrfen & 0x7, 1..=3)
            || ((lben_lbrfen & 0x70) != 0 && (loopbben & 0x0c) != 0);
        if let Err(e) = self.write_pll_config(channel, f.freqsel, low_band, lb_enabled) {
            self.turn_off_dsms()?;
            log::error!(
                "Failed to write pll_config! Device requires re-initialization (call initialize()) to restore DSM state."
            );
            return Err(e);
        }
        let mut freq_data = [0u8; 4];
        freq_data[0] = (f.nint >> 1) as u8;
        freq_data[1] = (((f.nint & 1) << 7) as u32 | ((f.nfrac >> 16) & 0x7f)) as u8;
        freq_data[2] = ((f.nfrac >> 8) & 0xff) as u8;
        freq_data[3] = (f.nfrac & 0xff) as u8;
        for (idx, value) in freq_data.iter().enumerate() {
            if let Err(e) = self.write(pll_base + idx as u8, *value) {
                self.turn_off_dsms()?;
                log::error!(
                    "Failed to write pll {}! Device requires re-initialization (call initialize()) to restore DSM state.",
                    pll_base + idx as u8
                );
                return Err(e);
            }
        }
        if (f.flags & LMS_FREQ_FLAGS_FORCE_VCOCAP) != 0 {
            f.vcocap_result = f.vcocap;
        } else {
            log::trace!("Tuning VCOCAP...");
            f.vcocap_result = self.tune_vcocap(f.vcocap, base, vcocap_reg_state)?;
        }
        Ok(())
    }

    pub(crate) fn set_frequency(&mut self, channel: Channel, freq: u64) -> crate::Result<()> {
        let mut f = freq.try_into()?;
        log::trace!("{f:?}");
        self.set_precalculated_frequency(channel, &mut f)
    }

    pub(crate) fn get_frequency(&mut self, channel: Channel) -> crate::Result<LmsFreq> {
        let mut f = LmsFreq::default();
        let base: u8 = if channel == Channel::Rx { 0x20 } else { 0x10 };
        let data = self.read(base)?;
        f.nint = (data as u16) << 1;
        let data = self.read(base + 1)?;
        f.nint |= ((data & 0x80) >> 7) as u16;
        f.nfrac = (data as u32 & 0x7f) << 16;
        let data = self.read(base + 2)?;
        f.nfrac |= (data as u32) << 8;
        let data = self.read(base + 3)?;
        f.nfrac |= data as u32;
        let data = self.read(base + 5)?;
        f.freqsel = data >> 2;
        let frange = f.freqsel & 7;
        if frange < 4 {
            return Err(crate::error::Error::BoardState(
                "PLL not configured (invalid FRANGE) — is the board initialized?",
            ));
        }
        f.x = 1 << (frange - 3);
        let data = self.read(base + 9)?;
        f.vcocap = data & 0x3f;
        Ok(f)
    }

    #[allow(dead_code)]
    pub(crate) fn peakdetect_enable(&mut self, enable: bool) -> crate::Result<()> {
        let mut data = self.read(0x44)?;
        if enable {
            data &= !(1 << 0);
        } else {
            data |= 1;
        }
        self.write(0x44, data)
    }

    pub(crate) fn get_quick_tune(
        &mut self,
        channel: Channel,
        xb200_enabled: bool,
    ) -> crate::Result<QuickTune> {
        let f = &self.get_frequency(channel)?;
        let xb_gpio = if xb200_enabled {
            let val = self.read_expansion_gpio()?;
            let mut gpio = LMS_FREQ_XB_200_ENABLE;
            match channel {
                Channel::Rx => {
                    gpio |= LMS_FREQ_XB_200_MODULE_RX;
                    gpio |= (((val & 0x30) >> 4) << LMS_FREQ_XB_200_PATH_SHIFT) as u8;
                    gpio |= (((val & 0x30000000) >> 28) << LMS_FREQ_XB_200_FILTER_SW_SHIFT) as u8;
                }
                Channel::Tx => {
                    gpio |= (((val & 0x0C) >> 2) << LMS_FREQ_XB_200_FILTER_SW_SHIFT) as u8;
                    gpio |= (((val & 0x0C000000) >> 26) << LMS_FREQ_XB_200_PATH_SHIFT) as u8;
                }
            }
            gpio
        } else {
            0
        };
        let mut flags = LMS_FREQ_FLAGS_FORCE_VCOCAP;
        let f_hz: u64 = f.into();
        if Band::from(f_hz) == Band::Low {
            flags |= LMS_FREQ_FLAGS_LOW_BAND;
        }
        Ok(QuickTune {
            freqsel: f.freqsel,
            vcocap: f.vcocap,
            nint: f.nint,
            nfrac: f.nfrac,
            flags,
            xb_gpio,
        })
    }

    fn write_pll_config(
        &mut self,
        channel: Channel,
        freqsel: u8,
        low_band: bool,
        lb_enabled: bool,
    ) -> crate::Result<()> {
        let addr = if channel == Channel::Tx { 0x15 } else { 0x25 };
        let mut regval = self.read(addr)?;
        if !lb_enabled {
            let selout = if low_band { 1 } else { 2 };
            regval = (freqsel << 2) | selout;
        } else {
            regval = (regval & !0xfc) | (freqsel << 2);
        }
        self.write(addr, regval)
    }

    fn vtune_high_to_norm(
        &mut self,
        base: u8,
        mut vcocap: u8,
        vcocap_reg_state: u8,
    ) -> crate::Result<u8> {
        for _ in 0..VTUNE_MAX_ITERATIONS {
            if vcocap >= VCOCAP_MAX_VALUE {
                log::trace!("vtune_high_to_norm: VCOCAP hit max value.");
                return Ok(VCOCAP_MAX_VALUE);
            }
            vcocap += 1;
            self.write_vcocap(base, vcocap, vcocap_reg_state)?;
            let vtune = self.get_vtune(base, VTUNE_DELAY_SMALL)?;
            if vtune == VcoState::Norm {
                log::trace!("VTUNE NORM @ VCOCAP={vcocap}");
                return Ok(vcocap - 1);
            }
        }
        log::error!("VTUNE High->Norm loop failed to converge.");
        Err(Error::CalibrationFailed(
            "VTUNE High->Norm loop failed to converge",
        ))
    }

    fn vtune_norm_to_high(
        &mut self,
        base: u8,
        mut vcocap: u8,
        vcocap_reg_state: u8,
    ) -> crate::Result<u8> {
        for _ in 0..VTUNE_MAX_ITERATIONS {
            log::trace!("base: {base}, vcocap: {vcocap}, vcocap_reg_state: {vcocap_reg_state}");
            if vcocap == 0 {
                log::debug!("vtune_norm_to_high: VCOCAP hit min value.");
                return Ok(0);
            }
            vcocap -= 1;
            self.write_vcocap(base, vcocap, vcocap_reg_state)?;
            let vtune = self.get_vtune(base, VTUNE_DELAY_SMALL)?;
            log::trace!("vtune: {vtune:?}");
            if vtune == VcoState::High {
                log::debug!("VTUNE HIGH @ VCOCAP={vcocap}");
                return Ok(vcocap);
            }
        }
        log::error!("VTUNE Norm->High loop failed to converge.");
        Err(Error::CalibrationFailed(
            "VTUNE Norm->High loop failed to converge",
        ))
    }

    fn vtune_low_to_norm(
        &mut self,
        base: u8,
        mut vcocap: u8,
        vcocap_reg_state: u8,
    ) -> crate::Result<u8> {
        for _ in 0..VTUNE_MAX_ITERATIONS {
            if vcocap == 0 {
                log::debug!("vtune_low_to_norm: VCOCAP hit min value.");
                return Ok(0);
            }
            vcocap -= 1;
            self.write_vcocap(base, vcocap, vcocap_reg_state)?;
            let vtune = self.get_vtune(base, VTUNE_DELAY_SMALL)?;
            if vtune == VcoState::Norm {
                log::debug!("VTUNE NORM @ VCOCAP={vcocap}");
                return Ok(vcocap + 1);
            }
        }
        log::error!("VTUNE Low->Norm loop failed to converge.");
        Err(Error::CalibrationFailed(
            "VTUNE Low->Norm loop failed to converge",
        ))
    }

    fn wait_for_vtune_value(
        &mut self,
        base: u8,
        target_value: VcoState,
        vcocap: &mut u8,
        vcocap_reg_state: u8,
    ) -> crate::Result<()> {
        const MAX_RETRIES: u32 = 15;
        let limit: u8 = if target_value == VcoState::High {
            0
        } else {
            VCOCAP_MAX_VALUE
        };
        let inc: i8 = if target_value == VcoState::High {
            -1
        } else {
            1
        };
        for i in 0..MAX_RETRIES {
            let vtune = self.get_vtune(base, 0)?;
            if vtune == target_value {
                log::debug!("VTUNE reached {target_value:?} at iteration {i}");
                return Ok(());
            } else {
                log::trace!("VTUNE was {vtune:?}. Waiting and retrying...");
                sleep(Duration::from_micros(10));
            }
        }
        log::trace!("Timed out while waiting for VTUNE={target_value:?}. Walking VCOCAP...");
        while *vcocap != limit {
            *vcocap = (*vcocap as i8 + inc) as u8;
            self.write_vcocap(base, *vcocap, vcocap_reg_state)?;
            let vtune = self.get_vtune(base, VTUNE_DELAY_SMALL)?;
            if vtune == target_value {
                log::debug!("VTUNE={vtune:?} reached with VCOCAP={vcocap}");
                return Ok(());
            }
        }
        log::debug!("VTUNE did not reach {target_value:?}. Tuning may not be nominal.");
        Ok(())
    }

    fn tune_vcocap(&mut self, vcocap_est: u8, base: u8, vcocap_reg_state: u8) -> crate::Result<u8> {
        let mut vcocap: u8 = vcocap_est;
        let mut vtune_high_limit: u8 = VCOCAP_MAX_VALUE;
        let mut vtune_low_limit: u8 = 0;
        let mut vtune = self.get_vtune(base, VTUNE_DELAY_LARGE)?;
        match vtune {
            VcoState::High => {
                log::trace!("Estimate HIGH: Walking down to NORM.");
                vtune_high_limit = self.vtune_high_to_norm(base, vcocap, vcocap_reg_state)?;
            }
            VcoState::Norm => {
                log::trace!("Estimate NORM: Walking up to HIGH.");
                vtune_high_limit = self.vtune_norm_to_high(base, vcocap, vcocap_reg_state)?;
            }
            VcoState::Low => {
                log::trace!("Estimate LOW: Walking down to NORM.");
                vtune_low_limit = self.vtune_low_to_norm(base, vcocap, vcocap_reg_state)?;
            }
        }
        if vtune_high_limit != VCOCAP_MAX_VALUE {
            match vtune {
                VcoState::Norm | VcoState::High => {
                    if (vtune_high_limit + VCOCAP_MAX_LOW_HIGH) < VCOCAP_MAX_VALUE {
                        vcocap = vtune_high_limit + VCOCAP_MAX_LOW_HIGH;
                    } else {
                        vcocap = VCOCAP_MAX_VALUE;
                        log::debug!("Clamping VCOCAP to {vcocap}.");
                    }
                }
                _ => {
                    log::error!("Invalid state");
                    return Err(Error::BoardState("VTUNE state mismatch after high_limit"));
                }
            }
            self.write_vcocap(base, vcocap, vcocap_reg_state)?;
            log::trace!("Waiting for VTUNE LOW @ VCOCAP={vcocap}");
            self.wait_for_vtune_value(base, VcoState::Low, &mut vcocap, vcocap_reg_state)?;
            log::trace!("Walking VTUNE LOW to NORM from VCOCAP={vcocap}");
            vtune_low_limit = self.vtune_low_to_norm(base, vcocap, vcocap_reg_state)?;
        } else {
            match vtune {
                VcoState::Low | VcoState::Norm => {
                    if (vtune_low_limit - VCOCAP_MAX_LOW_HIGH) > 0 {
                        vcocap = vtune_low_limit - VCOCAP_MAX_LOW_HIGH;
                    } else {
                        vcocap = 0;
                        log::debug!("Clamping VCOCAP to {vcocap}.");
                    }
                }
                _ => {
                    log::error!("Invalid state");
                    return Err(Error::BoardState("VTUNE state mismatch after low_limit"));
                }
            }
            self.write_vcocap(base, vcocap, vcocap_reg_state)?;
            log::trace!("Waiting for VTUNE HIGH @ VCOCAP={vcocap}");
            self.wait_for_vtune_value(base, VcoState::High, &mut vcocap, vcocap_reg_state)?;
            log::trace!("Walking VTUNE HIGH to NORM from VCOCAP={vcocap}");
            vtune_high_limit = self.vtune_high_to_norm(base, vcocap, vcocap_reg_state)?;
        }
        vcocap = vtune_high_limit + (vtune_low_limit - vtune_high_limit) / 2;
        log::trace!("VTUNE LOW:   {vtune_low_limit}");
        log::trace!("VTUNE NORM:  {vcocap}");
        log::trace!("VTUNE Est:   {vcocap_est}");
        log::trace!("VTUNE HIGH:  {vtune_high_limit}");
        self.write_vcocap(base, vcocap, vcocap_reg_state)?;
        vtune = self.get_vtune(base, VTUNE_DELAY_SMALL)?;
        if vtune != VcoState::Norm {
            log::error!("Final VCOCAP={vcocap} is not in VTUNE NORM region.");
            return Err(Error::TuningFailed);
        }
        Ok(vcocap)
    }

    fn turn_off_dsms(&mut self) -> crate::Result<()> {
        let mut data = self.read(0x09)?;
        data &= !0x05;
        self.write(0x09, data)
    }
}
