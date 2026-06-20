//! LMS6002D internal DC offset calibration engine.
//!
//! The LMS6002D contains dedicated submodules for measuring and compensating
//! DC offset in the I/Q signal path. Calibration runs iteratively per submodule,
//! adjusting gain settings if convergence is not achieved. The process sequences
//! through LPF tuning, TX LPF, RX LPF, and RX VGA2 submodules, backing up and
//! restoring surrounding register state. Each submodule runs a DC measurement
//! loop; if the result is suboptimal, the gain is stepped down and measurement
//! is retried until convergence or the minimum gain is reached.

use crate::Channel;
use crate::bladerf1::hardware::lms6002d::Lms6002d;
use crate::bladerf1::hardware::lms6002d::gain::{
    GAIN_SPEC_LNA, GAIN_SPEC_RXVGA1, GAIN_SPEC_RXVGA2, LnaGainCode,
};
use crate::error::{Error, Result};
use std::cmp::PartialEq;
use std::fmt::{Display, Formatter};

/// I/Q DC calibration pair with support for linear interpolation between samples.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct DcPair {
    /// I channel calibration value.
    pub i: i16,
    /// Q channel calibration value.
    pub q: i16,
}

impl DcPair {
    /// Creates a new I/Q pair.
    pub fn new(i: i16, q: i16) -> Self {
        Self { i, q }
    }

    /// Linearly interpolates between two calibration pairs at given sample positions.
    pub fn interp(x0: u32, y0: DcPair, x1: u32, y1: DcPair, x: u32) -> DcPair {
        DcPair {
            i: interp(x0, y0.i, x1, y1.i, x),
            q: interp(x0, y0.q, x1, y1.q, x),
        }
    }
}

/// AGC DC correction values at three gain settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AgcDcCorrection {
    /// DC correction at maximum AGC gain.
    pub max: DcPair,
    /// DC correction at mid AGC gain.
    pub mid: DcPair,
    /// DC correction at minimum AGC gain.
    pub min: DcPair,
}

