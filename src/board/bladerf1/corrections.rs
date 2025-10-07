use crate::Result;
use crate::bladerf1::BladeRf1;
use crate::nios::Nios;

/// Correction parameter selection
///
/// These values specify the correction parameter to modify or query when calling
/// bladerf_set_correction() or bladerf_get_correction(). Note that the meaning
/// of the `value` parameter to these functions depends upon the correction
/// parameter.
#[derive(Clone, Debug)]
pub enum Correction {
    /// Adjusts the in-phase DC offset. Valid values are \[-2048, 2048\], which are
    /// scaled to the available control bits.
    DcoffI,

    /// Adjusts the quadrature DC offset. Valid values are \[-2048, 2048\], which
    /// are scaled to the available control bits.
    DcoffQ,

    /// Adjusts phase correction of \[-10, 10\] degrees, via a provided count value
    /// of \[-4096, 4096\].
    Phase,

    /// Adjusts gain correction value in \[-1.0, 1.0\], via provided values in the
    /// range of \[-4096, 4096\].
    Gain,
}

impl BladeRf1 {
    /// Return the currently applied correction values for either DC, Phase or Gain.
    pub fn get_correction(&self, ch: u8, corr: &Correction) -> Result<i16> {
        // CHECK_BOARD_STATE(STATE_INITIALIZED);

        match corr {
            Correction::Phase => self
                .interface
                .lock()
                .unwrap()
                .nios_get_iq_phase_correction(ch),
            Correction::Gain => {
                let value = self
                    .interface
                    .lock()
                    .unwrap()
                    .nios_get_iq_gain_correction(ch)?;

                // Undo the gain control offset
                Ok(value - 4096)
            }
            Correction::DcoffI => self.lms.get_dc_offset_i(ch),
            Correction::DcoffQ => self.lms.get_dc_offset_q(ch),
            // _ => {
            //     log::error!("Invalid correction type: {corr}");
            //     Err(Error::Invalid)
            // }
        }
    }

    /// Apply correction values for either DC, Phase or Gain.
    pub fn set_correction(&self, ch: u8, corr: &Correction, value: i16) -> Result<()> {
        // CHECK_BOARD_STATE(STATE_INITIALIZED);

        match corr {
            Correction::Phase => self
                .interface
                .lock()
                .unwrap()
                .nios_set_iq_phase_correction(ch, value),
            Correction::Gain => {
                // Gain correction requires than an offset be applied
                self.interface
                    .lock()
                    .unwrap()
                    .nios_set_iq_gain_correction(ch, value + 4096)
            }
            Correction::DcoffI => self.lms.set_dc_offset_i(ch, value),
            Correction::DcoffQ => self.lms.set_dc_offset_q(ch, value),
            // _ => {
            //     log::error!("Invalid correction type: {corr}");
            //     Err(Error::Invalid)
            // }
        }
    }
}
