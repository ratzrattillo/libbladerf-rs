pub mod packet_retune;

use crate::{Channel, Result};

// Re-export retune2 packet types
pub use packet_retune::{NiosPktRetuneRequest, NiosPktRetuneResponse};

pub struct NiosProtocolBladeRf2;

impl NiosProtocolBladeRf2 {
    #[allow(clippy::too_many_arguments)]
    pub fn encode_retune(
        buf: Vec<u8>,
        channel: Channel,
        timestamp: u64,
        nios_profile: u16,
        rffe_profile: u8,
        port: u8,
        spdt: u8,
    ) -> NiosPktRetuneRequest {
        NiosPktRetuneRequest::try_from(buf)
            .expect("buffer should be valid")
            .prepare(channel, timestamp, nios_profile, rffe_profile, port, spdt)
    }

    pub fn decode_retune(response: Vec<u8>) -> Result<NiosPktRetuneResponse> {
        NiosPktRetuneResponse::try_from(response)
    }
}