fn interp(x0: u32, y0: i16, x1: u32, y1: i16, x: u32) -> i16 {
    if x1 == x0 {
        return y0;
    }
    let num = (y1 as i64 - y0 as i64) * (x as i64 - x0 as i64);
    let den = x1 as i64 - x0 as i64;
    (y0 as i64 + num / den) as i16
}
/// All DC calibration register values with accessor methods.
#[derive(Debug, PartialEq, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct DcCals {
    pub(crate) lpf_tuning: i16,
    pub(crate) tx_lpf_i: i16,
    pub(crate) tx_lpf_q: i16,
    pub(crate) rx_lpf_i: i16,
    pub(crate) rx_lpf_q: i16,
    pub(crate) dc_ref: i16,
    pub(crate) rxvga2a_i: i16,
    pub(crate) rxvga2a_q: i16,
    pub(crate) rxvga2b_i: i16,
    pub(crate) rxvga2b_q: i16,
}
impl DcCals {
    #[allow(clippy::too_many_arguments)]
    /// Creates a new `DcCals` from individual register values.
    pub fn new(
        lpf_tuning: i16,
        tx_lpf_i: i16,
        tx_lpf_q: i16,
        rx_lpf_i: i16,
        rx_lpf_q: i16,
        dc_ref: i16,
        rxvga2a_i: i16,
        rxvga2a_q: i16,
        rxvga2b_i: i16,
        rxvga2b_q: i16,
    ) -> Self {
        Self {
            lpf_tuning,
            tx_lpf_i,
            tx_lpf_q,
            rx_lpf_i,
            rx_lpf_q,
            dc_ref,
            rxvga2a_i,
            rxvga2a_q,
            rxvga2b_i,
            rxvga2b_q,
        }
    }
    /// Returns the LPF tuning module DC calibration value.
    pub fn lpf_tuning(&self) -> i16 {
        self.lpf_tuning
    }
    /// Returns the TX LPF I channel DC calibration value.
    pub fn tx_lpf_i(&self) -> i16 {
        self.tx_lpf_i
    }
    /// Returns the TX LPF Q channel DC calibration value.
    pub fn tx_lpf_q(&self) -> i16 {
        self.tx_lpf_q
    }
    /// Returns the RX LPF I channel DC calibration value.
    pub fn rx_lpf_i(&self) -> i16 {
        self.rx_lpf_i
    }
    /// Returns the RX LPF Q channel DC calibration value.
    pub fn rx_lpf_q(&self) -> i16 {
        self.rx_lpf_q
    }
    /// Returns the RX VGA2 DC reference value.
    pub fn dc_ref(&self) -> i16 {
        self.dc_ref
    }
    /// Returns the RX VGA2 stage 1 I channel DC calibration value.
    pub fn rxvga2a_i(&self) -> i16 {
        self.rxvga2a_i
    }
    /// Returns the RX VGA2 stage 1 Q channel DC calibration value.
    pub fn rxvga2a_q(&self) -> i16 {
        self.rxvga2a_q
    }
    /// Returns the RX VGA2 stage 2 I channel DC calibration value.
    pub fn rxvga2b_i(&self) -> i16 {
        self.rxvga2b_i
    }
    /// Returns the RX VGA2 stage 2 Q channel DC calibration value.
    pub fn rxvga2b_q(&self) -> i16 {
        self.rxvga2b_q
    }
}
impl Display for DcCals {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "LPF tuning module: {}", self.lpf_tuning)?;
        writeln!(f, "TX LPF I filter: {}", self.tx_lpf_i)?;
        writeln!(f, "TX LPF Q filter: {}", self.tx_lpf_q)?;
        writeln!(f, "RX LPF I filter: {}", self.rx_lpf_i)?;
        writeln!(f, "RX LPF Q filter: {}", self.rx_lpf_q)?;
        writeln!(f, "RX VGA2 DC reference module: {}", self.dc_ref)?;
        writeln!(f, "RX VGA2 stage 1, I channel: {}", self.rxvga2a_i)?;
        writeln!(f, "RX VGA2 stage 1, Q channel: {}", self.rxvga2a_q)?;
        writeln!(f, "RX VGA2 stage 2, I channel: {}", self.rxvga2b_i)?;
        writeln!(f, "RX VGA2 stage 2, Q channel: {}", self.rxvga2b_q)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DcCalState {
    clk_en: u8,
    reg0x72: u8,
    lna_gain: LnaGainCode,
    rxvga1_gain: i32,
    rxvga2_gain: i32,
    rxvga1_curr_gain: i32,
    rxvga2_curr_gain: i32,
}

/// DC calibration target submodule.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum DcCalModule {
    /// No valid module (error value).
    Invalid = -1,
    /// LPF tuning module.
    LpfTuning,
    /// TX LPF submodule.
    TxLpf,
    /// RX LPF submodule.
    RxLpf,
    /// RX VGA2 submodule.
    RxVga2,
}

impl DcCalModule {
    /// Base register address for this calibration module.
    pub(crate) const fn base_addr(self) -> u8 {
        match self {
            Self::LpfTuning => 0x00,
            Self::TxLpf => 0x30,
            Self::RxLpf => 0x50,
            Self::RxVga2 => 0x60,
            Self::Invalid => unreachable!(),
        }
    }

    /// Number of submodules within this calibration module.
    pub(crate) const fn num_submodules(self) -> u8 {
        match self {
            Self::LpfTuning => 1,
            Self::TxLpf => 2,
            Self::RxLpf => 2,
            Self::RxVga2 => 5,
            Self::Invalid => unreachable!(),
        }
    }

    /// Bit mask in register 0x09 to enable the calibration clock for this module.
    pub(crate) const fn cal_clock_mask(self) -> u8 {
        match self {
            Self::LpfTuning => 1 << 5,
            Self::TxLpf => 1 << 1,
            Self::RxLpf => 1 << 3,
            Self::RxVga2 => 1 << 4,
            Self::Invalid => unreachable!(),
        }
    }
}
/// RX DC calibration configuration with sample count for interpolation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RxCal {
    /// Number of sample points for gain-dependent interpolation.
    num_samples: u32,
    /// Timestamp of the calibration data.
    ts: u64,
    /// TX frequency at which the calibration was performed.
    tx_freq: u64,
}
impl RxCal {
    pub fn new(num_samples: u32, ts: u64, tx_freq: u64) -> Self {
        Self {
            num_samples,
            ts,
            tx_freq,
        }
    }

