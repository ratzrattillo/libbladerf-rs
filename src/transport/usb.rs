//! USB Transport and Commands for BladeRF
//!
//! This module provides:
//! - [`UsbTransport`]: Transport implementation for NIOS communication over USB
//! - [`DeviceCommands`]: Generic USB device-level commands
//! - [`InterfaceCommands`]: Generic USB interface-level commands
//! - [`BladeRf1Commands`]: BladeRF1-specific USB commands

use crate::transport::Transport;
use crate::{Direction, Error, Result};
use nusb::transfer::{Buffer, Bulk, ControlIn, ControlOut, ControlType, In, Out, Recipient};
use nusb::{Device, Endpoint, Interface, MaybeFuture};
use std::num::NonZero;
use std::time::Duration;

// ============================================================================
// Constants
// ============================================================================

/// USB endpoint addresses for NIOS communication.
pub const CONTROL_ENDPOINT_OUT: u8 = 0x02;
pub const CONTROL_ENDPOINT_IN: u8 = 0x82;

/// USB endpoint addresses for sample streaming.
pub const STREAM_ENDPOINT_RX: u8 = 0x81;
pub const STREAM_ENDPOINT_TX: u8 = 0x01;

/// Interface numbers
pub const USB_IF_NULL: u8 = 0;
pub const USB_IF_RF_LINK: u8 = 1;

/// BladeRF USB command codes
pub const BLADE_USB_CMD_RF_RX: u8 = 4;
pub const BLADE_USB_CMD_RF_TX: u8 = 5;
#[allow(dead_code)]
pub const BLADE_USB_CMD_QUERY_DEVICE_READY: u8 = 6;
#[allow(dead_code)]
pub const BLADE_USB_CMD_RESET: u8 = 105;
pub const BLADE_USB_CMD_SET_LOOPBACK: u8 = 113;
pub const BLADE_USB_CMD_GET_LOOPBACK: u8 = 114;

#[allow(dead_code)]
const TIMEOUT: Duration = Duration::from_millis(1);

// ============================================================================
// String Descriptors
// ============================================================================

#[repr(u8)]
pub enum StringDescriptors {
    /// Don't want to start with 0 as 0 is reserved for the language table
    Manufacturer = 0x1,
    Product,
    Serial,
    Fx3Firmware,
}

#[repr(u8)]
#[allow(dead_code)]
pub enum DescriptorTypes {
    /// Don't want to start with 0 as 0 is reserved for the language table
    #[allow(dead_code)]
    Device = 0x01,
    Configuration = 0x2,
    #[allow(dead_code)]
    String = 0x03,
    #[allow(dead_code)]
    Default = 0x06,
    #[allow(dead_code)]
    Bos = 0x0f,
}

// ============================================================================
// Generic USB Device Commands
// ============================================================================

/// Generic USB device-level commands.
///
/// These commands operate at the USB device level and are not specific
/// to any particular USB device.
pub trait DeviceCommands {
    /// Get a list of supported language codes for string descriptors.
    fn get_supported_languages(&self) -> Result<Vec<u16>>;

    /// Get a configuration descriptor by index.
    #[allow(dead_code)]
    fn get_configuration_descriptor(&self, descriptor_index: u8) -> Result<Vec<u8>>;

    /// Get a USB string descriptor by index.
    fn get_string_descriptor_simple(&self, descriptor_index: NonZero<u8>) -> Result<String>;

    /// Return the device's serial number.
    fn serial(&self) -> Result<String>;

    /// Return the device's manufacturer string.
    fn manufacturer(&self) -> Result<String>;

    /// Return the device's product name string.
    fn product(&self) -> Result<String>;
}

impl DeviceCommands for Device {
    fn get_supported_languages(&self) -> Result<Vec<u16>> {
        let languages = self
            .get_string_descriptor_supported_languages(Duration::from_secs(1))
            .wait()
            .map_err(|_| Error::Invalid)?
            .collect();
        Ok(languages)
    }

    fn get_configuration_descriptor(&self, descriptor_index: u8) -> Result<Vec<u8>> {
        let descriptor = self
            .get_descriptor(
                DescriptorTypes::Configuration as u8,
                descriptor_index,
                0x00,
                Duration::from_secs(1),
            )
            .wait()
            .map_err(|_| Error::Invalid)?;
        Ok(descriptor)
    }

    fn get_string_descriptor_simple(&self, descriptor_index: NonZero<u8>) -> Result<String> {
        let descriptor = self
            .get_string_descriptor(descriptor_index, 0x409, Duration::from_secs(1))
            .wait()
            .map_err(|_| Error::Invalid)?;
        Ok(descriptor)
    }

