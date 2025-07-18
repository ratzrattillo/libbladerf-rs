use crate::BladeRf1;
use anyhow::Result;
use bladerf_globals::BladerfLoopback;

impl BladeRf1 {
    pub fn set_loopback(&self, lb: BladerfLoopback) -> Result<()> {
        // CHECK_BOARD_STATE(STATE_INITIALIZED);

        match lb {
            BladerfLoopback::Firmware => {
                /* Samples won't reach the LMS when the device is in firmware
                 * loopback mode. By placing the LMS into a loopback mode, we ensure
                 * that the PAs will be disabled, and remain enabled across
                 * frequency changes.
                 */
                self.lms.set_loopback_mode(BladerfLoopback::Lna3)?;
                self.usb_set_firmware_loopback(true)?;
            }
            _ => {
                /* Query first, as the implementation of setting the mode
                 * may interrupt running streams. The API don't guarantee that
                 * switching loopback modes on the fly to work, but we can at least
                 * try to avoid unnecessarily interrupting things...*/
                let fw_lb_enabled: bool = self.usb_get_firmware_loopback()?;

                if fw_lb_enabled {
                    self.usb_set_firmware_loopback(false)?;
                }

                self.lms.set_loopback_mode(lb)?;
            }
        }
        Ok(())
    }

    pub fn get_loopback(&self) -> Result<BladerfLoopback> {
        // CHECK_BOARD_STATE(STATE_INITIALIZED);

        let mut lb = BladerfLoopback::None;

        let fw_lb_enabled = self.usb_get_firmware_loopback()?;
        if fw_lb_enabled {
            lb = BladerfLoopback::Firmware;
        }

        if lb == BladerfLoopback::None {
            lb = self.lms.get_loopback_mode()?;
        }

        Ok(lb)
    }

    pub fn is_loopback_mode_supported(&self, lb: BladerfLoopback) -> bool {
        let supported_modes = [
            BladerfLoopback::None,
            BladerfLoopback::BbTxlpfRxvga2,
            BladerfLoopback::BbTxlpfRxlpf,
            BladerfLoopback::BbTxvga1Rxlpf,
            BladerfLoopback::BbTxvga1Rxvga2,
            BladerfLoopback::Firmware,
            BladerfLoopback::Lna1,
            BladerfLoopback::Lna2,
            BladerfLoopback::Lna3,
        ];

        supported_modes.contains(&lb)
    }
}