    pub fn sample_count(&self) -> u32 {
        self.num_samples
    }

    pub fn timestamp(&self) -> u64 {
        self.ts
    }

    pub fn tx_frequency(&self) -> u64 {
        self.tx_freq
    }

    pub fn set_timestamp(&mut self, ts: u64) {
        self.ts = ts;
    }

    pub fn set_tx_frequency(&mut self, tx_freq: u64) {
        self.tx_freq = tx_freq;
    }
}

/// Backup of device state before RX DC calibration.
#[derive(Debug, Clone, PartialEq, Eq, Copy)]
pub struct RxCalBackup {
    /// Rational sample rate before calibration.
    rational_sample_rate: crate::bladerf1::hardware::si5338::RationalRate,
    /// Bandwidth setting before calibration.
    bandwidth: u32,
    /// TX frequency before calibration.
    tx_freq: u64,
}
impl RxCalBackup {
    pub fn new(
        rational_sample_rate: crate::bladerf1::hardware::si5338::RationalRate,
        bandwidth: u32,
        tx_freq: u64,
    ) -> Self {
        Self {
            rational_sample_rate,
            bandwidth,
            tx_freq,
        }
    }

    pub fn sample_rate(&self) -> &crate::bladerf1::hardware::si5338::RationalRate {
        &self.rational_sample_rate
    }

    pub(crate) fn sample_rate_mut(
        &mut self,
    ) -> &mut crate::bladerf1::hardware::si5338::RationalRate {
        &mut self.rational_sample_rate
    }

    pub fn bandwidth(&self) -> u32 {
        self.bandwidth
    }

    pub fn tx_frequency(&self) -> u64 {
        self.tx_freq
    }
}
fn dc_offset_i_addr(channel: Channel) -> u8 {
    if channel == Channel::Tx { 0x42 } else { 0x71 }
}

fn dc_offset_q_addr(channel: Channel) -> u8 {
    if channel == Channel::Tx { 0x43 } else { 0x72 }
}

fn scale_dc_offset(channel: Channel, mut value: i16) -> u8 {
    match channel {
        Channel::Rx => {
            value >>= 5;
            if value < 0 {
                if value <= -64 {
                    value = 0x3f;
                } else {
                    value = (-value) & 0x3f;
                }
                value |= 1 << 6;
            } else if value >= 64 {
                value = 0x3f;
            } else {
                value &= 0x3f;
            }
            value as u8
        }
        Channel::Tx => {
            value >>= 4;
            if value >= 0 {
                let ret = (if value >= 128 { 0x7f } else { value & 0x7f }) as u8;
                (1 << 7) | ret
            } else {
                (if value <= -128 { 0x00 } else { value & 0x7f }) as u8
            }
        }
    }
}
fn unscale_dc_offset(channel: Channel, mut regval: u8) -> i16 {
    match channel {
        Channel::Rx => {
            regval &= 0x7f;
            let value = if (regval & (1 << 6)) != 0 {
                -((regval & 0x3f) as i16)
            } else {
                (regval & 0x3f) as i16
            };
            value << 5
        }
        Channel::Tx => {
            let value = -(0x80 - regval as i16);
            value << 4
        }
    }
}
impl<'a> Lms6002d<'a> {
    pub(crate) fn calibrate_dc(&mut self, module: DcCalModule) -> Result<()> {
        let mut state = self.dc_cal_backup(module)?;
        if self.dc_cal_module_init(module, &mut state).is_err() {
            let _ = self.dc_cal_module_deinit(module);
            return self.dc_cal_restore(module, &state);
        }
        let mut converged = false;
        let mut limit_reached = false;
        while !converged && !limit_reached {
            if let Ok(c) = self.dc_cal_module(module, &mut state) {
                converged = c;
                if !converged {
                    if let Ok(l) = self.dc_cal_retry_adjustment(module, &mut state) {
                        limit_reached = l;
                    } else {
                        break;
                    }
                }
            } else {
                break;
            }
        }
        if !converged {
            log::warn!("DC Calibration (module={module:?}) failed to converge.");
        }
        let _ = self.dc_cal_module_deinit(module);
        self.dc_cal_restore(module, &state)
    }

