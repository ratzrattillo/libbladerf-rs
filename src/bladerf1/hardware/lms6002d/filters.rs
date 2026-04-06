use crate::bladerf1::hardware::lms6002d::LMS6002D;
use crate::{Channel, Error};
#[derive(PartialEq)]
pub enum LpfMode {
    Normal,
    Bypassed,
    Disabled,
}
impl LMS6002D {
    pub fn lpf_enable(&self, channel: Channel, enable: bool) -> crate::Result<()> {
        let addr = if channel == Channel::Rx { 0x54 } else { 0x34 };
        let mut data = self.read(addr)?;
        if enable {
            data |= 1 << 1;
        } else {
            data &= !(1 << 1);
        }
        self.write(addr, data)?;
        data = self.read(addr + 1)?;
        if data & (1 << 6) != 0 {
            data &= !(1 << 6);
            self.write(addr + 1, data)?;
        }
        Ok(())
    }
    pub fn lpf_get_mode(&self, channel: Channel) -> crate::Result<LpfMode> {
        let reg: u8 = if channel == Channel::Rx { 0x54 } else { 0x34 };
        let data_l = self.read(reg)?;
        let data_h = self.read(reg + 1)?;
        let lpf_enabled = (data_l & (1 << 1)) != 0;
        let lpf_bypassed = (data_h & (1 << 6)) != 0;
        if lpf_enabled && !lpf_bypassed {
            Ok(LpfMode::Normal)
        } else if !lpf_enabled && lpf_bypassed {
            Ok(LpfMode::Bypassed)
        } else if !lpf_enabled && !lpf_bypassed {
            Ok(LpfMode::Disabled)
        } else {
            log::error!("Invalid LPF configuration: {data_l:x}, {data_h:x}");
            Err(Error::Invalid)
        }
    }
    pub fn lpf_set_mode(&self, channel: Channel, mode: LpfMode) -> crate::Result<()> {
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
