use crate::BladeRf1;
use crate::Result;
use crate::hardware::lms6002d::LmsBw;
// use bladerf_globals::SdrRange;
use bladerf_globals::bladerf1::{BLADERF_BANDWIDTH_MAX, BLADERF_BANDWIDTH_MIN};
use bladerf_globals::range::{Range, RangeItem};
use bladerf_globals::{khz, mhz};

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
        /// LPF conversion table
        /// This table can be indexed into.
        pub const UINT_BANDWIDTHS: [u32; 16] = [
            mhz!(28),
            mhz!(20),
            mhz!(14),
            mhz!(12),
            mhz!(10),
            khz!(8750),
            mhz!(7),
            mhz!(6),
            khz!(5500),
            mhz!(5),
            khz!(3840),
            mhz!(3),
            khz!(2750),
            khz!(2500),
            khz!(1750),
            khz!(1500),
        ];

        let v = UINT_BANDWIDTHS
            .iter()
            .rev()
            .map(|bw| RangeItem::Value(*bw as f64))
            .collect();

        Range::new(v)
    }
}