    pub(crate) fn set_dc_cals(&mut self, dc_cals: DcCals) -> Result<()> {
        let cal_tx_lpf: bool = (dc_cals.tx_lpf_i >= 0) || (dc_cals.tx_lpf_q >= 0);
        let cal_rx_lpf: bool = (dc_cals.rx_lpf_i >= 0) || (dc_cals.rx_lpf_q >= 0);
        let cal_rxvga2: bool = (dc_cals.dc_ref >= 0)
            || (dc_cals.rxvga2a_i >= 0)
            || (dc_cals.rxvga2a_q >= 0)
            || (dc_cals.rxvga2b_i >= 0)
            || (dc_cals.rxvga2b_q >= 0);
        if dc_cals.lpf_tuning >= 0 {
            self.enable_lpf_cal_clock(true)?;
            self.set_dc_cal_value(0x00, 0, dc_cals.lpf_tuning as u8)?;
            self.enable_lpf_cal_clock(false)?;
        }
        if cal_tx_lpf {
            self.enable_txlpf_dccal_clock(true)?;
            if dc_cals.tx_lpf_i >= 0 {
                self.set_dc_cal_value(0x30, 0, dc_cals.tx_lpf_i as u8)?;
            }
            if dc_cals.tx_lpf_q >= 0 {
                self.set_dc_cal_value(0x30, 1, dc_cals.tx_lpf_q as u8)?;
            }
            self.enable_txlpf_dccal_clock(false)?;
        }
        if cal_rx_lpf {
            self.enable_rxlpf_dccal_clock(true)?;
            if dc_cals.rx_lpf_i >= 0 {
                self.set_dc_cal_value(0x50, 0, dc_cals.rx_lpf_i as u8)?;
            }
            if dc_cals.rx_lpf_q >= 0 {
                self.set_dc_cal_value(0x50, 1, dc_cals.rx_lpf_q as u8)?;
            }
            self.enable_rxlpf_dccal_clock(false)?;
        }
        if cal_rxvga2 {
            self.enable_rxvga2_dccal_clock(true)?;
            if dc_cals.dc_ref >= 0 {
                self.set_dc_cal_value(0x60, 0, dc_cals.dc_ref as u8)?;
            }
            if dc_cals.rxvga2a_i >= 0 {
                self.set_dc_cal_value(0x60, 1, dc_cals.rxvga2a_i as u8)?;
            }
            if dc_cals.rxvga2a_q >= 0 {
                self.set_dc_cal_value(0x60, 2, dc_cals.rxvga2a_q as u8)?;
            }
            if dc_cals.rxvga2b_i >= 0 {
                self.set_dc_cal_value(0x60, 3, dc_cals.rxvga2b_i as u8)?;
            }
            if dc_cals.rxvga2b_q >= 0 {
                self.set_dc_cal_value(0x60, 4, dc_cals.rxvga2b_q as u8)?;
            }
            self.enable_rxvga2_dccal_clock(false)?;
        }
        Ok(())
    }

    pub(crate) fn get_dc_cals(&mut self) -> Result<DcCals> {
        Ok(DcCals {
            lpf_tuning: self.get_dc_cal_value(0x00, 0)? as i16,
            tx_lpf_i: self.get_dc_cal_value(0x30, 0)? as i16,
            tx_lpf_q: self.get_dc_cal_value(0x30, 1)? as i16,
            rx_lpf_i: self.get_dc_cal_value(0x50, 0)? as i16,
            rx_lpf_q: self.get_dc_cal_value(0x50, 1)? as i16,
            dc_ref: self.get_dc_cal_value(0x60, 0)? as i16,
            rxvga2a_i: self.get_dc_cal_value(0x60, 1)? as i16,
            rxvga2a_q: self.get_dc_cal_value(0x60, 2)? as i16,
            rxvga2b_i: self.get_dc_cal_value(0x60, 3)? as i16,
            rxvga2b_q: self.get_dc_cal_value(0x60, 4)? as i16,
        })
    }

