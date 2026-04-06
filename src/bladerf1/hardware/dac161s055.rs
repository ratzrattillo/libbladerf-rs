use crate::bladerf1::nios_client::NiosInterface;
use crate::error::Result;
use crate::protocol::nios::NiosPkt8x16Target;
use std::sync::{Arc, Mutex};
#[derive(Clone)]
pub struct DAC161S055 {
    interface: Arc<Mutex<NiosInterface>>,
}
impl DAC161S055 {
    pub fn new(interface: Arc<Mutex<NiosInterface>>) -> Self {
        Self { interface }
    }
    pub fn write(&self, value: u16) -> Result<()> {
        let mut interface = self.interface.lock().unwrap();
        interface.nios_write::<u8, u16>(NiosPkt8x16Target::VctcxoDac, 0x28, 0x0u16)?;
        interface.nios_write::<u8, u16>(NiosPkt8x16Target::VctcxoDac, 0x8, value)
    }
}