    fn serial(&self) -> Result<String> {
        self.get_string_descriptor_simple(
            NonZero::try_from(StringDescriptors::Serial as u8).map_err(|_| Error::Invalid)?,
        )
    }

    fn manufacturer(&self) -> Result<String> {
        self.get_string_descriptor_simple(
            NonZero::try_from(StringDescriptors::Manufacturer as u8).map_err(|_| Error::Invalid)?,
        )
    }

    fn product(&self) -> Result<String> {
        self.get_string_descriptor_simple(
            NonZero::try_from(StringDescriptors::Product as u8).map_err(|_| Error::Invalid)?,
        )
    }
}

// ============================================================================
// BladeRF1 Device Commands
// ============================================================================

/// BladeRF1-specific device-level commands.
///
/// These commands operate at the USB device level and are specific to
/// the BladeRF1 hardware.
pub trait BladeRf1DeviceCommands: DeviceCommands {
    /// Return the device's FX3 firmware version string.
    ///
    /// The FX3 (Cypress CYUSB3014) is the USB controller chip used in BladeRF1.
    fn fx3_firmware_version(&self) -> Result<String>;
}

impl BladeRf1DeviceCommands for Device {
    fn fx3_firmware_version(&self) -> Result<String> {
        self.get_string_descriptor_simple(
            NonZero::try_from(StringDescriptors::Fx3Firmware as u8).map_err(|_| Error::Invalid)?,
        )
    }
}

// ============================================================================
// Generic USB Interface Commands
// ============================================================================

/// Generic USB interface-level commands.
///
/// These commands operate at the USB interface level and provide
/// basic vendor command and configuration functionality.
pub trait InterfaceCommands {
    /// Execute a vendor command that returns a 32-bit integer.
    fn usb_vendor_cmd_int(&self, cmd: u8) -> Result<u32>;

    /// Execute a vendor command with a wValue parameter, returning a 32-bit integer.
    fn usb_vendor_cmd_int_wvalue(&self, cmd: u8, wvalue: u16) -> Result<u32>;

    /// Change the USB alternate setting for this interface.
    fn usb_change_setting(&mut self, setting: u8) -> Result<()>;

    /// Set the USB configuration.
    #[allow(dead_code)]
    fn usb_set_configuration(&self, configuration: u16) -> Result<()>;
}

impl InterfaceCommands for Interface {
    fn usb_vendor_cmd_int(&self, cmd: u8) -> Result<u32> {
        let pkt = ControlIn {
            control_type: ControlType::Vendor,
            recipient: Recipient::Device,
            request: cmd,
            value: 0,
            index: 0,
            length: 0x4,
        };
        let vec = self.control_in(pkt, Duration::from_secs(5)).wait()?;

        log::debug!("get_vendor_cmd_int response data: {vec:?}");
        Ok(u32::from_le_bytes(
            vec.as_slice()[0..4]
                .try_into()
                .map_err(|_| Error::Invalid)?,
        ))
    }

    fn usb_vendor_cmd_int_wvalue(&self, cmd: u8, wvalue: u16) -> Result<u32> {
        let pkt = ControlIn {
            control_type: ControlType::Vendor,
            recipient: Recipient::Device,
            request: cmd,
            value: wvalue,
            index: 0,
            length: 0x4,
        };
        let vec = self.control_in(pkt, Duration::from_secs(5)).wait()?;

        log::trace!("vendor_cmd_int_wvalue response data: {vec:?}");
        Ok(u32::from_le_bytes(
            vec.as_slice()[0..4]
                .try_into()
                .map_err(|_| Error::Invalid)?,
        ))
    }

    fn usb_change_setting(&mut self, setting: u8) -> Result<()> {
        Ok(self.set_alt_setting(setting).wait()?)
    }

    fn usb_set_configuration(&self, configuration: u16) -> Result<()> {
        Ok(self
            .control_out(
                ControlOut {
                    control_type: ControlType::Standard,
                    recipient: Recipient::Device,
                    request: 0x09,
                    value: configuration,
                    index: 0x00,
                    data: &[],
                },
                TIMEOUT,
            )
            .wait()?)
    }
}

// ============================================================================
// BladeRF1-Specific Commands
// ============================================================================

/// BladeRF1-specific USB commands.
///
/// These commands are specific to the BladeRF1 device and use the
/// vendor command interface to control device-specific functionality.
pub trait BladeRf1Commands: InterfaceCommands {
    /// Enable or disable an RF module (RX or TX).
    fn usb_enable_module(&self, direction: Direction, enable: bool) -> Result<()>;