    pub(crate) fn set_dc_offset_i(&mut self, channel: Channel, value: i16) -> Result<()> {
        self.set_dc_offset(channel, dc_offset_i_addr(channel), value)
    }

    pub(crate) fn set_dc_offset_q(&mut self, channel: Channel, value: i16) -> Result<()> {
        self.set_dc_offset(channel, dc_offset_q_addr(channel), value)
    }

    pub(crate) fn get_dc_offset_i(&mut self, channel: Channel) -> Result<i16> {
        self.get_dc_offset(channel, dc_offset_i_addr(channel))
    }

    pub(crate) fn get_dc_offset_q(&mut self, channel: Channel) -> Result<i16> {
        self.get_dc_offset(channel, dc_offset_q_addr(channel))
    }

    fn dc_cal_loop(&mut self, base: u8, cal_address: u8, dc_cntval: u8) -> Result<u8> {
        log::debug!("Calibrating module {base:#x}:{cal_address:#x}");
        let mut val = self.read(base + 0x03)?;
        val &= !0x07;
        val |= cal_address & 0x07;
        self.write(base + 0x03, val)?;
        self.write(base + 0x02, dc_cntval)?;
        val |= 1 << 4;
        self.write(base + 0x03, val)?;
        val &= !(1 << 4);
        self.write(base + 0x03, val)?;
        val |= 1 << 5;
        self.write(base + 0x03, val)?;
        val &= !(1 << 5);
        self.write(base + 0x03, val)?;
        for _ in 0..25 {
            let val = self.read(base + 0x01)?;
            if ((val >> 1) & 1) == 0 {
                let dc_regval = self.read(base)? & 0x3f;
                log::debug!("DC_REGVAL: {dc_regval}");
                return Ok(dc_regval);
            }
        }
        log::warn!("DC calibration loop did not converge.");
        Err(Error::CalibrationFailed("loop did not converge"))
    }

    fn dc_cal_backup(&mut self, module: DcCalModule) -> Result<DcCalState> {
        let mut state = DcCalState {
            clk_en: self.read(0x09)?,
            reg0x72: 0,
            lna_gain: LnaGainCode::BypassLna1Lna2,
            rxvga1_gain: 0,
            rxvga2_gain: 0,
            rxvga1_curr_gain: 0,
            rxvga2_curr_gain: 0,
        };
        if module == DcCalModule::RxLpf || module == DcCalModule::RxVga2 {
            state.reg0x72 = self.read(0x72)?;
            state.lna_gain = LnaGainCode::from(self.lna_get_gain()?);
            state.rxvga1_gain = self.rxvga1_get_gain()?.db() as i32;
            state.rxvga2_gain = self.rxvga2_get_gain()?.db() as i32;
        }
        Ok(state)
    }

