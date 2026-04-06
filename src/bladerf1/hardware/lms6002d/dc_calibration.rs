use crate::Channel;
use crate::bladerf1::hardware::lms6002d::LMS6002D;
use crate::bladerf1::hardware::lms6002d::gain::{
    GAIN_SPEC_LNA, GAIN_SPEC_RXVGA1, GAIN_SPEC_RXVGA2, GainDb, LnaGainCode,
};
use crate::bladerf1::hardware::si5338::RationalRate;
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
    pub(crate) rational_sample_rate: RationalRate,
    pub(crate) bandwidth: u32,
    pub(crate) tx_freq: u64,
}
impl LMS6002D {
    pub fn dc_cal_loop(&self, base: u8, cal_address: u8, dc_cntval: u8) -> Result<u8> {
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
            val = self.read(base + 0x01)?;
            if ((val >> 1) & 1) == 0 {
                let dc_regval = self.read(base)? & 0x3f;
                log::debug!("DC_REGVAL: {dc_regval}");
                return Ok(dc_regval);
            }
        }
        log::warn!("DC calibration loop did not converge.");
        Err(Error::Invalid)
    }
    pub fn dc_cal_backup(&self, module: DcCalModule) -> Result<DcCalState> {
        let mut state = DcCalState {
            clk_en: self.read(0x09)?,
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
            state.reg0x72 = self.read(0x72)?;
            state.lna_gain = LnaGainCode::from(self.lna_get_gain()?);
            state.rxvga1_gain = self.rxvga1_get_gain()?.db as i32;
            state.rxvga2_gain = self.rxvga2_get_gain()?.db as i32;
        }
        Ok(state)
    }
    pub fn dc_cal_module_init(&self, module: DcCalModule, state: &mut DcCalState) -> Result<()> {
        let cal_clock = match module {
            DcCalModule::LpfTuning => {
                state.base_addr = 0x00;
                state.num_submodules = 1;
                1 << 5
            }
            DcCalModule::TxLpf => {
                state.base_addr = 0x30;
                state.num_submodules = 2;
                1 << 1
            }
            DcCalModule::RxLpf => {
                state.base_addr = 0x50;
                state.num_submodules = 2;
                1 << 3
            }
            DcCalModule::RxVga2 => {
                state.base_addr = 0x60;
                state.num_submodules = 5;
                1 << 4
            }
            _ => return Err(Error::Invalid),
        };
        self.write(0x09, state.clk_en | cal_clock)?;
        match module {
            DcCalModule::LpfTuning => {}
            DcCalModule::RxLpf | DcCalModule::RxVga2 => {
                if module == DcCalModule::RxVga2 {
                    self.clear(0x6e, 3 << 6)?;
                } else {
                    self.clear(0x5f, 1 << 7)?;
                }
                let val = state.reg0x72 & !(1 << 7);
                self.write(0x72, val)?;
                self.lna_set_gain(GainDb {
                    db: GAIN_SPEC_LNA.max,
                })?;
                state.rxvga1_curr_gain = GAIN_SPEC_RXVGA1.max as i32;
                self.rxvga1_set_gain(GainDb {
                    db: state.rxvga1_curr_gain as i8,
                })?;
                state.rxvga2_curr_gain = GAIN_SPEC_RXVGA2.max as i32;
                self.rxvga2_set_gain(GainDb {
                    db: state.rxvga2_curr_gain as i8,
                })?;
            }
            DcCalModule::TxLpf => {
                self.set(0x36, 1 << 7)?;
                self.clear(0x3f, 1 << 7)?;
            }
            _ => {
                return Err(Error::Invalid);
            }
        }
        Ok(())
    }
    pub fn dc_cal_submodule(
        &self,
        module: DcCalModule,
        submodule: u8,
        state: &DcCalState,
    ) -> Result<bool> {
        let mut converged: bool = false;
        if module == DcCalModule::RxVga2 {
            match submodule {
                0 => {
                    self.clear(0x64, 1 << 0)?;
                    self.write(0x68, 0x01)?;
                }
                1 => {
                    self.set(0x64, 1 << 0)?;
                    self.write(0x68, 0x06)?;
                }
                2 => {}
                3 => {
                    self.write(0x68, 0x60)?;
                }
                4 => {}
                _ => {
                    return Err(Error::Invalid);
                }
            }
        }
        let mut dc_regval = self.dc_cal_loop(state.base_addr, submodule, 31)?;
        if dc_regval == 31 {
            log::debug!("DC_REGVAL suboptimal value - retrying DC cal loop.");
            dc_regval = self.dc_cal_loop(state.base_addr, submodule, 0)?;
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
            val = self.read(0x55)?;
            val &= !0x3f;
            val |= dc_regval;
            self.write(0x55, val)?;
        }
        converged = true;
        Ok(converged)
    }
    pub fn dc_cal_retry_adjustment(
        &self,
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
                    self.rxvga1_set_gain(GainDb {
                        db: state.rxvga1_curr_gain as i8,
                    })?;
                } else {
                    limit_reached = true;
                }
            }
            DcCalModule::RxVga2 => {
                if state.rxvga1_curr_gain > GAIN_SPEC_RXVGA1.min as i32 {
                    state.rxvga1_curr_gain -= 1;
                    log::debug!("Retrying DC cal with RXVGA1={}", state.rxvga1_curr_gain);
                    self.rxvga1_set_gain(GainDb {
                        db: state.rxvga1_curr_gain as i8,
                    })?;
                } else if state.rxvga2_curr_gain > GAIN_SPEC_RXVGA2.min as i32 {
                    state.rxvga2_curr_gain -= 3;
                    log::debug!("Retrying DC cal with RXVGA2={}", state.rxvga2_curr_gain);
                    self.rxvga2_set_gain(GainDb {
                        db: state.rxvga2_curr_gain as i8,
                    })?;
                } else {
                    limit_reached = true;
                }
            }
            _ => {
                return Err(Error::Invalid);
            }
        }
        if limit_reached {
            log::debug!("DC Cal retry limit reached");
        }
        Ok(limit_reached)
    }
    pub fn dc_cal_module_deinit(&self, module: DcCalModule) -> Result<()> {
        match module {
            DcCalModule::LpfTuning => {}
            DcCalModule::RxLpf => {
                self.set(0x5f, 1 << 7)?;
            }
            DcCalModule::RxVga2 => {
                self.write(0x68, 0x01)?;
                self.clear(0x64, 1 << 0)?;
                self.set(0x6e, 3 << 6)?;
            }
            DcCalModule::TxLpf => {
                self.set(0x3f, 1 << 7)?;
                self.clear(0x36, 1 << 7)?;
            }
            _ => {
                return Err(Error::Invalid);
            }
        }
        Ok(())
    }
    pub fn dc_cal_restore(&self, module: DcCalModule, state: &DcCalState) -> Result<()> {
        self.write(0x09, state.clk_en)?;
        if module == DcCalModule::RxLpf || module == DcCalModule::RxVga2 {
            self.write(0x72, state.reg0x72)?;
            self.lna_set_gain(state.lna_gain.into())?;
            self.rxvga1_set_gain(GainDb {
                db: state.rxvga1_gain as i8,
            })?;
            self.rxvga2_set_gain(GainDb {
                db: state.rxvga2_gain as i8,
            })?;
        }
        Ok(())
    }
    pub fn dc_cal_module(&self, module: DcCalModule, state: &mut DcCalState) -> Result<bool> {
        let mut converged = true;
        for submodule in 0..state.num_submodules as u8 {
            converged = self.dc_cal_submodule(module, submodule, state)?;
            if !converged {
                return Err(Error::Invalid);
            }
        }
        Ok(converged)
    }
    pub fn calibrate_dc(&self, module: DcCalModule) -> Result<()> {
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
    pub fn set_cal_clock(&self, enable: bool, mask: u8) -> Result<()> {
        if enable {
            self.set(0x09, mask)
        } else {
            self.clear(0x09, mask)
        }
    }
    pub fn enable_lpf_cal_clock(&self, enable: bool) -> Result<()> {
        self.set_cal_clock(enable, 1 << 5)
    }
    pub fn enable_rxvga2_dccal_clock(&self, enable: bool) -> Result<()> {
        self.set_cal_clock(enable, 1 << 4)
    }
    pub fn enable_rxlpf_dccal_clock(&self, enable: bool) -> Result<()> {
        self.set_cal_clock(enable, 1 << 3)
    }
    pub fn enable_txlpf_dccal_clock(&self, enable: bool) -> Result<()> {
        self.set_cal_clock(enable, 1 << 1)
    }
    pub fn set_dc_cal_value(&self, base: u8, dc_addr: u8, value: u8) -> Result<u8> {
        let mut regval: u8 = 0x08 | dc_addr;
        self.write(base + 3, regval)?;
        self.write(base + 2, value)?;
        regval |= 1 << 4;
        self.write(base + 3, regval)?;
        regval &= !(1 << 4);
        self.write(base + 3, regval)?;
        self.read(base)
    }
    pub fn get_dc_cal_value(&self, base: u8, dc_addr: u8) -> Result<u8> {
        self.write(base + 3, 0x08 | dc_addr)?;
        self.read(base)
    }
    pub fn set_dc_cals(&self, dc_cals: DcCals) -> Result<()> {
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
    pub fn get_dc_cals(&self) -> Result<DcCals> {
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
    fn scale_dc_offset(channel: Channel, mut value: i16) -> Result<u8> {
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
                Ok(value as u8)
            }
            Channel::Tx => {
                value >>= 4;
                if value >= 0 {
                    let ret = (if value >= 128 { 0x7f } else { value & 0x7f }) as u8;
                    Ok((1 << 7) | ret)
                } else {
                    Ok((if value <= -128 { 0x00 } else { value & 0x7f }) as u8)
                }
            }
        }
    }
    fn unscale_dc_offset(channel: Channel, mut regval: u8) -> Result<i16> {
        match channel {
            Channel::Rx => {
                regval &= 0x7f;
                let value = if regval & (1 << 6) != 0 {
                    -((regval & 0x3f) as i16)
                } else {
                    (regval & 0x3f) as i16
                };
                Ok(value << 5)
            }
            Channel::Tx => {
                let value = -(0x80 - regval as i16);
                Ok((value) << 4)
            }
        }
    }
    fn set_dc_offset(&self, channel: Channel, addr: u8, value: i16) -> Result<()> {
        let regval = match channel {
            Channel::Rx => {
                let mut tmp = self.read(addr)?;
                tmp &= 1 << 7;
                Self::scale_dc_offset(channel, value)? | tmp
            }
            Channel::Tx => Self::scale_dc_offset(channel, value)?,
        };
        self.write(addr, regval)
    }
    pub fn set_dc_offset_i(&self, channel: Channel, value: i16) -> Result<()> {
        let addr = if channel == Channel::Tx { 0x42 } else { 0x71 };
        self.set_dc_offset(channel, addr, value)
    }
    pub fn set_dc_offset_q(&self, channel: Channel, value: i16) -> Result<()> {
        let addr = if channel == Channel::Tx { 0x43 } else { 0x72 };
        self.set_dc_offset(channel, addr, value)
    }
    fn get_dc_offset(&self, channel: Channel, addr: u8) -> Result<i16> {
        let regval = self.read(addr)?;
        Self::unscale_dc_offset(channel, regval)
    }
    pub fn get_dc_offset_i(&self, channel: Channel) -> Result<i16> {
        let addr = if channel == Channel::Tx { 0x42 } else { 0x71 };
        self.get_dc_offset(channel, addr)
    }
    pub fn get_dc_offset_q(&self, channel: Channel) -> Result<i16> {
        let addr = if channel == Channel::Tx { 0x43 } else { 0x72 };
        self.get_dc_offset(channel, addr)
    }
}
