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
use crate::maybe_future::Op;
use nusb::MaybeFuture;
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
            freqsel: f.freqsel.bits(),
            vcocap: f.vcocap,
            nint: f.nint,
            nfrac: f.nfrac,
            flags: f.flags,
            xb_gpio: f.xb_gpio,
        }
    }
}

impl TryFrom<QuickTune> for LmsFreq {
    type Error = Error;

    fn try_from(qt: QuickTune) -> crate::Result<Self> {
        if qt.nint > 0x1ff
            || qt.nfrac > 0x7f_ffff
            || qt.vcocap > VCOCAP_MAX_VALUE
            || (qt.flags & !(LMS_FREQ_FLAGS_LOW_BAND | LMS_FREQ_FLAGS_FORCE_VCOCAP)) != 0
        {
            return Err(Error::Argument("invalid quick-tune PLL parameters".into()));
        }
        Ok(Self {
            freqsel: FrequencySelect::try_from(qt.freqsel)?,
            vcocap: qt.vcocap,
            nint: qt.nint,
            nfrac: qt.nfrac,
            flags: qt.flags,
            xb_gpio: qt.xb_gpio,
            vcocap_result: 0,
        })
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
pub(crate) struct FrequencySelect(u8);

impl TryFrom<u8> for FrequencySelect {
    type Error = Error;

    fn try_from(bits: u8) -> crate::Result<Self> {
        if BANDS.iter().any(|band| band.value == bits) {
            Ok(Self(bits))
        } else {
            Err(Error::Argument("invalid PLL frequency selector".into()))
        }
    }
}

impl FrequencySelect {
    pub(crate) const fn bits(self) -> u8 {
        self.0
    }

    const fn divider(self) -> u8 {
        1 << ((self.0 & 7) - 3)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FreqRange {
    low: u64,
    high: u64,
    value: u8,
}
pub(crate) const BANDS: [FreqRange; 16] = [
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LmsFreq {
    /// Frequency selector: VCO choice and post-divider.
    pub(crate) freqsel: FrequencySelect,
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
    /// Final VCOCAP value after VTUNE convergence search.
    pub(crate) vcocap_result: u8,
}
impl From<&LmsFreq> for u64 {
    fn from(value: &LmsFreq) -> Self {
        let pll_coeff = ((value.nint as u64) << 23) + value.nfrac as u64;
        let div = u64::from(value.freqsel.divider()) << 23;
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
        if !(BLADERF_FREQUENCY_MIN as u64..=BLADERF_FREQUENCY_MAX as u64).contains(&value) {
            return Err(Error::Argument(
                "frequency outside the LMS6002D range".into(),
            ));
        }
        let freq = value;
        let freq_range = BANDS
            .iter()
            .find(|freq_range| (freq >= freq_range.low) && (freq <= freq_range.high))
            .ok_or(Error::Argument(
                "Could not determine frequency range".into(),
            ))?;
        let freqsel = FrequencySelect::try_from(freq_range.value)?;
        log::trace!("freqsel: {freqsel:?}");
        let vcocap = estimate_vcocap(freq as u32, freq_range.low as u32, freq_range.high as u32);
        log::trace!("vcocap: {vcocap}");
        let vco_x = u64::from(freqsel.divider());
        log::trace!("vco_x: {vco_x}");
        let coefficient = (((vco_x * freq) << 23) + u64::from(LMS_REFERENCE_HZ) / 2)
            / u64::from(LMS_REFERENCE_HZ);
        let nint = (coefficient >> 23) as u16;
        log::trace!("nint: {nint}");
        let nfrac = (coefficient & 0x7f_ffff) as u32;
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
    pub(crate) fn config_charge_pumps(
        &mut self,
        channel: Channel,
    ) -> impl MaybeFuture<Output = crate::Result<()>> {
        Op::new(async move {
            let base: u8 = if channel == Channel::Rx { 0x20 } else { 0x10 };
            let mut data = self.read(base + 6).await?;
            data &= !0x1f;
            data |= 0x0c;
            self.write(base + 6, data).await?;
            let mut data = self.read(base + 7).await?;
            data &= !0x1f;
            data |= 0x03;
            self.write(base + 7, data).await?;
            let mut data = self.read(base + 8).await?;
            data &= !0x1f;
            data |= 0x03;
            self.write(base + 8, data).await
        })
    }

    fn write_vcocap(
        &mut self,
        base: u8,
        vcocap: u8,
        vcocap_reg_state: u8,
    ) -> impl MaybeFuture<Output = crate::Result<()>> {
        Op::new(async move {
            if vcocap > VCOCAP_MAX_VALUE {
                return Err(Error::Argument("vcocap exceeds maximum value".into()));
            }
            log::trace!("Writing VCOCAP={vcocap}");
            self.write(base + 9, vcocap | vcocap_reg_state).await
        })
    }

    fn get_vtune(
        &mut self,
        base: u8,
        delay: u8,
    ) -> impl MaybeFuture<Output = crate::Result<VcoState>> {
        Op::new(async move {
            if delay != 0 {
                crate::maybe_future::sleep(Duration::from_micros(delay as u64)).await;
            }
            let vtune = self.read(base + 10).await?;
            VcoState::try_from(vtune >> 6)
        })
    }

    fn set_precalculated_frequency(
        &mut self,
        channel: Channel,
        f: &mut LmsFreq,
    ) -> impl MaybeFuture<Output = crate::Result<()>> {
        Op::new(async move {
            let restoration = self.nios.save_lms_registers(&[0x09]).await?;
            let result = self.program_frequency(channel, f).await;
            self.nios.finish_restoration(restoration, result).await
        })
    }

    fn program_frequency(
        &mut self,
        channel: Channel,
        f: &mut LmsFreq,
    ) -> impl MaybeFuture<Output = crate::Result<()>> {
        Op::new(async move {
            let base: u8 = if channel == Channel::Rx { 0x20 } else { 0x10 };
            let pll_base: u8 = base | 0x80;
            f.vcocap_result = 0xff;
            let mut data = self.read(0x09).await?;
            data |= 0x05;
            self.write(0x09, data).await?;
            let vcocap_reg_state = self.read(base + 9).await?;
            let vcocap_reg_state = vcocap_reg_state & !0x3f;
            self.write_vcocap(base, f.vcocap, vcocap_reg_state).await?;
            let low_band = (f.flags & LMS_FREQ_FLAGS_LOW_BAND) != 0;
            let lben_lbrfen = self.read(0x08).await?;
            let loopbben = self.read(0x46).await?;
            let lb_enabled = matches!(lben_lbrfen & 0x7, 1..=3)
                || ((lben_lbrfen & 0x70) != 0 && (loopbben & 0x0c) != 0);
            self.write_pll_config(channel, f.freqsel.bits(), low_band, lb_enabled)
                .await?;
            let mut freq_data = [0u8; 4];
            freq_data[0] = (f.nint >> 1) as u8;
            freq_data[1] = (((f.nint & 1) << 7) as u32 | ((f.nfrac >> 16) & 0x7f)) as u8;
            freq_data[2] = ((f.nfrac >> 8) & 0xff) as u8;
            freq_data[3] = (f.nfrac & 0xff) as u8;
            for (idx, value) in freq_data.iter().enumerate() {
                self.write(pll_base + idx as u8, *value).await?;
            }
            if (f.flags & LMS_FREQ_FLAGS_FORCE_VCOCAP) != 0 {
                f.vcocap_result = f.vcocap;
            } else {
                log::trace!("Tuning VCOCAP...");
                f.vcocap_result = self.tune_vcocap(f.vcocap, base, vcocap_reg_state).await?;
            }
            Ok(())
        })
    }

    pub(crate) fn set_frequency(
        &mut self,
        channel: Channel,
        freq: u64,
    ) -> impl MaybeFuture<Output = crate::Result<()>> {
        Op::new(async move {
            let mut f = freq.try_into()?;
            log::trace!("{f:?}");
            self.set_precalculated_frequency(channel, &mut f).await
        })
    }

    pub(crate) fn get_frequency(
        &mut self,
        channel: Channel,
    ) -> impl MaybeFuture<Output = crate::Result<LmsFreq>> {
        Op::new(async move {
            let base: u8 = if channel == Channel::Rx { 0x20 } else { 0x10 };
            let data = self.read(base).await?;
            let mut nint = (data as u16) << 1;
            let data = self.read(base + 1).await?;
            nint |= ((data & 0x80) >> 7) as u16;
            let mut nfrac = (data as u32 & 0x7f) << 16;
            let data = self.read(base + 2).await?;
            nfrac |= (data as u32) << 8;
            let data = self.read(base + 3).await?;
            nfrac |= data as u32;
            let data = self.read(base + 5).await?;
            if ((data >> 2) & 7) < 4 {
                return Err(crate::error::Error::NotInitialized);
            }
            let freqsel = FrequencySelect::try_from(data >> 2)
                .map_err(|_| Error::BoardState("invalid PLL frequency selector"))?;
            let data = self.read(base + 9).await?;
            Ok(LmsFreq {
                freqsel,
                nint,
                nfrac,
                vcocap: data & 0x3f,
                flags: 0,
                xb_gpio: 0,
                vcocap_result: 0,
            })
        })
    }

    #[allow(dead_code)]
    pub(crate) fn peakdetect_enable(
        &mut self,
        enable: bool,
    ) -> impl MaybeFuture<Output = crate::Result<()>> {
        Op::new(async move {
            let mut data = self.read(0x44).await?;
            if enable {
                data &= !(1 << 0);
            } else {
                data |= 1;
            }
            self.write(0x44, data).await
        })
    }

    pub(crate) fn get_quick_tune(
        &mut self,
        channel: Channel,
        xb200_enabled: bool,
    ) -> impl MaybeFuture<Output = crate::Result<QuickTune>> {
        Op::new(async move {
            let f = &self.get_frequency(channel).await?;
            let xb_gpio = if xb200_enabled {
                let val = self.read_expansion_gpio().await?;
                let mut gpio = LMS_FREQ_XB_200_ENABLE;
                match channel {
                    Channel::Rx => {
                        gpio |= LMS_FREQ_XB_200_MODULE_RX;
                        gpio |= (((val & 0x30) >> 4) << LMS_FREQ_XB_200_PATH_SHIFT) as u8;
                        gpio |=
                            (((val & 0x30000000) >> 28) << LMS_FREQ_XB_200_FILTER_SW_SHIFT) as u8;
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
                freqsel: f.freqsel.bits(),
                vcocap: f.vcocap,
                nint: f.nint,
                nfrac: f.nfrac,
                flags,
                xb_gpio,
            })
        })
    }

    fn write_pll_config(
        &mut self,
        channel: Channel,
        freqsel: u8,
        low_band: bool,
        lb_enabled: bool,
    ) -> impl MaybeFuture<Output = crate::Result<()>> {
        Op::new(async move {
            let addr = if channel == Channel::Tx { 0x15 } else { 0x25 };
            let mut regval = self.read(addr).await?;
            if !lb_enabled {
                let selout = if low_band { 1 } else { 2 };
                regval = (freqsel << 2) | selout;
            } else {
                regval = (regval & !0xfc) | (freqsel << 2);
            }
            self.write(addr, regval).await
        })
    }

    fn vtune_high_to_norm(
        &mut self,
        base: u8,
        mut vcocap: u8,
        vcocap_reg_state: u8,
    ) -> impl MaybeFuture<Output = crate::Result<u8>> {
        Op::new(async move {
            for _ in 0..VTUNE_MAX_ITERATIONS {
                if vcocap >= VCOCAP_MAX_VALUE {
                    log::trace!("vtune_high_to_norm: VCOCAP hit max value.");
                    return Ok(VCOCAP_MAX_VALUE);
                }
                vcocap += 1;
                self.write_vcocap(base, vcocap, vcocap_reg_state).await?;
                let vtune = self.get_vtune(base, VTUNE_DELAY_SMALL).await?;
                if vtune == VcoState::Norm {
                    log::trace!("VTUNE NORM @ VCOCAP={vcocap}");
                    return Ok(vcocap - 1);
                }
            }
            log::error!("VTUNE High->Norm loop failed to converge.");
            Err(Error::CalibrationFailed(
                "VTUNE High->Norm loop failed to converge",
            ))
        })
    }

    fn vtune_norm_to_high(
        &mut self,
        base: u8,
        mut vcocap: u8,
        vcocap_reg_state: u8,
    ) -> impl MaybeFuture<Output = crate::Result<u8>> {
        Op::new(async move {
            for _ in 0..VTUNE_MAX_ITERATIONS {
                log::trace!("base: {base}, vcocap: {vcocap}, vcocap_reg_state: {vcocap_reg_state}");
                if vcocap == 0 {
                    log::debug!("vtune_norm_to_high: VCOCAP hit min value.");
                    return Ok(0);
                }
                vcocap -= 1;
                self.write_vcocap(base, vcocap, vcocap_reg_state).await?;
                let vtune = self.get_vtune(base, VTUNE_DELAY_SMALL).await?;
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
        })
    }

    fn vtune_low_to_norm(
        &mut self,
        base: u8,
        mut vcocap: u8,
        vcocap_reg_state: u8,
    ) -> impl MaybeFuture<Output = crate::Result<u8>> {
        Op::new(async move {
            for _ in 0..VTUNE_MAX_ITERATIONS {
                if vcocap == 0 {
                    log::debug!("vtune_low_to_norm: VCOCAP hit min value.");
                    return Ok(0);
                }
                vcocap -= 1;
                self.write_vcocap(base, vcocap, vcocap_reg_state).await?;
                let vtune = self.get_vtune(base, VTUNE_DELAY_SMALL).await?;
                if vtune == VcoState::Norm {
                    log::debug!("VTUNE NORM @ VCOCAP={vcocap}");
                    return Ok(vcocap + 1);
                }
            }
            log::error!("VTUNE Low->Norm loop failed to converge.");
            Err(Error::CalibrationFailed(
                "VTUNE Low->Norm loop failed to converge",
            ))
        })
    }

    fn wait_for_vtune_value(
        &mut self,
        base: u8,
        target_value: VcoState,
        vcocap: &mut u8,
        vcocap_reg_state: u8,
    ) -> impl MaybeFuture<Output = crate::Result<()>> {
        Op::new(async move {
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
                let vtune = self.get_vtune(base, 0).await?;
                if vtune == target_value {
                    log::debug!("VTUNE reached {target_value:?} at iteration {i}");
                    return Ok(());
                } else {
                    log::trace!("VTUNE was {vtune:?}. Waiting and retrying...");
                    crate::maybe_future::sleep(Duration::from_micros(10)).await;
                }
            }
            log::trace!("Timed out while waiting for VTUNE={target_value:?}. Walking VCOCAP...");
            while *vcocap != limit {
                *vcocap = (*vcocap as i8 + inc) as u8;
                self.write_vcocap(base, *vcocap, vcocap_reg_state).await?;
                let vtune = self.get_vtune(base, VTUNE_DELAY_SMALL).await?;
                if vtune == target_value {
                    log::debug!("VTUNE={vtune:?} reached with VCOCAP={vcocap}");
                    return Ok(());
                }
            }
            log::debug!("VTUNE did not reach {target_value:?}. Tuning may not be nominal.");
            Ok(())
        })
    }

    fn tune_vcocap(
        &mut self,
        vcocap_est: u8,
        base: u8,
        vcocap_reg_state: u8,
    ) -> impl MaybeFuture<Output = crate::Result<u8>> {
        Op::new(async move {
            let mut vcocap: u8 = vcocap_est;
            let mut vtune_high_limit: u8 = VCOCAP_MAX_VALUE;
            let mut vtune_low_limit: u8 = 0;
            let mut vtune = self.get_vtune(base, VTUNE_DELAY_LARGE).await?;
            match vtune {
                VcoState::High => {
                    log::trace!("Estimate HIGH: Walking down to NORM.");
                    vtune_high_limit = self
                        .vtune_high_to_norm(base, vcocap, vcocap_reg_state)
                        .await?;
                }
                VcoState::Norm => {
                    log::trace!("Estimate NORM: Walking up to HIGH.");
                    vtune_high_limit = self
                        .vtune_norm_to_high(base, vcocap, vcocap_reg_state)
                        .await?;
                }
                VcoState::Low => {
                    log::trace!("Estimate LOW: Walking down to NORM.");
                    vtune_low_limit = self
                        .vtune_low_to_norm(base, vcocap, vcocap_reg_state)
                        .await?;
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
                self.write_vcocap(base, vcocap, vcocap_reg_state).await?;
                log::trace!("Waiting for VTUNE LOW @ VCOCAP={vcocap}");
                self.wait_for_vtune_value(base, VcoState::Low, &mut vcocap, vcocap_reg_state)
                    .await?;
                log::trace!("Walking VTUNE LOW to NORM from VCOCAP={vcocap}");
                vtune_low_limit = self
                    .vtune_low_to_norm(base, vcocap, vcocap_reg_state)
                    .await?;
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
                self.write_vcocap(base, vcocap, vcocap_reg_state).await?;
                log::trace!("Waiting for VTUNE HIGH @ VCOCAP={vcocap}");
                self.wait_for_vtune_value(base, VcoState::High, &mut vcocap, vcocap_reg_state)
                    .await?;
                log::trace!("Walking VTUNE HIGH to NORM from VCOCAP={vcocap}");
                vtune_high_limit = self
                    .vtune_high_to_norm(base, vcocap, vcocap_reg_state)
                    .await?;
            }
            vcocap = vtune_high_limit + (vtune_low_limit - vtune_high_limit) / 2;
            log::trace!("VTUNE LOW:   {vtune_low_limit}");
            log::trace!("VTUNE NORM:  {vcocap}");
            log::trace!("VTUNE Est:   {vcocap_est}");
            log::trace!("VTUNE HIGH:  {vtune_high_limit}");
            self.write_vcocap(base, vcocap, vcocap_reg_state).await?;
            vtune = self.get_vtune(base, VTUNE_DELAY_SMALL).await?;
            if vtune != VcoState::Norm {
                log::error!("Final VCOCAP={vcocap} is not in VTUNE NORM region.");
                return Err(Error::TuningFailed);
            }
            Ok(vcocap)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selectors_and_quick_tunes_preserve_frequency_at_band_boundaries() {
        for band in BANDS {
            let selector = FrequencySelect::try_from(band.value).unwrap();
            assert!(matches!(selector.divider(), 2 | 4 | 8 | 16));
            for frequency in [band.low, band.high, (band.low + band.high) / 2] {
                let pll = LmsFreq::try_from(frequency).unwrap();
                assert!(u64::from(&pll).abs_diff(frequency) <= 1);
                assert!(pll.nfrac <= 0x7f_ffff);
                let restored = LmsFreq::try_from(QuickTune::from(&pll)).unwrap();
                assert_eq!(u64::from(&restored), u64::from(&pll));
            }
        }
    }

    #[test]
    fn invalid_pll_parameters_are_rejected() {
        for bits in 0..=u8::MAX {
            assert_eq!(
                FrequencySelect::try_from(bits).is_ok(),
                BANDS.iter().any(|band| band.value == bits)
            );
        }
        assert!(LmsFreq::try_from(0).is_err());
        assert!(LmsFreq::try_from(u64::MAX).is_err());
        let pll = LmsFreq::try_from(915_000_000).unwrap();
        let mut quick = QuickTune::from(&pll);
        quick.nfrac = 0x80_0000;
        assert!(LmsFreq::try_from(quick).is_err());
        quick = QuickTune::from(&pll);
        quick.vcocap = 64;
        assert!(LmsFreq::try_from(quick).is_err());
    }
}
