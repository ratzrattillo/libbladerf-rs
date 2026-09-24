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
use nusb::transfer::{
    Buffer, Bulk, Completion, ControlIn, ControlOut, ControlType, EndpointDirection, In, Out,
    Recipient,
};
use nusb::{Device, Endpoint, Interface, MaybeFuture, Speed};
use std::future::Future;
use std::num::NonZero;
use std::task::{Context, Poll};
use std::time::Duration;

mod flash;
mod transaction;
use transaction::NiosExchange;
pub(crate) mod pending;
use pending::Pending;

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
/// Serialized through `UsbTransport`, which retains interrupted operations.
pub trait UsbInterfaceCommands {
    /// Issues a vendor IN command and returns the 32-bit integer response.
    fn usb_vendor_cmd_int(&mut self, cmd: VendorRequest) -> impl MaybeFuture<Output = Result<u32>>;
    /// Issues a vendor IN command with a `wValue` parameter and returns the 32-bit integer response.
    fn usb_vendor_cmd_int_w_value(
        &mut self,
        cmd: VendorRequest,
        w_value: u16,
    ) -> impl MaybeFuture<Output = Result<u32>>;
    /// Issues a vendor IN command with a `wIndex` parameter and returns the 32-bit integer response.
    fn usb_vendor_cmd_int_w_index(
        &mut self,
        cmd: VendorRequest,
        w_index: u16,
    ) -> impl MaybeFuture<Output = Result<u32>>;
}
impl UsbInterfaceCommands for UsbTransport {
    fn usb_vendor_cmd_int(&mut self, cmd: VendorRequest) -> impl MaybeFuture<Output = Result<u32>> {
        vendor_cmd_in_u32(self, cmd, 0, 0)
    }
    fn usb_vendor_cmd_int_w_value(
        &mut self,
        cmd: VendorRequest,
        w_value: u16,
    ) -> impl MaybeFuture<Output = Result<u32>> {
        vendor_cmd_in_u32(self, cmd, w_value, 0)
    }
    fn usb_vendor_cmd_int_w_index(
        &mut self,
        cmd: VendorRequest,
        w_index: u16,
    ) -> impl MaybeFuture<Output = Result<u32>> {
        vendor_cmd_in_u32(self, cmd, 0, w_index)
    }
}

