use crate::channel::Channel;
use crate::error::{Error, Result};
use crate::protocol::nios::NiosPacketError;
use crate::transport::Transport;
use nusb::transfer::{Buffer, Bulk, ControlIn, ControlOut, ControlType, In, Out, Recipient};
use nusb::{Device, Endpoint, Interface, MaybeFuture};
use std::num::NonZero;
use std::time::Duration;
pub const CONTROL_ENDPOINT_OUT: u8 = 0x02;
pub const CONTROL_ENDPOINT_IN: u8 = 0x82;
pub const STREAM_ENDPOINT_RX: u8 = 0x81;
pub const STREAM_ENDPOINT_TX: u8 = 0x01;
pub const USB_IF_NULL: u8 = 0;
pub const USB_IF_RF_LINK: u8 = 1;
pub const BLADE_USB_CMD_RF_RX: u8 = 4;
pub const BLADE_USB_CMD_RF_TX: u8 = 5;
pub const BLADE_USB_CMD_SET_LOOPBACK: u8 = 113;
pub const BLADE_USB_CMD_GET_LOOPBACK: u8 = 114;
pub const BLADE_USB_CMD_QUERY_DEVICE_READY: u8 = 6;
pub const BLADE_USB_CMD_RESET: u8 = 105;
const TIMEOUT: Duration = Duration::from_secs(1);
#[repr(u8)]
pub enum StringDescriptors {
    Manufacturer = 0x1,
    Product,
    Serial,
    Fx3Firmware,
}
#[repr(u8)]
pub enum DescriptorTypes {
    Configuration = 0x2,
}
pub trait DeviceCommands {
    fn get_supported_languages(&self) -> Result<Vec<u16>>;
    fn get_configuration_descriptor(&self, descriptor_index: u8) -> Result<Vec<u8>>;
    fn get_string_descriptor_simple(&self, descriptor_index: NonZero<u8>) -> Result<String>;
    fn serial(&self) -> Result<String>;
    fn manufacturer(&self) -> Result<String>;
    fn product(&self) -> Result<String>;
}
impl DeviceCommands for Device {
    fn get_supported_languages(&self) -> Result<Vec<u16>> {
        let languages = self
            .get_string_descriptor_supported_languages(TIMEOUT)
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
                TIMEOUT,
            )
            .wait()
            .map_err(|_| Error::Invalid)?;
        Ok(descriptor)
    }
    fn get_string_descriptor_simple(&self, descriptor_index: NonZero<u8>) -> Result<String> {
        let descriptor = self
            .get_string_descriptor(descriptor_index, 0x409, TIMEOUT)
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
pub trait BladeRf1DeviceCommands: DeviceCommands {
    fn fx3_firmware_version(&self) -> Result<String>;
}
impl BladeRf1DeviceCommands for Device {
    fn fx3_firmware_version(&self) -> Result<String> {
        self.get_string_descriptor_simple(
            NonZero::try_from(StringDescriptors::Fx3Firmware as u8).map_err(|_| Error::Invalid)?,
        )
    }
}
pub trait UsbInterfaceCommands {
    fn usb_vendor_cmd_int(&self, cmd: u8) -> Result<u32>;
    fn usb_vendor_cmd_int_wvalue(&self, cmd: u8, wvalue: u16) -> Result<u32>;
    fn usb_change_setting(&mut self, setting: u8) -> Result<()>;
    fn usb_set_configuration(&self, configuration: u16) -> Result<()>;
}
impl UsbInterfaceCommands for Interface {
    fn usb_vendor_cmd_int(&self, cmd: u8) -> Result<u32> {
        let pkt = ControlIn {
            control_type: ControlType::Vendor,
            recipient: Recipient::Device,
            request: cmd,
            value: 0,
            index: 0,
            length: 0x4,
        };
        let vec = self.control_in(pkt, TIMEOUT).wait()?;
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
        let vec = self.control_in(pkt, TIMEOUT).wait()?;
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
pub trait BladeRf1UsbInterfaceCommands: UsbInterfaceCommands {
    fn usb_enable_module(&self, channel: Channel, enable: bool) -> Result<()>;
    fn usb_set_firmware_loopback(&mut self, enable: bool) -> Result<()>;
    fn usb_get_firmware_loopback(&self) -> Result<bool>;
    fn usb_device_reset(&self) -> Result<()>;
    fn usb_is_firmware_ready(&self) -> Result<bool>;
}
impl BladeRf1UsbInterfaceCommands for Interface {
    fn usb_enable_module(&self, channel: Channel, enable: bool) -> Result<()> {
        let val = enable as u16;
        let cmd = if channel.is_rx() {
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
        self.control_out(pkt, TIMEOUT).wait()?;
        Ok(())
    }
    fn usb_is_firmware_ready(&self) -> Result<bool> {
        Ok(self.usb_vendor_cmd_int(BLADE_USB_CMD_QUERY_DEVICE_READY)? != 0)
    }
}
impl UsbInterfaceCommands for UsbTransport {
    fn usb_vendor_cmd_int(&self, cmd: u8) -> Result<u32> {
        self.interface.usb_vendor_cmd_int(cmd)
    }
    fn usb_vendor_cmd_int_wvalue(&self, cmd: u8, wvalue: u16) -> Result<u32> {
        self.interface.usb_vendor_cmd_int_wvalue(cmd, wvalue)
    }
    fn usb_change_setting(&mut self, setting: u8) -> Result<()> {
        self.release_endpoints();
        self.interface.set_alt_setting(setting).wait()?;
        Ok(())
    }
    fn usb_set_configuration(&self, configuration: u16) -> Result<()> {
        self.interface.usb_set_configuration(configuration)
    }
}
impl BladeRf1UsbInterfaceCommands for UsbTransport {
    fn usb_enable_module(&self, channel: Channel, enable: bool) -> Result<()> {
        self.interface.usb_enable_module(channel, enable)
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
struct NiosEndpoints {
    ep_out: Endpoint<Bulk, Out>,
    ep_in: Endpoint<Bulk, In>,
    buf_out: Option<Buffer>,
    buf_in: Option<Buffer>,
}
pub struct UsbTransport {
    interface: Interface,
    nios_endpoints: Option<NiosEndpoints>,
}
impl UsbTransport {
    const NIOS_PKT_SIZE: usize = 16;
    pub fn new(interface: Interface) -> Self {
        Self {
            interface,
            nios_endpoints: None,
        }
    }
    pub fn interface(&self) -> &Interface {
        &self.interface
    }
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
            let buf_out = Some(ep_out.allocate(Self::NIOS_PKT_SIZE));
            let buf_in = Some(ep_in.allocate(ep_in.max_packet_size()));
            self.nios_endpoints = Some(NiosEndpoints {
                ep_out,
                ep_in,
                buf_out,
                buf_in,
            });
        }
        Ok(self.nios_endpoints.as_mut().unwrap())
    }
    pub fn out_buffer(&mut self) -> Result<&mut [u8]> {
        Transport::out_buffer(self)
    }
    pub fn acquire_streaming_rx_endpoint(&self) -> Result<Endpoint<Bulk, In>> {
        self.interface
            .endpoint::<Bulk, In>(STREAM_ENDPOINT_RX)
            .map_err(|_| Error::EndpointBusy)
    }
    pub fn acquire_streaming_tx_endpoint(&self) -> Result<Endpoint<Bulk, Out>> {
        self.interface
            .endpoint::<Bulk, Out>(STREAM_ENDPOINT_TX)
            .map_err(|_| Error::EndpointBusy)
    }
}
impl Transport for UsbTransport {
    fn out_buffer(&mut self) -> Result<&mut [u8]> {
        let endpoints = self.ensure_nios_endpoints()?;
        let buf = endpoints.buf_out.as_mut().ok_or(Error::EndpointBusy)?;
        buf.clear();
        buf.extend_fill(Self::NIOS_PKT_SIZE, 0);
        Ok(buf)
    }
    fn submit(&mut self, timeout: Option<Duration>) -> Result<&[u8]> {
        let endpoints = self.ensure_nios_endpoints()?;
        let t = timeout.unwrap_or(TIMEOUT);
        let buf_out = endpoints.buf_out.take().ok_or(Error::EndpointBusy)?;
        log::trace!("submit: OUT buffer len = {}", buf_out.len());
        endpoints.ep_out.submit(buf_out);
        let mut response = endpoints
            .ep_out
            .wait_next_complete(t)
            .ok_or(Error::Timeout)?;
        response.status?;
        endpoints.buf_out = Some(response.buffer);
        let mut buf_in = endpoints.buf_in.take().ok_or(Error::EndpointBusy)?;
        buf_in.set_requested_len(endpoints.ep_in.max_packet_size());
        endpoints.ep_in.submit(buf_in);
        response = endpoints
            .ep_in
            .wait_next_complete(t)
            .ok_or(Error::Timeout)?;
        response.status?;
        endpoints.buf_in = Some(response.buffer);
        let in_buf = endpoints.buf_in.as_ref().unwrap();
        let in_len = in_buf.len();
        if in_len < Self::NIOS_PKT_SIZE {
            return Err(NiosPacketError::InvalidSize(in_len).into());
        }
        Ok(&in_buf[..Self::NIOS_PKT_SIZE])
    }
}
impl From<Interface> for UsbTransport {
    fn from(interface: Interface) -> Self {
        Self::new(interface)
    }
}
