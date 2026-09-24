//! Board-level operations for BladeRF1.
//!
//! This module contains [`BladeRf1`] — the primary device handle — and the
//! three session types that gate access to the device's USB alternate
//! settings:
//!
//! - [`RfLinkSession`] — normal RF operation (tuning, gain, streaming, etc.)
//! - [`FlashSession`] — SPI flash read/write/erase
//! - [`ConfigSession`] — FPGA loading and device configuration
//!
//! # Ownership model
//!
//! [`BladeRf1`] owns the USB device and the `NiosCore`
//! that serializes all register I/O. Session types borrow `&mut NiosCore`, so the
//! Rust borrow checker guarantees that at most one session is active at a time.
//! Users never access `NiosCore` directly; they call methods on the session.

mod bandwidth;
mod calibration;
pub(crate) mod corrections;
mod dac_trim;
pub(crate) mod firmware;
mod flash;
pub(crate) mod fpga;
mod frequency;
mod gain;
mod loopback;
mod lpf_mode;
pub use loopback::Loopback;
pub(crate) mod rf_port;
pub(crate) mod rx_mux;
mod sample_rate;
mod smb;
pub mod stream;
mod timestamp;
mod trigger;
mod vctcxo_tamer;
pub mod xb;
use crate::bladerf1::calibration::DcCalTable;
use crate::bladerf1::hardware::dac161s055::Dac161s055;
use crate::bladerf1::hardware::lms6002d::dc_calibration::DcCals;
use crate::bladerf1::hardware::lms6002d::{Band, Lms6002d};
use crate::bladerf1::hardware::si5338::Si5338;
use crate::bladerf1::hardware::spi_flash::FlashMeta;
use crate::channel::Channel;
use crate::error::Error;
use crate::flash::decode_flash_size;
use crate::maybe_future::{Op, sleep};
use crate::nios_client::NiosCore;
use crate::usb::{
    BladeRf1DeviceCommands, BladeRf1UsbInterfaceCommands, DeviceCommands, UsbAltSetting,
    UsbInterfaceCommands, UsbTransport,
};
pub use corrections::Correction;
pub use frequency::QuickTune;
pub use frequency::TuningMode;
use std::path::Path;
pub use trigger::{TriggerRole, TriggerState};
pub use vctcxo_tamer::VctcxoTamerMode;

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
    fn try_from(value: u8) -> std::result::Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Unknown),
            1 => Ok(Self::Flash),
            2 => Ok(Self::Host),
            _ => Err(Error::Unsupported("unknown FPGA source value")),
        }
    }
}
pub use gain::GainMode;
#[cfg(not(target_os = "android"))]
use nusb::DeviceInfo;
use nusb::{Device, MaybeFuture, Speed};
pub use rx_mux::RxMux;
pub use stream::{
    BLADERF_GPIO_8BIT_MODE, BLADERF_GPIO_HIGHLY_PACKED_MODE, BLADERF_GPIO_PACKET,
    BLADERF_GPIO_TIMESTAMP, BLADERF_GPIO_TIMESTAMP_DIV2, METADATA_HEADER_SIZE, MetadataHeader,
    RxStream, RxStreamBuilder, SampleFormat, TxStream, TxStreamBuilder,
};

/// Nuand BladeRF1 USB Vendor ID.
pub const BLADERF1_USB_VID: u16 = 0x2CF0;

/// Nuand BladeRF1 USB Product ID.
pub const BLADERF1_USB_PID: u16 = 0x5246;

/// GPIO bit that enables small DMA transfers on Hi-Speed USB.
pub const BLADERF_GPIO_FEATURE_SMALL_DMA_XFER: u16 = 1 << 7;

#[cfg(not(target_os = "android"))]
fn is_bladerf1(dev: &DeviceInfo) -> bool {
    dev.vendor_id() == BLADERF1_USB_VID && dev.product_id() == BLADERF1_USB_PID
}

