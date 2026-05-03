use crate::Channel;
use crate::bladerf1::hardware::lms6002d::gain::{
    GAIN_SPEC_LNA, GAIN_SPEC_RXVGA1, GAIN_SPEC_RXVGA2, LnaGainCode,
};
use crate::bladerf1::nios_client::NiosClient;
use crate::error::{Error, Result};
use std::cmp::PartialEq;
use std::fmt::{Display, Formatter};
#[derive(Debug)]
pub struct DcCals {
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
pub struct DcCalState {
    clk_en: u8,
    reg0x72: u8,
    lna_gain: LnaGainCode,
    rxvga1_gain: i32,
    rxvga2_gain: i32,
    base_addr: u8,
    num_submodules: u32,
    rxvga1_curr_gain: i32,
    rxvga2_curr_gain: i32,
}
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum DcCalModule {
    Invalid = -1,
    LpfTuning,
    TxLpf,
    RxLpf,
    RxVga2,
}
#[allow(dead_code)]
pub struct RxCal {
    num_samples: u32,
    pub(crate) ts: u64,
    pub(crate) tx_freq: u64,
}
pub struct RxCalBackup {
    pub(crate) rational_sample_rate: crate::bladerf1::hardware::si5338::RationalRate,
    pub(crate) bandwidth: u32,
    pub(crate) tx_freq: u64,
}
pub fn dc_cal_loop(nios: &mut NiosClient, base: u8, cal_address: u8, dc_cntval: u8) -> Result<u8> {
    log::debug!("Calibrating module {base:#x}:{cal_address:#x}");
    let mut val = super::read(nios, base + 0x03)?;
    val &= !0x07;
    val |= cal_address & 0x07;
    super::write(nios, base + 0x03, val)?;
    super::write(nios, base + 0x02, dc_cntval)?;
    val |= 1 << 4;
    super::write(nios, base + 0x03, val)?;
    val &= !(1 << 4);
    super::write(nios, base + 0x03, val)?;
    val |= 1 << 5;
    super::write(nios, base + 0x03, val)?;
    val &= !(1 << 5);
    super::write(nios, base + 0x03, val)?;
    for _ in 0..25 {
        val = super::read(nios, base + 0x01)?;
        if ((val >> 1) & 1) == 0 {
            let dc_regval = super::read(nios, base)? & 0x3f;
            log::debug!("DC_REGVAL: {dc_regval}");
            return Ok(dc_regval);
        }
    }
    log::warn!("DC calibration loop did not converge.");
    Err(Error::CalibrationFailed("loop did not converge"))
}
pub fn dc_cal_backup(nios: &mut NiosClient, module: DcCalModule) -> Result<DcCalState> {
    let mut state = DcCalState {
        clk_en: super::read(nios, 0x09)?,
        reg0x72: 0,
        lna_gain: LnaGainCode::BypassLna1Lna2,
        rxvga1_gain: 0,
        rxvga2_gain: 0,
        base_addr: 0,
        num_submodules: 0,
        rxvga1_curr_gain: 0,
        rxvga2_curr_gain: 0,
    };
    if module == DcCalModule::RxLpf || module == DcCalModule::RxVga2 {
        state.reg0x72 = super::read(nios, 0x72)?;
        state.lna_gain = LnaGainCode::from(super::gain::lna_get_gain(nios)?);
        state.rxvga1_gain = super::gain::rxvga1_get_gain(nios)?.db as i32;
        state.rxvga2_gain = super::gain::rxvga2_get_gain(nios)?.db as i32;
    }
    Ok(state)
}
pub fn dc_cal_module_init(
    nios: &mut NiosClient,
    module: DcCalModule,
    state: &mut DcCalState,
) -> Result<()> {
    match module {
        DcCalModule::LpfTuning => {
            state.base_addr = 0x00;
            state.num_submodules = 1;
            super::write(nios, 0x09, state.clk_en | (1 << 5))?;
        }
        DcCalModule::TxLpf => {
            state.base_addr = 0x30;
            state.num_submodules = 2;
            super::write(nios, 0x09, state.clk_en | (1 << 1))?;
            super::set(nios, 0x36, 1 << 7)?;
            super::clear(nios, 0x3f, 1 << 7)?;
        }
        DcCalModule::RxLpf => {
            state.base_addr = 0x50;
            state.num_submodules = 2;
            super::write(nios, 0x09, state.clk_en | (1 << 3))?;
            super::clear(nios, 0x5f, 1 << 7)?;
            super::write(nios, 0x72, state.reg0x72 & !(1 << 7))?;
            super::gain::lna_set_gain(nios, GAIN_SPEC_LNA.max.into())?;
            state.rxvga1_curr_gain = GAIN_SPEC_RXVGA1.max as i32;
            super::gain::rxvga1_set_gain(nios, (state.rxvga1_curr_gain as i8).into())?;
            state.rxvga2_curr_gain = GAIN_SPEC_RXVGA2.max as i32;
            super::gain::rxvga2_set_gain(nios, (state.rxvga2_curr_gain as i8).into())?;
        }
        DcCalModule::RxVga2 => {
            state.base_addr = 0x60;
            state.num_submodules = 5;
            super::write(nios, 0x09, state.clk_en | (1 << 4))?;
            super::clear(nios, 0x6e, 3 << 6)?;
            super::write(nios, 0x72, state.reg0x72 & !(1 << 7))?;
            super::gain::lna_set_gain(nios, GAIN_SPEC_LNA.max.into())?;
            state.rxvga1_curr_gain = GAIN_SPEC_RXVGA1.max as i32;
            super::gain::rxvga1_set_gain(nios, (state.rxvga1_curr_gain as i8).into())?;
            state.rxvga2_curr_gain = GAIN_SPEC_RXVGA2.max as i32;
            super::gain::rxvga2_set_gain(nios, (state.rxvga2_curr_gain as i8).into())?;
        }
        _ => return Err(Error::Unsupported("DC calibration module")),
    }
    Ok(())
}
pub fn dc_cal_submodule(
    nios: &mut NiosClient,
    module: DcCalModule,
    submodule: u8,
    state: &DcCalState,
) -> Result<bool> {
    let mut converged: bool = false;
    if module == DcCalModule::RxVga2 {
        match submodule {
            0 => {
                super::clear(nios, 0x64, 1 << 0)?;
                super::write(nios, 0x68, 0x01)?;
            }
            1 => {
                super::set(nios, 0x64, 1 << 0)?;
                super::write(nios, 0x68, 0x06)?;
            }
            2 => {}
            3 => {
                super::write(nios, 0x68, 0x60)?;
            }
            4 => {}
            _ => {
                return Err(Error::CalibrationFailed("invalid submodule index"));
            }
        }
    }
    let mut dc_regval = dc_cal_loop(nios, state.base_addr, submodule, 31)?;
    if dc_regval == 31 {
        log::debug!("DC_REGVAL suboptimal value - retrying DC cal loop.");
        dc_regval = dc_cal_loop(nios, state.base_addr, submodule, 0)?;
        if dc_regval == 0 {
            log::debug!("Bad DC_REGVAL detected. DC cal failed.");
            return Ok(converged);
        }
    }
    if module == DcCalModule::LpfTuning {
        let mut val = super::read(nios, 0x35)?;
        val &= !0x3f;
        val |= dc_regval;
        super::write(nios, 0x35, val)?;
        val = super::read(nios, 0x55)?;
        val &= !0x3f;
        val |= dc_regval;
        super::write(nios, 0x55, val)?;
    }
    converged = true;
    Ok(converged)
}
pub fn dc_cal_retry_adjustment(
    nios: &mut NiosClient,
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
                super::gain::rxvga1_set_gain(nios, (state.rxvga1_curr_gain as i8).into())?;
            } else {
                limit_reached = true;
            }
        }
        DcCalModule::RxVga2 => {
            if state.rxvga1_curr_gain > GAIN_SPEC_RXVGA1.min as i32 {
                state.rxvga1_curr_gain -= 1;
                log::debug!("Retrying DC cal with RXVGA1={}", state.rxvga1_curr_gain);
                super::gain::rxvga1_set_gain(nios, (state.rxvga1_curr_gain as i8).into())?;
            } else if state.rxvga2_curr_gain > GAIN_SPEC_RXVGA2.min as i32 {
                state.rxvga2_curr_gain -= 3;
                log::debug!("Retrying DC cal with RXVGA2={}", state.rxvga2_curr_gain);
                super::gain::rxvga2_set_gain(nios, (state.rxvga2_curr_gain as i8).into())?;
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
pub fn dc_cal_module_deinit(nios: &mut NiosClient, module: DcCalModule) -> Result<()> {
    match module {
        DcCalModule::LpfTuning => {}
        DcCalModule::RxLpf => {
            super::set(nios, 0x5f, 1 << 7)?;
        }
        DcCalModule::RxVga2 => {
            super::write(nios, 0x68, 0x01)?;
            super::clear(nios, 0x64, 1 << 0)?;
            super::set(nios, 0x6e, 3 << 6)?;
        }
        DcCalModule::TxLpf => {
            super::set(nios, 0x3f, 1 << 7)?;
            super::clear(nios, 0x36, 1 << 7)?;
        }
        _ => {
            return Err(Error::Unsupported("DC calibration module"));
        }
    }
    Ok(())
}
pub fn dc_cal_restore(
    nios: &mut NiosClient,
    module: DcCalModule,
    state: &DcCalState,
) -> Result<()> {
    super::write(nios, 0x09, state.clk_en)?;
    if module == DcCalModule::RxLpf || module == DcCalModule::RxVga2 {
        super::write(nios, 0x72, state.reg0x72)?;
        super::gain::lna_set_gain(nios, state.lna_gain.into())?;
        super::gain::rxvga1_set_gain(nios, (state.rxvga1_gain as i8).into())?;
        super::gain::rxvga2_set_gain(nios, (state.rxvga2_gain as i8).into())?;
    }
    Ok(())
}
pub fn dc_cal_module(
    nios: &mut NiosClient,
    module: DcCalModule,
    state: &mut DcCalState,
) -> Result<bool> {
    let mut converged = true;
    for submodule in 0..state.num_submodules as u8 {
        converged = dc_cal_submodule(nios, module, submodule, state)?;
        if !converged {
            return Err(Error::CalibrationFailed("submodule did not converge"));
        }
    }
    Ok(converged)
}
pub fn calibrate_dc(nios: &mut NiosClient, module: DcCalModule) -> Result<()> {
    let mut state = dc_cal_backup(nios, module)?;
    if dc_cal_module_init(nios, module, &mut state).is_err() {
        let _ = dc_cal_module_deinit(nios, module);
        return dc_cal_restore(nios, module, &state);
    }
    let mut converged = false;
    let mut limit_reached = false;
    while !converged && !limit_reached {
        if let Ok(c) = dc_cal_module(nios, module, &mut state) {
            converged = c;
            if !converged {
                if let Ok(l) = dc_cal_retry_adjustment(nios, module, &mut state) {
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
    let _ = dc_cal_module_deinit(nios, module);
    dc_cal_restore(nios, module, &state)
}
pub fn set_cal_clock(nios: &mut NiosClient, enable: bool, mask: u8) -> Result<()> {
    if enable {
        super::set(nios, 0x09, mask)
    } else {
        super::clear(nios, 0x09, mask)
    }
}
pub fn enable_lpf_cal_clock(nios: &mut NiosClient, enable: bool) -> Result<()> {
    set_cal_clock(nios, enable, 1 << 5)
}
pub fn enable_rxvga2_dccal_clock(nios: &mut NiosClient, enable: bool) -> Result<()> {
    set_cal_clock(nios, enable, 1 << 4)
}
pub fn enable_rxlpf_dccal_clock(nios: &mut NiosClient, enable: bool) -> Result<()> {
    set_cal_clock(nios, enable, 1 << 3)
}
pub fn enable_txlpf_dccal_clock(nios: &mut NiosClient, enable: bool) -> Result<()> {
    set_cal_clock(nios, enable, 1 << 1)
}
pub fn set_dc_cal_value(nios: &mut NiosClient, base: u8, dc_addr: u8, value: u8) -> Result<u8> {
    let mut regval: u8 = 0x08 | dc_addr;
    super::write(nios, base + 3, regval)?;
    super::write(nios, base + 2, value)?;
    regval |= 1 << 4;
    super::write(nios, base + 3, regval)?;
    regval &= !(1 << 4);
    super::write(nios, base + 3, regval)?;
    super::read(nios, base)
}
pub fn get_dc_cal_value(nios: &mut NiosClient, base: u8, dc_addr: u8) -> Result<u8> {
    super::write(nios, base + 3, 0x08 | dc_addr)?;
    super::read(nios, base)
}
pub fn set_dc_cals(nios: &mut NiosClient, dc_cals: DcCals) -> Result<()> {
    let cal_tx_lpf: bool = (dc_cals.tx_lpf_i >= 0) || (dc_cals.tx_lpf_q >= 0);
    let cal_rx_lpf: bool = (dc_cals.rx_lpf_i >= 0) || (dc_cals.rx_lpf_q >= 0);
    let cal_rxvga2: bool = (dc_cals.dc_ref >= 0)
        || (dc_cals.rxvga2a_i >= 0)
        || (dc_cals.rxvga2a_q >= 0)
        || (dc_cals.rxvga2b_i >= 0)
        || (dc_cals.rxvga2b_q >= 0);
    if dc_cals.lpf_tuning >= 0 {
        enable_lpf_cal_clock(nios, true)?;
        set_dc_cal_value(nios, 0x00, 0, dc_cals.lpf_tuning as u8)?;
        enable_lpf_cal_clock(nios, false)?;
    }
    if cal_tx_lpf {
        enable_txlpf_dccal_clock(nios, true)?;
        if dc_cals.tx_lpf_i >= 0 {
            set_dc_cal_value(nios, 0x30, 0, dc_cals.tx_lpf_i as u8)?;
        }
        if dc_cals.tx_lpf_q >= 0 {
            set_dc_cal_value(nios, 0x30, 1, dc_cals.tx_lpf_q as u8)?;
        }
        enable_txlpf_dccal_clock(nios, false)?;
    }
    if cal_rx_lpf {
        enable_rxlpf_dccal_clock(nios, true)?;
        if dc_cals.rx_lpf_i >= 0 {
            set_dc_cal_value(nios, 0x50, 0, dc_cals.rx_lpf_i as u8)?;
        }
        if dc_cals.rx_lpf_q >= 0 {
            set_dc_cal_value(nios, 0x50, 1, dc_cals.rx_lpf_q as u8)?;
        }
        enable_rxlpf_dccal_clock(nios, false)?;
    }
    if cal_rxvga2 {
        enable_rxvga2_dccal_clock(nios, true)?;
        if dc_cals.dc_ref >= 0 {
            set_dc_cal_value(nios, 0x60, 0, dc_cals.dc_ref as u8)?;
        }
        if dc_cals.rxvga2a_i >= 0 {
            set_dc_cal_value(nios, 0x60, 1, dc_cals.rxvga2a_i as u8)?;
        }
        if dc_cals.rxvga2a_q >= 0 {
            set_dc_cal_value(nios, 0x60, 2, dc_cals.rxvga2a_q as u8)?;
        }
        if dc_cals.rxvga2b_i >= 0 {
            set_dc_cal_value(nios, 0x60, 3, dc_cals.rxvga2b_i as u8)?;
        }
        if dc_cals.rxvga2b_q >= 0 {
            set_dc_cal_value(nios, 0x60, 4, dc_cals.rxvga2b_q as u8)?;
        }
        enable_rxvga2_dccal_clock(nios, false)?;
    }
    Ok(())
}
pub fn get_dc_cals(nios: &mut NiosClient) -> Result<DcCals> {
    Ok(DcCals {
        lpf_tuning: get_dc_cal_value(nios, 0x00, 0)? as i16,
        tx_lpf_i: get_dc_cal_value(nios, 0x30, 0)? as i16,
        tx_lpf_q: get_dc_cal_value(nios, 0x30, 1)? as i16,
        rx_lpf_i: get_dc_cal_value(nios, 0x50, 0)? as i16,
        rx_lpf_q: get_dc_cal_value(nios, 0x50, 1)? as i16,
        dc_ref: get_dc_cal_value(nios, 0x60, 0)? as i16,
        rxvga2a_i: get_dc_cal_value(nios, 0x60, 1)? as i16,
        rxvga2a_q: get_dc_cal_value(nios, 0x60, 2)? as i16,
        rxvga2b_i: get_dc_cal_value(nios, 0x60, 3)? as i16,
        rxvga2b_q: get_dc_cal_value(nios, 0x60, 4)? as i16,
    })
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
            let value = if regval & (1 << 6) != 0 {
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
fn set_dc_offset(nios: &mut NiosClient, channel: Channel, addr: u8, value: i16) -> Result<()> {
    let regval = match channel {
        Channel::Rx => {
            let mut tmp = super::read(nios, addr)?;
            tmp &= 1 << 7;
            scale_dc_offset(channel, value) | tmp
        }
        Channel::Tx => scale_dc_offset(channel, value),
    };
    super::write(nios, addr, regval)
}
pub fn set_dc_offset_i(nios: &mut NiosClient, channel: Channel, value: i16) -> Result<()> {
    let addr = if channel == Channel::Tx { 0x42 } else { 0x71 };
    set_dc_offset(nios, channel, addr, value)
}
pub fn set_dc_offset_q(nios: &mut NiosClient, channel: Channel, value: i16) -> Result<()> {
    let addr = if channel == Channel::Tx { 0x43 } else { 0x72 };
    set_dc_offset(nios, channel, addr, value)
}
fn get_dc_offset(nios: &mut NiosClient, channel: Channel, addr: u8) -> Result<i16> {
    let regval = super::read(nios, addr)?;
    Ok(unscale_dc_offset(channel, regval))
}
pub fn get_dc_offset_i(nios: &mut NiosClient, channel: Channel) -> Result<i16> {
    let addr = if channel == Channel::Tx { 0x42 } else { 0x71 };
    get_dc_offset(nios, channel, addr)
}
pub fn get_dc_offset_q(nios: &mut NiosClient, channel: Channel) -> Result<i16> {
    let addr = if channel == Channel::Tx { 0x43 } else { 0x72 };
    get_dc_offset(nios, channel, addr)
}
