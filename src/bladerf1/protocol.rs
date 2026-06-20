//! BladeRF1-specific NIOS retune protocol.
//!
//! Handles the LMS6002D retune command encoding/decoding, including
//! timestamped (scheduled) and immediate retune operations. Re-exports
//! the retune packet types and provides convenience functions for the
//! NIOsCore layer.

mod packet_retune;
use crate::bladerf1::hardware::lms6002d::{Band, Tune};
use crate::channel::Channel;
use crate::error::Result;
pub use packet_retune::{NiosPktRetuneRequest, NiosPktRetuneResponse};

/// Result of a retune operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetuneResult {
    duration: u64,
}
impl RetuneResult {
    pub fn new(duration: u64) -> Self {
        Self { duration }
    }

    pub fn duration(&self) -> u64 {
        self.duration
    }
}

/// Timestamping mode for a retune request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetuneTimestamp {
    /// Execute the retune immediately.
    Now,
    /// Clear any pending retune queue entries before executing.
    ClearQueue,
    /// Schedule the retune to execute at the given hardware timestamp.
    Scheduled(u64),
}
impl From<RetuneTimestamp> for u64 {
    fn from(ts: RetuneTimestamp) -> u64 {
        match ts {
            RetuneTimestamp::Now => 0,
            RetuneTimestamp::ClearQueue => u64::MAX,
            RetuneTimestamp::Scheduled(ts) => ts,
        }
    }
}
impl From<u64> for RetuneTimestamp {
    fn from(ts: u64) -> RetuneTimestamp {
        match ts {
            0 => RetuneTimestamp::Now,
            u64::MAX => RetuneTimestamp::ClearQueue,
            _ => RetuneTimestamp::Scheduled(ts),
        }
    }
}

/// Encodes a retune request packet into `buf`.
///
/// Populates the 16-byte retune packet with the channel, timestamp,
/// synthesizer parameters (nint, nfrac, freqsel, vcocap), band
/// selection, tune mode, and expansion board GPIO value.
#[allow(clippy::too_many_arguments)]
pub fn nios_encode_retune(
    buf: &mut [u8],
    channel: Channel,
    timestamp: RetuneTimestamp,
    nint: u16,
    nfrac: u32,
    freqsel: u8,
    vcocap: u8,
    band: Band,
    tune: Tune,
    xb_gpio: u8,
) -> Result<()> {
    NiosPktRetuneRequest::new(buf)?.prepare(
        channel,
        timestamp.into(),
        nint,
        nfrac,
        freqsel,
        vcocap,
        band,
        tune,
        xb_gpio,
    )
}

/// Decodes a retune response from the device.
pub fn nios_decode_retune(response: &[u8]) -> Result<NiosPktRetuneResponse<'_>> {
    NiosPktRetuneResponse::new(response)
}
