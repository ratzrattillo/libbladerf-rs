use crate::BladeRf1;
use crate::hardware::lms6002d::LmsBw;
use anyhow::Result;
use bladerf_globals::SdrRange;
use bladerf_globals::bladerf1::{BLADERF_BANDWIDTH_MAX, BLADERF_BANDWIDTH_MIN};

impl BladeRf1 {
    pub async fn set_bandwidth(&self, channel: u8, mut bandwidth: u32) -> Result<()> {
        //CHECK_BOARD_STATE(STATE_INITIALIZED);

        bandwidth = bandwidth.clamp(BLADERF_BANDWIDTH_MIN, BLADERF_BANDWIDTH_MAX);
        log::debug!("Clamped bandwidth to {bandwidth}");

        let bw: LmsBw = bandwidth.into();

        self.lms.lpf_enable(channel, true).await?;

        self.lms.set_bandwidth(channel, bw).await?;
        Ok(())
    }

    pub async fn get_bandwidth(&self, channel: u8) -> Result<u32> {
        //CHECK_BOARD_STATE(STATE_INITIALIZED);

        let bw: LmsBw = self.lms.get_bandwidth(channel).await?;
        Ok(bw.into())
    }

    pub fn get_bandwidth_range() -> SdrRange<u32> {
        SdrRange::<u32> {
            min: BLADERF_BANDWIDTH_MIN,
            max: BLADERF_BANDWIDTH_MAX,
            step: 1,
            scale: 1,
        }
    }
}
