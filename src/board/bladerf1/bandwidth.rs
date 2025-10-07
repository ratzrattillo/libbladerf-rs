use crate::Result;
use crate::bladerf1::BladeRf1;
use crate::hardware::lms6002d::{
    BLADERF_BANDWIDTH_MAX, BLADERF_BANDWIDTH_MIN, LmsBw, UINT_BANDWIDTHS,
};
use crate::range::{Range, RangeItem};

impl BladeRf1 {
    /// Set the bandwidth for a specific channel.
    /// The bandwidth is automatically clamped to supported values, which
    /// can also be determined by calling `get_bandwidth_range()`.
    pub fn set_bandwidth(&self, channel: u8, mut bandwidth: u32) -> Result<()> {
        // CHECK_BOARD_STATE(STATE_INITIALIZED);

        // TODO: Can we use get_bandwidth_range here for clamping?
        bandwidth = bandwidth.clamp(BLADERF_BANDWIDTH_MIN, BLADERF_BANDWIDTH_MAX);
        log::trace!("Clamped bandwidth to {bandwidth}");

        let bw: LmsBw = bandwidth.into();

        self.lms.lpf_enable(channel, true)?;

        self.lms.set_bandwidth(channel, bw)?;
        Ok(())
    }

    /// Get the current bandwidth on a specific channel
    pub fn get_bandwidth(&self, channel: u8) -> Result<u32> {
        // CHECK_BOARD_STATE(STATE_INITIALIZED);

        let bw: LmsBw = self.lms.get_bandwidth(channel)?;
        Ok(bw.into())
    }

    /// Retrieve the supported bandwidth range
    pub fn get_bandwidth_range() -> Range {
        let v = UINT_BANDWIDTHS
            .iter()
            .rev()
            .map(|bw| RangeItem::Value(*bw as f64))
            .collect();

        Range::new(v)
    }
}
