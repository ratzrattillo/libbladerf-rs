use crate::bladerf1::BladeRf1;
use crate::bladerf1::hardware::lms6002d;
pub use crate::bladerf1::hardware::lms6002d::loopback::Loopback;
use crate::error::Result;
use crate::transport::usb::BladeRf1UsbInterfaceCommands;
impl BladeRf1 {
    pub fn set_loopback(&mut self, lb: Loopback) -> Result<()> {
        match lb {
            Loopback::Firmware => {
                lms6002d::loopback::set_loopback_mode(&mut self.nios, Loopback::Lna3)?;
                self.nios.usb_set_firmware_loopback(true)
            }
            _ => {
                let fw_lb_enabled: bool = self.nios.usb_get_firmware_loopback()?;
                if fw_lb_enabled {
                    self.nios.usb_set_firmware_loopback(false)?;
                }
                lms6002d::loopback::set_loopback_mode(&mut self.nios, lb)
            }
        }
    }
    pub fn set_lms_loopback(&mut self, lb: Loopback) -> Result<()> {
        lms6002d::loopback::set_loopback_mode(&mut self.nios, lb)
    }
    pub fn get_lms_loopback(&mut self) -> Result<Loopback> {
        lms6002d::loopback::get_loopback_mode(&mut self.nios)
    }
    pub fn get_loopback(&mut self) -> Result<Loopback> {
        let mut lb = Loopback::None;
        let fw_lb_enabled = self.nios.usb_get_firmware_loopback()?;
        if fw_lb_enabled {
            lb = Loopback::Firmware;
        }
        if lb == Loopback::None {
            lb = lms6002d::loopback::get_loopback_mode(&mut self.nios)?;
        }
        Ok(lb)
    }
    pub fn is_loopback_mode_supported(&self, lb: Loopback) -> bool {
        let supported_modes = [
            Loopback::None,
            Loopback::BbTxlpfRxvga2,
            Loopback::BbTxlpfRxlpf,
            Loopback::BbTxvga1Rxlpf,
            Loopback::BbTxvga1Rxvga2,
            Loopback::Firmware,
            Loopback::Lna1,
            Loopback::Lna2,
            Loopback::Lna3,
        ];
        supported_modes.contains(&lb)
    }
}