fn vendor_cmd_in(
    transport: &mut UsbTransport,
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
    Op::new(async move {
        transport.prepare_io().await?;
        let interface = transport.interface.clone();
        transport.pending.begin(async move {
            let vec = interface.control_in(pkt, TIMEOUT).await?;
            require_length(length as usize, vec.len())?;
            if matches!(
                cmd,
                VendorRequest::RfRx
                    | VendorRequest::RfTx
                    | VendorRequest::BeginProg
                    | VendorRequest::FlashErase
            ) {
                require_length(4, vec.len())?;
                firmware_status(cmd, u32::from_le_bytes(vec[..4].try_into().unwrap()))?;
            }
            Ok(ControlResult::Data(vec))
        });
        match transport.finish_pending(TIMEOUT).await? {
            Some(ControlResult::Data(vec)) => Ok(vec),
            _ => Err(Error::Internal("missing vendor response")),
        }
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
    iface: &mut UsbTransport,
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
        &mut self,
        channel: Channel,
        enable: bool,
    ) -> impl MaybeFuture<Output = Result<()>>;
    /// Queries whether firmware loopback is currently enabled.
    fn usb_get_firmware_loopback(&mut self) -> impl MaybeFuture<Output = Result<bool>>;
    /// Resets the FX3 USB controller via a vendor control request.
    fn usb_device_reset(&mut self) -> impl MaybeFuture<Output = Result<()>>;
    /// Returns `true` if the firmware has reported readiness.
    fn usb_is_firmware_ready(&mut self) -> impl MaybeFuture<Output = Result<bool>>;
    /// Returns `true` if the FPGA has finished configuration.
    fn usb_is_fpga_configured(&mut self) -> impl MaybeFuture<Output = Result<bool>>;
    /// Signals the firmware to begin FPGA programming.
    fn usb_begin_fpga_prog(&mut self) -> impl MaybeFuture<Output = Result<()>>;
    /// Performs a bulk OUT transfer to the given endpoint address.
    ///
    /// `timeout` applies on native targets only; on wasm the transfer is
    /// awaited without a deadline because WebUSB cannot cancel transfers.
    fn usb_bulk_out(
        &mut self,
        endpoint: u8,
        data: &[u8],
        timeout: Duration,
    ) -> impl MaybeFuture<Output = Result<()>>;
}
impl BladeRf1UsbInterfaceCommands for UsbTransport {
    fn usb_enable_module(
        &mut self,
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
    fn usb_get_firmware_loopback(&mut self) -> impl MaybeFuture<Output = Result<bool>> {
        self.usb_vendor_cmd_int(VendorRequest::GetLoopback)
            .map_ok(|result| result != 0)
    }
    fn usb_device_reset(&mut self) -> impl MaybeFuture<Output = Result<()>> {
        let pkt = ControlOut {
            control_type: ControlType::Vendor,
            recipient: Recipient::Device,
            request: VendorRequest::Reset as u8,
            value: 0x0,
            index: 0x0,
            data: &[],
        };
        Op::new(async move {
            self.pending.finish(TIMEOUT).await?;
            self.current_alt_setting = None;
            let interface = self.interface.clone();
            self.pending.begin(async move {
                interface.control_out(pkt, TIMEOUT).await?;
                Ok(ControlResult::Done)
            });
            self.pending.finish(TIMEOUT).await.map(|_| ())
        })
    }
    fn usb_is_firmware_ready(&mut self) -> impl MaybeFuture<Output = Result<bool>> {
        self.usb_vendor_cmd_int(VendorRequest::QueryDeviceReady)
            .map_ok(|result| result != 0)
    }
    fn usb_is_fpga_configured(&mut self) -> impl MaybeFuture<Output = Result<bool>> {
        self.usb_vendor_cmd_int(VendorRequest::QueryFpgaStatus)
            .map(|result| match result? {
                0 => Ok(false),
                1 => Ok(true),
                _ => Err(Error::BoardState("unexpected FPGA status response")),
            })
    }
    fn usb_begin_fpga_prog(&mut self) -> impl MaybeFuture<Output = Result<()>> {
        self.usb_vendor_cmd_int(VendorRequest::BeginProg)
            .map(|result| firmware_status(VendorRequest::BeginProg, result?))
    }
    fn usb_bulk_out(
        &mut self,
        endpoint: u8,
        data: &[u8],
        timeout: Duration,
    ) -> impl MaybeFuture<Output = Result<()>> {
        Op::new(async move {
            self.prepare_io().await?;
            let mut ep = self
                .interface
                .endpoint::<Bulk, Out>(endpoint)
                .map_err(Error::EndpointBusy)?;
            let mut buf = ep.allocate(data.len());
            buf.extend_from_slice(data);
            let len = data.len();
            self.pending.begin(async move {
                ep.submit(buf);
                let completion = ep.next_complete().await;
                completion.status?;
                require_length(len, completion.actual_len)?;
                Ok(ControlResult::Done)
            });
            self.finish_pending(timeout).await.map(|_| ())
        })
    }
}

/// A bulk endpoint as used by the NIOS transport and the streaming pools.
///
/// Implemented for [`nusb::Endpoint`]; the streaming state machine is
/// generic over it so its lifecycle can be exercised without hardware.
pub(crate) trait BulkEndpoint: NonWasmSend {
    fn max_packet_size(&self) -> usize;
    fn allocate(&self, len: usize) -> Buffer;
    fn submit(&mut self, buffer: Buffer);
    fn pending(&self) -> usize;
    fn poll_next_complete(&mut self, cx: &mut Context<'_>) -> Poll<Completion>;
    #[cfg(not(target_arch = "wasm32"))]
    fn wait_next_complete(&mut self, timeout: Duration) -> Option<Completion>;
    fn can_cancel(&self) -> bool;
    fn cancel_all(&mut self);
    fn clear_halt(
        &mut self,
    ) -> impl MaybeFuture<Output = std::result::Result<(), nusb::Error>> + 'static;

    fn next_complete(&mut self) -> impl Future<Output = Completion> + NonWasmSend + '_ {
        std::future::poll_fn(|cx| self.poll_next_complete(cx))
    }
}

impl<Dir: EndpointDirection + 'static> BulkEndpoint for Endpoint<Bulk, Dir> {
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
    fn clear_halt(
        &mut self,
    ) -> impl MaybeFuture<Output = std::result::Result<(), nusb::Error>> + 'static {
        Endpoint::clear_halt(self)
    }
}

