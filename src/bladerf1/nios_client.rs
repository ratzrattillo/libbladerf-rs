use crate::bladerf1::hardware::lms6002d::{Band, Tune};
use crate::bladerf1::protocol::{nios_decode_retune, nios_encode_retune};
use crate::channel::Channel;
use crate::error::{Error, Result};
use crate::nios_client::NiosCore;
use crate::protocol::nios::NiosPkt8x32Target;
use crate::transport::Transport;
use crate::transport::usb::UsbTransport;
use crate::transport::usb::{BladeRf1UsbInterfaceCommands, UsbInterfaceCommands};
use nusb::Interface;
use std::ops::{Deref, DerefMut};
pub type NiosInterface = NiosClient;
impl From<Interface> for NiosInterface {
    fn from(interface: Interface) -> Self {
        NiosClient::new(UsbTransport::new(interface))
    }
}
pub struct NiosClient(NiosCore<UsbTransport>);
impl NiosClient {
    pub fn new(transport: UsbTransport) -> Self {
        Self(NiosCore::new(transport))
    }
    #[allow(clippy::too_many_arguments)]
    pub fn nios_retune(
        &mut self,
        channel: Channel,
        timestamp: u64,
        nint: u16,
        nfrac: u32,
        freqsel: u8,
        vcocap: u8,
        band: Band,
        tune: Tune,
        xb_gpio: u8,
    ) -> Result<()> {
        const RETUNE_NOW: u64 = 0x00;
        if timestamp == RETUNE_NOW {
            log::trace!("Clearing Retune Queue");
        }
        let out_buf = self.transport_mut().out_buffer()?;
        nios_encode_retune(
            out_buf, channel, timestamp, nint, nfrac, freqsel, vcocap, band, tune, xb_gpio,
        )?;
        let response = self.transport_mut().submit(None)?;
        let response_pkt = nios_decode_retune(response)?;
        if !response_pkt.is_success() {
            let is_immediate = response_pkt.duration() == RETUNE_NOW;
            return if is_immediate {
                Err(Error::TuningFailed)
            } else {
                Err(Error::RetuneQueueFull)
            };
        }
        Ok(())
    }
    pub fn nios_xb200_synth_write(&mut self, value: u32) -> Result<()> {
        self.nios_write::<u8, u32>(NiosPkt8x32Target::Adf4351, 0, value)
    }
}
impl Deref for NiosClient {
    type Target = NiosCore<UsbTransport>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl DerefMut for NiosClient {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
impl UsbInterfaceCommands for NiosClient {
    fn usb_vendor_cmd_int(&self, cmd: u8) -> Result<u32> {
        self.transport().usb_vendor_cmd_int(cmd)
    }
    fn usb_vendor_cmd_int_wvalue(&self, cmd: u8, wvalue: u16) -> Result<u32> {
        self.transport().usb_vendor_cmd_int_wvalue(cmd, wvalue)
    }
    fn usb_change_setting(&mut self, setting: u8) -> Result<()> {
        self.transport_mut().usb_change_setting(setting)
    }
    fn usb_set_configuration(&self, configuration: u16) -> Result<()> {
        self.transport().usb_set_configuration(configuration)
    }
}
impl BladeRf1UsbInterfaceCommands for NiosClient {
    fn usb_enable_module(&self, channel: Channel, enable: bool) -> Result<()> {
        self.transport().usb_enable_module(channel, enable)
    }
    fn usb_set_firmware_loopback(&mut self, enable: bool) -> Result<()> {
        self.transport_mut().usb_set_firmware_loopback(enable)
    }
    fn usb_get_firmware_loopback(&self) -> Result<bool> {
        self.transport().usb_get_firmware_loopback()
    }
    fn usb_device_reset(&self) -> Result<()> {
        self.transport().usb_device_reset()
    }
    fn usb_is_firmware_ready(&self) -> Result<bool> {
        self.transport().usb_is_firmware_ready()
    }
}
