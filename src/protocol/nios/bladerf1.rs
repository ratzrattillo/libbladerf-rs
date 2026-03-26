pub mod packet_retune;

use crate::{Band, Channel, Result, Tune};

// Re-export retune packet types
pub use packet_retune::{NiosPktRetuneRequest, NiosPktRetuneResponse};

pub struct NiosProtocolBladeRf1;

impl NiosProtocolBladeRf1 {
    #[allow(clippy::too_many_arguments)]
    pub fn encode_retune(
        buf: Vec<u8>,
        channel: Channel,
        timestamp: u64,
        nint: u16,
        nfrac: u32,
        freqsel: u8,
        vcocap: u8,
        band: Band,
        tune: Tune,
        xb_gpio: u8,
    ) -> Result<NiosPktRetuneRequest> {
        NiosPktRetuneRequest::try_from(buf)?.prepare(
            channel, timestamp, nint, nfrac, freqsel, vcocap, band, tune, xb_gpio,
        )
    }

    pub fn decode_retune(response: Vec<u8>) -> Result<NiosPktRetuneResponse> {
        NiosPktRetuneResponse::try_from(response)
    }
}
