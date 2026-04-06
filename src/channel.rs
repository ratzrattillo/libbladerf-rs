use crate::{Error, Result};
#[derive(PartialEq, Debug, Clone, Copy)]
#[repr(u8)]
pub enum Channel {
    Rx = 0,
    Tx = 1,
}
impl Channel {
    pub fn is_tx(&self) -> bool {
        *self == Channel::Tx
    }
    pub fn is_rx(&self) -> bool {
        *self == Channel::Rx
    }
}
impl TryFrom<u8> for Channel {
    type Error = Error;
    fn try_from(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Channel::Rx),
            1 => Ok(Channel::Tx),
            _ => {
                log::error!("unsupported channel!");
                Err(Error::Invalid)
            }
        }
    }
}
