//! SMB (Smart Magnetic Blade) clock mode for the Si5338.
//!
//! The Si5338 clock generator can operate in SMB mode, routing one of its
//! multisynth outputs to the SMA clock connector. This allows the board to
//! provide an external clock output or accept an external clock input for
//! synchronization. When in output mode the Si5338 drives the connector; in
//! input mode it locks to an external reference.

use crate::bladerf1::board::RfLinkSession;
use crate::bladerf1::hardware::si5338;
use crate::error::Result;
use crate::maybe_future::Op;
use crate::range::{Range, RangeItem};
use nusb::MaybeFuture;

impl RfLinkSession<'_> {
    /// Sets the SMB clock mode on the Si5338.
    ///
    /// Routes the Si5338 multisynth output to the SMB connector, configures
    /// the connector as an external clock input, or disables SMB operation.
    ///
    /// Returns `Error::BoardState` if the board is not initialized.
    pub fn set_smb_mode(&mut self, mode: si5338::SmbMode) -> impl MaybeFuture<Output = Result<()>> {
        Op::new(async move {
            self.require_initialized().await?;
            self.si().set_smb_mode(mode).await
        })
    }

    /// Returns the current SMB clock mode.
    ///
    /// Returns `Error::BoardState` if the board is not initialized.
    pub fn get_smb_mode(&mut self) -> impl MaybeFuture<Output = Result<si5338::SmbMode>> {
        Op::new(async move {
            self.require_initialized().await?;
            self.si().get_smb_mode().await
        })
    }

    /// Sets the SMB output frequency in Hz.
    ///
    /// Reconfigures the Si5338 multisynth to produce the requested frequency
    /// (rounded to the nearest achievable value). The actual achieved
    /// frequency is returned.
    ///
    /// Returns `Error::BoardState` if the board is not initialized.
    pub fn set_smb_freq(&mut self, rate: u32) -> impl MaybeFuture<Output = Result<u32>> {
        Op::new(async move {
            self.require_initialized().await?;
            self.si().set_smb_freq(rate).await
        })
    }

    /// Returns the current SMB output frequency in Hz.
    ///
    /// Returns `Error::BoardState` if the board is not initialized.
    pub fn get_smb_freq(&mut self) -> impl MaybeFuture<Output = Result<u32>> {
        Op::new(async move {
            self.require_initialized().await?;
            self.si().get_smb_freq().await
        })
    }

    /// Sets the SMB output frequency using a rational numerator/denominator.
    ///
    /// Provides exact multisynth configuration for frequencies that cannot be
    /// represented as a simple integer. Returns the actual achieved rational
    /// rate.
    ///
    /// Returns `Error::BoardState` if the board is not initialized.
    pub fn set_rational_smb_freq(
        &mut self,
        rate: si5338::RationalRate,
    ) -> impl MaybeFuture<Output = Result<si5338::RationalRate>> {
        Op::new(async move {
            self.require_initialized().await?;
            self.si().set_rational_smb_freq(rate).await
        })
    }

    /// Returns the current SMB output rate as a rational numerator/denominator.
    ///
    /// Returns `Error::BoardState` if the board is not initialized.
    pub fn get_rational_smb_freq(
        &mut self,
    ) -> impl MaybeFuture<Output = Result<si5338::RationalRate>> {
        Op::new(async move {
            self.require_initialized().await?;
            self.si().get_rational_smb_freq().await
        })
    }

    /// Returns the valid frequency range for the SMB clock output.
    ///
    /// The range is a single continuous step from the Si5338 minimum to
    /// maximum SMB frequency with 1 Hz granularity.
    pub fn get_smb_freq_range() -> Range {
        Range::new(vec![RangeItem::Step(
            si5338::BLADERF_SMB_FREQUENCY_MIN as f64,
            si5338::BLADERF_SMB_FREQUENCY_MAX as f64,
            1f64,
            1f64,
        )])
    }
}
