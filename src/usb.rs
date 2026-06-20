//! USB transport layer and vendor command interface.
//!
//! Wraps the `nusb` crate for BladeRF device communication. Manages
//! USB alternate settings, NIOS packet bulk transfers, and vendor-specific
//! control requests. Provides traits (`UsbInterfaceCommands`,
//! `BladeRf1UsbInterfaceCommands`, `DeviceCommands`, `BladeRf1DeviceCommands`)
//! that abstract over the USB interface for use by higher layers.

use crate::channel::Channel;
use crate::error::{Error, Result};
use crate::protocol::nios::NiosPacketError;
use nusb::transfer::{Buffer, Bulk, ControlIn, ControlOut, ControlType, In, Out, Recipient};
use nusb::{Device, Endpoint, Interface, MaybeFuture, Speed};
use std::num::NonZero;
use std::time::Duration;

/// USB endpoint address for the control OUT bulk endpoint.
pub const CONTROL_ENDPOINT_OUT: u8 = 0x02;
/// USB endpoint address for the control IN bulk endpoint.
pub const CONTROL_ENDPOINT_IN: u8 = 0x82;
/// USB endpoint address for the RX streaming bulk endpoint.
pub const STREAM_ENDPOINT_RX: u8 = 0x81;
/// USB endpoint address for the TX streaming bulk endpoint.
pub const STREAM_ENDPOINT_TX: u8 = 0x01;

/// USB alternate setting for the BladeRF interface.
///
/// Each setting reconfigures the bulk endpoints for a different
/// communication mode: general control, RF link streaming, SPI flash
/// access, or configuration.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UsbAltSetting {
    /// No active endpoints; default idle state.
    Null = 0,
    /// RF link mode; NIOS control + streaming endpoints active.
    RfLink = 1,
    /// SPI flash mode; flash access endpoints active.
    SpiFlash = 2,
    /// Configuration mode; board configuration endpoints active.
    Config = 3,
}

impl TryFrom<u8> for UsbAltSetting {
    type Error = u8;

    fn try_from(value: u8) -> std::result::Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Null),
            1 => Ok(Self::RfLink),
            2 => Ok(Self::SpiFlash),
            3 => Ok(Self::Config),
            _ => Err(value),
        }
    }
}

/// Vendor-specific USB control request identifiers.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VendorRequest {
    /// Queries the FPGA configuration status.
    QueryFpgaStatus = 1,
    /// Signals the device to begin FPGA programming.
    BeginProg = 2,
    /// Enables/disables the RX streaming module.
    RfRx = 4,
    /// Enables/disables the TX streaming module.
    RfTx = 5,
    /// Queries whether the firmware is ready.
    QueryDeviceReady = 6,
    /// Reads the SPI flash device ID.
    QueryFlashId = 7,
    /// Queries the FPGA bitstream source.
    QueryFpgaSource = 8,
    /// Reads from the SPI flash.
    FlashRead = 100,
    /// Writes to the SPI flash.
    FlashWrite = 101,
    /// Erases a region of the SPI flash.
    FlashErase = 102,
    /// Resets the FX3 USB controller.
    Reset = 105,
    /// Reads from the FX3 page buffer.
    ReadPageBuffer = 107,
    /// Writes to the FX3 page buffer.
    WritePageBuffer = 108,
    /// Reads a calibration cache entry.
    ReadCalCache = 110,
    /// Sets the firmware loopback mode.
    SetLoopback = 113,
    /// Gets the firmware loopback mode.
    GetLoopback = 114,
    /// Reads a log entry from the firmware.
    ReadLogEntry = 115,
}

const TIMEOUT: Duration = Duration::from_secs(3);

/// Standard USB string descriptor indices for the BladeRF.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StringDescriptors {
    /// Manufacturer string descriptor index.
    Manufacturer = 0x1,
    /// Product string descriptor index.
    Product,
    /// Serial number string descriptor index.
    Serial,
    /// FX3 firmware version string descriptor index.
    Fx3Firmware,
}

