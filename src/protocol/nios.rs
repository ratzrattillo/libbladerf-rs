pub mod packet_generic;
pub mod targets;
use crate::error::Error;
use crate::protocol::nios::packet_generic::{NiosNum, NiosPktDecoder};
pub use packet_generic::{NiosPacket, NiosPkt, NiosPktFlags, NiosPktStatus};
pub use targets::{
    NiosPkt8x8Target, NiosPkt8x16AddrIqCorr, NiosPkt8x16Target, NiosPkt8x32Target,
    NiosPkt32x32Target,
};
#[derive(thiserror::Error, Debug)]
pub enum NiosPacketError {
    #[error("nfrac value {0} exceeds maximum 0x7FFFFF")]
    NfracOverflow(u32),
    #[error("freqsel value {0} exceeds maximum {1}")]
    FreqselOverflow(u8, u8),
    #[error("vcocap value {0} exceeds maximum {1}")]
    VcocapOverflow(u8, u8),
    #[error("invalid packet size: expected 16 bytes, got {0}")]
    InvalidSize(usize),
}
pub const MIN_RESPONSE_SIZE: usize = 16;
pub fn nios_encode_read<A: NiosNum, D: NiosNum>(buf: &mut [u8], target: u8, addr: A) {
    NiosPkt::<A, D>::new(buf).prepare_read(target, addr);
}
pub fn nios_encode_write<A: NiosNum, D: NiosNum>(buf: &mut [u8], target: u8, addr: A, data: D) {
    NiosPkt::<A, D>::new(buf).prepare_write(target, addr, data);
}
pub fn nios_decode_read<A: NiosNum, D: NiosNum>(response: &[u8]) -> Result<D, Error> {
    NiosPktDecoder::decode_data::<A, D>(response)
}
pub fn nios_decode_write(_response: &[u8]) -> Result<(), Error> {
    Ok(())
}
