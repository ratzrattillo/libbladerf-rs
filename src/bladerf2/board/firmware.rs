//! FX3 firmware version.

use crate::bladerf2::board::BladeRf2;
use crate::error::Result;
use crate::usb::Fx3DeviceCommands;
use crate::version::SemanticVersion;
use nusb::MaybeFuture;

impl BladeRf2 {
    /// Returns the FX3 firmware version string (e.g. `2.4.0-git-a3d5c55f`).
    pub fn fx3_firmware_version(&self) -> impl MaybeFuture<Output = Result<String>> {
        self.device.fx3_firmware_version()
    }

    /// Returns the parsed FX3 firmware version.
    pub fn firmware_version(&self) -> impl MaybeFuture<Output = Result<SemanticVersion>> {
        self.device
            .fx3_firmware_version()
            .map(|version| version?.parse())
    }
}
