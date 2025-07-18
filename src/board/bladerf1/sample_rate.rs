use crate::BladeRf1;
use anyhow::Result;
use bladerf_globals::bladerf1::{BLADERF_SAMPLERATE_MIN, BLADERF_SAMPLERATE_REC_MAX};
use bladerf_globals::{BladerfRationalRate, SdrRange};

impl BladeRf1 {
    pub fn set_sample_rate(&self, channel: u8, rate: u32) -> Result<u32> {
        //CHECK_BOARD_STATE(STATE_INITIALIZED);
        self.si5338.set_sample_rate(channel, rate)
    }

    pub fn get_sample_rate(&self, channel: u8) -> Result<u32> {
        //CHECK_BOARD_STATE(STATE_INITIALIZED);
        self.si5338.get_sample_rate(channel)
    }

    pub fn get_sample_rate_range() -> SdrRange<u32> {
        SdrRange {
            min: BLADERF_SAMPLERATE_MIN,
            max: BLADERF_SAMPLERATE_REC_MAX,
            step: 1,
            scale: 1,
        }
    }

    pub fn set_rational_sample_rate(
        &self,
        channel: u8,
        rate: &mut BladerfRationalRate,
    ) -> Result<BladerfRationalRate> {
        //CHECK_BOARD_STATE(STATE_INITIALIZED);
        self.si5338.set_rational_sample_rate(channel, rate)
    }

    pub fn get_rational_sample_rate(&self, channel: u8) -> Result<BladerfRationalRate> {
        // CHECK_BOARD_STATE(STATE_INITIALIZED);
        self.si5338.get_rational_sample_rate(channel)
    }
}
