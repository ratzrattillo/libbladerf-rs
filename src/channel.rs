use crate::{Error, Result};

/// RF channel direction.
///
/// BladeRF1 has one RX channel and one TX channel operating in half-duplex
/// or full-duplex mode depending on the streaming configuration.
#[derive(PartialEq, Debug, Clone, Copy)]
#[repr(u8)]
pub enum Channel {
    /// Receive channel.
    Rx = 0,
    /// Transmit channel.
    Tx = 1,
}

impl Channel {
    /// Returns `true` if this is the TX channel.
    pub fn is_tx(&self) -> bool {
        *self == Channel::Tx
    }

    /// Returns `true` if this is the RX channel.
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
            _ => Err(Error::Argument("invalid channel value".into())),
        }
    }
}
