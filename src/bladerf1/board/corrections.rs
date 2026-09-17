//! IQ correction and DC calibration for BladeRF1.
//!
//! Provides correction parameters to compensate for imperfections inherent
//! in direct-conversion receiver architectures. DC offset is caused by
//! local oscillator leakage and DC mismatches in I/Q paths. IQ imbalance
//! (phase and gain mismatch) arises from imperfect matching between the
//! in-phase and quadrature signal paths.
//!
//! Also provides helpers for RX DC calibration, including backup/restore
//! of state required during the calibration sweep.

use crate::bladerf1::board::RfLinkSession;
use crate::bladerf1::board::TuningMode;
use crate::bladerf1::hardware::lms6002d;
use crate::bladerf1::hardware::lms6002d::dc_calibration::{DcCalModule, DcCals};
use crate::channel::Channel;
use crate::error::Result;
use crate::maybe_future::Op;
use nusb::MaybeFuture;
/// Converts a duration in milliseconds to a sample count at the given sample rate.
#[macro_export]
macro_rules! ms_to_samples {
    ($ms:expr, $rate:expr) => {
        (($ms * $rate) / 1_000)
    };
}
/// IQ correction parameters for compensating direct-converter imperfections.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Correction {
    /// DC offset correction for the I (in-phase) channel component.
    DcOffI,
    /// DC offset correction for the Q (quadrature) channel component.
    DcOffQ,
    /// Phase imbalance correction between I and Q channels in ppm.
    Phase,
    /// Gain imbalance correction between I and Q channels in ppm.
    Gain,
}
impl RfLinkSession<'_> {
    /// Returns the current value of the requested IQ correction parameter.
    ///
    /// DC offset values are read from LMS6002D registers. IQ phase and gain
    /// corrections are read from the FPGA's internal correction registers.
    ///
    /// Returns `Error::NotInitialized` if the board has not been initialized.
    pub fn get_correction(
        &mut self,
        ch: Channel,
        corr: &Correction,
    ) -> impl MaybeFuture<Output = Result<i16>> {
        Op::new(async move {
            self.require_initialized().await?;
            match corr {
                Correction::Phase => self.nios.nios_get_iq_phase_correction(ch).await,
                Correction::Gain => {
                    let value = self.nios.nios_get_iq_gain_correction(ch).await?;
                    Ok(value - 4_096)
                }
                Correction::DcOffI => self.lms().get_dc_offset_i(ch).await,
                Correction::DcOffQ => self.lms().get_dc_offset_q(ch).await,
            }
        })
    }
    /// Sets an IQ correction parameter to the given value.
    ///
    /// DC offset values are written to LMS6002D registers. IQ phase and gain
    /// corrections are written to the FPGA's internal correction registers
    /// (gain is offset by 4096 internally).
    ///
    /// Returns `Error::NotInitialized` if the board has not been initialized.
    pub fn set_correction(
        &mut self,
        ch: Channel,
        corr: &Correction,
        value: i16,
    ) -> impl MaybeFuture<Output = Result<()>> {
        Op::new(async move {
            self.require_initialized().await?;
            match corr {
                Correction::Phase => self.nios.nios_set_iq_phase_correction(ch, value).await,
                Correction::Gain => {
                    self.nios
                        .nios_set_iq_gain_correction(ch, value + 4_096)
                        .await
                }
                Correction::DcOffI => self.lms().set_dc_offset_i(ch, value).await,
                Correction::DcOffQ => self.lms().set_dc_offset_q(ch, value).await,
            }
        })
    }
    /// Runs DC calibration on the TX LPF path of the LMS6002D.
    ///
    /// Returns `Error::NotInitialized` if the board has not been initialized.
    pub fn cal_tx_lpf(&mut self) -> impl MaybeFuture<Output = Result<()>> {
        Op::new(async move {
            self.require_initialized().await?;
            self.calibrate_dc(DcCalModule::TxLpf).await
        })
    }
    /// Runs DC calibration on the specified LMS6002D module.
    ///
    /// Returns `Error::NotInitialized` if the board has not been initialized.
    pub fn calibrate_dc(&mut self, module: DcCalModule) -> impl MaybeFuture<Output = Result<()>> {
        Op::new(async move {
            self.require_initialized().await?;
            self.lms().calibrate_dc(module).await
        })
    }
    /// Applies a full set of DC calibration parameters to the LMS6002D.
    ///
    /// Returns `Error::NotInitialized` if the board has not been initialized.
    pub fn set_dc_cals(&mut self, dc_cals: DcCals) -> impl MaybeFuture<Output = Result<()>> {
        Op::new(async move {
            self.require_initialized().await?;
            self.lms().set_dc_cals(dc_cals).await
        })
    }
    /// Returns the current DC calibration parameters from the LMS6002D.
    ///
    /// Returns `Error::NotInitialized` if the board has not been initialized.
    pub fn get_dc_cals(&mut self) -> impl MaybeFuture<Output = Result<DcCals>> {
        Op::new(async move {
            self.require_initialized().await?;
            self.lms().get_dc_cals().await
        })
    }
    /// Sample rate used during RX DC calibration sweep (3 MSPS).
    pub const RX_CAL_RATE: u64 = 3_000_000;
    /// Bandwidth used during RX DC calibration sweep (1.5 MHz).
    pub const RX_CAL_BW: u64 = 1_500_000;
    /// Timestamp increment between calibration steps (15 ms in samples).
    pub const RX_CAL_TS_INC: u64 = ms_to_samples!(15, Self::RX_CAL_RATE);
    /// Number of samples per calibration measurement (5 ms).
    pub const RX_CAL_COUNT: u64 = ms_to_samples!(5, Self::RX_CAL_RATE);
    /// Maximum sweep length for the RX DC calibration sequence.
    pub const RX_CAL_MAX_SWEEP_LEN: u64 = 2 * 2_048 / 32;
    /// Saves the RX channel state required to restore after DC calibration.
    ///
    /// Captures the rational sample rate, bandwidth, and TX frequency so
    /// they can be restored via `set_rx_cal_backup()` after the calibration
    /// sweep modifies these parameters.
    ///
    /// Returns `Error::NotInitialized` if the board has not been initialized.
    pub fn get_rx_cal_backup(
        &mut self,
    ) -> impl MaybeFuture<Output = Result<lms6002d::dc_calibration::RxCalBackup>> {
        Op::new(async move {
            self.require_initialized().await?;
            Ok(lms6002d::dc_calibration::RxCalBackup::new(
                self.get_rational_sample_rate(Channel::Rx).await?,
                self.get_bandwidth(Channel::Rx).await?,
                self.get_frequency(Channel::Tx).await?,
            ))
        })
    }
    /// Restores the RX channel state from a previous `get_rx_cal_backup()` call.
    ///
    /// Restores the rational sample rate, bandwidth, and TX frequency to their
    /// pre-calibration values. Use after completing an RX DC calibration sweep.
    ///
    /// Returns `Error::NotInitialized` if the board has not been initialized.
    pub fn set_rx_cal_backup(
        &mut self,
        rx_cal_backup: &mut lms6002d::dc_calibration::RxCalBackup,
    ) -> impl MaybeFuture<Output = Result<()>> {
        Op::new(async move {
            self.require_initialized().await?;
            self.set_rational_sample_rate(Channel::Rx, rx_cal_backup.sample_rate_mut())
                .await?;
            self.set_bandwidth(Channel::Rx, rx_cal_backup.bandwidth())
                .await?;
            self.set_frequency(Channel::Tx, rx_cal_backup.tx_frequency(), TuningMode::Fpga)
                .await
        })
    }
    /// Updates the RX and TX frequencies for the next step of the DC calibration sweep.
    ///
    /// Sets the RX frequency and adjusts the TX frequency to maintain at least
    /// 1 MHz separation if the frequency difference is too small. Advances the
    /// retune timestamp by `RX_CAL_TS_INC`.
    ///
    /// Returns `Error::NotInitialized` if the board has not been initialized.
    pub fn rx_cal_update_frequency(
        &mut self,
        cal: &mut lms6002d::dc_calibration::RxCal,
        rx_freq: u64,
    ) -> impl MaybeFuture<Output = Result<()>> {
        Op::new(async move {
            self.require_initialized().await?;
            let f_diff: u64 = cal.tx_frequency().abs_diff(rx_freq);
            log::debug!("Set F_RX = {rx_freq}");
            log::debug!("F_diff(RX, TX) = {f_diff}");
            if f_diff < 1_000_000 {
                let new_tx_freq =
                    if rx_freq >= (lms6002d::frequency::get_frequency_min() + 1_000_000) as u64 {
                        rx_freq - 1_000_000
                    } else {
                        rx_freq + 1_000_000
                    };
                cal.set_tx_frequency(new_tx_freq);
                self.set_frequency(Channel::Tx, new_tx_freq, TuningMode::Fpga)
                    .await?;
                log::debug!("Adjusted TX frequency: {new_tx_freq}");
            }
            self.set_frequency(Channel::Rx, rx_freq, TuningMode::Fpga)
                .await?;
            cal.set_timestamp(cal.timestamp() + Self::RX_CAL_TS_INC);
            Ok(())
        })
    }
    /// Sets both I and Q DC offset corrections for the RX channel in one call.
    ///
    /// Returns `Error::NotInitialized` if the board has not been initialized.
    pub fn set_rx_dc_corr(&mut self, i: i16, q: i16) -> impl MaybeFuture<Output = Result<()>> {
        Op::new(async move {
            self.require_initialized().await?;
            self.set_correction(Channel::Rx, &Correction::DcOffI, i)
                .await?;
            self.set_correction(Channel::Rx, &Correction::DcOffQ, q)
                .await
        })
    }
}
