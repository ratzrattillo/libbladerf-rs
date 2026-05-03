use crate::bladerf1::BladeRf1;
use crate::bladerf1::board::TuningMode;
use crate::bladerf1::hardware::lms6002d;
use crate::bladerf1::hardware::lms6002d::dc_calibration::{DcCalModule, DcCals};
use crate::channel::Channel;
use crate::error::Result;
#[macro_export]
macro_rules! ms_to_samples {
    ($ms:expr, $rate:expr) => {
        (($ms * $rate) / 1_000)
    };
}
#[derive(Clone, Debug)]
pub enum Correction {
    DcOffI,
    DcOffQ,
    Phase,
    Gain,
}
impl BladeRf1 {
    pub fn get_correction(&mut self, ch: Channel, corr: &Correction) -> Result<i16> {
        match corr {
            Correction::Phase => self.nios.nios_get_iq_phase_correction(ch),
            Correction::Gain => {
                let value = self.nios.nios_get_iq_gain_correction(ch)?;
                Ok(value - 4_096)
            }
            Correction::DcOffI => lms6002d::dc_calibration::get_dc_offset_i(&mut self.nios, ch),
            Correction::DcOffQ => lms6002d::dc_calibration::get_dc_offset_q(&mut self.nios, ch),
        }
    }
    pub fn set_correction(&mut self, ch: Channel, corr: &Correction, value: i16) -> Result<()> {
        match corr {
            Correction::Phase => self.nios.nios_set_iq_phase_correction(ch, value),
            Correction::Gain => self.nios.nios_set_iq_gain_correction(ch, value + 4_096),
            Correction::DcOffI => {
                lms6002d::dc_calibration::set_dc_offset_i(&mut self.nios, ch, value)
            }
            Correction::DcOffQ => {
                lms6002d::dc_calibration::set_dc_offset_q(&mut self.nios, ch, value)
            }
        }
    }
    pub fn cal_tx_lpf(&mut self) -> Result<()> {
        self.calibrate_dc(DcCalModule::TxLpf)
    }
    pub fn calibrate_dc(&mut self, module: DcCalModule) -> Result<()> {
        lms6002d::dc_calibration::calibrate_dc(&mut self.nios, module)
    }
    pub fn set_dc_cals(&mut self, dc_cals: DcCals) -> Result<()> {
        lms6002d::dc_calibration::set_dc_cals(&mut self.nios, dc_cals)
    }
    pub fn get_dc_cals(&mut self) -> Result<DcCals> {
        lms6002d::dc_calibration::get_dc_cals(&mut self.nios)
    }
    pub const RX_CAL_RATE: u64 = 3_000_000;
    pub const RX_CAL_BW: u64 = 1_500_000;
    pub const RX_CAL_TS_INC: u64 = ms_to_samples!(15, Self::RX_CAL_RATE);
    pub const RX_CAL_COUNT: u64 = ms_to_samples!(5, Self::RX_CAL_RATE);
    pub const RX_CAL_MAX_SWEEP_LEN: u64 = 2 * 2_048 / 32;
    pub fn get_rx_cal_backup(&mut self) -> Result<lms6002d::dc_calibration::RxCalBackup> {
        Ok(lms6002d::dc_calibration::RxCalBackup {
            rational_sample_rate: self.get_rational_sample_rate(Channel::Rx)?,
            bandwidth: self.get_bandwidth(Channel::Rx)?,
            tx_freq: self.get_frequency(Channel::Tx)?,
        })
    }
    pub fn set_rx_cal_backup(
        &mut self,
        rx_cal_backup: &mut lms6002d::dc_calibration::RxCalBackup,
    ) -> Result<()> {
        self.set_rational_sample_rate(Channel::Rx, &mut rx_cal_backup.rational_sample_rate)?;
        self.set_bandwidth(Channel::Rx, rx_cal_backup.bandwidth)?;
        self.set_frequency(Channel::Tx, rx_cal_backup.tx_freq, TuningMode::Fpga)
    }
    pub fn rx_cal_update_frequency(
        &mut self,
        cal: &mut lms6002d::dc_calibration::RxCal,
        rx_freq: u64,
    ) -> Result<()> {
        let f_diff: u64 = cal.tx_freq.abs_diff(rx_freq);
        log::debug!("Set F_RX = {rx_freq}");
        log::debug!("F_diff(RX, TX) = {f_diff}");
        if f_diff < 1_000_000 {
            if rx_freq >= (lms6002d::frequency::get_frequency_min() + 1_000_000) as u64 {
                cal.tx_freq = rx_freq - 1_000_000;
            } else {
                cal.tx_freq = rx_freq + 1_000_000;
            }
            self.set_frequency(Channel::Tx, cal.tx_freq, TuningMode::Fpga)?;
            log::debug!("Adjusted TX frequency: {}", cal.tx_freq);
        }
        self.set_frequency(Channel::Rx, rx_freq, TuningMode::Fpga)?;
        cal.ts += Self::RX_CAL_TS_INC;
        Ok(())
    }
    pub fn sample_mean(samples: &[i16]) -> Result<(i16, i16)> {
        let len_i16 = i16::try_from(samples.len())
            .map_err(|_| crate::error::Error::Argument("sample count exceeds i16 range".into()))?;
        if len_i16 == 0 {
            return Err(crate::error::Error::Argument(
                "sample_mean requires at least one sample".into(),
            ));
        }
        let mean_i = samples.iter().step_by(2).sum::<i16>() / len_i16;
        let mean_q = samples.iter().skip(1).step_by(2).sum::<i16>() / len_i16;
        Ok((mean_i, mean_q))
    }
    pub fn set_rx_dc_corr(&mut self, i: i16, q: i16) -> Result<()> {
        self.set_correction(Channel::Rx, &Correction::DcOffI, i)?;
        self.set_correction(Channel::Rx, &Correction::DcOffQ, q)
    }
}
