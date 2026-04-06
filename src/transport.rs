pub mod mock;
pub mod usb;
use crate::Result;
use std::time::Duration;
pub trait Transport: Send {
    fn out_buffer(&mut self) -> Result<&mut [u8]>;
    fn submit(&mut self, timeout: Option<Duration>) -> Result<&[u8]>;
}