    fn dc_cal_module_init(&mut self, module: DcCalModule, state: &mut DcCalState) -> Result<()> {
        match module {
            DcCalModule::LpfTuning => {
                self.write(0x09, state.clk_en | module.cal_clock_mask())?;
            }
            DcCalModule::TxLpf => {
                self.write(0x09, state.clk_en | module.cal_clock_mask())?;
                self.set(0x36, 1 << 7)?;
                self.clear(0x3f, 1 << 7)?;
            }
            DcCalModule::RxLpf => {
                self.write(0x09, state.clk_en | module.cal_clock_mask())?;
                self.clear(0x5f, 1 << 7)?;
                self.write(0x72, state.reg0x72 & !(1 << 7))?;
                self.lna_set_gain(GAIN_SPEC_LNA.max.into())?;
                state.rxvga1_curr_gain = GAIN_SPEC_RXVGA1.max as i32;
                self.rxvga1_set_gain((state.rxvga1_curr_gain as i8).into())?;
                state.rxvga2_curr_gain = GAIN_SPEC_RXVGA2.max as i32;
                self.rxvga2_set_gain((state.rxvga2_curr_gain as i8).into())?;
            }
            DcCalModule::RxVga2 => {
                self.write(0x09, state.clk_en | module.cal_clock_mask())?;
                self.clear(0x6e, 3 << 6)?;
                self.write(0x72, state.reg0x72 & !(1 << 7))?;
                self.lna_set_gain(GAIN_SPEC_LNA.max.into())?;
                state.rxvga1_curr_gain = GAIN_SPEC_RXVGA1.max as i32;
                self.rxvga1_set_gain((state.rxvga1_curr_gain as i8).into())?;
                state.rxvga2_curr_gain = GAIN_SPEC_RXVGA2.max as i32;
                self.rxvga2_set_gain((state.rxvga2_curr_gain as i8).into())?;
            }
            _ => return Err(Error::Unsupported("DC calibration module")),
        }
        Ok(())
    }

    fn dc_cal_submodule(
        &mut self,
        module: DcCalModule,
        submodule: u8,
        _state: &DcCalState,
    ) -> Result<bool> {
        let mut converged: bool = false;
        if module == DcCalModule::RxVga2 {
            match submodule {
                0 => {
                    self.clear(0x64, 0x01)?;
                    self.write(0x68, 0x01)?;
                }
                1 => {
                    self.set(0x64, 0x01)?;
                    self.write(0x68, 0x06)?;
                }
                2 => {}
                3 => {
                    self.write(0x68, 0x60)?;
                }
                4 => {}
                _ => {
                    return Err(Error::CalibrationFailed("invalid submodule index"));
                }
            }
        }
        let base = module.base_addr();
        let mut dc_regval = self.dc_cal_loop(base, submodule, 31)?;
        if dc_regval == 31 {
            log::debug!("DC_REGVAL suboptimal value - retrying DC cal loop.");
            dc_regval = self.dc_cal_loop(base, submodule, 0)?;
            if dc_regval == 0 {
                log::debug!("Bad DC_REGVAL detected. DC cal failed.");
                return Ok(converged);
            }
        }
        if module == DcCalModule::LpfTuning {
            let mut val = self.read(0x35)?;
            val &= !0x3f;
            val |= dc_regval;
            self.write(0x35, val)?;
            let mut val = self.read(0x55)?;
            val &= !0x3f;
            val |= dc_regval;
            self.write(0x55, val)?;
        }
        converged = true;
        Ok(converged)
    }

    fn dc_cal_retry_adjustment(
        &mut self,
        module: DcCalModule,
        state: &mut DcCalState,
    ) -> Result<bool> {
        let mut limit_reached: bool = false;
        match module {
            DcCalModule::LpfTuning | DcCalModule::TxLpf => {
                limit_reached = true;
            }
            DcCalModule::RxLpf => {
                if state.rxvga1_curr_gain > GAIN_SPEC_RXVGA1.min as i32 {
                    state.rxvga1_curr_gain -= 1;
                    log::debug!("Retrying DC cal with RXVGA1={}", state.rxvga1_curr_gain);
                    self.rxvga1_set_gain((state.rxvga1_curr_gain as i8).into())?;
                } else {
                    limit_reached = true;
                }
            }
            DcCalModule::RxVga2 => {
                if state.rxvga1_curr_gain > GAIN_SPEC_RXVGA1.min as i32 {
                    state.rxvga1_curr_gain -= 1;
                    log::debug!("Retrying DC cal with RXVGA1={}", state.rxvga1_curr_gain);
                    self.rxvga1_set_gain((state.rxvga1_curr_gain as i8).into())?;
                } else if state.rxvga2_curr_gain > GAIN_SPEC_RXVGA2.min as i32 {
                    state.rxvga2_curr_gain -= 3;
                    log::debug!("Retrying DC cal with RXVGA2={}", state.rxvga2_curr_gain);
                    self.rxvga2_set_gain((state.rxvga2_curr_gain as i8).into())?;
                } else {
                    limit_reached = true;
                }
            }
            _ => {
                return Err(Error::Unsupported("DC calibration module"));
            }
        }
        if limit_reached {
            log::debug!("DC Cal retry limit reached");
        }
        Ok(limit_reached)
    }

