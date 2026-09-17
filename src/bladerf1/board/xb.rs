//! Expansion board detection and generic GPIO access.
//!
//! Provides `ExpansionBoard` enum for identifying attached boards
//! (XB-100, XB-200, XB-300, or none), along with detection logic,
//! attach/enable helpers, and low-level expansion GPIO R/W operations.

#[cfg(feature = "xb100")]
mod xb100;
#[cfg(feature = "xb200")]
pub mod xb200;
#[cfg(feature = "xb300")]
mod xb300;

use crate::bladerf1::board::RfLinkSession;
use crate::error::{Error, Result};
use crate::maybe_future::Op;
#[cfg(any(feature = "xb100", feature = "xb200", feature = "xb300"))]
use crate::nios_client::NiosCore;
use nusb::MaybeFuture;

/// Identifies the expansion board attached to the BladeRF1.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExpansionBoard {
    /// No expansion board is attached or enabled.
    XbNone = 0,
    /// XB-100 LED expansion board (feature-gated).
    #[cfg(feature = "xb100")]
    Xb100,
    /// XB-200 transverter board (feature-gated).
    #[cfg(feature = "xb200")]
    Xb200,
    /// XB-300 amplifier board (feature-gated).
    #[cfg(feature = "xb300")]
    Xb300,
}

#[cfg(any(feature = "xb100", feature = "xb200"))]
impl NiosCore {
    /// Reads the expansion GPIO and checks if `check_mask` bits are set.
    /// Returns `false` if all GPIO bits read as 1 (no board present).
    pub(crate) fn detect_xb_board(
        &mut self,
        check_mask: u32,
    ) -> impl MaybeFuture<Output = Result<bool>> {
        Op::new(async move {
            let gpio = self.nios_expansion_gpio_read().await?;
            if gpio == 0xffffffff {
                return Ok(false);
            }
            Ok((gpio & check_mask) != 0)
        })
    }
}

#[cfg(feature = "xb300")]
impl NiosCore {
    /// Reads the expansion GPIO direction register and checks if `check_mask` bits indicate a board.
    pub(crate) fn detect_xb_board_by_dir(
        &mut self,
        check_mask: u32,
    ) -> impl MaybeFuture<Output = Result<bool>> {
        Op::new(async move {
            let gpio_dir = self.nios_expansion_gpio_dir_read().await?;
            Ok((gpio_dir & check_mask) != 0)
        })
    }
}

#[cfg(feature = "xb200")]
impl NiosCore {
    /// Returns `true` if the XB-200 board is currently enabled (RF_ON bit set).
    pub(crate) fn xb200_is_enabled(&mut self) -> impl MaybeFuture<Output = Result<bool>> {
        Op::new(async move { self.detect_xb_board(xb200::BLADERF_XB_RF_ON).await })
    }
}

impl RfLinkSession<'_> {
    /// Reads the full expansion GPIO value.
    pub fn expansion_gpio_read(&mut self) -> impl MaybeFuture<Output = Result<u32>> {
        Op::new(async move { self.nios.nios_expansion_gpio_read().await })
    }

    /// Writes the full expansion GPIO value (all bits).
    pub fn expansion_gpio_write(&mut self, val: u32) -> impl MaybeFuture<Output = Result<()>> {
        Op::new(async move { self.nios.nios_expansion_gpio_write(0xffffffff, val).await })
    }

    /// Writes the expansion GPIO value with a mask — only bits set in `mask` are updated.
    pub fn expansion_gpio_masked_write(
        &mut self,
        mask: u32,
        val: u32,
    ) -> impl MaybeFuture<Output = Result<()>> {
        Op::new(async move { self.nios.nios_expansion_gpio_write(mask, val).await })
    }

    /// Reads the expansion GPIO direction register.
    pub fn expansion_gpio_dir_read(&mut self) -> impl MaybeFuture<Output = Result<u32>> {
        Op::new(async move { self.nios.nios_expansion_gpio_dir_read().await })
    }

    /// Writes the full expansion GPIO direction register.
    pub fn expansion_gpio_dir_write(&mut self, val: u32) -> impl MaybeFuture<Output = Result<()>> {
        self.nios.nios_expansion_gpio_dir_write(0xffffffff, val)
    }

    /// Writes the expansion GPIO direction register with a mask.
    pub fn expansion_gpio_dir_masked_write(
        &mut self,
        mask: u32,
        val: u32,
    ) -> impl MaybeFuture<Output = Result<()>> {
        Op::new(async move { self.nios.nios_expansion_gpio_dir_write(mask, val).await })
    }

    /// Detects and returns the currently attached expansion board.
    /// Returns `ExpansionBoard::XbNone` if no recognized board is present.
    pub fn expansion_get_attached(&mut self) -> impl MaybeFuture<Output = Result<ExpansionBoard>> {
        Op::new(async move {
            self.require_initialized().await?;
            if self.nios.nios_expansion_gpio_read().await? == 0xffffffff {
                return Ok(ExpansionBoard::XbNone);
            }
            #[cfg(feature = "xb100")]
            if self.nios.detect_xb_board(xb100::XB100_DETECT_MASK).await? {
                return Ok(ExpansionBoard::Xb100);
            }
            #[cfg(feature = "xb200")]
            if self.nios.detect_xb_board(xb200::BLADERF_XB_RF_ON).await? {
                return Ok(ExpansionBoard::Xb200);
            }
            #[cfg(feature = "xb300")]
            if self
                .nios
                .detect_xb_board_by_dir(xb300::XB300_DETECT_MASK)
                .await?
            {
                return Ok(ExpansionBoard::Xb300);
            }
            Ok(ExpansionBoard::XbNone)
        })
    }

    /// Attaches and enables the specified expansion board. Performs detection,
    /// attach, enable, and init in sequence. Switching between different board
    /// types is not supported. Returns `Error::Unsupported` on mismatch.
    pub fn expansion_attach(
        &mut self,
        xb: ExpansionBoard,
    ) -> impl MaybeFuture<Output = Result<()>> {
        Op::new(async move {
            self.require_initialized().await?;
            let attached = self.expansion_get_attached().await?;
            if xb != attached && attached != ExpansionBoard::XbNone {
                log::error!("Switching XB types is not supported.");
                return Err(Error::Unsupported("switching XB types"));
            }
            #[cfg(feature = "xb100")]
            if xb == ExpansionBoard::Xb100 {
                self.xb100_attach().await?;
                self.xb100_enable(true).await?;
                self.xb100_init().await?;
                return Ok(());
            }
            #[cfg(feature = "xb200")]
            if xb == ExpansionBoard::Xb200 {
                self.xb200_attach().await?;
                self.xb200_enable(true).await?;
                self.xb200_init().await?;
                return Ok(());
            }
            #[cfg(feature = "xb300")]
            if xb == ExpansionBoard::Xb300 {
                self.xb300_attach().await?;
                self.xb300_enable(true).await?;
                self.xb300_init().await?;
                return Ok(());
            }
            if xb == ExpansionBoard::XbNone {
                log::error!("Disabling an attached XB is not supported.");
                return Err(Error::Unsupported("disabling attached XB"));
            }
            log::error!("Unknown xb type: {xb:?}");
            Err(Error::Unsupported("unknown XB type"))
        })
    }
}
