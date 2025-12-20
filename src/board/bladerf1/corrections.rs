use crate::bladerf::Channel;
use crate::bladerf1::{BladeRf1, SampleFormat};
use crate::bladerf1::Loopback::BbTxvga1Rxvga2;
use crate::hardware::lms6002d::dc_calibration::{DcCalModule, DcCals};
use crate::nios::Nios;
use crate::Result;

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
    pub fn get_correction(&self, ch: Channel, corr: &Correction) -> Result<i16> {
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
    pub fn set_correction(&self, ch: Channel, corr: &Correction, value: i16) -> Result<()> {
        // CHECK_BOARD_STATE(STATE_INITIALIZED);

        match corr {
            Correction::Phase => self
                .interface
                .lock()
                .unwrap()
                .nios_set_iq_phase_correction(ch, value),
            Correction::Gain => {
                // Gain correction requires that an offset be applied
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

    /*******************************************************************************
 * LMS6002D DC offset calibration
 ******************************************************************************/

    /// We've found that running samples through the LMS6 tends to be required
    /// for the TX LPF calibration to converge
    pub fn tx_lpf_dummy_tx(&self) {
        struct bladerf_metadata meta;
        int16_t zero_sample[] = { 0, 0 };

        memset(&meta, 0, sizeof(meta));

        let loopback_backup = self.get_loopback()?;

        let sample_rate_backup = self.get_rational_sample_rate(Channel::Tx)?;


        let status = self.set_loopback(BbTxvga1Rxvga2);
        if (status != 0) {
            goto out;
        }

        let status = self.set_sample_rate(Channel::Tx, 3000000);
        if (status != 0) {
            goto out;
        }

        let status = self.sync_config(Channel::Tx, SampleFormat::Sc16Q11Meta, 64, 16384, 16, 1000);
        if (status != 0) {
            goto out;
        }

        let status = self.enable_module(Channel::Tx, true);
        if (status != 0) {
            goto out;
        }

        meta.flags = BLADERF_META_FLAG_TX_BURST_START |
            BLADERF_META_FLAG_TX_BURST_END   |
            BLADERF_META_FLAG_TX_NOW;

        let status = self.sync_tx(zero_sample, 1, &meta, 2000);
        if (status != 0) {
            goto out;
        }

        out:
            status = self.enable_module(Channel::Tx, false);
            if (status != 0 && retval == 0) {
                retval = status;
            }

            status = self.set_rational_sample_rate(Channel::Tx, &sample_rate_backup);
            if (status != 0 && retval == 0) {
                retval = status;
            }

            status = self.set_loopback(loopback_backup);
            if (status != 0 && retval == 0) {
                retval = status;
            }

            return retval;
    }
    pub fn cal_tx_lpf(&self) -> Result<()> {
        self.tx_lpf_dummy_tx()?;
        self.bladerf_calibrate_dc(DcCalModule::TxLpf)
    }

    pub fn calibrate_dc(&self, module: DcCalModule) -> Result<()> {
        self.lms.calibrate_dc(module)
    }

    /******************************************************************************/
    /* DC Calibration */
    /******************************************************************************/

    pub fn set_dc_cals(&self, dc_cals: DcCals) -> Result<()> {
        self.lms.set_dc_cals(dc_cals)
    }

    pub fn get_dc_cals(&self) -> Result<DcCals> {
        self.lms.get_dc_cals()
    }
}
