use crate::Result;
use crate::bladerf1::BladeRf1;
use crate::hardware::lms6002d::{
    BLADERF_BANDWIDTH_MAX, BLADERF_BANDWIDTH_MIN, LmsBw, UINT_BANDWIDTHS,
};
use crate::range::{Range, RangeItem};

impl BladeRf1 {
    pub fn set_bandwidth(&self, channel: u8, mut bandwidth: u32) -> Result<()> {
        // CHECK_BOARD_STATE(STATE_INITIALIZED);

        bandwidth = bandwidth.clamp(BLADERF_BANDWIDTH_MIN, BLADERF_BANDWIDTH_MAX);
        log::trace!("Clamped bandwidth to {bandwidth}");

        let bw: LmsBw = bandwidth.into();

        self.lms.lpf_enable(channel, true)?;

        self.lms.set_bandwidth(channel, bw)?;
        Ok(())
    }

    pub fn get_bandwidth(&self, channel: u8) -> Result<u32> {
        // CHECK_BOARD_STATE(STATE_INITIALIZED);

        let bw: LmsBw = self.lms.get_bandwidth(channel)?;
        Ok(bw.into())
    }

    pub fn get_bandwidth_range() -> Range {
        let v = UINT_BANDWIDTHS
            .iter()
            .rev()
            .map(|bw| RangeItem::Value(*bw as f64))
            .collect();

        Range::new(v)
    }
}
