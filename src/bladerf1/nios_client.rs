use crate::bladerf1::hardware::lms6002d::{Band, Tune};
use crate::bladerf1::protocol::{nios_decode_retune, nios_encode_retune};
use crate::channel::Channel;
use crate::error::{Error, Result};
use crate::nios_client::NiosCore;
use crate::protocol::nios::NiosPkt8x32Target;
use crate::protocol::nios::packet_generic::NiosNum;
use crate::transport::Transport;
use crate::transport::usb::UsbTransport;
use crate::transport::usb::{BladeRf1UsbInterfaceCommands, UsbInterfaceCommands};
use crate::version::SemanticVersion;
use nusb::Interface;
impl From<Interface> for NiosClient {
    fn from(interface: Interface) -> Self {
        NiosClient::new(UsbTransport::new(interface))
    }
}
pub struct RetuneResult {
    pub duration: u64,
}

pub struct NiosClient(NiosCore<UsbTransport>);
impl NiosClient {
    pub fn new(transport: UsbTransport) -> Self {
        Self(NiosCore::new(transport))
    }
    pub fn transport(&self) -> &UsbTransport {
        self.0.transport()
    }
    pub fn get_alt_setting(&self) -> u8 {
        self.0.get_alt_setting()
    }
    pub fn nios_read<A: NiosNum + Send, D: NiosNum + Send>(
        &mut self,
        id: impl Into<u8>,
        addr: A,
    ) -> Result<D> {
        self.0.nios_read(id, addr)
    }
    pub fn nios_write<A: NiosNum + Send, D: NiosNum + Send>(
        &mut self,
        id: impl Into<u8>,
        addr: A,
        data: D,
    ) -> Result<()> {
        self.0.nios_write(id, addr, data)
    }
    pub fn nios_config_read(&mut self) -> Result<u32> {
        self.0.nios_config_read()
    }
    pub fn nios_config_write(&mut self, value: u32) -> Result<()> {
        self.0.nios_config_write(value)
    }
    pub fn nios_expansion_gpio_read(&mut self) -> Result<u32> {
        self.0.nios_expansion_gpio_read()
    }
    pub fn nios_expansion_gpio_write(&mut self, mask: u32, val: u32) -> Result<()> {
        self.0.nios_expansion_gpio_write(mask, val)
    }
    pub fn nios_expansion_gpio_dir_read(&mut self) -> Result<u32> {
        self.0.nios_expansion_gpio_dir_read()
    }
    pub fn nios_expansion_gpio_dir_write(&mut self, mask: u32, val: u32) -> Result<()> {
        self.0.nios_expansion_gpio_dir_write(mask, val)
    }
    pub fn nios_get_fpga_version(&mut self) -> Result<SemanticVersion> {
        self.0.nios_get_fpga_version()
    }
    pub fn nios_get_iq_gain_correction(&mut self, ch: Channel) -> Result<i16> {
        self.0.nios_get_iq_gain_correction(ch)
    }
    pub fn nios_get_iq_phase_correction(&mut self, ch: Channel) -> Result<i16> {
        self.0.nios_get_iq_phase_correction(ch)
    }
    pub fn nios_set_iq_gain_correction(&mut self, ch: Channel, value: i16) -> Result<()> {
        self.0.nios_set_iq_gain_correction(ch, value)
    }
    pub fn nios_set_iq_phase_correction(&mut self, ch: Channel, value: i16) -> Result<()> {
        self.0.nios_set_iq_phase_correction(ch, value)
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
    ) -> Result<RetuneResult> {
        const RETUNE_NOW: u64 = 0x00;
        if timestamp == RETUNE_NOW {
            log::trace!("Clearing Retune Queue");
        }
        let out_buf = self.0.transport_mut().out_buffer()?;
        nios_encode_retune(
            out_buf, channel, timestamp, nint, nfrac, freqsel, vcocap, band, tune, xb_gpio,
        )?;
        let response = self.0.transport_mut().submit(None)?;
        let response_pkt = nios_decode_retune(response)?;
        if !response_pkt.is_success() {
            let is_immediate = response_pkt.duration() == RETUNE_NOW;
            return if is_immediate {
                Err(Error::TuningFailed)
            } else {
                Err(Error::RetuneQueueFull)
            };
        }
        Ok(RetuneResult {
            duration: response_pkt.duration(),
        })
    }
    pub fn nios_xb200_synth_write(&mut self, value: u32) -> Result<()> {
        self.nios_write::<u8, u32>(NiosPkt8x32Target::Adf4_351, 0, value)
    }
}
impl UsbInterfaceCommands for NiosClient {
    fn usb_vendor_cmd_int(&self, cmd: u8) -> Result<u32> {
        self.transport().usb_vendor_cmd_int(cmd)
    }
    fn usb_vendor_cmd_int_w_value(&self, cmd: u8, wvalue: u16) -> Result<u32> {
        self.transport().usb_vendor_cmd_int_w_value(cmd, wvalue)
    }
    fn usb_vendor_cmd_int_w_index(&self, cmd: u8, windex: u16) -> Result<u32> {
        self.transport().usb_vendor_cmd_int_w_index(cmd, windex)
    }
    fn usb_vendor_cmd_out_w_index(&self, cmd: u8, windex: u16, data: &[u8]) -> Result<()> {
        self.transport()
            .usb_vendor_cmd_out_w_index(cmd, windex, data)
    }
    fn usb_vendor_cmd_in_w_index_data(&self, cmd: u8, windex: u16, buf: &mut [u8]) -> Result<()> {
        self.transport()
            .usb_vendor_cmd_in_w_index_data(cmd, windex, buf)
    }
    fn usb_change_setting(&mut self, setting: u8) -> Result<()> {
        self.0.transport_mut().usb_change_setting(setting)
    }
}
impl BladeRf1UsbInterfaceCommands for NiosClient {
    fn usb_enable_module(&self, channel: Channel, enable: bool) -> Result<()> {
        self.transport().usb_enable_module(channel, enable)
    }
    fn usb_set_firmware_loopback(&mut self, enable: bool) -> Result<()> {
        self.0.transport_mut().usb_set_firmware_loopback(enable)
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
