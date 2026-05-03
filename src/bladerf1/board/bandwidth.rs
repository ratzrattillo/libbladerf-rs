use crate::bladerf1::BladeRf1;
use crate::bladerf1::hardware::lms6002d;
use crate::bladerf1::hardware::lms6002d::bandwidth::LmsBandwidth;
use crate::channel::Channel;
use crate::error::{Error, Result};
use crate::range::Range;
impl BladeRf1 {
    pub fn set_bandwidth(&mut self, channel: Channel, mut bandwidth: u32) -> Result<u32> {
        let bandwidth_range = lms6002d::bandwidth::get_bandwidth_range();
        bandwidth = bandwidth.clamp(
            bandwidth_range
                .min()
                .ok_or(Error::HardwareState("bandwidth range has no minimum"))? as u32,
            bandwidth_range
                .max()
                .ok_or(Error::HardwareState("bandwidth range has no maximum"))? as u32,
        );
        log::trace!("Clamped bandwidth to {bandwidth}");
        let bw: LmsBandwidth = bandwidth.into();
        lms6002d::filters::lpf_enable(&mut self.nios, channel, true)?;
        lms6002d::bandwidth::set_bandwidth(&mut self.nios, channel, bw)?;
        let actual: u32 = bw.into();
        Ok(actual)
    }
    pub fn get_bandwidth(&mut self, channel: Channel) -> Result<u32> {
        let bw: LmsBandwidth = lms6002d::bandwidth::get_bandwidth(&mut self.nios, channel)?;
        Ok(bw.into())
    }
    pub fn get_bandwidth_range() -> Range {
        lms6002d::bandwidth::get_bandwidth_range()
    }
}