type NiosEndpoints = NiosExchange<Endpoint<Bulk, Out>, Endpoint<Bulk, In>>;

/// Concrete USB transport wrapping an `nusb` interface.
///
/// Manages NIOS packet endpoints, alternate setting tracking, and
/// provides access to streaming endpoints. Implements the USB command
/// traits for delegation to the underlying interface.
pub struct UsbTransport {
    interface: Interface,
    nios_endpoints: Option<NiosEndpoints>,
    current_alt_setting: Option<UsbAltSetting>,
    pending: Pending<ControlResult>,
    speed: Speed,
}

enum ControlResult {
    Data(Vec<u8>),
    Done,
    Setting(UsbAltSetting),
    RxEndpoint(Endpoint<Bulk, In>),
    TxEndpoint(Endpoint<Bulk, Out>),
    FlashPage([u8; 256]),
}
impl UsbTransport {
    /// Creates a new `UsbTransport` from an nusb `Interface`.
    pub fn new(interface: Interface, speed: Speed) -> Self {
        let current_alt_setting =
            UsbAltSetting::try_from(interface.get_alt_setting()).unwrap_or(UsbAltSetting::Null);
        Self {
            interface,
            nios_endpoints: None,
            current_alt_setting: Some(current_alt_setting),
            pending: Pending::default(),
            speed,
        }
    }
    /// Returns the cached current USB alternate setting.
    pub fn current_alt_setting(&self) -> Option<UsbAltSetting> {
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
            self.finish_pending(TIMEOUT).await?;
            if self.current_alt_setting == Some(setting) {
                return Ok(());
            }
            self.release_endpoints().await?;
            self.current_alt_setting = None;
            let interface = self.interface.clone();
            self.pending.begin(async move {
                interface.set_alt_setting(setting as u8).await?;
                Ok(ControlResult::Setting(setting))
            });
            self.finish_pending(TIMEOUT).await.map(|_| ())
        })
    }
    /// Sets the firmware loopback mode, cycling the alt setting to Null then
    /// RfLink so that NIOS packet URBs are released before the change.
    pub fn usb_set_firmware_loopback(
        &mut self,
        enable: bool,
    ) -> impl MaybeFuture<Output = Result<()>> {
        Op::new(async move {
            self.prepare_io().await?;
            self.release_endpoints().await?;
            self.current_alt_setting = None;
            let interface = self.interface.clone();
            self.pending.begin(async move {
                let bytes = interface
                    .control_in(
                        ControlIn {
                            control_type: ControlType::Vendor,
                            recipient: Recipient::Device,
                            request: VendorRequest::SetLoopback as u8,
                            value: u16::from(enable),
                            index: 0,
                            length: 4,
                        },
                        TIMEOUT,
                    )
                    .await?;
                require_length(4, bytes.len())?;
                firmware_ack(
                    VendorRequest::SetLoopback,
                    u32::from(enable),
                    u32::from_le_bytes(bytes.try_into().unwrap()),
                )?;
                interface.set_alt_setting(UsbAltSetting::Null as u8).await?;
                interface
                    .set_alt_setting(UsbAltSetting::RfLink as u8)
                    .await?;
                Ok(ControlResult::Setting(UsbAltSetting::RfLink))
            });
            self.finish_pending(TIMEOUT).await.map(|_| ())
        })
    }
    async fn finish_pending(&mut self, timeout: Duration) -> Result<Option<ControlResult>> {
        if self.current_alt_setting.is_none() && !self.pending.is_pending() {
            return Err(Error::RecoveryRequired);
        }
        let result = self.pending.finish(timeout).await?;
        if let Some(ControlResult::Setting(setting)) = result {
            self.current_alt_setting = Some(setting);
        }
        self.current_alt_setting.ok_or(Error::RecoveryRequired)?;
        Ok(result)
    }

    pub(crate) async fn prepare_io(&mut self) -> Result<()> {
        self.finish_pending(TIMEOUT).await?;
        if let Some(endpoints) = &mut self.nios_endpoints {
            endpoints.finish(TIMEOUT).await?;
        }
        Ok(())
    }
    /// Finishes pending NIOS transactions before releasing their endpoints.
    ///
    /// Called before switching USB alternate settings to ensure clean
    /// endpoint teardown. Incomplete transactions retain their endpoints for retry.
    pub fn release_endpoints(&mut self) -> impl MaybeFuture<Output = Result<()>> {
        Op::new(async move {
            if let Some(endpoints) = self.nios_endpoints.as_mut() {
                endpoints.finish(RELEASE_TIMEOUT).await?;
            }
            self.nios_endpoints = None;
            Ok(())
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
            self.nios_endpoints = Some(NiosExchange::new(ep_out, ep_in));
        }
        self.nios_endpoints
            .as_mut()
            .ok_or(Error::EndpointNotAvailable)
    }
    /// Exchanges a NIOS packet, first settling any abandoned previous transaction.
    ///
    /// Timeouts and cancelled waits retain pending transfers on every backend.
    pub fn exchange(
        &mut self,
        request: &[u8; 16],
        timeout: Option<Duration>,
    ) -> impl MaybeFuture<Output = Result<[u8; 16]>> {
        Op::new(async move {
            let t = timeout.unwrap_or(TIMEOUT);
            self.finish_pending(t).await?;
            let endpoints = self.ensure_nios_endpoints()?;
            endpoints.exchange(request, t).await
        })
    }
    /// Acquires the RX streaming bulk IN endpoint.
    ///
    /// Returns an error if the endpoint is already claimed by another
    /// consumer.
    pub async fn acquire_streaming_rx_endpoint(&mut self) -> Result<Endpoint<Bulk, In>> {
        self.prepare_io().await?;
        let mut endpoint = self
            .interface
            .endpoint::<Bulk, In>(STREAM_ENDPOINT_RX)
            .map_err(Error::EndpointBusy)?;
        self.pending.begin(async move {
            endpoint.clear_halt().await?;
            Ok(ControlResult::RxEndpoint(endpoint))
        });
        match self.finish_pending(TIMEOUT).await? {
            Some(ControlResult::RxEndpoint(endpoint)) => Ok(endpoint),
            _ => Err(Error::Internal("missing RX endpoint")),
        }
    }
    /// Acquires the TX streaming bulk OUT endpoint.
    ///
    /// Returns an error if the endpoint is already claimed by another
    /// consumer.
    pub async fn acquire_streaming_tx_endpoint(&mut self) -> Result<Endpoint<Bulk, Out>> {
        self.prepare_io().await?;
        let mut endpoint = self
            .interface
            .endpoint::<Bulk, Out>(STREAM_ENDPOINT_TX)
            .map_err(Error::EndpointBusy)?;
        self.pending.begin(async move {
            endpoint.clear_halt().await?;
            Ok(ControlResult::TxEndpoint(endpoint))
        });
        match self.finish_pending(TIMEOUT).await? {
            Some(ControlResult::TxEndpoint(endpoint)) => Ok(endpoint),
            _ => Err(Error::Internal("missing TX endpoint")),
        }
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
