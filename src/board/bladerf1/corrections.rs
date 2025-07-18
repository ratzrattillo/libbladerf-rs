use crate::BladeRf1;
use crate::nios::Nios;
use anyhow::Result;

/**
 * Correction parameter selection
 *
 * These values specify the correction parameter to modify or query when calling
 * bladerf_set_correction() or bladerf_get_correction(). Note that the meaning
 * of the `value` parameter to these functions depends upon the correction
 * parameter.
 *
 */
pub enum BladeRf1Correction {
    /**
     * Adjusts the in-phase DC offset. Valid values are [-2048, 2048], which are
     * scaled to the available control bits.
     */
    DcoffI,

    /**
     * Adjusts the quadrature DC offset. Valid values are [-2048, 2048], which
     * are scaled to the available control bits.
     */
    DcoffQ,

    /**
     * Adjusts phase correction of [-10, 10] degrees, via a provided count value
     * of [-4096, 4096].
     */
    Phase,

    /**
     * Adjusts gain correction value in [-1.0, 1.0], via provided values in the
     * range of [-4096, 4096].
     */
    Gain,
}

impl BladeRf1 {
    /******************************************************************************/
    /* DC/Phase/Gain Correction */
    /******************************************************************************/

    pub fn get_correction(&self, ch: u8, corr: BladeRf1Correction) -> Result<i16> {
        //CHECK_BOARD_STATE(STATE_INITIALIZED);

        match corr {
            BladeRf1Correction::Phase => self.interface.nios_get_iq_phase_correction(ch),
            BladeRf1Correction::Gain => {
                let value = self.interface.nios_get_iq_gain_correction(ch)?;

                /* Undo the gain control offset */
                Ok(value - 4096)
            }
            BladeRf1Correction::DcoffI => self.lms.get_dc_offset_i(ch),
            BladeRf1Correction::DcoffQ => self.lms.get_dc_offset_q(ch),
            // _ => {
            //     Err(anyhow!("Invalid correction type: {corr}"));
            // }
        }
    }

    pub fn set_correction(&self, ch: u8, corr: BladeRf1Correction, value: i16) -> Result<()> {
        //CHECK_BOARD_STATE(STATE_INITIALIZED);

        match corr {
            BladeRf1Correction::Phase => self.interface.nios_set_iq_phase_correction(ch, value),
            BladeRf1Correction::Gain => {
                /* Gain correction requires than an offset be applied */
                self.interface.nios_set_iq_gain_correction(ch, value + 4096)
            }
            BladeRf1Correction::DcoffI => self.lms.set_dc_offset_i(ch, value),
            BladeRf1Correction::DcoffQ => self.lms.set_dc_offset_q(ch, value),
            // _ => {
            //     Err(anyhow!("Invalid correction type: {corr}"));
            // }
        }
    }
}
