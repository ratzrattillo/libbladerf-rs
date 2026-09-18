//! XB-300 amplifier board support.
//!
//! The XB-300 provides a TSS-53LNB+ low-noise amplifier (LNA) on the RX path,
//! a SE2623L power amplifier (PA) on the TX path, and an auxiliary amplifier
//! (Amp 3) for the antenna connector. It also includes a power detector that
//! measures RF output power via SPI-over-GPIO.

use crate::bladerf1::board::RfLinkSession;
use crate::error::Result;
use crate::maybe_future::Op;
use nusb::MaybeFuture;
pub(crate) const BLADERF_XB_AUX_EN: u32 = 0x000002;
pub(crate) const BLADERF_XB_TX_LED: u32 = 0x000010;
pub(crate) const BLADERF_XB_RX_LED: u32 = 0x000020;
pub(crate) const BLADERF_XB_TRX_TXN: u32 = 0x000040;
pub(crate) const BLADERF_XB_TRX_RXN: u32 = 0x000080;
pub(crate) const BLADERF_XB_TRX_MASK: u32 = 0x0000c0;
pub(crate) const BLADERF_XB_PA_EN: u32 = 0x000200;
pub(crate) const BLADERF_XB_LNA_EN: u32 = 0x000400;
pub(crate) const BLADERF_XB_CS: u32 = 0x010000;
pub(crate) const BLADERF_XB_CSEL: u32 = 0x040000;
pub(crate) const BLADERF_XB_DOUT: u32 = 0x100000;
pub(crate) const BLADERF_XB_SCLK: u32 = 0x400000;
pub(crate) const XB300_DETECT_MASK: u32 = BLADERF_XB_CS | BLADERF_XB_CSEL | BLADERF_XB_LNA_EN;
/// XB-300 transmit/receive switch position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BladeRfXb300Trx {
    /// Antenna connected to the PA (transmit) path.
    Tx = 0,
    /// Antenna connected to the LNA (receive) path.
    Rx,
    /// Neither path selected.
    Unset,
}
/// XB-300 amplifier stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BladeRfXb300Amplifier {
    /// SE2623L power amplifier on the TX path.
    Pa = 0,
    /// TSS-53LNB+ low-noise amplifier on the RX path.
    Lna,
    /// Auxiliary amplifier on the antenna connector.
    Aux,
}
impl RfLinkSession<'_> {
    /// Configures the expansion GPIOs for the XB-300 and powers it up with
    /// the LNA disabled. Requires the board to be initialized.
    pub fn xb300_attach(&mut self) -> impl MaybeFuture<Output = Result<()>> {
        Op::new(async move {
            self.require_initialized().await?;
            let mut val = BLADERF_XB_TX_LED
                | BLADERF_XB_RX_LED
                | BLADERF_XB_TRX_MASK
                | BLADERF_XB_PA_EN
                | BLADERF_XB_LNA_EN
                | BLADERF_XB_CSEL
                | BLADERF_XB_SCLK
                | BLADERF_XB_CS;
            self.nios
                .nios_expansion_gpio_dir_write(0xffffffff, val)
                .await?;
            val = BLADERF_XB_CS | BLADERF_XB_LNA_EN;
            self.nios.nios_expansion_gpio_write(0xffffffff, val).await?;
            Ok(())
        })
    }
    /// Selects the XB-300 on the expansion bus and reads the power detector
    /// once to prime it. `enable` is accepted for API symmetry with the
    /// other boards and currently has no effect, as in libbladeRF.
    pub fn xb300_enable(&mut self, _enable: bool) -> impl MaybeFuture<Output = Result<()>> {
        Op::new(async move {
            self.require_initialized().await?;
            let val = BLADERF_XB_CS | BLADERF_XB_CSEL | BLADERF_XB_LNA_EN;
            self.nios.nios_expansion_gpio_write(0xffffffff, val).await?;
            let _pwr = self.xb300_get_output_power().await?;
            Ok(())
        })
    }
    /// Puts the board into its default state (TRX switch on the TX path).
    pub fn xb300_init(&mut self) -> impl MaybeFuture<Output = Result<()>> {
        Op::new(async move {
            self.require_initialized().await?;
            log::debug!("Setting TRX path to TX");
            self.xb300_set_trx(BladeRfXb300Trx::Tx).await
        })
    }
    /// Sets the transmit/receive switch position.
    pub fn xb300_set_trx(&mut self, trx: BladeRfXb300Trx) -> impl MaybeFuture<Output = Result<()>> {
        Op::new(async move {
            self.require_initialized().await?;
            let mut val = self.nios.nios_expansion_gpio_read().await?;
            val &= !BLADERF_XB_TRX_MASK;
            match trx {
                BladeRfXb300Trx::Rx => val |= BLADERF_XB_TRX_RXN,
                BladeRfXb300Trx::Tx => val |= BLADERF_XB_TRX_TXN,
                BladeRfXb300Trx::Unset => {}
            }
            self.nios.nios_expansion_gpio_write(0xffffffff, val).await
        })
    }
    /// Reads the transmit/receive switch position.
    pub fn xb300_get_trx(&mut self) -> impl MaybeFuture<Output = Result<BladeRfXb300Trx>> {
        Op::new(async move {
            self.require_initialized().await?;
            let mut val = self.nios.nios_expansion_gpio_read().await?;
            val &= BLADERF_XB_TRX_MASK;
            let trx = if val == 0 {
                BladeRfXb300Trx::Unset
            } else if (val & BLADERF_XB_TRX_RXN) != 0 {
                BladeRfXb300Trx::Rx
            } else {
                BladeRfXb300Trx::Tx
            };
            Ok(trx)
        })
    }
    /// Enables or disables one of the amplifier stages (and its LED).
    pub fn xb300_set_amplifier_enable(
        &mut self,
        amp: BladeRfXb300Amplifier,
        enable: bool,
    ) -> impl MaybeFuture<Output = Result<()>> {
        Op::new(async move {
            self.require_initialized().await?;
            let mut val = self.nios.nios_expansion_gpio_read().await?;
            match amp {
                BladeRfXb300Amplifier::Pa => {
                    if enable {
                        val |= BLADERF_XB_TX_LED;
                        val |= BLADERF_XB_PA_EN;
                    } else {
                        val &= !BLADERF_XB_TX_LED;
                        val &= !BLADERF_XB_PA_EN;
                    }
                }
                BladeRfXb300Amplifier::Lna => {
                    if enable {
                        val |= BLADERF_XB_RX_LED;
                        val &= !BLADERF_XB_LNA_EN;
                    } else {
                        val &= !BLADERF_XB_RX_LED;
                        val |= BLADERF_XB_LNA_EN;
                    }
                }
                BladeRfXb300Amplifier::Aux => {
                    if enable {
                        val |= BLADERF_XB_AUX_EN;
                    } else {
                        val &= !BLADERF_XB_AUX_EN;
                    }
                }
            }
            self.nios.nios_expansion_gpio_write(0xffffffff, val).await
        })
    }
    /// Returns whether the given amplifier stage is enabled.
    pub fn xb300_get_amplifier_enable(
        &mut self,
        amp: BladeRfXb300Amplifier,
    ) -> impl MaybeFuture<Output = Result<bool>> {
        Op::new(async move {
            self.require_initialized().await?;
            let val = self.nios.nios_expansion_gpio_read().await?;
            match amp {
                BladeRfXb300Amplifier::Pa => Ok((val & BLADERF_XB_PA_EN) != 0),
                BladeRfXb300Amplifier::Lna => Ok((val & BLADERF_XB_LNA_EN) != 0),
                BladeRfXb300Amplifier::Aux => Ok((val & BLADERF_XB_AUX_EN) != 0),
            }
        })
    }
    /// Reads the RF output power in dBm from the board's power detector.
    pub fn xb300_get_output_power(&mut self) -> impl MaybeFuture<Output = Result<f32>> {
        Op::new(async move {
            self.require_initialized().await?;
            let mut ret = 0;
            let mut val = self.nios.nios_expansion_gpio_read().await?;
            val &= !(BLADERF_XB_CS | BLADERF_XB_SCLK | BLADERF_XB_CSEL);
            self.nios
                .nios_expansion_gpio_write(0xffffffff, BLADERF_XB_SCLK | val)
                .await?;
            self.nios
                .nios_expansion_gpio_write(0xffffffff, BLADERF_XB_CS | BLADERF_XB_SCLK | val)
                .await?;
            for i in 1u32..=14u32 {
                self.nios.nios_expansion_gpio_write(0xffffffff, val).await?;
                self.nios
                    .nios_expansion_gpio_write(0xffffffff, BLADERF_XB_SCLK | val)
                    .await?;
                let rval = self.nios.nios_expansion_gpio_read().await?;
                if (2..=11).contains(&i) {
                    ret |= (!!(rval & BLADERF_XB_DOUT)) << (11 - i);
                }
            }
            let volt = (1.8f32 / 1_024.0f32) * ret as f32;
            let volt2 = volt * volt;
            let volt3 = volt2 * volt;
            let volt4 = volt3 * volt;
            let pwr = -503.933f32 * volt4 + 1_409.489f32 * volt3 - 1_487.84f32 * volt2
                + 722.9793f32 * volt
                - 114.7529f32;
            Ok(pwr)
        })
    }
}
