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
    GAIN_SPEC_LNA, GAIN_SPEC_RXVGA1, GAIN_SPEC_RXVGA2,
};
use crate::error::{Error, Result};
use crate::maybe_future::Op;
use nusb::MaybeFuture;
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
/// A validated six-bit LMS6002D DC calibration register value.
#[derive(
    Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy, serde::Serialize, serde::Deserialize,
)]
#[serde(try_from = "u8", into = "u8")]
pub struct DcCalValue(u8);

impl DcCalValue {
    /// Validates a register value in the inclusive range `0..=63`.
    ///
    /// # Errors
    /// Returns an argument error for values above 63.
    pub fn new(value: u8) -> Result<Self> {
        if value > 63 {
            return Err(Error::Argument(
                "DC calibration value exceeds six bits".into(),
            ));
        }
        Ok(Self(value))
    }

    /// Returns the six-bit register value.
    pub fn get(self) -> u8 {
        self.0
    }
}

impl TryFrom<u8> for DcCalValue {
    type Error = Error;
    fn try_from(value: u8) -> Result<Self> {
        Self::new(value)
    }
}

impl From<DcCalValue> for u8 {
    fn from(value: DcCalValue) -> Self {
        value.0
    }
}

impl Display for DcCalValue {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// Named DC calibration updates; absent fields leave their registers unchanged.
///
/// `Default` changes nothing. Readback fills every field. JSON accepts null or
/// omitted fields for unchanged registers and integers in `0..=63` for updates.
#[derive(Debug, Default, PartialEq, Eq, Clone, Copy, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DcCals {
    /// LPF tuning module.
    pub lpf_tuning: Option<DcCalValue>,
    /// TX LPF I filter.
    pub tx_lpf_i: Option<DcCalValue>,
    /// TX LPF Q filter.
    pub tx_lpf_q: Option<DcCalValue>,
    /// RX LPF I filter.
    pub rx_lpf_i: Option<DcCalValue>,
    /// RX LPF Q filter.
    pub rx_lpf_q: Option<DcCalValue>,
    /// RX VGA2 DC reference.
    pub dc_ref: Option<DcCalValue>,
    /// RX VGA2 stage A, I channel.
    pub rxvga2a_i: Option<DcCalValue>,
    /// RX VGA2 stage A, Q channel.
    pub rxvga2a_q: Option<DcCalValue>,
    /// RX VGA2 stage B, I channel.
    pub rxvga2b_i: Option<DcCalValue>,
    /// RX VGA2 stage B, Q channel.
    pub rxvga2b_q: Option<DcCalValue>,
}