/// USB device string descriptor operations.
///
/// Implemented for `nusb::Device` to provide convenient access to
/// manufacturer, product, and serial number strings.
pub trait DeviceCommands {
    /// Returns the list of supported language IDs.
    fn get_supported_languages(&self) -> Result<Vec<u16>>;
    /// Reads a string descriptor by index using the default language (US English).
    fn get_string_descriptor_simple(&self, descriptor_index: NonZero<u8>) -> Result<String>;
    /// Returns the device serial number.
    fn serial(&self) -> Result<String>;
    /// Returns the device manufacturer string.
    fn manufacturer(&self) -> Result<String>;
    /// Returns the device product string.
    fn product(&self) -> Result<String>;
}
impl DeviceCommands for Device {
    fn get_supported_languages(&self) -> Result<Vec<u16>> {
        let languages = self
            .get_string_descriptor_supported_languages(TIMEOUT)
            .wait()?
            .collect();
        Ok(languages)
    }
    fn get_string_descriptor_simple(&self, descriptor_index: NonZero<u8>) -> Result<String> {
        let descriptor = self
            .get_string_descriptor(descriptor_index, 0x409, TIMEOUT)
            .wait()?;
        Ok(descriptor)
    }
    fn serial(&self) -> Result<String> {
        self.get_string_descriptor_simple(
            NonZero::new(StringDescriptors::Serial as u8)
                .expect("Serial descriptor index is non-zero"),
        )
    }
    fn manufacturer(&self) -> Result<String> {
        self.get_string_descriptor_simple(
            NonZero::new(StringDescriptors::Manufacturer as u8)
                .expect("Manufacturer descriptor index is non-zero"),
        )
    }
    fn product(&self) -> Result<String> {
        self.get_string_descriptor_simple(
            NonZero::new(StringDescriptors::Product as u8)
                .expect("Product descriptor index is non-zero"),
        )
    }
}

/// BladeRF1-specific USB device string descriptor operations.
///
/// Extends `DeviceCommands` with BladeRF1-specific descriptors.
pub trait BladeRf1DeviceCommands: DeviceCommands {
    /// Returns the FX3 firmware version string.
    fn fx3_firmware_version(&self) -> Result<String>;
}
impl BladeRf1DeviceCommands for Device {
    fn fx3_firmware_version(&self) -> Result<String> {
        self.get_string_descriptor_simple(
            NonZero::new(StringDescriptors::Fx3Firmware as u8)
                .expect("Fx3Firmware descriptor index is non-zero"),
        )
    }
}

/// USB interface-level commands for vendor requests and alt setting changes.
///
/// Implemented for `Interface`, `UsbTransport`, and `NiosCore`. Provides
/// a common interface for issuing vendor-specific control transfers and
/// managing alternate settings.
pub trait UsbInterfaceCommands {
    /// Issues a vendor IN command and returns the 32-bit integer response.
    fn usb_vendor_cmd_int(&self, cmd: VendorRequest) -> Result<u32>;
    /// Issues a vendor IN command with a `wValue` parameter and returns the 32-bit integer response.
    fn usb_vendor_cmd_int_w_value(&self, cmd: VendorRequest, w_value: u16) -> Result<u32>;
    /// Issues a vendor IN command with a `wIndex` parameter and returns the 32-bit integer response.
    fn usb_vendor_cmd_int_w_index(&self, cmd: VendorRequest, w_index: u16) -> Result<u32>;
    /// Issues a vendor OUT command with a `wIndex` parameter and data payload.
    fn usb_vendor_cmd_out_w_index(
        &self,
        cmd: VendorRequest,
        w_index: u16,
        data: &[u8],
    ) -> Result<()>;
    /// Issues a vendor IN command with a `wIndex` parameter and fills `buf` with the response data.
    fn usb_vendor_cmd_in_w_index_data(
        &self,
        cmd: VendorRequest,
        w_index: u16,
        buf: &mut [u8],
    ) -> Result<()>;
    /// Switches the USB interface to the specified alternate setting.
    fn usb_change_setting(&mut self, setting: UsbAltSetting) -> Result<()>;
}
impl UsbInterfaceCommands for Interface {
    fn usb_vendor_cmd_int(&self, cmd: VendorRequest) -> Result<u32> {
        let vec = vendor_cmd_in(self, cmd, 0, 0, 4)?;
        Ok(u32::from_le_bytes(vec[0..4].try_into().unwrap()))
    }
    fn usb_vendor_cmd_int_w_value(&self, cmd: VendorRequest, w_value: u16) -> Result<u32> {
        let vec = vendor_cmd_in(self, cmd, w_value, 0, 4)?;
        Ok(u32::from_le_bytes(vec[0..4].try_into().unwrap()))
    }
    fn usb_vendor_cmd_int_w_index(&self, cmd: VendorRequest, w_index: u16) -> Result<u32> {
        let vec = vendor_cmd_in(self, cmd, 0, w_index, 4)?;
        Ok(u32::from_le_bytes(vec[0..4].try_into().unwrap()))
    }
    fn usb_vendor_cmd_out_w_index(
        &self,
        cmd: VendorRequest,
        w_index: u16,
        data: &[u8],
    ) -> Result<()> {
        let pkt = ControlOut {
            control_type: ControlType::Vendor,
            recipient: Recipient::Device,
            request: cmd as u8,
            value: 0,
            index: w_index,
            data,
        };
        self.control_out(pkt, TIMEOUT).wait()?;
        Ok(())
    }
    fn usb_vendor_cmd_in_w_index_data(
        &self,
        cmd: VendorRequest,
        w_index: u16,
        buf: &mut [u8],
    ) -> Result<()> {
        let length = u16::try_from(buf.len())
            .map_err(|_| Error::Argument("buffer length exceeds u16 maximum".into()))?;
        let vec = vendor_cmd_in(self, cmd, 0, w_index, length)?;
        let copy_len = buf.len().min(vec.len());
        buf[..copy_len].copy_from_slice(&vec[..copy_len]);
        Ok(())
    }
    fn usb_change_setting(&mut self, setting: UsbAltSetting) -> Result<()> {
        Ok(self.set_alt_setting(setting as u8).wait()?)
    }
}

