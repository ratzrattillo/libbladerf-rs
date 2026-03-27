use crate::Channel;
use crate::Result;
use crate::bladerf1::Loopback::BbTxvga1Rxvga2;
use crate::bladerf1::{BladeRf1, BladeRf1TxStreamer, Loopback};
use crate::hardware::lms6002d::BLADERF_FREQUENCY_MIN;
use crate::hardware::lms6002d::dc_calibration::{DcCalModule, DcCals, RxCal, RxCalBackup};
use crate::hardware::si5338::RationalRate;
use crate::nios2::Nios;
use num_complex::Complex32;

/// Convert ms to samples
#[macro_export]
macro_rules! ms_to_samples {
    ($ms:expr, $rate:expr) => {
        (($ms * $rate) / 1000)
    };
}
// pub(crate) use crate::ms_to_samples;

/// Round f32 to i16
/// could also use to_i16 from num-traits crate...
pub fn float_to_int16(val: f32) -> i16 {
    if (val - 0.5) <= i16::MIN as f32 {
        return i16::MIN;
    }
    if (val + 0.5) >= i16::MAX as f32 {
        return i16::MAX;
    }
    if val >= 0f32 {
        (val + 0.5) as i16
    } else {
        (val - 0.5) as i16
    }
}

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
    DcOffI,

    /// Adjusts the quadrature DC offset. Valid values are \[-2048, 2048\], which
    /// are scaled to the available control bits.
    DcOffQ,

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
            Correction::DcOffI => self.lms.get_dc_offset_i(ch),
            Correction::DcOffQ => self.lms.get_dc_offset_q(ch),
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
            Correction::DcOffI => self.lms.set_dc_offset_i(ch, value),
            Correction::DcOffQ => self.lms.set_dc_offset_q(ch, value),
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
    pub fn tx_lpf_dummy_tx(&self) -> Result<()> {
        // let mut meta = BladerfMetadata::default();
        let loopback_backup = self.get_loopback()?;
        let mut sample_rate_backup = self.get_rational_sample_rate(Channel::Tx)?;

        let restore_backup = |sample_rate_backup: &mut RationalRate, loopback_backup: Loopback| {
            let _ = self.enable_module(Channel::Tx, false);
            let _ = self.set_rational_sample_rate(Channel::Tx, sample_rate_backup);
            self.set_loopback(loopback_backup)
        };

        if self.set_loopback(BbTxvga1Rxvga2).is_err() {
            return restore_backup(&mut sample_rate_backup, loopback_backup);
        }

        if self.set_sample_rate(Channel::Tx, 3_000_000).is_err() {
            return restore_backup(&mut sample_rate_backup, loopback_backup);
        }

        // if self.sync_config(Channel::Tx, SampleFormat::Sc16Q11Meta, 64, 16384, 16, 1000).is_err()
        // {
        //     return restore_backup(&mut sample_rate_backup, loopback_backup);
        // }

        if self.enable_module(Channel::Tx, true).is_err() {
            return restore_backup(&mut sample_rate_backup, loopback_backup);
        }

        // meta.flags = MetadataFlags::TxBurstStart | MetadataFlags::TxBurstEnd | MetadataFlags::TxNow;

        let mut streamer = BladeRf1TxStreamer::new(self.clone(), 1, Some(1), None)?;
        if streamer
            .write(&[&[Complex32::new(0.0, 0.0)]], None, false, 2000)
            .is_err()
        {
            return restore_backup(&mut sample_rate_backup, loopback_backup);
        }

        // TODO: Implement sync_tx
        // int16_t zero_sample[] = { 0, 0 };
        // if self.sync_tx(zero_sample, 1, &meta, 2000).is_err() {
        //     return restore_backup(&sample_rate_backup, loopback_backup);
        // }

        restore_backup(&mut sample_rate_backup, loopback_backup)
    }

    pub fn cal_tx_lpf(&self) -> Result<()> {
        self.tx_lpf_dummy_tx()?;
        self.calibrate_dc(DcCalModule::TxLpf)
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

    /*******************************************************************************
     * RX DC offset calibration
     ******************************************************************************/

    pub const RX_CAL_RATE: u64 = 3000000;
    pub const RX_CAL_BW: u64 = 1500000;
    pub const RX_CAL_TS_INC: u64 = ms_to_samples!(15, Self::RX_CAL_RATE);
    pub const RX_CAL_COUNT: u64 = ms_to_samples!(5, Self::RX_CAL_RATE);
    // -2048 : 32 : 2048
    pub const RX_CAL_MAX_SWEEP_LEN: u64 = 2 * 2048 / 32;

    pub fn get_rx_cal_backup(&self) -> Result<RxCalBackup> {
        Ok(RxCalBackup {
            rational_sample_rate: self.get_rational_sample_rate(Channel::Rx)?,
            bandwidth: self.get_bandwidth(Channel::Rx)?,
            tx_freq: self.get_frequency(Channel::Tx)?,
        })
    }

    pub fn set_rx_cal_backup(&self, rx_cal_backup: &mut RxCalBackup) -> Result<()> {
        self.set_rational_sample_rate(Channel::Rx, &mut rx_cal_backup.rational_sample_rate)?;
        self.set_bandwidth(Channel::Rx, rx_cal_backup.bandwidth)?;
        self.set_frequency(Channel::Tx, rx_cal_backup.tx_freq)
    }

    /// Ensure TX >= 1 MHz away from the RX frequency to avoid any potential
    /// artifacts from the PLLs interfering with one another
    pub fn rx_cal_update_frequency(&self, cal: &mut RxCal, rx_freq: u64) -> Result<()> {
        let f_diff: u64 = cal.tx_freq.abs_diff(rx_freq);

        log::debug!("Set F_RX = {rx_freq}");
        log::debug!("F_diff(RX, TX) = {f_diff}");

        if f_diff < 1000000 {
            if rx_freq >= (BLADERF_FREQUENCY_MIN + 1000000) as u64 {
                cal.tx_freq = rx_freq - 1000000;
            } else {
                cal.tx_freq = rx_freq + 1000000;
            }

            self.set_frequency(Channel::Tx, cal.tx_freq)?;

            log::debug!("Adjusted TX frequency: {}", cal.tx_freq);
        }

        self.set_frequency(Channel::Rx, rx_freq)?;

        cal.ts += Self::RX_CAL_TS_INC;

        Ok(())
    }

    pub fn sample_mean(samples: &[i16]) -> Result<(i16, i16)> {
        let len = samples.len() as i16;

        let mean_i = samples.iter().step_by(2).sum::<i16>() / len;
        let mean_q = samples.iter().skip(1).step_by(2).sum::<i16>() / len;

        Ok((mean_i, mean_q))
    }

    pub fn sample_mean_complex(samples: &[Complex32]) -> Result<Complex32> {
        let len = samples.len() as f32;
        Ok(samples.iter().sum::<Complex32>() / Complex32::new(len, len))
    }

    pub fn set_rx_dc_corr(&self, i: i16, q: i16) -> Result<()> {
        self.set_correction(Channel::Rx, &Correction::DcOffI, i)?;
        self.set_correction(Channel::Rx, &Correction::DcOffQ, q)
    }
}