/// Primary device handle for the BladeRF1.
///
/// Owns the USB device and the internal `NiosCore`.
/// Construct via [`from_first`](BladeRf1::from_first),
/// [`from_serial`](BladeRf1::from_serial),
/// [`from_bus_addr`](BladeRf1::from_bus_addr),
/// [`from_fd`](BladeRf1::from_fd) (Linux and Android only), or
/// [`from_device`](BladeRf1::from_device) with an already opened
/// [`nusb::Device`] (the only option on WebUSB).
///
/// Every I/O method returns a [`MaybeFuture`]: call `.wait()` to block the
/// current thread (native targets only) or `.await` it from async code.
/// The async path works with any executor; no runtime feature is required.
///
/// On construction the device waits for FX3 firmware readiness and
/// auto-loads DC calibration tables from `<serial>_dc_rx.json` and
/// `<serial>_dc_tx.json`. The directory searched defaults to the current
/// directory; use [`load_dc_cal_tables_from_dir`](BladeRf1::load_dc_cal_tables_from_dir)
/// to load from an explicit path, which is required on platforms without a
/// meaningful working directory such as Android.
///
/// On native targets, dropping the handle disables the RX and TX modules
/// (best-effort, blocking). Use [`close`](BladeRf1::close) for an explicit,
/// non-blocking shutdown; on wasm it is the only shutdown path.
pub struct BladeRf1 {
    device: Device,
    nios: NiosCore,
    dc_rx_table: Option<DcCalTable>,
    dc_tx_table: Option<DcCalTable>,
    closed: bool,
}
impl BladeRf1 {
    /// Lists all BladeRF1 devices currently connected to the host.
    ///
    /// Not available on Android, where USB enumeration is not permitted; open
    /// devices with [`from_fd`](BladeRf1::from_fd) instead.
    #[cfg(not(target_os = "android"))]
    pub fn list_bladerf1()
    -> impl MaybeFuture<Output = crate::Result<impl Iterator<Item = DeviceInfo>>> {
        nusb::list_devices()
            .map_ok(|devices| devices.filter(is_bladerf1))
            .map_err(Error::from)
    }
    fn build(
        device: Device,
        cal_table_dir: Option<&Path>,
    ) -> impl MaybeFuture<Output = crate::Result<Self>> {
        Op::new(async move {
            let manufacturer = device.manufacturer().await;
            let product = device.product().await;
            let serial = device.serial().await;
            let languages = device.get_supported_languages().await;
            log::debug!("Manufacturer: {manufacturer:?}");
            log::debug!("Product: {product:?}");
            log::debug!("Serial: {serial:?}");
            log::debug!("Speed: {:?}", device.speed());
            log::debug!("Languages: {languages:x?}");
            let interface = device.detach_and_claim_interface(0).await?;
            let speed = match device.speed() {
                Some(speed) => speed,
                None => {
                    let speed = Self::infer_speed(&interface).ok_or(Error::UnsupportedSpeed)?;
                    log::debug!("Speed not reported by the host; inferred {speed:?}");
                    speed
                }
            };
            if speed < Speed::High {
                log::error!("BladeRF requires High/Super/SuperPlus speeds");
                return Err(Error::UnsupportedSpeed);
            }
            let nios = NiosCore::new(UsbTransport::new(interface, speed));
            let mut result = Self {
                device,
                nios,
                dc_rx_table: None,
                dc_tx_table: None,
                closed: false,
            };
            result.wait_until_ready().await?;
            Self::auto_load_tables(&mut result, cal_table_dir).await;
            Ok(result)
        })
    }
    /// Derives the bus speed from the bulk endpoint max packet size.
    ///
    /// WebUSB does not expose the negotiated speed. Bulk endpoints are
    /// 1024 bytes at SuperSpeed and 512 bytes at High-Speed.
    fn infer_speed(interface: &nusb::Interface) -> Option<Speed> {
        interface
            .descriptors()
            .flat_map(|desc| desc.endpoints())
            .find(|ep| ep.transfer_type() == nusb::descriptors::TransferType::Bulk)
            .and_then(|ep| match ep.max_packet_size() {
                1024 => Some(Speed::Super),
                512 => Some(Speed::High),
                64 => Some(Speed::Full),
                _ => None,
            })
    }
    fn wait_until_ready(&mut self) -> impl MaybeFuture<Output = crate::Result<()>> {
        Op::new(async move {
            const MAX_RETRIES: u32 = 30;
            for i in 0..MAX_RETRIES {
                match self.nios.interface().usb_is_firmware_ready().await {
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
    async fn auto_load_tables(result: &mut Self, dir: Option<&Path>) {
        let serial = match result.device.serial().await {
            Ok(s) => s,
            Err(e) => {
                log::warn!("Failed to read serial number, skipping DC cal table auto-load: {e}");
                return;
            }
        };
        let base = dir.unwrap_or_else(|| Path::new("."));
        let rx_path = base.join(format!("{serial}_dc_rx.json"));
        let tx_path = base.join(format!("{serial}_dc_tx.json"));
        if rx_path.exists() {
            match DcCalTable::load(&rx_path) {
                Ok(tbl) => {
                    log::debug!("Loaded RX DC cal table from {}", rx_path.display());
                    result.dc_rx_table = Some(tbl);
                }
                Err(e) => log::warn!("Failed to parse RX DC cal table {}: {e}", rx_path.display()),
            }
        }
        if tx_path.exists() {
            match DcCalTable::load(&tx_path) {
                Ok(tbl) => {
                    log::debug!("Loaded TX DC cal table from {}", tx_path.display());
                    result.dc_tx_table = Some(tbl);
                }
                Err(e) => log::warn!("Failed to parse TX DC cal table {}: {e}", tx_path.display()),
            }
        }
    }
    /// Opens the first BladeRF1 device found.
    ///
    /// DC calibration tables are auto-loaded from the current directory. Not
    /// available on Android, which forbids USB enumeration; use
    /// [`from_fd`](BladeRf1::from_fd) there.
    #[cfg(not(target_os = "android"))]
    pub fn from_first() -> impl MaybeFuture<Output = crate::Result<Self>> {
        Op::new(async move {
            let info = Self::list_bladerf1().await?.next().ok_or(Error::NotFound)?;
            let device = info.open().await?;
            Self::build(device, None).await
        })
    }
    /// Opens a BladeRF1 device matching the given serial number string.
    ///
    /// DC calibration tables are auto-loaded from the current directory. Not
    /// available on Android, which forbids USB enumeration; use
    /// [`from_fd`](BladeRf1::from_fd) there.
    #[cfg(not(target_os = "android"))]
    pub fn from_serial(serial: &str) -> impl MaybeFuture<Output = crate::Result<Self>> {
        Op::new(async move {
            let info = Self::list_bladerf1()
                .await?
                .find(|dev| dev.serial_number() == Some(serial))
                .ok_or(Error::NotFound)?;
            let device = info.open().await?;
            Self::build(device, None).await
        })
    }
    /// Opens a BladeRF1 device at the given USB bus number and address.
    ///
    /// DC calibration tables are auto-loaded from the current directory. Not
    /// available on Android, which forbids USB enumeration, nor on WebUSB,
    /// which does not expose bus topology; use
    /// [`from_fd`](BladeRf1::from_fd) or [`from_device`](BladeRf1::from_device).
    #[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
    pub fn from_bus_addr(
        bus_number: &str,
        bus_addr: u8,
    ) -> impl MaybeFuture<Output = crate::Result<Self>> {
        Op::new(async move {
            let info = Self::list_bladerf1()
                .await?
                .find(|dev| dev.bus_id() == bus_number && dev.device_address() == bus_addr)
                .ok_or(Error::NotFound)?;
            let device = info.open().await?;
            Self::build(device, None).await
        })
    }
    /// Opens a BladeRF1 from an already opened [`nusb::Device`].
    ///
    /// Use this when the device was obtained outside this crate, e.g. from
    /// [`nusb::request_device`] or `nusb::Device::from_js` on WebUSB, or
    /// from a custom enumeration filter. The device must be a BladeRF1; no
    /// VID/PID check is performed. DC calibration tables are auto-loaded from
    /// the current directory where a file system is available.
    ///
    /// # Errors
    /// Returns an error if interface 0 cannot be claimed, the bus speed is
    /// below High-Speed, or the firmware never reports ready.
    pub fn from_device(device: Device) -> impl MaybeFuture<Output = crate::Result<Self>> {
        Self::build(device, None)
    }
    /// Opens a BladeRF1 device from a pre-opened file descriptor.
    ///
    /// Available on Linux and Android. On Android the file descriptor is
    /// obtained from `UsbDeviceConnection.getFileDescriptor()`; the host
    /// performs no USB enumeration. DC calibration tables are auto-loaded
    /// from the current directory. On Android, use
    /// [`load_dc_cal_tables_from_dir`](BladeRf1::load_dc_cal_tables_from_dir)
    /// to specify an explicit directory.
    ///
    /// # Errors
    /// Returns an error if the descriptor does not refer to a usable BladeRF1
    /// device, if interface claiming fails, or if the device never becomes
    /// ready.
    #[cfg(any(target_os = "linux", target_os = "android"))]
    pub fn from_fd(fd: std::os::fd::OwnedFd) -> impl MaybeFuture<Output = crate::Result<Self>> {
        Op::new(async move {
            let device = Device::from_fd(fd).await?;
            Self::build(device, None).await
        })
    }

    /// Loads DC calibration tables for this device from `dir` by serial number.
    ///
    /// Searches `dir` for `<serial>_dc_rx.json` and `<serial>_dc_tx.json` and
    /// applies whichever are present, overwriting any currently loaded tables.
    /// Missing or unparsable tables are logged and skipped. Useful when the
    /// tables become available after construction, e.g. after the application
    /// downloads them to its private storage.
    ///
    /// # Errors
    /// Returns an error only if the device serial number cannot be read.
    pub fn load_dc_cal_tables_from_dir(
        &mut self,
        dir: &Path,
    ) -> impl MaybeFuture<Output = crate::Result<()>> {
        Op::new(async move {
            let serial = self.device.serial().await?;
            let rx_path = dir.join(format!("{serial}_dc_rx.json"));
            let tx_path = dir.join(format!("{serial}_dc_tx.json"));
            if rx_path.exists() {
                match DcCalTable::load(&rx_path) {
                    Ok(tbl) => self.dc_rx_table = Some(tbl),
                    Err(e) => {
                        log::warn!("Failed to parse RX DC cal table {}: {e}", rx_path.display())
                    }
                }
            }
            if tx_path.exists() {
                match DcCalTable::load(&tx_path) {
                    Ok(tbl) => self.dc_tx_table = Some(tbl),
                    Err(e) => {
                        log::warn!("Failed to parse TX DC cal table {}: {e}", tx_path.display())
                    }
                }
            }
            Ok(())
        })
    }
    /// Returns the device serial number string.
    pub fn serial(&self) -> impl MaybeFuture<Output = crate::Result<String>> {
        self.device.serial()
    }

    /// Loads a DC calibration table from a JSON file for the given channel.
    pub fn load_dc_cal_table(&mut self, channel: Channel, path: &Path) -> crate::Result<()> {
        let table = DcCalTable::load(path)?;
        match channel {
            Channel::Rx => self.dc_rx_table = Some(table),
            Channel::Tx => self.dc_tx_table = Some(table),
        }
        Ok(())
    }

    /// Removes the DC calibration table for the given channel.
    pub fn clear_dc_cal_table(&mut self, channel: Channel) {
        match channel {
            Channel::Rx => self.dc_rx_table = None,
            Channel::Tx => self.dc_tx_table = None,
        }
    }

    /// Installs an in-memory DC calibration table for the given channel.
    pub fn set_dc_cal_table(&mut self, channel: Channel, table: DcCalTable) {
        match channel {
            Channel::Rx => self.dc_rx_table = Some(table),
            Channel::Tx => self.dc_tx_table = Some(table),
        }
    }

    /// Returns the USB connection speed.
    pub fn speed(&self) -> Speed {
        self.nios.transport().speed()
    }

    /// Returns the FX3 firmware version as a string.
    pub fn fx3_firmware_version(&self) -> impl MaybeFuture<Output = crate::Result<String>> {
        self.device.fx3_firmware_version()
    }

    /// Creates an [`RfLinkSession`] for normal RF operation.
    ///
    /// Switches the USB alt setting to RfLink if not already there.
    /// If streams are active the device is already in RfLink mode and
    /// no redundant USB switch is performed.
    pub fn rf_link_session(
        &mut self,
    ) -> impl MaybeFuture<Output = crate::Result<RfLinkSession<'_>>> {
        Op::new(async move {
            self.nios.usb_change_setting(UsbAltSetting::RfLink).await?;
            Ok(RfLinkSession {
                nios: &mut self.nios,
                dc_rx_table: self.dc_rx_table.as_ref(),
                dc_tx_table: self.dc_tx_table.as_ref(),
            })
        })
    }

    /// Creates a [`FlashSession`] for SPI flash access.
    ///
    /// Returns [`Error::StreamsActive`] if any stream is currently running,
    /// since switching the USB alt setting would disrupt active transfers.
    pub fn flash_session(&mut self) -> impl MaybeFuture<Output = crate::Result<FlashSession<'_>>> {
        Op::new(async move {
            if self.nios.active_streams() > 0 {
                return Err(Error::StreamsActive);
            }
            self.nios
                .usb_change_setting(UsbAltSetting::SpiFlash)
                .await?;
            let result = self
                .nios
                .interface()
                .usb_vendor_cmd_int(crate::usb::VendorRequest::QueryFlashId)
                .await?;
            let manufacturer_id = ((result >> 8) & 0xFF) as u8;
            let device_id = (result & 0xFF) as u8;
            let flash_size_bytes = decode_flash_size(manufacturer_id, device_id)?;
            let total_pages = flash_size_bytes
                / crate::bladerf1::hardware::spi_flash::BLADERF_FLASH_PAGE_SIZE as u32;
            let total_sectors = flash_size_bytes / (64 * 1024);
            Ok(FlashSession {
                nios: &mut self.nios,
                flash_meta: FlashMeta {
                    flash_size_bytes,
                    total_pages,
                    total_sectors,
                },
            })
        })
    }

    /// Creates a [`ConfigSession`] for FPGA loading and device configuration.
    ///
    /// Returns [`Error::StreamsActive`] if any stream is currently running.
    pub fn config_session(
        &mut self,
    ) -> impl MaybeFuture<Output = crate::Result<ConfigSession<'_>>> {
        Op::new(async move {
            if self.nios.active_streams() > 0 {
                return Err(Error::StreamsActive);
            }
            self.nios.usb_change_setting(UsbAltSetting::Config).await?;
            Ok(ConfigSession {
                nios: &mut self.nios,
            })
        })
    }

    /// Resets the device, causing it to re-enumerate on the USB bus.
    pub fn device_reset(&mut self) -> impl MaybeFuture<Output = crate::Result<()>> {
        Op::new(async move { self.nios.interface().usb_device_reset().await })
    }

    /// Returns `true` if the FPGA has been configured (loaded and ready).
    pub fn is_fpga_configured(&mut self) -> impl MaybeFuture<Output = crate::Result<bool>> {
        Op::new(async move { self.nios.interface().usb_is_fpga_configured().await })
    }
}

impl BladeRf1 {
    /// Disables the RX and TX modules and releases the device.
    ///
    /// This is the non-blocking counterpart of the `Drop` implementation
    /// and the only shutdown path on wasm. After `close()` returns, the
    /// `Drop` implementation performs no further I/O.
    ///
    /// # Errors
    /// Returns the first USB error encountered while disabling the modules;
    /// the device is released regardless.
    pub fn close(mut self) -> impl MaybeFuture<Output = crate::Result<()>> {
        Op::new(async move {
            let rx = self
                .nios
                .interface()
                .usb_enable_module(Channel::Rx, false)
                .await;
            let tx = self
                .nios
                .interface()
                .usb_enable_module(Channel::Tx, false)
                .await;
            self.closed = true;
            drop(self);
            rx.and(tx)
        })
    }
}

impl Drop for BladeRf1 {
    fn drop(&mut self) {
        if self.closed {
            return;
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            log::debug!("BladeRf1::drop — shutting down device");
            let _ = self
                .nios
                .interface()
                .usb_enable_module(Channel::Rx, false)
                .wait();
            let _ = self
                .nios
                .interface()
                .usb_enable_module(Channel::Tx, false)
                .wait();
        }
        #[cfg(target_arch = "wasm32")]
        log::warn!("BladeRf1 dropped without close(); RX/TX modules left enabled");
    }
}

/// Session for normal RF operation (tuning, gain, streaming, initialization, etc.).
///
/// Borrows `&mut NiosCore` from [`BladeRf1`], so the borrow checker prevents
/// concurrent access. Also holds references to the DC calibration tables
/// stored on [`BladeRf1`] so that [`initialize`](RfLinkSession::initialize)
/// can apply them after the standard init sequence.
pub struct RfLinkSession<'a> {
    pub(crate) nios: &'a mut NiosCore,
    pub(crate) dc_rx_table: Option<&'a DcCalTable>,
    pub(crate) dc_tx_table: Option<&'a DcCalTable>,
}

/// Session for SPI flash read/write/erase operations.
///
/// Owns flash metadata queried from the device at session creation.
/// Returns [`Error::StreamsActive`] if any stream is running when
/// [`BladeRf1::flash_session`] is called.
pub struct FlashSession<'a> {
    pub(crate) nios: &'a mut NiosCore,
    pub(crate) flash_meta: FlashMeta,
}

