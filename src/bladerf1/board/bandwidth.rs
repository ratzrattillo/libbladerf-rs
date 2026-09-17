//! Bandwidth and LPF control for BladeRF1.
//!
//! Configures the digital low-pass filter (LPF) bandwidth on the LMS6002D.
//! The LPF calibration and bandwidth settings follow the LMS6002D programming
//! guide, with the hardware selecting the closest calibrated filter response
//! to the requested bandwidth.

use crate::bladerf1::board::RfLinkSession;
use crate::bladerf1::hardware::lms6002d;
use crate::bladerf1::hardware::lms6002d::bandwidth::LmsBandwidth;
use crate::channel::Channel;
use crate::error::{Error, Result};
use crate::maybe_future::Op;
use crate::range::Range;
use nusb::MaybeFuture;
impl RfLinkSession<'_> {
    /// Sets the LPF bandwidth for the given channel in Hz.
    ///
    /// Clamps the requested bandwidth to the hardware-supported range,
    /// enables the LPF, and programs the LMS6002D. The chip selects the
    /// closest calibrated filter response to the requested value.
    ///
    /// Returns the actual bandwidth applied by the hardware, which may
    /// differ from the requested value due to the discrete set of calibrated
    /// filter settings.
    ///
    /// Returns `Error::NotInitialized` if the board has not been initialized.
    pub fn set_bandwidth(
        &mut self,
        channel: Channel,
        mut bandwidth: u32,
    ) -> impl MaybeFuture<Output = Result<u32>> {
        Op::new(async move {
            self.require_initialized().await?;
            let bandwidth_range = lms6002d::bandwidth::get_bandwidth_range();
            bandwidth = bandwidth.clamp(
                bandwidth_range
                    .min()
                    .ok_or(Error::Internal("bandwidth range has no minimum"))?
                    as u32,
                bandwidth_range
                    .max()
                    .ok_or(Error::Internal("bandwidth range has no maximum"))?
                    as u32,
            );
            log::trace!("Clamped bandwidth to {bandwidth}");
            let bw: LmsBandwidth = bandwidth.into();
            self.lms().lpf_enable(channel, true).await?;
            self.lms().set_bandwidth(channel, bw).await?;
            let actual: u32 = bw.into();
            Ok(actual)
        })
    }
    /// Returns the current LPF bandwidth for the given channel in Hz.
    ///
    /// Reads the calibrated bandwidth value from the LMS6002D registers.
    ///
    /// Returns `Error::NotInitialized` if the board has not been initialized.
    pub fn get_bandwidth(&mut self, channel: Channel) -> impl MaybeFuture<Output = Result<u32>> {
        Op::new(async move {
            self.require_initialized().await?;
            let bw: LmsBandwidth = self.lms().get_bandwidth(channel).await?;
            Ok(bw.into())
        })
    }
    /// Returns the supported LPF bandwidth range in Hz.
    pub fn get_bandwidth_range() -> Range {
        lms6002d::bandwidth::get_bandwidth_range()
    }
}
