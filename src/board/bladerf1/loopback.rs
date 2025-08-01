use crate::BladeRf1;
use crate::Result;
use bladerf_globals::BladeRf1Loopback;

impl BladeRf1 {
    pub fn set_loopback(&self, lb: BladeRf1Loopback) -> Result<()> {
        // CHECK_BOARD_STATE(STATE_INITIALIZED);

        match lb {
            BladeRf1Loopback::Firmware => {
                // Samples won't reach the LMS when the device is in firmware
                // loopback mode. By placing the LMS into a loopback mode, we ensure
                // that the PAs will be disabled, and remain enabled across
                // frequency changes.
                self.lms.set_loopback_mode(BladeRf1Loopback::Lna3)?;
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

    pub fn get_loopback(&self) -> Result<BladeRf1Loopback> {
        // CHECK_BOARD_STATE(STATE_INITIALIZED);

        let mut lb = BladeRf1Loopback::None;

        let fw_lb_enabled = self.usb_get_firmware_loopback()?;
        if fw_lb_enabled {
            lb = BladeRf1Loopback::Firmware;
        }

        if lb == BladeRf1Loopback::None {
            lb = self.lms.get_loopback_mode()?;
        }

        Ok(lb)
    }

    pub fn is_loopback_mode_supported(&self, lb: BladeRf1Loopback) -> bool {
        let supported_modes = [
            BladeRf1Loopback::None,
            BladeRf1Loopback::BbTxlpfRxvga2,
            BladeRf1Loopback::BbTxlpfRxlpf,
            BladeRf1Loopback::BbTxvga1Rxlpf,
            BladeRf1Loopback::BbTxvga1Rxvga2,
            BladeRf1Loopback::Firmware,
            BladeRf1Loopback::Lna1,
            BladeRf1Loopback::Lna2,
            BladeRf1Loopback::Lna3,
        ];

        supported_modes.contains(&lb)
    }
}
