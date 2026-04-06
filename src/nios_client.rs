use crate::channel::Channel;
use crate::error::Result;
use crate::protocol::nios::packet_generic::NiosNum;
use crate::protocol::nios::{
    NiosPkt8x16AddrIqCorr, NiosPkt8x16Target, NiosPkt8x32Target, NiosPkt32x32Target,
    nios_decode_read, nios_decode_write, nios_encode_read, nios_encode_write,
};
use crate::transport::Transport;
use crate::transport::usb::UsbTransport;
use crate::version::SemanticVersion;
pub struct NiosCore<T: Transport> {
    transport: T,
}
impl<T: Transport> NiosCore<T> {
    pub fn new(transport: T) -> Self {
        Self { transport }
    }
    pub fn transport(&self) -> &T {
        &self.transport
    }
    pub fn transport_mut(&mut self) -> &mut T {
        &mut self.transport
    }
    pub fn nios_read<A: NiosNum + Send, D: NiosNum + Send>(
        &mut self,
        id: impl Into<u8>,
        addr: A,
    ) -> Result<D> {
        let out_buf = self.transport.out_buffer()?;
        log::trace!("nios_read: DMA buffer len = {} bytes", out_buf.len());
        nios_encode_read::<A, D>(out_buf, id.into(), addr);
        let response = self.transport.submit(None)?;
        log::trace!("nios_read: response len = {} bytes", response.len());
        nios_decode_read::<A, D>(response)
    }
    pub fn nios_write<A: NiosNum + Send, D: NiosNum + Send>(
        &mut self,
        id: impl Into<u8>,
        addr: A,
        data: D,
    ) -> Result<()> {
        let out_buf = self.transport.out_buffer()?;
        nios_encode_write::<A, D>(out_buf, id.into(), addr, data);
        let response = self.transport.submit(None)?;
        nios_decode_write(response)
    }
    pub fn nios_config_read(&mut self) -> Result<u32> {
        self.nios_read::<u8, u32>(NiosPkt8x32Target::Control, 0)
    }
    pub fn nios_config_write(&mut self, value: u32) -> Result<()> {
        self.nios_write::<u8, u32>(NiosPkt8x32Target::Control, 0, value)
    }
    pub fn nios_expansion_gpio_read(&mut self) -> Result<u32> {
        self.nios_read::<u32, u32>(NiosPkt32x32Target::Exp, u32::MAX)
    }
    pub fn nios_expansion_gpio_write(&mut self, mask: u32, val: u32) -> Result<()> {
        self.nios_write::<u32, u32>(NiosPkt32x32Target::Exp, mask, val)
    }
    pub fn nios_expansion_gpio_dir_read(&mut self) -> Result<u32> {
        self.nios_read::<u32, u32>(NiosPkt32x32Target::ExpDir, u32::MAX)
    }
    pub fn nios_expansion_gpio_dir_write(&mut self, mask: u32, val: u32) -> Result<()> {
        self.nios_write::<u32, u32>(NiosPkt32x32Target::ExpDir, mask, val)
    }
    pub fn nios_get_fpga_version(&mut self) -> Result<SemanticVersion> {
        let regval = self.nios_read::<u8, u32>(NiosPkt8x32Target::Version, 0)?;
        log::trace!("Read FPGA version word: {regval:#010x}");
        let version = SemanticVersion {
            major: ((regval >> 24) & 0xff) as u16,
            minor: ((regval >> 16) & 0xff) as u16,
            patch: ((regval & 0xffff) as u16).to_be(),
        };
        Ok(version)
    }
    pub fn nios_get_iq_gain_correction(&mut self, ch: Channel) -> Result<i16> {
        let addr = match ch {
            Channel::Rx => NiosPkt8x16AddrIqCorr::RxGain,
            Channel::Tx => NiosPkt8x16AddrIqCorr::TxGain,
        };
        Ok(self.nios_read::<u8, u16>(NiosPkt8x16Target::IqCorr, addr.into())? as i16)
    }
    pub fn nios_get_iq_phase_correction(&mut self, ch: Channel) -> Result<i16> {
        let addr = match ch {
            Channel::Rx => NiosPkt8x16AddrIqCorr::RxPhase,
            Channel::Tx => NiosPkt8x16AddrIqCorr::TxPhase,
        };
        Ok(self.nios_read::<u8, u16>(NiosPkt8x16Target::IqCorr, addr.into())? as i16)
    }
    pub fn nios_set_iq_gain_correction(&mut self, ch: Channel, value: i16) -> Result<()> {
        let addr = match ch {
            Channel::Rx => NiosPkt8x16AddrIqCorr::RxGain,
            Channel::Tx => NiosPkt8x16AddrIqCorr::TxGain,
        };
        self.nios_write::<u8, u16>(NiosPkt8x16Target::IqCorr, addr.into(), value as u16)
    }
    pub fn nios_set_iq_phase_correction(&mut self, ch: Channel, value: i16) -> Result<()> {
        let addr = match ch {
            Channel::Rx => NiosPkt8x16AddrIqCorr::RxPhase,
            Channel::Tx => NiosPkt8x16AddrIqCorr::TxPhase,
        };
        self.nios_write::<u8, u16>(NiosPkt8x16Target::IqCorr, addr.into(), value as u16)
    }
}
impl NiosCore<UsbTransport> {
    pub fn get_alt_setting(&self) -> u8 {
        self.transport.interface().get_alt_setting()
    }
}
