//! BladeRF1-specific NIOS retune protocol.
//!
//! Handles the LMS6002D retune command encoding/decoding, including
//! timestamped (scheduled) and immediate retune operations. Re-exports
//! the retune packet types and provides convenience functions for the
//! NIOsCore layer.

mod packet_retune;
use crate::bladerf1::hardware::lms6002d::{Band, Tune};
use crate::channel::Channel;
use crate::error::{Error, Result};
use crate::protocol::nios::NiosPacketError;
pub use packet_retune::{NiosPktRetuneRequest, NiosPktRetuneResponse};

/// Duration measured in FPGA timestamp ticks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TimestampTicks(pub u64);

/// Confirmed outcome of an immediate, scheduled, or queue-clear retune request.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetuneResult {
    /// An immediate tune completed with valid measurement fields.
    Immediate {
        /// Measured duration in timestamp ticks, possibly zero if timestamps are stopped.
        duration: TimestampTicks,
        /// VCO capacitor code selected by the FPGA.
        vcocap: u8,
    },
    /// A future retune was accepted into the queue.
    Scheduled,
    /// Pending retunes were cleared.
    QueueCleared,
}
impl RetuneResult {
    /// Validates a response against the kind of retune requested.
    ///
    /// # Errors
    /// Returns a protocol error for malformed responses, `TuningFailed` for immediate
    /// tune failures, or `RetuneQueueFull` when a scheduled request fails.
    pub fn decode(request: RetuneTimestamp, bytes: &[u8]) -> Result<Self> {
        let response = nios_decode_retune(bytes)?;
        if !response.is_success() {
            return Err(match request {
                RetuneTimestamp::Scheduled(_) => Error::RetuneQueueFull,
                RetuneTimestamp::Now | RetuneTimestamp::ClearQueue => Error::TuningFailed,
            });
        }
        match request {
            RetuneTimestamp::Now if response.vcocap_valid() => Ok(Self::Immediate {
                duration: TimestampTicks(response.duration()),
                vcocap: response.vcocap(),
            }),
            RetuneTimestamp::Now => Err(NiosPacketError::ResponseMismatch.into()),
            RetuneTimestamp::Scheduled(_) => Ok(Self::Scheduled),
            RetuneTimestamp::ClearQueue => Ok(Self::QueueCleared),
        }
    }

    /// Returns the duration only for a measured immediate retune.
    pub fn duration(self) -> Option<TimestampTicks> {
        match self {
            Self::Immediate { duration, .. } => Some(duration),
            Self::Scheduled | Self::QueueCleared => None,
        }
    }
}

/// Timestamping mode for a retune request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetuneTimestamp {
    /// Execute the retune immediately.
    Now,
    /// Clear pending retunes without applying another frequency change.
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
    if matches!(timestamp, RetuneTimestamp::Scheduled(0 | u64::MAX)) {
        return Err(Error::Argument(
            "reserved scheduled-retune timestamp".into(),
        ));
    }
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