    fn dc_cal_module_deinit(&mut self, module: DcCalModule) -> Result<()> {
        match module {
            DcCalModule::LpfTuning => {}
            DcCalModule::RxLpf => {
                self.set(0x5f, 1 << 7)?;
            }
            DcCalModule::RxVga2 => {
                self.write(0x68, 0x01)?;
                self.clear(0x64, 0x01)?;
                self.set(0x6e, 3 << 6)?;
            }
            DcCalModule::TxLpf => {
                self.set(0x3f, 1 << 7)?;
                self.clear(0x36, 1 << 7)?;
            }
            _ => {
                return Err(Error::Unsupported("DC calibration module"));
            }
        }
        Ok(())
    }

    fn dc_cal_restore(&mut self, module: DcCalModule, state: &DcCalState) -> Result<()> {
        self.write(0x09, state.clk_en)?;
        if module == DcCalModule::RxLpf || module == DcCalModule::RxVga2 {
            self.write(0x72, state.reg0x72)?;
            self.lna_set_gain(state.lna_gain.into())?;
            self.rxvga1_set_gain((state.rxvga1_gain as i8).into())?;
            self.rxvga2_set_gain((state.rxvga2_gain as i8).into())?;
        }
        Ok(())
    }

    fn dc_cal_module(&mut self, module: DcCalModule, state: &mut DcCalState) -> Result<bool> {
        let mut converged = true;
        for submodule in 0..module.num_submodules() {
            converged = self.dc_cal_submodule(module, submodule, state)?;
            if !converged {
                return Err(Error::CalibrationFailed("submodule did not converge"));
            }
        }
        Ok(converged)
    }

    fn set_cal_clock(&mut self, enable: bool, mask: u8) -> Result<()> {
        if enable {
            self.set(0x09, mask)
        } else {
            self.clear(0x09, mask)
        }
    }

    fn enable_lpf_cal_clock(&mut self, enable: bool) -> Result<()> {
        self.set_cal_clock(enable, 1 << 5)
    }

    fn enable_rxvga2_dccal_clock(&mut self, enable: bool) -> Result<()> {
        self.set_cal_clock(enable, 1 << 4)
    }

    fn enable_rxlpf_dccal_clock(&mut self, enable: bool) -> Result<()> {
        self.set_cal_clock(enable, 1 << 3)
    }

    fn enable_txlpf_dccal_clock(&mut self, enable: bool) -> Result<()> {
        self.set_cal_clock(enable, 1 << 1)
    }

    fn set_dc_cal_value(&mut self, base: u8, dc_addr: u8, value: u8) -> Result<u8> {
        let mut regval: u8 = 0x08 | dc_addr;
        self.write(base + 3, regval)?;
        self.write(base + 2, value)?;
        regval |= 1 << 4;
        self.write(base + 3, regval)?;
        regval &= !(1 << 4);
        self.write(base + 3, regval)?;
        self.read(base)
    }

    fn get_dc_cal_value(&mut self, base: u8, dc_addr: u8) -> Result<u8> {
        self.write(base + 3, 0x08 | dc_addr)?;
        self.read(base)
    }

    fn set_dc_offset(&mut self, channel: Channel, addr: u8, value: i16) -> Result<()> {
        let regval = match channel {
            Channel::Rx => {
                let tmp = self.read(addr)?;
                tmp & (1 << 7) | scale_dc_offset(channel, value)
            }
            Channel::Tx => scale_dc_offset(channel, value),
        };
        self.write(addr, regval)
    }

    fn get_dc_offset(&mut self, channel: Channel, addr: u8) -> Result<i16> {
        let regval = self.read(addr)?;
        Ok(unscale_dc_offset(channel, regval))
    }
}
