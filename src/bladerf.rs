#![allow(dead_code)]
use crate::Error;

///  Stream direction
#[derive(PartialEq, Debug, Clone, Copy)]
#[repr(u8)]
pub enum Channel {
    Rx = 0, // Receive1
    Tx = 1, // Transmit1
}

impl Channel {
    pub fn is_tx(&self) -> bool {
        *self == Channel::Tx
    }
}

impl TryFrom<u8> for Channel {
    type Error = Error;
    fn try_from(value: u8) -> crate::Result<Self> {
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

// #[macro_export]
macro_rules! khz {
    ($value:expr) => {
        ($value * 1000u32)
    };
}
pub(crate) use khz;

// #[macro_export]
macro_rules! mhz {
    ($value:expr) => {
        ($value * 1000000u32)
    };
}
pub(crate) use mhz;

// #[macro_export]
// macro_rules! ghz {
//     ($value:expr) => {
//         ($value * 1000000000u32)
//     };
// }
// pub(crate) use ghz;

///  Stream direction
#[derive(PartialEq, Debug, Clone, Copy)]
#[repr(u8)]
pub enum Direction {
    Rx = 0, // Receive direction
    Tx = 1, // Transmit direction
}
