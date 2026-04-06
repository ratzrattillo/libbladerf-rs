use crate::bladerf1::BladeRf1;
use crate::bladerf1::hardware::lms6002d::LMS6002D;
use crate::bladerf1::hardware::lms6002d::bandwidth::LmsBandwidth;
use crate::channel::Channel;
use crate::error::Result;
use crate::range::Range;
impl BladeRf1 {
    pub fn set_bandwidth(&self, channel: Channel, mut bandwidth: u32) -> Result<()> {
        let bandwidth_range = LMS6002D::get_bandwidth_range();
        bandwidth = bandwidth.clamp(
            bandwidth_range.min().unwrap() as u32,
            bandwidth_range.max().unwrap() as u32,
        );
        log::trace!("Clamped bandwidth to {bandwidth}");
        let bw: LmsBandwidth = bandwidth.into();
        self.lms.lpf_enable(channel, true)?;
        self.lms.set_bandwidth(channel, bw)?;
        Ok(())
    }
    pub fn get_bandwidth(&self, channel: Channel) -> Result<u32> {
        let bw: LmsBandwidth = self.lms.get_bandwidth(channel)?;
        Ok(bw.into())
    }
    pub fn get_bandwidth_range() -> Range {
        LMS6002D::get_bandwidth_range()
    }
}
