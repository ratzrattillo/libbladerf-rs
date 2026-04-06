use crate::bladerf1::BladeRf1;
use crate::bladerf1::board::{BladeRf1TxStreamer, SampleFormat};
use crate::bladerf1::hardware::lms6002d::LMS6002D;
use crate::bladerf1::hardware::lms6002d::dc_calibration::{
    DcCalModule, DcCals, RxCal, RxCalBackup,
};
use crate::bladerf1::hardware::lms6002d::loopback::Loopback;
use crate::bladerf1::hardware::lms6002d::loopback::Loopback::BbTxvga1Rxvga2;
use crate::bladerf1::hardware::si5338::RationalRate;
use crate::channel::Channel;
use crate::error::Result;
use std::time::Duration;
#[macro_export]
macro_rules! ms_to_samples {
    ($ms:expr, $rate:expr) => {
        (($ms * $rate) / 1000)
    };
}
#[allow(dead_code)]
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
#[derive(Clone, Debug)]
pub enum Correction {
    DcOffI,
    DcOffQ,
    Phase,
    Gain,
}
impl BladeRf1 {
    pub fn get_correction(&self, ch: Channel, corr: &Correction) -> Result<i16> {
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
                Ok(value - 4096)
            }
            Correction::DcOffI => self.lms.get_dc_offset_i(ch),
            Correction::DcOffQ => self.lms.get_dc_offset_q(ch),
        }
    }
    pub fn set_correction(&self, ch: Channel, corr: &Correction, value: i16) -> Result<()> {
        match corr {
            Correction::Phase => self
                .interface
                .lock()
                .unwrap()
                .nios_set_iq_phase_correction(ch, value),
            Correction::Gain => self
                .interface
                .lock()
                .unwrap()
                .nios_set_iq_gain_correction(ch, value + 4096),
            Correction::DcOffI => self.lms.set_dc_offset_i(ch, value),
            Correction::DcOffQ => self.lms.set_dc_offset_q(ch, value),
        }
    }
    pub fn tx_lpf_dummy_tx(&self) -> Result<()> {
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
        if self.enable_module(Channel::Tx, true).is_err() {
            return restore_backup(&mut sample_rate_backup, loopback_backup);
        }
        let mut streamer = BladeRf1TxStreamer::new(self.clone(), 512, 1, SampleFormat::Sc16Q11)?;
        {
            let mut buffer = streamer.get_buffer(None)?;
            buffer.extend_from_slice(&[0, 0, 0, 0]);
            if streamer.submit(buffer, 4).is_err()
                || streamer
                    .wait_completion(Some(Duration::from_millis(2000)))
                    .is_err()
            {
                return restore_backup(&mut sample_rate_backup, loopback_backup);
            }
        }
        restore_backup(&mut sample_rate_backup, loopback_backup)
    }
    pub fn cal_tx_lpf(&self) -> Result<()> {
        self.tx_lpf_dummy_tx()?;
        self.calibrate_dc(DcCalModule::TxLpf)
    }
    pub fn calibrate_dc(&self, module: DcCalModule) -> Result<()> {
        self.lms.calibrate_dc(module)
    }
    pub fn set_dc_cals(&self, dc_cals: DcCals) -> Result<()> {
        self.lms.set_dc_cals(dc_cals)
    }
    pub fn get_dc_cals(&self) -> Result<DcCals> {
        self.lms.get_dc_cals()
    }
    pub const RX_CAL_RATE: u64 = 3000000;
    pub const RX_CAL_BW: u64 = 1500000;
    pub const RX_CAL_TS_INC: u64 = ms_to_samples!(15, Self::RX_CAL_RATE);
    pub const RX_CAL_COUNT: u64 = ms_to_samples!(5, Self::RX_CAL_RATE);
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
    pub fn rx_cal_update_frequency(&self, cal: &mut RxCal, rx_freq: u64) -> Result<()> {
        let f_diff: u64 = cal.tx_freq.abs_diff(rx_freq);
        log::debug!("Set F_RX = {rx_freq}");
        log::debug!("F_diff(RX, TX) = {f_diff}");
        if f_diff < 1000000 {
            if rx_freq >= (LMS6002D::get_frequency_min() + 1000000) as u64 {
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
    pub fn set_rx_dc_corr(&self, i: i16, q: i16) -> Result<()> {
        self.set_correction(Channel::Rx, &Correction::DcOffI, i)?;
        self.set_correction(Channel::Rx, &Correction::DcOffQ, q)
    }
}