    /// Set firmware loopback mode.
    fn usb_set_firmware_loopback(&mut self, enable: bool) -> Result<()>;

    /// Get firmware loopback mode status.
    fn usb_get_firmware_loopback(&self) -> Result<bool>;

    /// Reset the BladeRF1 device.
    #[allow(dead_code)]
    fn usb_device_reset(&self) -> Result<()>;

    /// Check if firmware is ready.
    #[allow(dead_code)]
    fn usb_is_firmware_ready(&self) -> Result<bool>;
}

impl BladeRf1Commands for Interface {
    fn usb_enable_module(&self, direction: Direction, enable: bool) -> Result<()> {
        let val = enable as u16;
        let cmd = if direction == Direction::Rx {
            BLADE_USB_CMD_RF_RX
        } else {
            BLADE_USB_CMD_RF_TX
        };

        let _fx3_ret = self.usb_vendor_cmd_int_wvalue(cmd, val)?;
        Ok(())
    }

    fn usb_set_firmware_loopback(&mut self, enable: bool) -> Result<()> {
        self.usb_vendor_cmd_int_wvalue(BLADE_USB_CMD_SET_LOOPBACK, enable as u16)?;
        self.usb_change_setting(USB_IF_NULL)?;
        self.usb_change_setting(USB_IF_RF_LINK)?;
        Ok(())
    }

    fn usb_get_firmware_loopback(&self) -> Result<bool> {
        let result = self.usb_vendor_cmd_int(BLADE_USB_CMD_GET_LOOPBACK)?;
        Ok(result != 0)
    }

    fn usb_device_reset(&self) -> Result<()> {
        let pkt = ControlOut {
            control_type: ControlType::Vendor,
            recipient: Recipient::Device,
            request: BLADE_USB_CMD_RESET,
            value: 0x0,
            index: 0x0,
            data: &[],
        };
        self.control_out(pkt, Duration::from_secs(100)).wait()?;
        Ok(())
    }

    fn usb_is_firmware_ready(&self) -> Result<bool> {
        Ok(self.usb_vendor_cmd_int(BLADE_USB_CMD_QUERY_DEVICE_READY)? != 0)
    }
}

// ============================================================================
// USB Transport
// ============================================================================

/// Cached NIOS endpoints for USB bulk transfers.
struct NiosEndpoints {
    ep_out: Endpoint<Bulk, Out>,
    ep_in: Endpoint<Bulk, In>,
}

/// USB transport for all BladeRF USB communication.
///
/// This transport handles:
/// - NIOS communication via endpoints 0x02 (OUT) and 0x82 (IN)
/// - Sample streaming via endpoints 0x01 (TX) and 0x81 (RX)
/// - Control transfers
///
/// Endpoints are lazily acquired on first use and cached for efficiency.
/// Call [`release_endpoints`](Self::release_endpoints) before changing the alternate setting.
pub struct UsbTransport {
    interface: Interface,
    buf: Vec<u8>,
    nios_endpoints: Option<NiosEndpoints>,
}

impl UsbTransport {
    /// Create a new USB transport wrapping a USB interface.
    pub fn new(interface: Interface) -> Self {
        Self {
            interface,
            buf: vec![0; 1024], // USB 3.0 SuperSpeed max packet size
            nios_endpoints: None,
        }
    }

    /// Returns a reference to the underlying USB interface.
    pub fn interface(&self) -> &Interface {
        &self.interface
    }

    /// Release cached NIOS endpoints.
    ///
    /// This must be called before `set_alt_setting` to allow the alternate setting
    /// change to succeed. Endpoints will be re-acquired on the next transaction.
    pub fn release_endpoints(&mut self) {
        self.nios_endpoints = None;
    }

    fn ensure_nios_endpoints(&mut self) -> Result<&mut NiosEndpoints> {
        if self.nios_endpoints.is_none() {
            let ep_out = self
                .interface
                .endpoint::<Bulk, Out>(CONTROL_ENDPOINT_OUT)
                .map_err(|_| Error::EndpointBusy)?;
            let ep_in = self
                .interface
                .endpoint::<Bulk, In>(CONTROL_ENDPOINT_IN)
                .map_err(|_| Error::EndpointBusy)?;
            self.nios_endpoints = Some(NiosEndpoints { ep_out, ep_in });
        }
        Ok(self.nios_endpoints.as_mut().unwrap())
    }

