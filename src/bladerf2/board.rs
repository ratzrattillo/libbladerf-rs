//! Board-level operations for BladeRF2.
//!
//! This module contains [`BladeRf2`] — the primary device handle — and the
//! session types that gate access to the device's USB alternate settings:
//!
//! - [`RfLinkSession`] — normal RF operation
//! - [`FlashSession`] — SPI flash and calibration data access
//!
//! # Ownership model
//!
//! [`BladeRf2`] owns the USB device and the `NiosCore` that serializes all
//! register I/O. Session types borrow `&mut NiosCore`, so the Rust borrow
//! checker guarantees that at most one session is active at a time.

pub(crate) mod calibration;
mod debug;
mod firmware;
pub(crate) mod fpga;
use crate::bladerf2::hardware::spi_flash::FlashMeta;
use crate::error::Error;
use crate::flash::decode_flash_size;
use crate::maybe_future::{Op, await_maybe, sleep};
use crate::nios_client::NiosCore;
use crate::usb::{
    DeviceCommands, Fx3UsbInterfaceCommands, UsbAltSetting, UsbInterfaceCommands, UsbTransport,
    VendorRequest,
};
pub use calibration::DEFAULT_VCTCXO_TRIM;
pub use fpga::{
    BLADERF_FLASH_FPGA_SIZE_A4, BLADERF_FLASH_FPGA_SIZE_A5, BLADERF_FLASH_FPGA_SIZE_A9, FpgaSource,
    fpga_bytes,
};
#[cfg(not(target_os = "android"))]
use nusb::DeviceInfo;
use nusb::{Device, MaybeFuture, Speed};

/// Nuand BladeRF2 USB Vendor ID.
pub const BLADERF2_USB_VID: u16 = 0x2CF0;

/// Nuand BladeRF2 USB Product ID.
pub const BLADERF2_USB_PID: u16 = 0x5250;

#[cfg(not(target_os = "android"))]
fn is_bladerf2(dev: &DeviceInfo) -> bool {
    dev.vendor_id() == BLADERF2_USB_VID && dev.product_id() == BLADERF2_USB_PID
}

/// Primary device handle for the BladeRF2.
///
/// Owns the USB device and the internal `NiosCore`.
/// Construct via [`from_first`](BladeRf2::from_first),
/// [`from_serial`](BladeRf2::from_serial), or
/// [`from_device`](BladeRf2::from_device) with an already opened
/// [`nusb::Device`].
///
/// Every I/O method returns a [`MaybeFuture`]: call `.wait()` to block the
/// current thread (native targets only) or `.await` it from async code.
pub struct BladeRf2 {
    device: Device,
    nios: NiosCore,
}

impl BladeRf2 {
    /// Lists all BladeRF2 devices currently connected to the host.
    #[cfg(not(target_os = "android"))]
    pub fn list_bladerf2()
    -> impl MaybeFuture<Output = crate::Result<impl Iterator<Item = DeviceInfo>>> {
        nusb::list_devices()
            .map_ok(|devices| devices.filter(is_bladerf2))
            .map_err(Error::from)
    }

    fn build(device: Device) -> impl MaybeFuture<Output = crate::Result<Self>> {
        Op::new(async move {
            let manufacturer = device.manufacturer().await;
            let product = device.product().await;
            let serial = device.serial().await;
            log::debug!("Manufacturer: {manufacturer:?}");
            log::debug!("Product: {product:?}");
            log::debug!("Serial: {serial:?}");
            log::debug!("Speed: {:?}", device.speed());
            let interface = await_maybe(device.detach_and_claim_interface(0)).await?;
            let speed = device.speed().ok_or(Error::UnsupportedSpeed)?;
            if speed < Speed::High {
                log::error!("BladeRF requires High/Super/SuperPlus speeds");
                return Err(Error::UnsupportedSpeed);
            }
            let nios = NiosCore::new(UsbTransport::new(interface, speed));
            let mut result = Self { device, nios };
            result.wait_until_ready().await?;
            Ok(result)
        })
    }

