//! Timestamp read access for RX and TX channels.
//!
//! Exposes the per-channel NIOS timestamp counter, which tickts at the
//! reference clock rate. Timestamps are used for stream sample alignment
//! and latency measurement.

use crate::bladerf1::board::RfLinkSession;
use crate::channel::Channel;
use crate::error::Result;

impl RfLinkSession<'_> {
    /// Reads the 64-bit timestamp counter for the given channel.
    ///
    /// The counter increments at the reference clock rate. Use this value
    /// alongside stream metadata timestamps to compute latency or correlate
    /// RX/TX samples.
    ///
    /// Returns `Error::BoardState` if the board is not initialized.
    pub fn get_timestamp(&mut self, channel: Channel) -> Result<u64> {
        self.require_initialized()?;
        self.nios.nios_get_timestamp(channel)
    }
}
