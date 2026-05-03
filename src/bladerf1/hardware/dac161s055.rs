use crate::bladerf1::nios_client::NiosClient;
use crate::error::Result;
use crate::protocol::nios::NiosPkt8x16Target;

pub fn write(nios: &mut NiosClient, value: u16) -> Result<()> {
    nios.nios_write::<u8, u16>(NiosPkt8x16Target::VctcxoDac, 0x28, 0x0u16)?;
    nios.nios_write::<u8, u16>(NiosPkt8x16Target::VctcxoDac, 0x8, value)
}

pub fn read(nios: &mut NiosClient) -> Result<u16> {
    nios.nios_read::<u8, u16>(NiosPkt8x16Target::VctcxoDac, 0x98)
}
