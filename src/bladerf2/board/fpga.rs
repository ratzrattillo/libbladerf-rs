//! FPGA status, version, source, and bitstream size.

use crate::bladerf2::board::RfLinkSession;
use crate::error::{Error, Result};
use crate::flash::FpgaSize;
use crate::maybe_future::Op;
use crate::usb::{Fx3UsbInterfaceCommands, UsbInterfaceCommands, VendorRequest};
use nusb::MaybeFuture;

/// Bitstream size in bytes of the xA4 (Cyclone V 49 kLE) FPGA.
pub const BLADERF_FLASH_FPGA_SIZE_A4: usize = 2_632_660;
/// Bitstream size in bytes of the xA5 (Cyclone V 77 kLE) FPGA.
pub const BLADERF_FLASH_FPGA_SIZE_A5: usize = 4_244_820;
/// Bitstream size in bytes of the xA9 (Cyclone V 301 kLE) FPGA.
pub const BLADERF_FLASH_FPGA_SIZE_A9: usize = 12_858_972;

/// Returns the bitstream size in bytes for a BladeRF2 FPGA package.
///
/// # Errors
/// Returns [`Error::Unsupported`] for BladeRF1 packages (`KLE40`, `KLE115`).
pub fn fpga_bytes(size: FpgaSize) -> Result<usize> {
    match size {
        FpgaSize::A4 => Ok(BLADERF_FLASH_FPGA_SIZE_A4),
        FpgaSize::A5 => Ok(BLADERF_FLASH_FPGA_SIZE_A5),
        FpgaSize::A9 => Ok(BLADERF_FLASH_FPGA_SIZE_A9),
        FpgaSize::KLE40 | FpgaSize::KLE115 => {
            Err(Error::Unsupported("BladeRF1 FPGA size on a BladeRF2"))
        }
    }
}

/// Source from which the FPGA bitstream was loaded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FpgaSource {
    /// The source could not be determined.
    Unknown = 0,
    /// The FPGA was loaded from the SPI flash by the FX3 firmware.
    Flash = 1,
    /// The FPGA was loaded by the host over USB.
    Host = 2,
}

impl TryFrom<u8> for FpgaSource {
    type Error = Error;
    fn try_from(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::Unknown),
            1 => Ok(Self::Flash),
            2 => Ok(Self::Host),
            _ => Err(Error::Unsupported("unknown FPGA source value")),
        }
    }
}

impl RfLinkSession<'_> {
    /// Returns `true` if the FPGA has completed configuration.
    pub fn is_fpga_configured(&mut self) -> impl MaybeFuture<Output = Result<bool>> {
        Op::new(async move { self.nios.control().await?.usb_is_fpga_configured().await })
    }

    /// Returns the FPGA version as a string.
    ///
    /// Requires a configured FPGA; the NIOS does not answer otherwise.
    pub fn fpga_version(&mut self) -> impl MaybeFuture<Output = Result<String>> {
        self.nios
            .nios_get_fpga_version()
            .map_ok(|version| version.to_string())
    }

    /// Queries whether the currently loaded FPGA came from flash or was loaded
    /// by the host.
    pub fn get_fpga_source(&mut self) -> impl MaybeFuture<Output = Result<FpgaSource>> {
        Op::new(async move {
            let result = self
                .nios
                .control()
                .await?
                .usb_vendor_cmd_int(VendorRequest::QueryFpgaSource)
                .await?;
            FpgaSource::try_from(result as u8)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bitstream_sizes_match_libbladerf() {
        assert_eq!(fpga_bytes(FpgaSize::A4).unwrap(), 2_632_660);
        assert_eq!(fpga_bytes(FpgaSize::A5).unwrap(), 4_244_820);
        assert_eq!(fpga_bytes(FpgaSize::A9).unwrap(), 12_858_972);
        assert!(fpga_bytes(FpgaSize::KLE40).is_err());
        assert!(fpga_bytes(FpgaSize::KLE115).is_err());
    }
}