    /// Take the internal buffer for use as a packet.
    ///
    /// The buffer will be returned after the transaction completes.
    pub fn take_buf(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.buf)
    }

    /// Return a buffer to the internal pool.
    pub fn return_buf(&mut self, buf: Vec<u8>) {
        self.buf = buf;
    }

    // ========================================================================
    // Streaming Endpoint Methods
    // ========================================================================

    /// Acquire the RX streaming endpoint (0x81).
    ///
    /// The returned endpoint can be used to create a reader for receiving samples.
    /// The endpoint is released when dropped.
    pub fn acquire_streaming_rx_endpoint(&self) -> Result<Endpoint<Bulk, In>> {
        self.interface
            .endpoint::<Bulk, In>(STREAM_ENDPOINT_RX)
            .map_err(|_| Error::EndpointBusy)
    }

    /// Acquire the TX streaming endpoint (0x01).
    ///
    /// The returned endpoint can be used to create a writer for transmitting samples.
    /// The endpoint is released when dropped.
    pub fn acquire_streaming_tx_endpoint(&self) -> Result<Endpoint<Bulk, Out>> {
        self.interface
            .endpoint::<Bulk, Out>(STREAM_ENDPOINT_TX)
            .map_err(|_| Error::EndpointBusy)
    }
}

impl Transport for UsbTransport {
    fn transact(&mut self, request: Vec<u8>, timeout: Option<Duration>) -> Result<Vec<u8>> {
        const NIOS_PKT_SIZE: usize = 16;

        let endpoints = self.ensure_nios_endpoints()?;
        let max_pkt_size = endpoints.ep_in.max_packet_size();
        let t = timeout.unwrap_or(Duration::from_millis(1000));

        log::trace!("transact: request len = {}", request.len());

        // Convert to Buffer - the packet should already be truncated by into_packet()
        let buffer = Buffer::from(request);

        // Submit OUT transfer
        endpoints.ep_out.submit(buffer);
        let mut response = endpoints
            .ep_out
            .wait_next_complete(t)
            .ok_or(Error::Timeout)?;
        response.status?;

        // Submit IN transfer for response
        response.buffer.set_requested_len(max_pkt_size);
        endpoints.ep_in.submit(response.buffer);
        response = endpoints
            .ep_in
            .wait_next_complete(t)
            .ok_or(Error::Timeout)?;
        response.status?;

        // Convert Buffer back to Vec<u8> (zero-cost)
        let buf = response.buffer.into_vec();

        // Verify minimum size
        if buf.len() < NIOS_PKT_SIZE {
            return Err(crate::NiosPacketError::InvalidSize(buf.len()).into());
        }

        Ok(buf)
    }
}

impl From<Interface> for UsbTransport {
    fn from(interface: Interface) -> Self {
        Self::new(interface)
    }
}

// Implement UsbInterfaceCommands for UsbTransport
impl InterfaceCommands for UsbTransport {
    fn usb_vendor_cmd_int(&self, cmd: u8) -> Result<u32> {
        self.interface.usb_vendor_cmd_int(cmd)
    }

    fn usb_vendor_cmd_int_wvalue(&self, cmd: u8, wvalue: u16) -> Result<u32> {
        self.interface.usb_vendor_cmd_int_wvalue(cmd, wvalue)
    }

    fn usb_change_setting(&mut self, setting: u8) -> Result<()> {
        // Release cached endpoints before changing alternate setting
        self.release_endpoints();
        self.interface.set_alt_setting(setting).wait()?;
        Ok(())
    }

    fn usb_set_configuration(&self, configuration: u16) -> Result<()> {
        self.interface.usb_set_configuration(configuration)
    }
}

// Implement BladeRf1Commands for UsbTransport
impl BladeRf1Commands for UsbTransport {
    fn usb_enable_module(&self, direction: Direction, enable: bool) -> Result<()> {
        self.interface.usb_enable_module(direction, enable)
    }

    fn usb_set_firmware_loopback(&mut self, enable: bool) -> Result<()> {
        self.interface
            .usb_vendor_cmd_int_wvalue(BLADE_USB_CMD_SET_LOOPBACK, enable as u16)?;
        self.usb_change_setting(USB_IF_NULL)?;
        self.usb_change_setting(USB_IF_RF_LINK)?;
        Ok(())
    }

    fn usb_get_firmware_loopback(&self) -> Result<bool> {
        self.interface.usb_get_firmware_loopback()
    }

    fn usb_device_reset(&self) -> Result<()> {
        self.interface.usb_device_reset()
    }

    fn usb_is_firmware_ready(&self) -> Result<bool> {
        self.interface.usb_is_firmware_ready()
    }
}
