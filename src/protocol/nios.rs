pub mod bladerf1;
pub mod bladerf2;
pub mod packet_generic;
pub mod targets;

use crate::Error;
use crate::protocol::nios::packet_generic::NiosNum;

pub use packet_generic::{NiosPacket, NiosPkt, NiosPktFlags, NiosPktStatus};
pub use targets::{
    NiosPkt8x8Target, NiosPkt8x16AddrIqCorr, NiosPkt8x16Target, NiosPkt8x32Target,
    NiosPkt32x32Target,
};

pub use bladerf1::{NiosPktRetuneRequest, NiosPktRetuneResponse};

pub struct NiosProtocol;

impl NiosProtocol {
    pub const MIN_RESPONSE_SIZE: usize = 16;

    pub fn encode_read<A: NiosNum, D: NiosNum>(buf: Vec<u8>, target: u8, addr: A) -> NiosPkt<A, D> {
        NiosPkt::try_from(buf)
            .expect("buffer should be valid")
            .prepare_read(target, addr)
    }

    pub fn encode_write<A: NiosNum, D: NiosNum>(
        buf: Vec<u8>,
        target: u8,
        addr: A,
        data: D,
    ) -> NiosPkt<A, D> {
        NiosPkt::try_from(buf)
            .expect("buffer should be valid")
            .prepare_write(target, addr, data)
    }

    pub fn decode_read<A: NiosNum, D: NiosNum>(response: Vec<u8>) -> Result<(D, Vec<u8>), Error> {
        let pkt = NiosPkt::<A, D>::try_from(response)?;
        let data = pkt.data();
        let buf = pkt.into_inner();
        Ok((data, buf))
    }

    pub fn decode_write<A: NiosNum, D: NiosNum>(response: Vec<u8>) -> Result<Vec<u8>, Error> {
        let pkt = NiosPkt::<A, D>::try_from(response)?;
        let success = pkt.is_success();
        let buf = pkt.into_inner();
        if success {
            Ok(buf)
        } else {
            Err(Error::NiosWriteFailed)
        }
    }
}
