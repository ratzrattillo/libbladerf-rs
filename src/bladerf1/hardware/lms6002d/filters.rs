use crate::bladerf1::nios_client::NiosClient;
use crate::{Channel, Error};
#[derive(PartialEq)]
pub enum LpfMode {
    Normal,
    Bypassed,
    Disabled,
}
pub fn lpf_enable(nios: &mut NiosClient, channel: Channel, enable: bool) -> crate::Result<()> {
    let addr = if channel == Channel::Rx { 0x54 } else { 0x34 };
    let mut data = super::read(nios, addr)?;
    if enable {
        data |= 1 << 1;
    } else {
        data &= !(1 << 1);
    }
    super::write(nios, addr, data)?;
    data = super::read(nios, addr + 1)?;
    if data & (1 << 6) != 0 {
        data &= !(1 << 6);
        super::write(nios, addr + 1, data)?;
    }
    Ok(())
}
pub fn lpf_get_mode(nios: &mut NiosClient, channel: Channel) -> crate::Result<LpfMode> {
    let reg: u8 = if channel == Channel::Rx { 0x54 } else { 0x34 };
    let data_l = super::read(nios, reg)?;
    let data_h = super::read(nios, reg + 1)?;
    let lpf_enabled = (data_l & (1 << 1)) != 0;
    let lpf_bypassed = (data_h & (1 << 6)) != 0;
    match (lpf_enabled, lpf_bypassed) {
        (true, false) => Ok(LpfMode::Normal),
        (false, true) => Ok(LpfMode::Bypassed),
        (false, false) => Ok(LpfMode::Disabled),
        (true, true) => {
            log::error!("Invalid LPF configuration: {data_l:x}, {data_h:x}");
            Err(Error::HardwareState(
                "LPF enabled and bypassed simultaneously",
            ))
        }
    }
}
pub fn lpf_set_mode(nios: &mut NiosClient, channel: Channel, mode: LpfMode) -> crate::Result<()> {
    let reg: u8 = if channel == Channel::Rx { 0x54 } else { 0x34 };
    let mut data_l = super::read(nios, reg)?;
    let mut data_h = super::read(nios, reg + 1)?;
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
    super::write(nios, reg, data_l)?;
    super::write(nios, reg + 1, data_h)
}
