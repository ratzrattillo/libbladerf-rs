//! LMS6002D LPF mode configuration.
//!
//! The digital LPF can be enabled in normal filtering mode, bypassed to
//! pass the full signal chain, or disabled entirely.

use crate::bladerf1::hardware::lms6002d::Lms6002d;
use crate::{Channel, Error};

/// LPF operating mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LpfMode {
    /// LPF is enabled and actively filtering.
    Normal,
    /// LPF is bypassed; signal passes through unfiltered.
    Bypassed,
    /// LPF is disabled entirely.
    Disabled,
}
impl<'a> Lms6002d<'a> {
    pub(crate) fn lpf_enable(&mut self, channel: Channel, enable: bool) -> crate::Result<()> {
        let addr = if channel == Channel::Rx { 0x54 } else { 0x34 };
        let mut data = self.read(addr)?;
        if enable {
            data |= 1 << 1;
        } else {
            data &= !(1 << 1);
        }
        self.write(addr, data)?;
        let mut data = self.read(addr + 1)?;
        if (data & (1 << 6)) != 0 {
            data &= !(1 << 6);
            self.write(addr + 1, data)?;
        }
        Ok(())
    }

    pub(crate) fn lpf_get_mode(&mut self, channel: Channel) -> crate::Result<LpfMode> {
        let reg: u8 = if channel == Channel::Rx { 0x54 } else { 0x34 };
        let data_l = self.read(reg)?;
        let data_h = self.read(reg + 1)?;
        let lpf_enabled = (data_l & (1 << 1)) != 0;
        let lpf_bypassed = (data_h & (1 << 6)) != 0;
        match (lpf_enabled, lpf_bypassed) {
            (true, false) => Ok(LpfMode::Normal),
            (false, true) => Ok(LpfMode::Bypassed),
            (false, false) => Ok(LpfMode::Disabled),
            (true, true) => {
                log::error!("Invalid LPF configuration: {data_l:x}, {data_h:x}");
                Err(Error::BoardState("LPF enabled and bypassed simultaneously"))
            }
        }
    }

    pub(crate) fn lpf_set_mode(&mut self, channel: Channel, mode: LpfMode) -> crate::Result<()> {
        let reg: u8 = if channel == Channel::Rx { 0x54 } else { 0x34 };
        let mut data_l = self.read(reg)?;
        let mut data_h = self.read(reg + 1)?;
        match mode {
            LpfMode::Normal => {
                data_l |= 1 << 1;
                data_h &= !(1 << 6);
            }
            LpfMode::Bypassed => {
                data_l &= !(1 << 1);
                data_h |= 1 << 6;
            }
            LpfMode::Disabled => {
                data_l &= !(1 << 1);
                data_h &= !(1 << 6);
            }
        }
        self.write(reg, data_l)?;
        self.write(reg + 1, data_h)
    }
}