impl DcCals {
    fn modules(self) -> [(DcCalModule, [Option<DcCalValue>; 5]); 4] {
        [
            (
                DcCalModule::LpfTuning,
                [self.lpf_tuning, None, None, None, None],
            ),
            (
                DcCalModule::TxLpf,
                [self.tx_lpf_i, self.tx_lpf_q, None, None, None],
            ),
            (
                DcCalModule::RxLpf,
                [self.rx_lpf_i, self.rx_lpf_q, None, None, None],
            ),
            (
                DcCalModule::RxVga2,
                [
                    self.dc_ref,
                    self.rxvga2a_i,
                    self.rxvga2a_q,
                    self.rxvga2b_i,
                    self.rxvga2b_q,
                ],
            ),
        ]
    }
}
impl Display for DcCals {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        for (module, values) in self.modules() {
            for (index, value) in values
                .into_iter()
                .take(module.num_submodules().into())
                .enumerate()
            {
                match value {
                    Some(value) => writeln!(f, "{module:?}[{index}]: {value}")?,
                    None => writeln!(f, "{module:?}[{index}]: unchanged")?,
                }
            }
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DcCalState {
    clk_en: u8,
    reg0x72: u8,
    rxvga1_curr_gain: i32,
    rxvga2_curr_gain: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Measurement {
    Value(u8),
    Incomplete,
}

fn calibrated_value(initial: Measurement, retry: Option<Measurement>) -> Option<u8> {
    match (initial, retry) {
        (Measurement::Incomplete, _) => None,
        (Measurement::Value(31), Some(Measurement::Value(value))) if value != 0 => Some(value),
        (Measurement::Value(31), _) => None,
        (Measurement::Value(value), _) => Some(value),
    }
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
    fn temporary_registers(self) -> Result<&'static [u8]> {
        match self {
            Self::LpfTuning => Ok(&[0x02, 0x03, 0x09]),
            Self::TxLpf => Ok(&[0x32, 0x33, 0x36, 0x3f, 0x09]),
            Self::RxLpf => Ok(&[0x52, 0x53, 0x5f, 0x72, 0x75, 0x76, 0x65, 0x09]),
            Self::RxVga2 => Ok(&[0x62, 0x63, 0x64, 0x68, 0x6e, 0x72, 0x75, 0x76, 0x65, 0x09]),
            Self::Invalid => Err(Error::Unsupported("DC calibration module")),
        }
    }
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
    /// Creates a configuration from sample count, timestamp and TX frequency.
    pub fn new(num_samples: u32, ts: u64, tx_freq: u64) -> Self {
        Self {
            num_samples,
            ts,
            tx_freq,
        }
    }

    /// Number of gain sample points used for interpolation.
    pub fn sample_count(&self) -> u32 {
        self.num_samples
    }

    /// Timestamp recorded with the calibration data.
    pub fn timestamp(&self) -> u64 {
        self.ts
    }

    /// TX frequency in Hz at which the calibration was performed.
    pub fn tx_frequency(&self) -> u64 {
        self.tx_freq
    }

    /// Sets the timestamp.
    pub fn set_timestamp(&mut self, ts: u64) {
        self.ts = ts;
    }

    /// Sets the TX frequency in Hz.
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
    /// Captures the settings to restore after calibration.
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

    /// Sample rate to restore.
    pub fn sample_rate(&self) -> &crate::bladerf1::hardware::si5338::RationalRate {
        &self.rational_sample_rate
    }

    pub(crate) fn sample_rate_mut(
        &mut self,
    ) -> &mut crate::bladerf1::hardware::si5338::RationalRate {
        &mut self.rational_sample_rate
    }

    /// Bandwidth in Hz to restore.
    pub fn bandwidth(&self) -> u32 {
        self.bandwidth
    }

    /// TX frequency in Hz to restore.
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
    pub(crate) fn calibrate_dc(
        &mut self,
        module: DcCalModule,
    ) -> impl MaybeFuture<Output = Result<()>> {
        Op::new(async move {
            let restoration = self
                .nios
                .save_lms_registers(module.temporary_registers()?)
                .await?;
            let result = self.run_dc_calibration(module).await;
            self.nios.finish_restoration(restoration, result).await
        })
    }

    fn run_dc_calibration(&mut self, module: DcCalModule) -> impl MaybeFuture<Output = Result<()>> {
        Op::new(async move {
            let mut state = self.dc_cal_backup(module).await?;
            self.dc_cal_module_init(module, &mut state).await?;
            loop {
                if self.dc_cal_module(module, &mut state).await? {
                    return Ok(());
                }
                if self.dc_cal_retry_adjustment(module, &mut state).await? {
                    return Err(Error::CalibrationFailed("gain adjustment limit reached"));
                }
            }
        })
    }

    pub(crate) fn set_dc_cals(&mut self, dc_cals: DcCals) -> impl MaybeFuture<Output = Result<()>> {
        Op::new(async move {
            if dc_cals == DcCals::default() {
                return Ok(());
            }
            let restoration = self
                .nios
                .save_lms_registers(&[0x02, 0x03, 0x32, 0x33, 0x52, 0x53, 0x62, 0x63, 0x09])
                .await?;
            let result = self.write_dc_cals(dc_cals).await;
            self.nios.finish_restoration(restoration, result).await
        })
    }

    fn write_dc_cals(&mut self, dc_cals: DcCals) -> impl MaybeFuture<Output = Result<()>> {
        Op::new(async move {
            for (module, values) in dc_cals.modules() {
                if values.iter().all(Option::is_none) {
                    continue;
                }
                self.set(0x09, module.cal_clock_mask()).await?;
                for (index, value) in values.into_iter().enumerate() {
                    if let Some(value) = value {
                        self.set_dc_cal_value(module.base_addr(), index as u8, value.get())
                            .await?;
                    }
                }
                self.clear(0x09, module.cal_clock_mask()).await?;
            }
            Ok(())
        })
    }

    pub(crate) fn get_dc_cals(&mut self) -> impl MaybeFuture<Output = Result<DcCals>> {
        Op::new(async move {
            let restoration = self
                .nios
                .save_lms_registers(&[0x03, 0x33, 0x53, 0x63])
                .await?;
            let result = self.read_dc_cals().await;
            self.nios.finish_restoration(restoration, result).await
        })
    }

    fn read_dc_cals(&mut self) -> impl MaybeFuture<Output = Result<DcCals>> {
        Op::new(async move {
            Ok(DcCals {
                lpf_tuning: Some(self.get_dc_cal_value(0x00, 0).await?),
                tx_lpf_i: Some(self.get_dc_cal_value(0x30, 0).await?),
                tx_lpf_q: Some(self.get_dc_cal_value(0x30, 1).await?),
                rx_lpf_i: Some(self.get_dc_cal_value(0x50, 0).await?),
                rx_lpf_q: Some(self.get_dc_cal_value(0x50, 1).await?),
                dc_ref: Some(self.get_dc_cal_value(0x60, 0).await?),
                rxvga2a_i: Some(self.get_dc_cal_value(0x60, 1).await?),
                rxvga2a_q: Some(self.get_dc_cal_value(0x60, 2).await?),
                rxvga2b_i: Some(self.get_dc_cal_value(0x60, 3).await?),
                rxvga2b_q: Some(self.get_dc_cal_value(0x60, 4).await?),
            })
        })
    }

    pub(crate) fn set_dc_offset_i(
        &mut self,
        channel: Channel,
        value: i16,
    ) -> impl MaybeFuture<Output = Result<()>> {
        self.set_dc_offset(channel, dc_offset_i_addr(channel), value)
    }

    pub(crate) fn set_dc_offset_q(
        &mut self,
        channel: Channel,
        value: i16,
    ) -> impl MaybeFuture<Output = Result<()>> {
        self.set_dc_offset(channel, dc_offset_q_addr(channel), value)
    }

    pub(crate) fn get_dc_offset_i(
        &mut self,
        channel: Channel,
    ) -> impl MaybeFuture<Output = Result<i16>> {
        self.get_dc_offset(channel, dc_offset_i_addr(channel))
    }

    pub(crate) fn get_dc_offset_q(
        &mut self,
        channel: Channel,
    ) -> impl MaybeFuture<Output = Result<i16>> {
        self.get_dc_offset(channel, dc_offset_q_addr(channel))
    }

    fn dc_cal_loop(
        &mut self,
        base: u8,
        cal_address: u8,
        dc_cntval: u8,
    ) -> impl MaybeFuture<Output = Result<Measurement>> {
        Op::new(async move {
            log::debug!("Calibrating module {base:#x}:{cal_address:#x}");
            let mut val = self.read(base + 0x03).await?;
            val &= !0x07;
            val |= cal_address & 0x07;
            self.write(base + 0x03, val).await?;
            self.write(base + 0x02, dc_cntval).await?;
            val |= 1 << 4;
            self.write(base + 0x03, val).await?;
            val &= !(1 << 4);
            self.write(base + 0x03, val).await?;
            val |= 1 << 5;
            self.write(base + 0x03, val).await?;
            val &= !(1 << 5);
            self.write(base + 0x03, val).await?;
            for _ in 0..25 {
                let val = self.read(base + 0x01).await?;
                if ((val >> 1) & 1) == 0 {
                    let dc_regval = self.read(base).await? & 0x3f;
                    log::debug!("DC_REGVAL: {dc_regval}");
                    return Ok(Measurement::Value(dc_regval));
                }
            }
            log::warn!("DC calibration loop did not converge.");
            Ok(Measurement::Incomplete)
        })
    }

    fn dc_cal_backup(
        &mut self,
        module: DcCalModule,
    ) -> impl MaybeFuture<Output = Result<DcCalState>> {
        Op::new(async move {
            let mut state = DcCalState {
                clk_en: self.read(0x09).await?,
                reg0x72: 0,
                rxvga1_curr_gain: 0,
                rxvga2_curr_gain: 0,
            };
            if module == DcCalModule::RxLpf || module == DcCalModule::RxVga2 {
                state.reg0x72 = self.read(0x72).await?;
            }
            Ok(state)
        })
    }

    fn dc_cal_module_init(
        &mut self,
        module: DcCalModule,
        state: &mut DcCalState,
    ) -> impl MaybeFuture<Output = Result<()>> {
        Op::new(async move {
            match module {
                DcCalModule::LpfTuning => {
                    self.write(0x09, state.clk_en | module.cal_clock_mask())
                        .await?;
                }
                DcCalModule::TxLpf => {
                    self.write(0x09, state.clk_en | module.cal_clock_mask())
                        .await?;
                    self.set(0x36, 1 << 7).await?;
                    self.clear(0x3f, 1 << 7).await?;
                }
                DcCalModule::RxLpf => {
                    self.write(0x09, state.clk_en | module.cal_clock_mask())
                        .await?;
                    self.clear(0x5f, 1 << 7).await?;
                    self.write(0x72, state.reg0x72 & !(1 << 7)).await?;
                    self.lna_set_gain(GAIN_SPEC_LNA.max.into()).await?;
                    state.rxvga1_curr_gain = GAIN_SPEC_RXVGA1.max as i32;
                    self.rxvga1_set_gain((state.rxvga1_curr_gain as i8).into())
                        .await?;
                    state.rxvga2_curr_gain = GAIN_SPEC_RXVGA2.max as i32;
                    self.rxvga2_set_gain((state.rxvga2_curr_gain as i8).into())
                        .await?;
                }
                DcCalModule::RxVga2 => {
                    self.write(0x09, state.clk_en | module.cal_clock_mask())
                        .await?;
                    self.clear(0x6e, 3 << 6).await?;
                    self.write(0x72, state.reg0x72 & !(1 << 7)).await?;
                    self.lna_set_gain(GAIN_SPEC_LNA.max.into()).await?;
                    state.rxvga1_curr_gain = GAIN_SPEC_RXVGA1.max as i32;
                    self.rxvga1_set_gain((state.rxvga1_curr_gain as i8).into())
                        .await?;
                    state.rxvga2_curr_gain = GAIN_SPEC_RXVGA2.max as i32;
                    self.rxvga2_set_gain((state.rxvga2_curr_gain as i8).into())
                        .await?;
                }
                _ => return Err(Error::Unsupported("DC calibration module")),
            }
            Ok(())
        })
    }

    fn dc_cal_submodule(
        &mut self,
        module: DcCalModule,
        submodule: u8,
        _state: &DcCalState,
    ) -> impl MaybeFuture<Output = Result<bool>> {
        Op::new(async move {
            if module == DcCalModule::RxVga2 {
                match submodule {
                    0 => {
                        self.clear(0x64, 0x01).await?;
                        self.write(0x68, 0x01).await?;
                    }
                    1 => {
                        self.set(0x64, 0x01).await?;
                        self.write(0x68, 0x06).await?;
                    }
                    2 => {}
                    3 => {
                        self.write(0x68, 0x60).await?;
                    }
                    4 => {}
                    _ => {
                        return Err(Error::CalibrationFailed("invalid submodule index"));
                    }
                }
            }
            let base = module.base_addr();
            let initial = self.dc_cal_loop(base, submodule, 31).await?;
            let second = if initial == Measurement::Value(31) {
                Some(self.dc_cal_loop(base, submodule, 0).await?)
            } else {
                None
            };
            let Some(dc_regval) = calibrated_value(initial, second) else {
                return Ok(false);
            };
            if module == DcCalModule::LpfTuning {
                let mut val = self.read(0x35).await?;
                val &= !0x3f;
                val |= dc_regval;
                self.write(0x35, val).await?;
                let mut val = self.read(0x55).await?;
                val &= !0x3f;
                val |= dc_regval;
                self.write(0x55, val).await?;
            }
            Ok(true)
        })
    }

    fn dc_cal_retry_adjustment(
        &mut self,
        module: DcCalModule,
        state: &mut DcCalState,
    ) -> impl MaybeFuture<Output = Result<bool>> {
        Op::new(async move {
            let mut limit_reached: bool = false;
            match module {
                DcCalModule::LpfTuning | DcCalModule::TxLpf => {
                    limit_reached = true;
                }
                DcCalModule::RxLpf => {
                    if state.rxvga1_curr_gain > GAIN_SPEC_RXVGA1.min as i32 {
                        state.rxvga1_curr_gain -= 1;
                        log::debug!("Retrying DC cal with RXVGA1={}", state.rxvga1_curr_gain);
                        self.rxvga1_set_gain((state.rxvga1_curr_gain as i8).into())
                            .await?;
                    } else {
                        limit_reached = true;
                    }
                }
                DcCalModule::RxVga2 => {
                    if state.rxvga1_curr_gain > GAIN_SPEC_RXVGA1.min as i32 {
                        state.rxvga1_curr_gain -= 1;
                        log::debug!("Retrying DC cal with RXVGA1={}", state.rxvga1_curr_gain);
                        self.rxvga1_set_gain((state.rxvga1_curr_gain as i8).into())
                            .await?;
                    } else if state.rxvga2_curr_gain > GAIN_SPEC_RXVGA2.min as i32 {
                        state.rxvga2_curr_gain -= 3;
                        log::debug!("Retrying DC cal with RXVGA2={}", state.rxvga2_curr_gain);
                        self.rxvga2_set_gain((state.rxvga2_curr_gain as i8).into())
                            .await?;
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
        })
    }

    fn dc_cal_module(
        &mut self,
        module: DcCalModule,
        state: &mut DcCalState,
    ) -> impl MaybeFuture<Output = Result<bool>> {
        Op::new(async move {
            let mut converged = true;
            for submodule in 0..module.num_submodules() {
                converged = self.dc_cal_submodule(module, submodule, state).await?;
                if !converged {
                    return Ok(false);
                }
            }
            Ok(converged)
        })
    }

    fn set_dc_cal_value(
        &mut self,
        base: u8,
        dc_addr: u8,
        value: u8,
    ) -> impl MaybeFuture<Output = Result<u8>> {
        Op::new(async move {
            let mut regval: u8 = 0x08 | dc_addr;
            self.write(base + 3, regval).await?;
            self.write(base + 2, value).await?;
            regval |= 1 << 4;
            self.write(base + 3, regval).await?;
            regval &= !(1 << 4);
            self.write(base + 3, regval).await?;
            self.read(base).await
        })
    }

    fn get_dc_cal_value(
        &mut self,
        base: u8,
        dc_addr: u8,
    ) -> impl MaybeFuture<Output = Result<DcCalValue>> {
        Op::new(async move {
            self.write(base + 3, 0x08 | dc_addr).await?;
            Ok(DcCalValue(self.read(base).await? & 0x3f))
        })
    }

    fn set_dc_offset(
        &mut self,
        channel: Channel,
        addr: u8,
        value: i16,
    ) -> impl MaybeFuture<Output = Result<()>> {
        Op::new(async move {
            let regval = match channel {
                Channel::Rx => {
                    let tmp = self.read(addr).await?;
                    tmp & (1 << 7) | scale_dc_offset(channel, value)
                }
                Channel::Tx => scale_dc_offset(channel, value),
            };
            self.write(addr, regval).await
        })
    }

    fn get_dc_offset(
        &mut self,
        channel: Channel,
        addr: u8,
    ) -> impl MaybeFuture<Output = Result<i16>> {
        Op::new(async move {
            let regval = self.read(addr).await?;
            Ok(unscale_dc_offset(channel, regval))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_updates_distinguish_zero_from_absent_and_reject_invalid_values() {
        let update: DcCals = serde_json::from_str(r#"{"tx_lpf_i":0,"rx_lpf_i":63}"#).unwrap();
        assert_eq!(update.tx_lpf_i.unwrap().get(), 0);
        assert_eq!(update.rx_lpf_i.unwrap().get(), 63);
        assert_eq!(update.tx_lpf_q, None);
        assert_eq!(
            serde_json::from_str::<DcCals>("{}").unwrap(),
            DcCals::default()
        );
        assert_eq!(
            serde_json::from_str::<DcCals>(&serde_json::to_string(&update).unwrap()).unwrap(),
            update
        );
        for value in 64..=255 {
            assert!(DcCalValue::new(value).is_err());
        }
        for json in [
            r#"{"tx_lpf_i":-1}"#,
            r#"{"dc_ref":64}"#,
            r#"{"dc_ref":256}"#,
            r#"{"typo":0}"#,
        ] {
            assert!(serde_json::from_str::<DcCals>(json).is_err());
        }
    }

    #[test]
    fn faq_4_7_retries_code_31_from_zero_and_accepts_changed_codes() {
        for code in 0..64 {
            if code != 31 {
                assert_eq!(calibrated_value(Measurement::Value(code), None), Some(code));
            }
            assert_eq!(
                calibrated_value(Measurement::Value(31), Some(Measurement::Value(code))),
                (code != 0).then_some(code),
            );
        }
        assert_eq!(calibrated_value(Measurement::Value(31), None), None);
        assert_eq!(calibrated_value(Measurement::Incomplete, None), None);
        assert_eq!(
            calibrated_value(Measurement::Value(31), Some(Measurement::Incomplete)),
            None
        );
    }
}
