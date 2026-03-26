pub mod usb;

use crate::Result;
use std::time::Duration;

pub trait Transport: Send {
    fn transact(&mut self, request: Vec<u8>, timeout: Option<Duration>) -> Result<Vec<u8>>;
}
