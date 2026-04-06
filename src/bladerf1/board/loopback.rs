use crate::bladerf1::BladeRf1;
use crate::bladerf1::hardware::lms6002d::loopback::Loopback;
use crate::error::Result;
use crate::transport::usb::BladeRf1UsbInterfaceCommands;
impl BladeRf1 {
    pub fn set_loopback(&self, lb: Loopback) -> Result<()> {
        match lb {
            Loopback::Firmware => {
                self.lms.set_loopback_mode(Loopback::Lna3)?;
                self.interface
                    .lock()
                    .unwrap()
                    .usb_set_firmware_loopback(true)
            }
            _ => {
                let fw_lb_enabled: bool =
                    self.interface.lock().unwrap().usb_get_firmware_loopback()?;
                if fw_lb_enabled {
                    self.interface
                        .lock()
                        .unwrap()
                        .usb_set_firmware_loopback(false)?;
                }
                self.lms.set_loopback_mode(lb)
            }
        }
    }
    pub fn get_loopback(&self) -> Result<Loopback> {
        let mut lb = Loopback::None;
        let fw_lb_enabled = self.interface.lock().unwrap().usb_get_firmware_loopback()?;
        if fw_lb_enabled {
            lb = Loopback::Firmware;
        }
        if lb == Loopback::None {
            lb = self.lms.get_loopback_mode()?;
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
