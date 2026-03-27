use crate::Channel;
use crate::hardware::lms6002d::{Band, Tune};
use crate::nios2::{NiosNum, NiosPacket};
use crate::protocol::nios::bladerf1::NiosProtocolBladeRf1;
use crate::protocol::nios::{
    NiosPkt8x16AddrIqCorr, NiosPkt8x16Target, NiosPkt8x32Target, NiosPkt32x32Target, NiosProtocol,
};
use crate::transport::usb::{BladeRf1Commands, InterfaceCommands};
use crate::{Direction, Error, Result, SemanticVersion};
use nusb::Interface;
use std::time::Duration;

pub type NiosInterface = NiosClient<UsbTransport>;

impl From<Interface> for NiosInterface {
    fn from(interface: Interface) -> Self {
        NiosClient::new(UsbTransport::new(interface))
    }
}

pub struct NiosClient<T: Transport> {
    transport: T,
    buf: Vec<u8>,
}

impl<T: Transport> NiosClient<T> {
    pub fn new(transport: T) -> Self {
        Self {
            transport,
            buf: vec![0; 1024], // Reusable buffer for zero-copy
        }
    }

    fn take_buf(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.buf)
    }

    fn return_buf(&mut self, buf: Vec<u8>) {
        self.buf = buf;
    }

    fn transact(&mut self, request: Vec<u8>, timeout: Option<Duration>) -> Result<Vec<u8>> {
        self.transport.transact(request, timeout)
    }

    pub fn transport(&self) -> &T {
        &self.transport
    }

    pub fn transport_mut(&mut self) -> &mut T {
        &mut self.transport
    }
}

impl NiosClient<UsbTransport> {
    pub fn get_alt_setting(&self) -> u8 {
        self.transport.interface().get_alt_setting()
    }
}

pub trait Nios: Send {
    #[allow(clippy::too_many_arguments)]
    fn nios_retune(
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
    ) -> Result<()>;

    fn nios_read<A: NiosNum + Send, D: NiosNum + Send>(
        &mut self,
        id: impl Into<u8>,
        addr: A,
    ) -> Result<D>;

    fn nios_write<A: NiosNum + Send, D: NiosNum + Send>(
        &mut self,
        id: impl Into<u8>,
        addr: A,
        data: D,
    ) -> Result<()>;

    fn nios_config_read(&mut self) -> Result<u32>;

    fn nios_config_write(&mut self, value: u32) -> Result<()>;

    fn nios_xb200_synth_write(&mut self, value: u32) -> Result<()>;

    fn nios_expansion_gpio_read(&mut self) -> Result<u32>;

    fn nios_expansion_gpio_write(&mut self, mask: u32, val: u32) -> Result<()>;

    fn nios_expansion_gpio_dir_read(&mut self) -> Result<u32>;

    fn nios_expansion_gpio_dir_write(&mut self, mask: u32, val: u32) -> Result<()>;

    fn nios_get_fpga_version(&mut self) -> Result<SemanticVersion>;

    fn nios_get_iq_gain_correction(&mut self, ch: Channel) -> Result<i16>;

    fn nios_get_iq_phase_correction(&mut self, ch: Channel) -> Result<i16>;

    fn nios_set_iq_gain_correction(&mut self, ch: Channel, value: i16) -> Result<()>;

    fn nios_set_iq_phase_correction(&mut self, ch: Channel, value: i16) -> Result<()>;
}

// Re-export NiosNum from packet_generic
use crate::transport::Transport;
use crate::transport::usb::UsbTransport;

impl<T: Transport> Nios for NiosClient<T> {
    fn nios_retune(
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

        // Encode request
        let request = NiosProtocolBladeRf1::encode_retune(
            self.take_buf(),
            channel,
            timestamp,
            nint,
            nfrac,
            freqsel,
            vcocap,
            band,
            tune,
            xb_gpio,
        )?;

        // Send and receive
        let response_buf = self.transact(request.into_packet(), None)?;

        // Decode response
        let response = NiosProtocolBladeRf1::decode_retune(response_buf)?;

        if !response.is_success() {
            let is_immediate = response.duration() == RETUNE_NOW;
            self.return_buf(response.into_inner());
            return if is_immediate {
                Err(Error::TuningFailed)
            } else {
                Err(Error::RetuneQueueFull)
            };
        }

        self.return_buf(response.into_inner());
        Ok(())
    }

    fn nios_read<A: NiosNum + Send, D: NiosNum + Send>(
        &mut self,
        id: impl Into<u8>,
        addr: A,
    ) -> Result<D> {
        // Encode request
        let request = NiosProtocol::encode_read::<A, D>(self.take_buf(), id.into(), addr);
        log::trace!("{request:?}");

        // Send and receive
        let response_buf = self.transact(request.into_packet(), None)?;
        log::trace!("response buf: {} bytes", response_buf.len());

        // Decode response
        let (data, buf) = NiosProtocol::decode_read::<A, D>(response_buf)?;
        self.return_buf(buf);
        Ok(data)
    }

    fn nios_write<A: NiosNum + Send, D: NiosNum + Send>(
        &mut self,
        id: impl Into<u8>,
        addr: A,
        data: D,
    ) -> Result<()> {
        // Encode request
        let request = NiosProtocol::encode_write::<A, D>(self.take_buf(), id.into(), addr, data);

        // Send and receive
        let response_buf = self.transact(request.into_packet(), None)?;

        // Decode response
        let buf = NiosProtocol::decode_write::<A, D>(response_buf)?;
        self.return_buf(buf);
        Ok(())
    }