fn vendor_cmd_in(
    iface: &Interface,
    cmd: VendorRequest,
    value: u16,
    index: u16,
    length: u16,
) -> Result<Vec<u8>> {
    let pkt = ControlIn {
        control_type: ControlType::Vendor,
        recipient: Recipient::Device,
        request: cmd as u8,
        value,
        index,
        length,
    };
    let vec = iface.control_in(pkt, TIMEOUT).wait()?;
    if length as usize >= 4 && vec.len() < 4 {
        return Err(Error::UsbControlResponseTooShort {
            expected: 4,
            actual: vec.len(),
        });
    }
    Ok(vec)
}

/// BladeRF1-specific USB interface commands.
///
/// Extends `UsbInterfaceCommands` with operations for streaming
/// module control, firmware loopback, device reset, FPGA programming,
/// and bulk OUT transfers.
pub trait BladeRf1UsbInterfaceCommands: UsbInterfaceCommands {
    /// Enables or disables the USB streaming module for the given channel.
    fn usb_enable_module(&self, channel: Channel, enable: bool) -> Result<()>;
    /// Sets the firmware loopback mode, cycling the alt setting to Null then RfLink.
    fn usb_set_firmware_loopback(&mut self, enable: bool) -> Result<()>;
    /// Queries whether firmware loopback is currently enabled.
    fn usb_get_firmware_loopback(&self) -> Result<bool>;
    /// Resets the FX3 USB controller via a vendor control request.
    fn usb_device_reset(&self) -> Result<()>;
    /// Returns `true` if the firmware has reported readiness.
    fn usb_is_firmware_ready(&self) -> Result<bool>;
    /// Returns `true` if the FPGA has finished configuration.
    fn usb_is_fpga_configured(&self) -> Result<bool>;
    /// Signals the firmware to begin FPGA programming.
    fn usb_begin_fpga_prog(&self) -> Result<()>;
    /// Performs a bulk OUT transfer to the given endpoint address.
    fn usb_bulk_out(&self, endpoint: u8, data: &[u8], timeout: Duration) -> Result<()>;
}
impl BladeRf1UsbInterfaceCommands for Interface {
    fn usb_enable_module(&self, channel: Channel, enable: bool) -> Result<()> {
        let val = enable as u16;
        let cmd = if channel.is_rx() {
            VendorRequest::RfRx
        } else {
            VendorRequest::RfTx
        };
        let fx3_ret = self.usb_vendor_cmd_int_w_value(cmd, val)?;
        if fx3_ret != 0 {
            log::warn!("usb_enable_module({channel:?}, {enable}): firmware returned {fx3_ret:#x}");
        }
        Ok(())
    }
    fn usb_set_firmware_loopback(&mut self, enable: bool) -> Result<()> {
        let fx3_ret = self.usb_vendor_cmd_int_w_value(VendorRequest::SetLoopback, enable as u16)?;
        if fx3_ret != 0 {
            log::warn!("usb_set_firmware_loopback({enable}): firmware returned {fx3_ret:#x}");
        }
        self.usb_change_setting(UsbAltSetting::Null)?;
        self.usb_change_setting(UsbAltSetting::RfLink)?;
        Ok(())
    }
    fn usb_get_firmware_loopback(&self) -> Result<bool> {
        let result = self.usb_vendor_cmd_int(VendorRequest::GetLoopback)?;
        Ok(result != 0)
    }
    fn usb_device_reset(&self) -> Result<()> {
        let pkt = ControlOut {
            control_type: ControlType::Vendor,
            recipient: Recipient::Device,
            request: VendorRequest::Reset as u8,
            value: 0x0,
            index: 0x0,
            data: &[],
        };
        self.control_out(pkt, TIMEOUT).wait()?;
        Ok(())
    }
    fn usb_is_firmware_ready(&self) -> Result<bool> {
        Ok(self.usb_vendor_cmd_int(VendorRequest::QueryDeviceReady)? != 0)
    }
    fn usb_is_fpga_configured(&self) -> Result<bool> {
        let result = self.usb_vendor_cmd_int(VendorRequest::QueryFpgaStatus)?;
        match result {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(Error::BoardState("unexpected FPGA status response")),
        }
    }
    fn usb_begin_fpga_prog(&self) -> Result<()> {
        let result = self.usb_vendor_cmd_int(VendorRequest::BeginProg)?;
        if result != 0 {
            Err(Error::BoardState("BEGIN_PROG returned non-zero status"))
        } else {
            Ok(())
        }
    }
    fn usb_bulk_out(&self, endpoint: u8, data: &[u8], timeout: Duration) -> Result<()> {
        let mut ep = self
            .endpoint::<Bulk, Out>(endpoint)
            .map_err(Error::EndpointBusy)?;
        let mut buf = ep.allocate(data.len());
        buf.extend_from_slice(data);
        ep.submit(buf);
        let completion = ep.wait_next_complete(timeout).ok_or(Error::Timeout)?;
        completion.status?;
        Ok(())
    }
}