/// Session for FPGA loading and device configuration.
///
/// Returns [`Error::StreamsActive`] if any stream is running when
/// [`BladeRf1::config_session`] is called.
pub struct ConfigSession<'a> {
    pub(crate) nios: &'a mut NiosCore,
}

impl RfLinkSession<'_> {
    fn lms(&mut self) -> Lms6002d<'_> {
        Lms6002d { nios: self.nios }
    }

    fn si(&mut self) -> Si5338<'_> {
        Si5338 { nios: self.nios }
    }

    fn dac(&mut self) -> Dac161s055<'_> {
        Dac161s055 { nios: self.nios }
    }

    /// Checks that the device has been initialized by reading the config GPIO.
    ///
    /// Returns [`Error::BoardState`] if the lower 7 bits of GPIO are zero,
    /// meaning [`initialize`](RfLinkSession::initialize) has not yet been
    /// called (or the FPGA was just reloaded, resetting NIOS).
    fn require_initialized(&mut self) -> impl MaybeFuture<Output = crate::Result<()>> {
        Op::new(async move {
            let cfg = self.config_gpio_read().await?;
            if (cfg & 0x7f) == 0 {
                return Err(Error::NotInitialized);
            }
            Ok(())
        })
    }

    /// Returns the FPGA version as a string.
    pub fn fpga_version(&mut self) -> impl MaybeFuture<Output = crate::Result<String>> {
        Op::new(async move {
            let version = self.nios.nios_get_fpga_version().await?;
            Ok(format!("{version}"))
        })
    }

    /// Reads the full 32-bit config GPIO register.
    pub fn config_gpio_read(&mut self) -> impl MaybeFuture<Output = crate::Result<u32>> {
        Op::new(async move { self.nios.nios_config_read().await })
    }

    /// Writes the config GPIO register, automatically setting the small DMA
    /// transfer bit when connected at Hi-Speed USB.
    pub fn config_gpio_write(
        &mut self,
        mut data: u32,
    ) -> impl MaybeFuture<Output = crate::Result<()>> {
        Op::new(async move {
            log::trace!("[config_gpio_write] data: {data}");
            let speed = self.nios.transport().speed();
            if speed == Speed::High {
                data |= BLADERF_GPIO_FEATURE_SMALL_DMA_XFER as u32;
            } else {
                data &= !(BLADERF_GPIO_FEATURE_SMALL_DMA_XFER as u32);
            }
            log::trace!("[config_gpio_write] data after speed check: {data}");
            self.nios.nios_config_write(data).await
        })
    }

    /// Read-modify-write on the config GPIO register.
    ///
    /// The provided closure mutates the current GPIO value. The small DMA
    /// transfer bit is forced to the correct value for the current USB speed
    /// after the closure returns.
    pub fn config_gpio_modify(
        &mut self,
        f: impl FnOnce(u32) -> u32 + Send,
    ) -> impl MaybeFuture<Output = crate::Result<()>> {
        let small_dma = BLADERF_GPIO_FEATURE_SMALL_DMA_XFER as u32;
        let speed = self.nios.transport().speed();
        let mask = if speed == Speed::High { small_dma } else { 0 };
        self.nios
            .nios_config_modify(move |gpio| (f(gpio) & !small_dma) | mask)
    }

    /// Initializes the BladeRF1 for RF operation.
    ///
    /// When `force` is `false`, initialization is skipped if the device is
    /// already in an initialized state (determined by reading the config GPIO).
    /// When `force` is `true`, initialization is performed regardless.
    ///
    /// The init sequence configures the LMS6002D transceiver, sets default
    /// sample rates (1 MHz), DAC trim (0), frequencies (TX 2.447 GHz,
    /// RX 2.484 GHz), and gain mode (MGC). After the standard init sequence,
    /// any loaded DC calibration tables are applied to the LMS6002D registers
    /// and the current frequencies are re-tuned to activate the corrections.
    pub fn initialize(&mut self, force: bool) -> impl MaybeFuture<Output = crate::Result<()>> {
        Op::new(async move {
            let alt_setting = self.nios.get_alt_setting();
            log::trace!("[*] Init - Default Alt Setting {alt_setting:?}");
            if alt_setting != Some(UsbAltSetting::RfLink) {
                self.nios.usb_change_setting(UsbAltSetting::RfLink).await?;
                log::trace!("[*] Init - Set Alt Setting to 0x01");
            }
            let cfg = self.config_gpio_read().await?;
            if force || (cfg & 0x7f) == 0 {
                log::trace!(
                    "[*] Init - {}initializing device (GPIO={cfg:#04x})",
                    if force { "Force " } else { "" }
                );
                self.config_gpio_write(0x57).await?;
                self.lms().enable_rffe(Channel::Tx, false).await?;
                self.lms().enable_rffe(Channel::Rx, false).await?;
                self.lms().write(0x05, 0x3e).await?;
                self.lms().write(0x47, 0x40).await?;
                self.lms().write(0x59, 0x29).await?;
                self.lms().write(0x64, 0x36).await?;
                self.lms().write(0x79, 0x37).await?;
                self.lms().set(0x3f, 0x80).await?;
                self.lms().set(0x5f, 0x80).await?;
                self.lms().set(0x6e, 0xc0).await?;
                self.lms().config_charge_pumps(Channel::Tx).await?;
                self.lms().config_charge_pumps(Channel::Rx).await?;
                {
                    let _actual_tx = self.si().set_sample_rate(Channel::Tx, 1_000_000).await?;
                    let _actual_rx = self.si().set_sample_rate(Channel::Rx, 1_000_000).await?;
                    self.dac().write(0).await?;
                }
                self.set_frequency(Channel::Tx, 2_447_000_000, TuningMode::Fpga)
                    .await?;
                self.set_frequency(Channel::Rx, 2_484_000_000, TuningMode::Fpga)
                    .await?;
                self.set_gain_mode(Channel::Rx, GainMode::Mgc).await?;
            } else {
                log::trace!("[*] Init - Device already initialized: {cfg:#04x}");
            }
            self.apply_dc_cal_tables().await?;
            Ok(())
        })
    }

    /// Applies DC calibration register values from the loaded tables to the
    /// LMS6002D, then re-tunes the current RX/TX frequencies so the
    /// corrections take effect.
    fn apply_dc_cal_tables(&mut self) -> impl MaybeFuture<Output = crate::Result<()>> {
        Op::new(async move {
            if self.dc_rx_table.is_none() && self.dc_tx_table.is_none() {
                return Ok(());
            }
            let rx = self.dc_rx_table.map(|t| t.reg_vals());
            let tx = self.dc_tx_table.map(|t| t.reg_vals());

            let mut cals = DcCals::new(-1, -1, -1, -1, -1, -1, -1, -1, -1, -1);

            if let Some(rx) = rx {
                cals.lpf_tuning = rx.lpf_tuning;
                cals.rx_lpf_i = rx.rx_lpf_i;
                cals.rx_lpf_q = rx.rx_lpf_q;
                cals.dc_ref = rx.dc_ref;
                cals.rxvga2a_i = rx.rxvga2a_i;
                cals.rxvga2a_q = rx.rxvga2a_q;
                cals.rxvga2b_i = rx.rxvga2b_i;
                cals.rxvga2b_q = rx.rxvga2b_q;
            }

            if let Some(tx) = tx {
                cals.tx_lpf_i = tx.tx_lpf_i;
                cals.tx_lpf_q = tx.tx_lpf_q;

                if rx.is_none() {
                    cals.lpf_tuning = tx.lpf_tuning;
                    cals.rx_lpf_i = tx.rx_lpf_i;
                    cals.rx_lpf_q = tx.rx_lpf_q;
                    cals.dc_ref = tx.dc_ref;
                    cals.rxvga2a_i = tx.rxvga2a_i;
                    cals.rxvga2a_q = tx.rxvga2a_q;
                    cals.rxvga2b_i = tx.rxvga2b_i;
                    cals.rxvga2b_q = tx.rxvga2b_q;
                }
            }

            if rx.is_some()
                && tx.is_none()
                && let Some(rx) = rx
            {
                cals.tx_lpf_i = rx.tx_lpf_i;
                cals.tx_lpf_q = rx.tx_lpf_q;
            }

            self.lms().set_dc_cals(cals).await?;

            let rx_f = self.get_frequency(Channel::Rx).await.ok();
            let tx_f = self.get_frequency(Channel::Tx).await.ok();

            if let Some(f) = rx_f {
                self.set_frequency(Channel::Rx, f, TuningMode::Fpga).await?;
            }
            if let Some(f) = tx_f {
                self.set_frequency(Channel::Tx, f, TuningMode::Fpga).await?;
            }
            Ok(())
        })
    }

    /// Enables or disables the RF front-end and USB streaming module for the
    /// given channel.
    ///
    /// Requires the device to be initialized (see [`initialize`](RfLinkSession::initialize)).
    pub fn enable_module(
        &mut self,
        channel: Channel,
        enable: bool,
    ) -> impl MaybeFuture<Output = crate::Result<()>> {
        Op::new(async move {
            self.require_initialized().await?;
            self.lms().enable_rffe(channel, enable).await?;
            self.nios
                .interface()
                .usb_enable_module(channel, enable)
                .await
        })
    }

    /// Queries whether the currently loaded FPGA came from flash or was loaded
    /// by the host.
    pub fn get_fpga_source(&mut self) -> impl MaybeFuture<Output = crate::Result<FpgaSource>> {
        Op::new(async move {
            let result = self
                .nios
                .interface()
                .usb_vendor_cmd_int(crate::usb::VendorRequest::QueryFpgaSource)
                .await?;
            FpgaSource::try_from(result as u8)
        })
    }

    /// Selects the LNA/PA band on the LMS6002D and updates the config GPIO
    /// band-select bits accordingly.
    ///
    /// Low band (< 1.5 GHz) uses LNA1/PA1; high band (>= 1.5 GHz) uses
    /// LNA2/PA2.
    pub fn band_select(
        &mut self,
        channel: Channel,
        band: Band,
    ) -> impl MaybeFuture<Output = crate::Result<()>> {
        Op::new(async move {
            let band_value = match band {
                Band::Low => 2,
                Band::High => 1,
            };
            log::trace!("Selecting {band:?} band");
            self.lms().select_band(channel, band).await?;
            self.config_gpio_modify(|gpio| {
                let clear_mask = if channel == Channel::Tx {
                    3 << 3
                } else {
                    3 << 5
                };
                let shift = if channel == Channel::Tx {
                    band_value << 3
                } else {
                    band_value << 5
                };
                (gpio & !clear_mask) | shift
            })
            .await
        })
    }
}
