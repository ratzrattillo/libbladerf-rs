use crate::Result;
use crate::bladerf1::BladeRf1;
use crate::hardware::si5338::{BLADERF_SAMPLERATE_MIN, BLADERF_SAMPLERATE_REC_MAX, RationalRate};
use crate::range::{Range, RangeItem};

impl BladeRf1 {
    pub fn set_sample_rate(&self, channel: u8, rate: u32) -> Result<u32> {
        // CHECK_BOARD_STATE(STATE_INITIALIZED);
        self.si5338.set_sample_rate(channel, rate)
    }

    pub fn get_sample_rate(&self, channel: u8) -> Result<u32> {
        // CHECK_BOARD_STATE(STATE_INITIALIZED);
        self.si5338.get_sample_rate(channel)
    }

    pub fn get_sample_rate_range() -> Range {
        Range {
            items: vec![RangeItem::Step(
                BLADERF_SAMPLERATE_MIN as f64,
                BLADERF_SAMPLERATE_REC_MAX as f64,
                1f64,
                1f64,
            )],
        }
    }

    pub fn set_rational_sample_rate(
        &self,
        channel: u8,
        rate: &mut RationalRate,
    ) -> Result<RationalRate> {
        // CHECK_BOARD_STATE(STATE_INITIALIZED);
        self.si5338.set_rational_sample_rate(channel, rate)
    }

    pub fn get_rational_sample_rate(&self, channel: u8) -> Result<RationalRate> {
        // CHECK_BOARD_STATE(STATE_INITIALIZED);
        self.si5338.get_rational_sample_rate(channel)
    }
}