    fn nios_config_read(&mut self) -> Result<u32> {
        self.nios_read::<u8, u32>(NiosPkt8x32Target::Control, 0)
    }

    fn nios_config_write(&mut self, value: u32) -> Result<()> {
        self.nios_write::<u8, u32>(NiosPkt8x32Target::Control, 0, value)
    }

    fn nios_xb200_synth_write(&mut self, value: u32) -> Result<()> {
        self.nios_write::<u8, u32>(NiosPkt8x32Target::Adf4351, 0, value)
    }

    fn nios_expansion_gpio_read(&mut self) -> Result<u32> {
        self.nios_read::<u32, u32>(NiosPkt32x32Target::Exp, u32::MAX)
    }

    fn nios_expansion_gpio_write(&mut self, mask: u32, val: u32) -> Result<()> {
        self.nios_write::<u32, u32>(NiosPkt32x32Target::Exp, mask, val)
    }

    fn nios_expansion_gpio_dir_read(&mut self) -> Result<u32> {
        self.nios_read::<u32, u32>(NiosPkt32x32Target::ExpDir, u32::MAX)
    }

    fn nios_expansion_gpio_dir_write(&mut self, mask: u32, val: u32) -> Result<()> {
        self.nios_write::<u32, u32>(NiosPkt32x32Target::ExpDir, mask, val)
    }

    fn nios_get_fpga_version(&mut self) -> Result<SemanticVersion> {
        let regval = self.nios_read::<u8, u32>(NiosPkt8x32Target::Version, 0)?;
        log::trace!("Read FPGA version word: {regval:#010x}");

        let version = SemanticVersion {
            major: ((regval >> 24) & 0xff) as u16,
            minor: ((regval >> 16) & 0xff) as u16,
            patch: ((regval & 0xffff) as u16).to_be(),
        };
        Ok(version)
    }

    fn nios_get_iq_gain_correction(&mut self, ch: Channel) -> Result<i16> {
        let addr = match ch {
            Channel::Rx => NiosPkt8x16AddrIqCorr::RxGain,
            Channel::Tx => NiosPkt8x16AddrIqCorr::TxGain,
        };
        Ok(self.nios_read::<u8, u16>(NiosPkt8x16Target::IqCorr, addr.into())? as i16)
    }

    fn nios_get_iq_phase_correction(&mut self, ch: Channel) -> Result<i16> {
        let addr = match ch {
            Channel::Rx => NiosPkt8x16AddrIqCorr::RxPhase,
            Channel::Tx => NiosPkt8x16AddrIqCorr::TxPhase,
        };
        Ok(self.nios_read::<u8, u16>(NiosPkt8x16Target::IqCorr, addr.into())? as i16)
    }

    fn nios_set_iq_gain_correction(&mut self, ch: Channel, value: i16) -> Result<()> {
        let addr = match ch {
            Channel::Rx => NiosPkt8x16AddrIqCorr::RxGain,
            Channel::Tx => NiosPkt8x16AddrIqCorr::TxGain,
        };
        self.nios_write::<u8, u16>(NiosPkt8x16Target::IqCorr, addr.into(), value as u16)
    }

    fn nios_set_iq_phase_correction(&mut self, ch: Channel, value: i16) -> Result<()> {
        let addr = match ch {
            Channel::Rx => NiosPkt8x16AddrIqCorr::RxPhase,
            Channel::Tx => NiosPkt8x16AddrIqCorr::TxPhase,
        };
        self.nios_write::<u8, u16>(NiosPkt8x16Target::IqCorr, addr.into(), value as u16)
    }
}

// Implement UsbInterfaceCommands for NiosClient<UsbTransport>
impl InterfaceCommands for NiosClient<UsbTransport> {
    fn usb_vendor_cmd_int(&self, cmd: u8) -> Result<u32> {
        self.transport.usb_vendor_cmd_int(cmd)
    }

    fn usb_vendor_cmd_int_wvalue(&self, cmd: u8, wvalue: u16) -> Result<u32> {
        self.transport.usb_vendor_cmd_int_wvalue(cmd, wvalue)
    }

    fn usb_change_setting(&mut self, setting: u8) -> Result<()> {
        self.transport.usb_change_setting(setting)
    }

    fn usb_set_configuration(&self, configuration: u16) -> Result<()> {
        self.transport.usb_set_configuration(configuration)
    }
}

// Implement BladeRf1Commands for NiosClient<UsbTransport>
impl BladeRf1Commands for NiosClient<UsbTransport> {
    fn usb_enable_module(&self, direction: Direction, enable: bool) -> Result<()> {
        self.transport.usb_enable_module(direction, enable)
    }

    fn usb_set_firmware_loopback(&mut self, enable: bool) -> Result<()> {
        self.transport.usb_set_firmware_loopback(enable)
    }

    fn usb_get_firmware_loopback(&self) -> Result<bool> {
        self.transport.usb_get_firmware_loopback()
    }

    fn usb_device_reset(&self) -> Result<()> {
        self.transport.usb_device_reset()
    }

    fn usb_is_firmware_ready(&self) -> Result<bool> {
        self.transport.usb_is_firmware_ready()
    }
}
