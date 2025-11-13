use crate::Result;
use crate::bladerf1::BladeRf1;
use crate::hardware::lms6002d::Loopback;

impl BladeRf1 {
    /// Set the loopback config to one of the supported `BladeRf1::hardware::lms6002d::Loopback` modes
    /// this is usually only required for testing purposes.
    pub fn set_loopback(&self, lb: Loopback) -> Result<()> {
        // CHECK_BOARD_STATE(STATE_INITIALIZED);

        match lb {
            Loopback::Firmware => {
                // Samples won't reach the LMS when the device is in firmware
                // loopback mode. By placing the LMS into a loopback mode, we ensure
                // that the PAs will be disabled, and remain enabled across
                // frequency changes.
                self.lms.set_loopback_mode(Loopback::Lna3)?;
                self.usb_set_firmware_loopback(true)
            }
            _ => {
                // Query first, as the implementation of setting the mode
                // may interrupt running streams. The API don't guarantee that
                // switching loopback modes on the fly to work, but we can at least
                // try to avoid unnecessarily interrupting things...
                let fw_lb_enabled: bool = self.usb_get_firmware_loopback()?;

                if fw_lb_enabled {
                    self.usb_set_firmware_loopback(false)?;
                }

                self.lms.set_loopback_mode(lb)
            }
        }
    }

    /// Get the currently active loopback mode
    pub fn get_loopback(&self) -> Result<Loopback> {
        // CHECK_BOARD_STATE(STATE_INITIALIZED);

        let mut lb = Loopback::None;

        let fw_lb_enabled = self.usb_get_firmware_loopback()?;
        if fw_lb_enabled {
            lb = Loopback::Firmware;
        }

        if lb == Loopback::None {
            lb = self.lms.get_loopback_mode()?;
        }

        Ok(lb)
    }

    /// Check if the specified loopback mode is supported on the BladerRf1
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
