use crate::error::Result;
use crate::transport::Transport;
use std::time::Duration;
pub struct MockTransport {
    out_buf: [u8; 16],
    in_buf: [u8; 16],
}
impl MockTransport {
    pub fn new() -> Self {
        Self {
            out_buf: [0u8; 16],
            in_buf: [0u8; 16],
        }
    }
    pub fn set_response(&mut self, response: [u8; 16]) {
        self.in_buf = response;
    }
    pub fn last_request(&self) -> &[u8; 16] {
        &self.out_buf
    }
}
impl Default for MockTransport {
    fn default() -> Self {
        Self::new()
    }
}
impl Transport for MockTransport {
    fn out_buffer(&mut self) -> Result<&mut [u8]> {
        self.out_buf.fill(0);
        Ok(&mut self.out_buf)
    }
    fn submit(&mut self, _timeout: Option<Duration>) -> Result<&[u8]> {
        Ok(&self.in_buf)
    }
}
impl std::fmt::Debug for MockTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MockTransport")
            .field("out_buf", &self.out_buf)
            .field("in_buf", &self.in_buf)
            .finish()
    }
}