    fn wait_until_ready(&mut self) -> impl MaybeFuture<Output = crate::Result<()>> {
        Op::new(async move {
            const MAX_RETRIES: u32 = 30;
            for i in 0..MAX_RETRIES {
                match self.nios.control().await?.usb_is_firmware_ready().await {
                    Ok(true) => return Ok(()),
                    Ok(false) => {
                        if i == 0 {
                            log::info!("Waiting for device to become ready...");
                        } else {
                            log::debug!("Retry {}/{}.", i + 1, MAX_RETRIES);
                        }
                        sleep(std::time::Duration::from_secs(1)).await;
                    }
                    Err(e) => {
                        log::warn!(
                            "Firmware does not support device ready query ({e:#}). \
                             Ensure flash-autoloading completes before opening the device."
                        );
                        return Ok(());
                    }
                }
            }
            log::debug!("Timed out while waiting for device.");
            Err(Error::Timeout)
        })
    }

    /// Opens the first BladeRF2 device found.
    #[cfg(not(target_os = "android"))]
    pub fn from_first() -> impl MaybeFuture<Output = crate::Result<Self>> {
        Op::new(async move {
            let info = await_maybe(Self::list_bladerf2())
                .await?
                .next()
                .ok_or(Error::NotFound)?;
            let device = await_maybe(info.open()).await?;
            Self::build(device).await
        })
    }

    /// Opens a BladeRF2 device matching the given serial number string.
    #[cfg(not(target_os = "android"))]
    pub fn from_serial(serial: &str) -> impl MaybeFuture<Output = crate::Result<Self>> {
        Op::new(async move {
            let info = await_maybe(Self::list_bladerf2())
                .await?
                .find(|dev| dev.serial_number() == Some(serial))
                .ok_or(Error::NotFound)?;
            let device = await_maybe(info.open()).await?;
            Self::build(device).await
        })
    }

    /// Opens a BladeRF2 from an already opened [`nusb::Device`].
    ///
    /// The device must be a BladeRF2; no VID/PID check is performed.
    ///
    /// # Errors
    /// Returns an error if interface 0 cannot be claimed, the bus speed is
    /// below High-Speed, or the firmware never reports ready.
    pub fn from_device(device: Device) -> impl MaybeFuture<Output = crate::Result<Self>> {
        Self::build(device)
    }

    /// Returns the device serial number string.
    pub fn serial(&self) -> impl MaybeFuture<Output = crate::Result<String>> {
        self.device.serial()
    }

    /// Creates an [`RfLinkSession`] for normal RF operation.
    ///
    /// Switches the USB alt setting to RfLink if not already there.
    pub fn rf_link_session(
        &mut self,
    ) -> impl MaybeFuture<Output = crate::Result<RfLinkSession<'_>>> {
        Op::new(async move {
            self.nios.usb_change_setting(UsbAltSetting::RfLink).await?;
            Ok(RfLinkSession {
                device: &self.device,
                nios: &mut self.nios,
            })
        })
    }

    /// Creates a [`FlashSession`] for SPI flash access.
    ///
    /// # Errors
    /// Returns [`Error::StreamsActive`] for any live stream claim.
    pub fn flash_session(&mut self) -> impl MaybeFuture<Output = crate::Result<FlashSession<'_>>> {
        Op::new(async move {
            self.nios.streams.require_idle()?;
            self.nios
                .usb_change_setting(UsbAltSetting::SpiFlash)
                .await?;

            let result = self
                .nios
                .control()
                .await?
                .usb_vendor_cmd_int(VendorRequest::QueryFlashId)
                .await?;

            let manufacturer_id = ((result >> 8) & 0xFF) as u8;
            let device_id = (result & 0xFF) as u8;
            let flash_size_bytes = decode_flash_size(manufacturer_id, device_id)?;

            Ok(FlashSession {
                nios: &mut self.nios,
                flash_meta: FlashMeta::new(flash_size_bytes)?,
            })
        })
    }
}

/// Session for normal RF operation.
///
/// Borrows `&mut NiosCore` from [`BladeRf2`], so the borrow checker prevents
/// concurrent access.
pub struct RfLinkSession<'a> {
    #[allow(dead_code)]
    device: &'a Device,
    pub(crate) nios: &'a mut NiosCore,
}

/// Session for SPI flash read/write/erase operations.
///
/// Owns flash metadata queried from the device at session creation.
/// Returns [`Error::StreamsActive`] if any stream holds an endpoint claim when
/// [`BladeRf2::flash_session`] is called.
pub struct FlashSession<'a> {
    pub(crate) nios: &'a mut NiosCore,
    pub(crate) flash_meta: FlashMeta,
}
