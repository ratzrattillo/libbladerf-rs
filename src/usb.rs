//! USB transport layer and vendor command interface.
//!
//! Wraps the `nusb` crate for BladeRF device communication. Manages
//! USB alternate settings, NIOS packet bulk transfers, and vendor-specific
//! control requests. Provides traits (`UsbInterfaceCommands`,
//! `BladeRf1UsbInterfaceCommands`, `DeviceCommands`, `BladeRf1DeviceCommands`)
//! that abstract over the USB interface for use by higher layers.
//!
//! All I/O methods return [`MaybeFuture`]: call `.wait()` to block (native
//! only) or `.await` from async code.

use crate::channel::Channel;
use crate::error::{Error, Result};
use crate::maybe_future::{NonWasmSend, Op};
use crate::protocol::nios::NiosPacketError;
use nusb::transfer::{
    Buffer, Bulk, Completion, ControlIn, ControlOut, ControlType, EndpointDirection, In, Out,
    Recipient, TransferError,
};
use nusb::{Device, Endpoint, Interface, MaybeFuture, Speed};
use std::future::Future;
use std::num::NonZero;
use std::task::{Context, Poll};
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
const RELEASE_TIMEOUT: Duration = Duration::from_secs(5);

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
    fn get_supported_languages(&self) -> impl MaybeFuture<Output = Result<Vec<u16>>>;
    /// Reads a string descriptor by index using the default language (US English).
    fn get_string_descriptor_simple(
        &self,
        descriptor_index: NonZero<u8>,
    ) -> impl MaybeFuture<Output = Result<String>>;
    /// Returns the device serial number.
    fn serial(&self) -> impl MaybeFuture<Output = Result<String>>;
    /// Returns the device manufacturer string.
    fn manufacturer(&self) -> impl MaybeFuture<Output = Result<String>>;
    /// Returns the device product string.
    fn product(&self) -> impl MaybeFuture<Output = Result<String>>;
}
impl DeviceCommands for Device {
    fn get_supported_languages(&self) -> impl MaybeFuture<Output = Result<Vec<u16>>> {
        self.get_string_descriptor_supported_languages(TIMEOUT)
            .map_ok(|languages| languages.collect())
            .map_err(Error::from)
    }
    fn get_string_descriptor_simple(
        &self,
        descriptor_index: NonZero<u8>,
    ) -> impl MaybeFuture<Output = Result<String>> {
        self.get_string_descriptor(descriptor_index, 0x409, TIMEOUT)
            .map_err(Error::from)
    }
    fn serial(&self) -> impl MaybeFuture<Output = Result<String>> {
        self.get_string_descriptor_simple(
            NonZero::new(StringDescriptors::Serial as u8)
                .expect("Serial descriptor index is non-zero"),
        )
    }
    fn manufacturer(&self) -> impl MaybeFuture<Output = Result<String>> {
        self.get_string_descriptor_simple(
            NonZero::new(StringDescriptors::Manufacturer as u8)
                .expect("Manufacturer descriptor index is non-zero"),
        )
    }
    fn product(&self) -> impl MaybeFuture<Output = Result<String>> {
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
    fn fx3_firmware_version(&self) -> impl MaybeFuture<Output = Result<String>>;
}
impl BladeRf1DeviceCommands for Device {
    fn fx3_firmware_version(&self) -> impl MaybeFuture<Output = Result<String>> {
        self.get_string_descriptor_simple(
            NonZero::new(StringDescriptors::Fx3Firmware as u8)
                .expect("Fx3Firmware descriptor index is non-zero"),
        )
    }
}

/// USB interface-level vendor control requests.
///
/// Implemented for `nusb::Interface`. `UsbTransport` and `NiosCore` expose
/// their interface through `interface()`; alternate-setting changes live on
/// `UsbTransport`, which must release its NIOS endpoints first.
pub trait UsbInterfaceCommands {
    /// Issues a vendor IN command and returns the 32-bit integer response.
    fn usb_vendor_cmd_int(&self, cmd: VendorRequest) -> impl MaybeFuture<Output = Result<u32>>;
    /// Issues a vendor IN command with a `wValue` parameter and returns the 32-bit integer response.
    fn usb_vendor_cmd_int_w_value(
        &self,
        cmd: VendorRequest,
        w_value: u16,
    ) -> impl MaybeFuture<Output = Result<u32>>;
    /// Issues a vendor IN command with a `wIndex` parameter and returns the 32-bit integer response.
    fn usb_vendor_cmd_int_w_index(
        &self,
        cmd: VendorRequest,
        w_index: u16,
    ) -> impl MaybeFuture<Output = Result<u32>>;
    /// Issues a vendor OUT command with a `wIndex` parameter and data payload.
    fn usb_vendor_cmd_out_w_index(
        &self,
        cmd: VendorRequest,
        w_index: u16,
        data: &[u8],
    ) -> impl MaybeFuture<Output = Result<()>>;
    /// Issues a vendor IN command with a `wIndex` parameter and fills `buf` with the response data.
    fn usb_vendor_cmd_in_w_index_data(
        &self,
        cmd: VendorRequest,
        w_index: u16,
        buf: &mut [u8],
    ) -> impl MaybeFuture<Output = Result<()>>;
}
impl UsbInterfaceCommands for Interface {
    fn usb_vendor_cmd_int(&self, cmd: VendorRequest) -> impl MaybeFuture<Output = Result<u32>> {
        vendor_cmd_in_u32(self, cmd, 0, 0)
    }
    fn usb_vendor_cmd_int_w_value(
        &self,
        cmd: VendorRequest,
        w_value: u16,
    ) -> impl MaybeFuture<Output = Result<u32>> {
        vendor_cmd_in_u32(self, cmd, w_value, 0)
    }
    fn usb_vendor_cmd_int_w_index(
        &self,
        cmd: VendorRequest,
        w_index: u16,
    ) -> impl MaybeFuture<Output = Result<u32>> {
        vendor_cmd_in_u32(self, cmd, 0, w_index)
    }
    fn usb_vendor_cmd_out_w_index(
        &self,
        cmd: VendorRequest,
        w_index: u16,
        data: &[u8],
    ) -> impl MaybeFuture<Output = Result<()>> {
        let pkt = ControlOut {
            control_type: ControlType::Vendor,
            recipient: Recipient::Device,
            request: cmd as u8,
            value: 0,
            index: w_index,
            data,
        };
        self.control_out(pkt, TIMEOUT).map_err(Error::from)
    }
    fn usb_vendor_cmd_in_w_index_data(
        &self,
        cmd: VendorRequest,
        w_index: u16,
        buf: &mut [u8],
    ) -> impl MaybeFuture<Output = Result<()>> {
        Op::new(async move {
            let length = u16::try_from(buf.len())
                .map_err(|_| Error::Argument("buffer length exceeds u16 maximum".into()))?;
            let vec = vendor_cmd_in(self, cmd, 0, w_index, length).await?;
            buf.copy_from_slice(&vec);
            Ok(())
        })
    }
}

fn vendor_cmd_in(
    iface: &Interface,
    cmd: VendorRequest,
    value: u16,
    index: u16,
    length: u16,
) -> impl MaybeFuture<Output = Result<Vec<u8>>> {
    let pkt = ControlIn {
        control_type: ControlType::Vendor,
        recipient: Recipient::Device,
        request: cmd as u8,
        value,
        index,
        length,
    };
    iface.control_in(pkt, TIMEOUT).map(move |response| {
        let vec = response?;
        require_length(length as usize, vec.len())?;
        Ok(vec)
    })
}

pub(crate) fn require_length(expected: usize, actual: usize) -> Result<()> {
    if expected == actual {
        Ok(())
    } else {
        Err(Error::UsbTransferLength { expected, actual })
    }
}

pub(crate) fn firmware_status(request: VendorRequest, status: u32) -> Result<()> {
    firmware_ack(request, 0, status)
}

fn firmware_ack(request: VendorRequest, expected: u32, status: u32) -> Result<()> {
    if status == expected {
        Ok(())
    } else {
        Err(Error::FirmwareStatus {
            request: request as u8,
            status,
        })
    }
}

fn vendor_cmd_in_u32(
    iface: &Interface,
    cmd: VendorRequest,
    value: u16,
    index: u16,
) -> impl MaybeFuture<Output = Result<u32>> {
    vendor_cmd_in(iface, cmd, value, index, 4)
        .map_ok(|vec| u32::from_le_bytes(vec[0..4].try_into().unwrap()))
}

/// BladeRF1-specific USB interface commands.
///
/// Extends `UsbInterfaceCommands` with operations for streaming
/// module control, firmware loopback, device reset, FPGA programming,
/// and bulk OUT transfers.
pub trait BladeRf1UsbInterfaceCommands: UsbInterfaceCommands {
    /// Enables or disables the USB streaming module for the given channel.
    fn usb_enable_module(
        &self,
        channel: Channel,
        enable: bool,
    ) -> impl MaybeFuture<Output = Result<()>>;
    /// Queries whether firmware loopback is currently enabled.
    fn usb_get_firmware_loopback(&self) -> impl MaybeFuture<Output = Result<bool>>;
    /// Resets the FX3 USB controller via a vendor control request.
    fn usb_device_reset(&self) -> impl MaybeFuture<Output = Result<()>>;
    /// Returns `true` if the firmware has reported readiness.
    fn usb_is_firmware_ready(&self) -> impl MaybeFuture<Output = Result<bool>>;
    /// Returns `true` if the FPGA has finished configuration.
    fn usb_is_fpga_configured(&self) -> impl MaybeFuture<Output = Result<bool>>;
    /// Signals the firmware to begin FPGA programming.
    fn usb_begin_fpga_prog(&self) -> impl MaybeFuture<Output = Result<()>>;
    /// Performs a bulk OUT transfer to the given endpoint address.
    ///
    /// `timeout` applies on native targets only; on wasm the transfer is
    /// awaited without a deadline because WebUSB cannot cancel transfers.
    fn usb_bulk_out(
        &self,
        endpoint: u8,
        data: &[u8],
        timeout: Duration,
    ) -> impl MaybeFuture<Output = Result<()>>;
}
impl BladeRf1UsbInterfaceCommands for Interface {
    fn usb_enable_module(
        &self,
        channel: Channel,
        enable: bool,
    ) -> impl MaybeFuture<Output = Result<()>> {
        let cmd = if channel.is_rx() {
            VendorRequest::RfRx
        } else {
            VendorRequest::RfTx
        };
        self.usb_vendor_cmd_int_w_value(cmd, enable as u16)
            .map(move |result| firmware_status(cmd, result?))
    }
    fn usb_get_firmware_loopback(&self) -> impl MaybeFuture<Output = Result<bool>> {
        self.usb_vendor_cmd_int(VendorRequest::GetLoopback)
            .map_ok(|result| result != 0)
    }
    fn usb_device_reset(&self) -> impl MaybeFuture<Output = Result<()>> {
        let pkt = ControlOut {
            control_type: ControlType::Vendor,
            recipient: Recipient::Device,
            request: VendorRequest::Reset as u8,
            value: 0x0,
            index: 0x0,
            data: &[],
        };
        self.control_out(pkt, TIMEOUT).map_err(Error::from)
    }
    fn usb_is_firmware_ready(&self) -> impl MaybeFuture<Output = Result<bool>> {
        self.usb_vendor_cmd_int(VendorRequest::QueryDeviceReady)
            .map_ok(|result| result != 0)
    }
    fn usb_is_fpga_configured(&self) -> impl MaybeFuture<Output = Result<bool>> {
        self.usb_vendor_cmd_int(VendorRequest::QueryFpgaStatus)
            .map(|result| match result? {
                0 => Ok(false),
                1 => Ok(true),
                _ => Err(Error::BoardState("unexpected FPGA status response")),
            })
    }
    fn usb_begin_fpga_prog(&self) -> impl MaybeFuture<Output = Result<()>> {
        self.usb_vendor_cmd_int(VendorRequest::BeginProg)
            .map(|result| firmware_status(VendorRequest::BeginProg, result?))
    }
    fn usb_bulk_out(
        &self,
        endpoint: u8,
        data: &[u8],
        timeout: Duration,
    ) -> impl MaybeFuture<Output = Result<()>> {
        Op::new(async move {
            let mut ep = self
                .endpoint::<Bulk, Out>(endpoint)
                .map_err(Error::EndpointBusy)?;
            let mut buf = ep.allocate(data.len());
            buf.extend_from_slice(data);
            ep.submit(buf);
            let completion = next_complete(&mut ep, timeout).await?;
            completion.status?;
            require_length(data.len(), completion.actual_len)
        })
    }
}

/// A bulk endpoint as used by the NIOS transport and the streaming pools.
///
/// Implemented for [`nusb::Endpoint`]; the streaming state machine is
/// generic over it so its lifecycle can be exercised without hardware.
pub(crate) trait BulkEndpoint: NonWasmSend {
    fn address(&self) -> u8;
    fn max_packet_size(&self) -> usize;
    fn allocate(&self, len: usize) -> Buffer;
    fn submit(&mut self, buffer: Buffer);
    fn pending(&self) -> usize;
    fn poll_next_complete(&mut self, cx: &mut Context<'_>) -> Poll<Completion>;
    #[cfg(not(target_arch = "wasm32"))]
    fn wait_next_complete(&mut self, timeout: Duration) -> Option<Completion>;
    fn can_cancel(&self) -> bool;
    fn cancel_all(&mut self);
    fn clear_halt(&mut self) -> impl MaybeFuture<Output = std::result::Result<(), nusb::Error>>;

    fn next_complete(&mut self) -> impl Future<Output = Completion> + NonWasmSend + '_ {
        std::future::poll_fn(|cx| self.poll_next_complete(cx))
    }
}

impl<Dir: EndpointDirection> BulkEndpoint for Endpoint<Bulk, Dir> {
    fn address(&self) -> u8 {
        self.endpoint_address()
    }
    fn max_packet_size(&self) -> usize {
        Endpoint::max_packet_size(self)
    }
    fn allocate(&self, len: usize) -> Buffer {
        Endpoint::allocate(self, len)
    }
    fn submit(&mut self, buffer: Buffer) {
        Endpoint::submit(self, buffer)
    }
    fn pending(&self) -> usize {
        Endpoint::pending(self)
    }
    fn poll_next_complete(&mut self, cx: &mut Context<'_>) -> Poll<Completion> {
        Endpoint::poll_next_complete(self, cx)
    }
    #[cfg(not(target_arch = "wasm32"))]
    fn wait_next_complete(&mut self, timeout: Duration) -> Option<Completion> {
        Endpoint::wait_next_complete(self, timeout)
    }
    fn can_cancel(&self) -> bool {
        cfg!(not(target_arch = "wasm32"))
    }
    fn cancel_all(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        Endpoint::cancel_all(self);
    }
    fn clear_halt(&mut self) -> impl MaybeFuture<Output = std::result::Result<(), nusb::Error>> {
        Endpoint::clear_halt(self)
    }
}

/// Awaits the next completion on `ep`, bounded by `timeout`.
///
/// On timeout the pending transfers are cancelled and collected where the
/// endpoint supports cancellation so it is left idle; WebUSB transfers
/// cannot be cancelled, so there the in-flight transfer is abandoned and
/// `Error::Timeout` is returned.
pub(crate) async fn next_complete<E: BulkEndpoint>(
    ep: &mut E,
    timeout: Duration,
) -> Result<Completion> {
    let Some(completion) = crate::maybe_future::timeout(timeout, ep.next_complete()).await else {
        if ep.can_cancel() {
            ep.cancel_all();
            drop(drain_pending(ep, RELEASE_TIMEOUT).await);
        }
        return Err(Error::Timeout);
    };
    Ok(completion)
}

/// Collects all pending completions on `ep` and returns their buffers.
///
/// Each completion is bounded by `deadline`; on expiry the remaining
/// transfers are cancelled where cancellation is available (left pending
/// on WebUSB) and a warning is logged.
pub(crate) async fn drain_pending<E: BulkEndpoint>(ep: &mut E, deadline: Duration) -> Vec<Buffer> {
    let mut buffers = Vec::with_capacity(ep.pending());
    while ep.pending() > 0 {
        let Some(completion) = crate::maybe_future::timeout(deadline, ep.next_complete()).await
        else {
            log::warn!(
                "timeout draining endpoint {:#04x}, {} transfers remain",
                ep.address(),
                ep.pending()
            );
            break;
        };
        match completion.status {
            Ok(()) | Err(TransferError::Cancelled) => {}
            Err(e) => log::warn!(
                "transfer error draining endpoint {:#04x}: {e}",
                ep.address()
            ),
        }
        buffers.push(completion.buffer);
    }
    buffers
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
    /// Releases NIOS endpoints, switches the alt setting, and updates the cached setting.
    pub fn usb_change_setting(
        &mut self,
        setting: UsbAltSetting,
    ) -> impl MaybeFuture<Output = Result<()>> {
        Op::new(async move {
            self.release_endpoints().await;
            self.interface.set_alt_setting(setting as u8).await?;
            self.current_alt_setting = setting;
            Ok(())
        })
    }
    /// Sets the firmware loopback mode, cycling the alt setting to Null then
    /// RfLink so that NIOS packet URBs are released before the change.
    pub fn usb_set_firmware_loopback(
        &mut self,
        enable: bool,
    ) -> impl MaybeFuture<Output = Result<()>> {
        Op::new(async move {
            let fx3_ret = self
                .interface
                .usb_vendor_cmd_int_w_value(VendorRequest::SetLoopback, enable as u16)
                .await?;
            firmware_ack(VendorRequest::SetLoopback, u32::from(enable), fx3_ret)?;
            self.usb_change_setting(UsbAltSetting::Null).await?;
            self.usb_change_setting(UsbAltSetting::RfLink).await?;
            Ok(())
        })
    }
    /// Cancels pending NIOS transfers and releases the cached endpoints.
    ///
    /// Called before switching USB alternate settings to ensure clean
    /// endpoint teardown. Waits up to 5 seconds for in-flight transfers.
    pub fn release_endpoints(&mut self) -> impl MaybeFuture<Output = ()> {
        Op::new(async move {
            if let Some(mut endpoints) = self.nios_endpoints.take() {
                endpoints.ep_out.cancel_all();
                endpoints.ep_in.cancel_all();
                drop(drain_pending(&mut endpoints.ep_out, RELEASE_TIMEOUT).await);
                drop(drain_pending(&mut endpoints.ep_in, RELEASE_TIMEOUT).await);
            }
        })
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
    ///
    /// On native targets the transaction is bounded by `timeout` (default
    /// 3 s); on expiry the NIOS endpoints are cancelled and released so the
    /// next call starts from a clean state. On wasm the transaction is
    /// awaited without a deadline.
    pub fn submit(
        &mut self,
        timeout: Option<Duration>,
    ) -> impl MaybeFuture<Output = Result<&[u8]>> {
        Op::new(async move {
            let t = timeout.unwrap_or(TIMEOUT);
            let endpoints = self.ensure_nios_endpoints()?;
            if let Err(e) = Self::transact(endpoints, t).await {
                if matches!(e, Error::Timeout) {
                    self.release_endpoints().await;
                }
                return Err(e);
            }
            let in_buf = self
                .nios_endpoints
                .as_ref()
                .and_then(|e| e.buf_in.as_ref())
                .ok_or(Error::EndpointNotAvailable)?;
            let in_len = in_buf.len();
            if in_len != Self::NIOS_PKT_SIZE {
                return Err(NiosPacketError::InvalidSize(in_len).into());
            }
            Ok(&in_buf[..Self::NIOS_PKT_SIZE])
        })
    }
    async fn transact(endpoints: &mut NiosEndpoints, timeout: Duration) -> Result<()> {
        let buf_out = endpoints
            .buf_out
            .take()
            .ok_or(Error::EndpointNotAvailable)?;
        log::trace!("submit: OUT buffer len = {}", buf_out.len());
        endpoints.ep_out.submit(buf_out);
        let response = next_complete(&mut endpoints.ep_out, timeout).await?;
        let actual_len = response.actual_len;
        endpoints.buf_out = Some(response.buffer);
        response.status?;
        require_length(Self::NIOS_PKT_SIZE, actual_len)?;
        let mut buf_in = endpoints.buf_in.take().ok_or(Error::EndpointNotAvailable)?;
        buf_in.set_requested_len(endpoints.ep_in.max_packet_size());
        endpoints.ep_in.submit(buf_in);
        let response = next_complete(&mut endpoints.ep_in, timeout).await?;
        endpoints.buf_in = Some(response.buffer);
        response.status?;
        Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_transfers_require_exact_lengths() {
        for expected in [4, 16, 64, 256] {
            assert!(require_length(expected, expected).is_ok());
            for actual in [0, expected - 1, expected + 1] {
                assert!(matches!(
                    require_length(expected, actual),
                    Err(Error::UsbTransferLength { .. })
                ));
            }
        }
    }

    #[test]
    fn firmware_status_and_echo_have_distinct_success_values() {
        assert!(firmware_status(VendorRequest::RfRx, 0).is_ok());
        assert!(matches!(
            firmware_status(VendorRequest::RfTx, 1),
            Err(Error::FirmwareStatus {
                request: 5,
                status: 1
            })
        ));
        assert!(firmware_ack(VendorRequest::SetLoopback, 1, 1).is_ok());
        assert!(firmware_ack(VendorRequest::SetLoopback, 1, 0).is_err());
    }
}
