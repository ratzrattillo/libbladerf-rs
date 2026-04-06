mod packet_retune;
use crate::bladerf1::hardware::lms6002d::{Band, Tune};
use crate::channel::Channel;
use crate::error::Result;
pub use packet_retune::{NiosPktRetuneRequest, NiosPktRetuneResponse};
#[allow(clippy::too_many_arguments)]
pub fn nios_encode_retune(
    buf: &mut [u8],
    channel: Channel,
    timestamp: u64,
    nint: u16,
    nfrac: u32,
    freqsel: u8,
    vcocap: u8,
    band: Band,
    tune: Tune,
    xb_gpio: u8,
) -> Result<()> {
    NiosPktRetuneRequest::new(buf).prepare(
        channel, timestamp, nint, nfrac, freqsel, vcocap, band, tune, xb_gpio,
    )
}
pub fn nios_decode_retune(response: &[u8]) -> Result<NiosPktRetuneResponse<'_>> {
    NiosPktRetuneResponse::new(response)
}