impl UsbInterfaceCommands for UsbTransport {
    /// Delegates to the underlying interface.
    fn usb_vendor_cmd_int(&self, cmd: VendorRequest) -> Result<u32> {
        self.interface.usb_vendor_cmd_int(cmd)
    }
    /// Delegates to the underlying interface.
    fn usb_vendor_cmd_int_w_value(&self, cmd: VendorRequest, wvalue: u16) -> Result<u32> {
        self.interface.usb_vendor_cmd_int_w_value(cmd, wvalue)
    }
    /// Delegates to the underlying interface.
    fn usb_vendor_cmd_int_w_index(&self, cmd: VendorRequest, windex: u16) -> Result<u32> {
        self.interface.usb_vendor_cmd_int_w_index(cmd, windex)
    }
    /// Delegates to the underlying interface.
    fn usb_vendor_cmd_out_w_index(
        &self,
        cmd: VendorRequest,
        windex: u16,
        data: &[u8],
    ) -> Result<()> {
        self.interface.usb_vendor_cmd_out_w_index(cmd, windex, data)
    }
    /// Delegates to the underlying interface.
    fn usb_vendor_cmd_in_w_index_data(
        &self,
        cmd: VendorRequest,
        windex: u16,
        buf: &mut [u8],
    ) -> Result<()> {
        self.interface
            .usb_vendor_cmd_in_w_index_data(cmd, windex, buf)
    }
    /// Releases NIOS endpoints, switches the alt setting, and updates the cached setting.
    fn usb_change_setting(&mut self, setting: UsbAltSetting) -> Result<()> {
        self.release_endpoints();
        self.interface.set_alt_setting(setting as u8).wait()?;
        self.current_alt_setting = setting;
        Ok(())
    }
}

