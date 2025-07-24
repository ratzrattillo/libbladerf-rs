use crate::nios::Nios;
use crate::{BladeRf1, Result};
use bladerf_globals::bladerf1::BladeRf1Correction;

impl BladeRf1 {
    /****************************************************************************/
    /* DC/Phase/Gain Correction */
    /****************************************************************************/

    pub fn get_correction(&self, ch: u8, corr: &BladeRf1Correction) -> Result<i16> {
        // CHECK_BOARD_STATE(STATE_INITIALIZED);

        match corr {
            BladeRf1Correction::Phase => self.interface.nios_get_iq_phase_correction(ch),
            BladeRf1Correction::Gain => {
                let value = self.interface.nios_get_iq_gain_correction(ch)?;

                // Undo the gain control offset
                Ok(value - 4096)
            }
            BladeRf1Correction::DcoffI => self.lms.get_dc_offset_i(ch),
            BladeRf1Correction::DcoffQ => self.lms.get_dc_offset_q(ch),
            // _ => {
            //     log::error!("Invalid correction type: {corr}");
            //     Err(Error::Invalid)
            // }
        }
    }

    pub fn set_correction(&self, ch: u8, corr: &BladeRf1Correction, value: i16) -> Result<()> {
        // CHECK_BOARD_STATE(STATE_INITIALIZED);

        match corr {
            BladeRf1Correction::Phase => self.interface.nios_set_iq_phase_correction(ch, value),
            BladeRf1Correction::Gain => {
                // Gain correction requires than an offset be applied
                self.interface.nios_set_iq_gain_correction(ch, value + 4096)
            }
            BladeRf1Correction::DcoffI => self.lms.set_dc_offset_i(ch, value),
            BladeRf1Correction::DcoffQ => self.lms.set_dc_offset_q(ch, value),
            // _ => {
            //     log::error!("Invalid correction type: {corr}");
            //     Err(Error::Invalid)
            // }
        }
    }
}
