//! Loopback mode control for BladeRF1.
//!
//! Configures internal loopback paths on the LMS6002D for testing and
//! calibration without external cabling. Baseband loopback routes signals
//! internally between the TX and RX digital analog paths. RF loopback
//! routes signals between the TX output and RX input at radio frequency,
//! passing through the antenna port connectors. Firmware loopback operates
//! at the USB transport level, returning transmitted samples as received
//! without passing through the RF chain.

use crate::bladerf1::board::RfLinkSession;
/// Loopback mode for routing signals internally for testing.
///
/// Re-exported from the LMS6002D driver. Includes baseband loopback
/// (BB variants), RF loopback (LNA variants), and firmware loopback.
pub use crate::bladerf1::hardware::lms6002d::loopback::Loopback;
use crate::error::Result;
use crate::usb::BladeRf1UsbInterfaceCommands;
impl RfLinkSession<'_> {
    /// Sets the loopback mode.
    ///
    /// `Loopback::Firmware` enables USB-level firmware loopback while
    /// configuring the LMS6002D RF loopback through LNA3. All other modes
    /// first disable firmware loopback if it was active, then configure the
    /// LMS6002D loopback path.
    ///
    /// Returns `Error::NotInitialized` if the board has not been initialized.
    pub fn set_loopback(&mut self, lb: Loopback) -> Result<()> {
        self.require_initialized()?;
        match lb {
            Loopback::Firmware => {
                self.lms().set_loopback_mode(Loopback::Lna3)?;
                self.nios.usb_set_firmware_loopback(true)
            }
            _ => {
                let fw_lb_enabled: bool = self.nios.usb_get_firmware_loopback()?;
                if fw_lb_enabled {
                    self.nios.usb_set_firmware_loopback(false)?;
                }
                self.lms().set_loopback_mode(lb)
            }
        }
    }
    /// Sets the loopback mode on the LMS6002D only, without affecting firmware loopback.
    ///
    /// Returns `Error::NotInitialized` if the board has not been initialized.
    pub fn set_lms_loopback(&mut self, lb: Loopback) -> Result<()> {
        self.require_initialized()?;
        self.lms().set_loopback_mode(lb)
    }
    /// Returns the current LMS6002D loopback mode, independent of firmware loopback.
    ///
    /// Returns `Error::NotInitialized` if the board has not been initialized.
    pub fn get_lms_loopback(&mut self) -> Result<Loopback> {
        self.require_initialized()?;
        self.lms().get_loopback_mode()
    }
    /// Returns the current effective loopback mode.
    ///
    /// Checks for firmware loopback first; if not active, returns the
    /// LMS6002D loopback mode.
    ///
    /// Returns `Error::NotInitialized` if the board has not been initialized.
    pub fn get_loopback(&mut self) -> Result<Loopback> {
        self.require_initialized()?;
        let mut lb = Loopback::None;
        let fw_lb_enabled = self.nios.usb_get_firmware_loopback()?;
        if fw_lb_enabled {
            lb = Loopback::Firmware;
        }
        if lb == Loopback::None {
            lb = self.lms().get_loopback_mode()?;
        }
        Ok(lb)
    }
    /// Returns true if the given loopback mode is supported on BladeRF1.
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