impl BladeRf1UsbInterfaceCommands for UsbTransport {
    /// Delegates to the underlying interface.
    fn usb_enable_module(&self, channel: Channel, enable: bool) -> Result<()> {
        self.interface.usb_enable_module(channel, enable)
    }
    /// Sets firmware loopback, using `self.usb_change_setting()` to
    /// properly release NIOS packet URBs before the alt-setting change.
    fn usb_set_firmware_loopback(&mut self, enable: bool) -> Result<()> {
        let fx3_ret = self
            .interface
            .usb_vendor_cmd_int_w_value(VendorRequest::SetLoopback, enable as u16)?;
        if fx3_ret != 0 {
            log::warn!("usb_set_firmware_loopback({enable}): firmware returned {fx3_ret:#x}");
        }
        self.usb_change_setting(UsbAltSetting::Null)?;
        self.usb_change_setting(UsbAltSetting::RfLink)?;
        Ok(())
    }
    /// Delegates to the underlying interface.
    fn usb_get_firmware_loopback(&self) -> Result<bool> {
        self.interface.usb_get_firmware_loopback()
    }
    /// Delegates to the underlying interface.
    fn usb_device_reset(&self) -> Result<()> {
        self.interface.usb_device_reset()
    }
    /// Delegates to the underlying interface.
    fn usb_is_firmware_ready(&self) -> Result<bool> {
        self.interface.usb_is_firmware_ready()
    }
    /// Delegates to the underlying interface.
    fn usb_is_fpga_configured(&self) -> Result<bool> {
        self.interface.usb_is_fpga_configured()
    }
    /// Delegates to the underlying interface.
    fn usb_begin_fpga_prog(&self) -> Result<()> {
        self.interface.usb_begin_fpga_prog()
    }
    /// Delegates to the underlying interface.
    fn usb_bulk_out(&self, endpoint: u8, data: &[u8], timeout: Duration) -> Result<()> {
        self.interface.usb_bulk_out(endpoint, data, timeout)
    }
}

struct NiosEndpoints {
    ep_out: Endpoint<Bulk, Out>,
    ep_in: Endpoint<Bulk, In>,
    buf_out: Option<Buffer>,
    buf_in: Option<Buffer>,
}

