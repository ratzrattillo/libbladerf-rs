//! LMS6002D LPF mode configuration.
//!
//! The digital LPF can be enabled in normal filtering mode, bypassed to
//! pass the full signal chain, or disabled entirely.

use crate::bladerf1::hardware::lms6002d::Lms6002d;
use crate::maybe_future::Op;
use crate::{Channel, Error};
use nusb::MaybeFuture;

bitflags::bitflags! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    struct LpfControlFlags: u8 {
        const ENABLE = 1 << 1;
        const _ = !0;
    }

    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    struct LpfTuningFlags: u8 {
        const BYPASS = 1 << 6;
        const _ = !0;
    }
}

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
    pub(crate) fn lpf_enable(
        &mut self,
        channel: Channel,
        enable: bool,
    ) -> impl MaybeFuture<Output = crate::Result<()>> {
        Op::new(async move {
            let addr = if channel == Channel::Rx { 0x54 } else { 0x34 };
            let mut data = LpfControlFlags::from_bits_retain(self.read(addr).await?);
            data.set(LpfControlFlags::ENABLE, enable);
            self.write(addr, data.bits()).await?;
            let mut data = LpfTuningFlags::from_bits_retain(self.read(addr + 1).await?);
            if data.contains(LpfTuningFlags::BYPASS) {
                data.remove(LpfTuningFlags::BYPASS);
                self.write(addr + 1, data.bits()).await?;
            }
            Ok(())
        })
    }

    pub(crate) fn lpf_get_mode(
        &mut self,
        channel: Channel,
    ) -> impl MaybeFuture<Output = crate::Result<LpfMode>> {
        Op::new(async move {
            let reg: u8 = if channel == Channel::Rx { 0x54 } else { 0x34 };
            let data_l = LpfControlFlags::from_bits_retain(self.read(reg).await?);
            let data_h = LpfTuningFlags::from_bits_retain(self.read(reg + 1).await?);
            let lpf_enabled = data_l.contains(LpfControlFlags::ENABLE);
            let lpf_bypassed = data_h.contains(LpfTuningFlags::BYPASS);
            match (lpf_enabled, lpf_bypassed) {
                (true, false) => Ok(LpfMode::Normal),
                (false, true) => Ok(LpfMode::Bypassed),
                (false, false) => Ok(LpfMode::Disabled),
                (true, true) => {
                    log::error!("Invalid LPF configuration: {data_l:x}, {data_h:x}");
                    Err(Error::BoardState("LPF enabled and bypassed simultaneously"))
                }
            }
        })
    }

    pub(crate) fn lpf_set_mode(
        &mut self,
        channel: Channel,
        mode: LpfMode,
    ) -> impl MaybeFuture<Output = crate::Result<()>> {
        Op::new(async move {
            let reg: u8 = if channel == Channel::Rx { 0x54 } else { 0x34 };
            let mut data_l = LpfControlFlags::from_bits_retain(self.read(reg).await?);
            let mut data_h = LpfTuningFlags::from_bits_retain(self.read(reg + 1).await?);
            data_l.set(LpfControlFlags::ENABLE, mode == LpfMode::Normal);
            data_h.set(LpfTuningFlags::BYPASS, mode == LpfMode::Bypassed);
            self.write(reg, data_l.bits()).await?;
            self.write(reg + 1, data_h.bits()).await
        })
    }
}