/// Concrete USB transport wrapping an `nusb` interface.
///
/// Manages NIOS packet endpoints, alternate setting tracking, and
/// provides access to streaming endpoints. Implements the USB command
/// traits for delegation to the underlying interface.
pub struct UsbTransport {
    interface: Interface,
    nios_endpoints: Option<NiosEndpoints>,
    current_alt_setting: UsbAltSetting,
    speed: Speed,
}
impl UsbTransport {
    const NIOS_PKT_SIZE: usize = 16;
    /// Creates a new `UsbTransport` from an nusb `Interface`.
    pub fn new(interface: Interface, speed: Speed) -> Self {
        let current_alt_setting =
            UsbAltSetting::try_from(interface.get_alt_setting()).unwrap_or(UsbAltSetting::Null);
        Self {
            interface,
            nios_endpoints: None,
            current_alt_setting,
            speed,
        }
    }
    /// Returns a shared reference to the underlying nusb `Interface`.
    pub fn interface(&self) -> &Interface {
        &self.interface
    }
    /// Returns the cached current USB alternate setting.
    pub fn current_alt_setting(&self) -> UsbAltSetting {
        self.current_alt_setting
    }
    /// Returns the USB bus speed (full/high/superspeed).
    pub fn speed(&self) -> Speed {
        self.speed
    }
    /// Cancels pending NIOS transfers and releases the cached endpoints.
    ///
    /// Called before switching USB alternate settings to ensure clean
    /// endpoint teardown. Waits up to 5 seconds for in-flight transfers.
    pub fn release_endpoints(&mut self) {
        if let Some(mut endpoints) = self.nios_endpoints.take() {
            endpoints.ep_out.cancel_all();
            endpoints.ep_in.cancel_all();
            let deadline = std::time::Instant::now() + Duration::from_secs(5);
            while endpoints.ep_out.pending() > 0 || endpoints.ep_in.pending() > 0 {
                let remaining = deadline.saturating_duration_since(std::time::Instant::now());
                let timeout = remaining.min(Duration::from_secs(1));
                if timeout.is_zero() {
                    log::warn!(
                        "Timeout collecting cancelled NiosPkt transfers, OUT={} IN={} remain",
                        endpoints.ep_out.pending(),
                        endpoints.ep_in.pending()
                    );
                    break;
                }
                if endpoints.ep_out.pending() > 0
                    && let Some(completion) = endpoints.ep_out.wait_next_complete(timeout)
                {
                    match completion.status {
                        Ok(()) | Err(nusb::transfer::TransferError::Cancelled) => {}
                        Err(e) => log::warn!("NiosPkt OUT transfer error during release: {e}"),
                    }
                }
                if endpoints.ep_in.pending() > 0
                    && let Some(completion) = endpoints.ep_in.wait_next_complete(timeout)
                {
                    match completion.status {
                        Ok(()) | Err(nusb::transfer::TransferError::Cancelled) => {}
                        Err(e) => log::warn!("NiosPkt IN transfer error during release: {e}"),
                    }
                }
            }
        }
    }
    fn ensure_nios_endpoints(&mut self) -> Result<&mut NiosEndpoints> {
        if self.nios_endpoints.is_none() {
            let ep_out = self
                .interface
                .endpoint::<Bulk, Out>(CONTROL_ENDPOINT_OUT)
                .map_err(Error::EndpointBusy)?;
            let ep_in = self
                .interface
                .endpoint::<Bulk, In>(CONTROL_ENDPOINT_IN)
                .map_err(Error::EndpointBusy)?;
            let buf_out = Some(ep_out.allocate(Self::NIOS_PKT_SIZE));
            let buf_in = Some(ep_in.allocate(ep_in.max_packet_size()));
            self.nios_endpoints = Some(NiosEndpoints {
                ep_out,
                ep_in,
                buf_out,
                buf_in,
            });
        }
        self.nios_endpoints
            .as_mut()
            .ok_or(Error::EndpointNotAvailable)
    }
    /// Returns a mutable 16-byte buffer for constructing a NIOS packet.
    ///
    /// Lazily initializes the NIOS bulk endpoints and allocates the
    /// output buffer on first call.
    pub fn out_buffer(&mut self) -> Result<&mut [u8]> {
        let endpoints = self.ensure_nios_endpoints()?;
        let buf = endpoints
            .buf_out
            .as_mut()
            .ok_or(Error::EndpointNotAvailable)?;
        buf.clear();
        buf.extend_fill(Self::NIOS_PKT_SIZE, 0);
        Ok(buf)
    }
    /// Submits a NIOS packet and returns the response data.
    ///
    /// Performs a paired bulk OUT/IN transfer: submits the pre-filled
    /// output buffer and waits for the corresponding IN response.
    /// Returns a slice of exactly 16 bytes on success.
    pub fn submit(&mut self, timeout: Option<Duration>) -> Result<&[u8]> {
        let endpoints = self.ensure_nios_endpoints()?;
        let t = timeout.unwrap_or(TIMEOUT);
        let buf_out = endpoints
            .buf_out
            .take()
            .ok_or(Error::EndpointNotAvailable)?;
        log::trace!("submit: OUT buffer len = {}", buf_out.len());
        endpoints.ep_out.submit(buf_out);
        let mut response = endpoints
            .ep_out
            .wait_next_complete(t)
            .ok_or(Error::Timeout)?;
        response.status?;
        endpoints.buf_out = Some(response.buffer);
        let mut buf_in = endpoints.buf_in.take().ok_or(Error::EndpointNotAvailable)?;
        buf_in.set_requested_len(endpoints.ep_in.max_packet_size());
        endpoints.ep_in.submit(buf_in);
        response = endpoints
            .ep_in
            .wait_next_complete(t)
            .ok_or(Error::Timeout)?;
        response.status?;
        endpoints.buf_in = Some(response.buffer);
        let in_buf = endpoints
            .buf_in
            .as_ref()
            .ok_or(Error::EndpointNotAvailable)?;
        let in_len = in_buf.len();
        if in_len < Self::NIOS_PKT_SIZE {
            return Err(NiosPacketError::InvalidSize(in_len).into());
        }
        Ok(&in_buf[..Self::NIOS_PKT_SIZE])
    }
    /// Acquires the RX streaming bulk IN endpoint.
    ///
    /// Returns an error if the endpoint is already claimed by another
    /// consumer.
    pub fn acquire_streaming_rx_endpoint(&self) -> Result<Endpoint<Bulk, In>> {
        self.interface
            .endpoint::<Bulk, In>(STREAM_ENDPOINT_RX)
            .map_err(Error::EndpointBusy)
    }
    /// Acquires the TX streaming bulk OUT endpoint.
    ///
    /// Returns an error if the endpoint is already claimed by another
    /// consumer.
    pub fn acquire_streaming_tx_endpoint(&self) -> Result<Endpoint<Bulk, Out>> {
        self.interface
            .endpoint::<Bulk, Out>(STREAM_ENDPOINT_TX)
            .map_err(Error::EndpointBusy)
    }
}
