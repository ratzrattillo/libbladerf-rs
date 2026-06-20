//! XB-300 amplifier board support.
//!
//! The XB-300 provides a TSS-53LNB+ low-noise amplifier (LNA) on the RX path,
//! a SE2623L power amplifier (PA) on the TX path, and an auxiliary amplifier
//! (Amp 3) for the antenna connector. It also includes a power detector that
//! measures RF output power via SPI-over-GPIO.

use crate::bladerf1::board::RfLinkSession;
use crate::error::Result;
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BladeRfXb300Trx {
    Tx = 0,
    Rx,
    Unset,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BladeRfXb300Amplifier {
    Pa = 0,
    Lna,
    Aux,
}
impl RfLinkSession<'_> {
    pub fn xb300_attach(&mut self) -> Result<()> {
        self.require_initialized()?;
        let mut val = BLADERF_XB_TX_LED
            | BLADERF_XB_RX_LED
            | BLADERF_XB_TRX_MASK
            | BLADERF_XB_PA_EN
            | BLADERF_XB_LNA_EN
            | BLADERF_XB_CSEL
            | BLADERF_XB_SCLK
            | BLADERF_XB_CS;
        self.nios.nios_expansion_gpio_dir_write(0xffffffff, val)?;
        val = BLADERF_XB_CS | BLADERF_XB_LNA_EN;
        self.nios.nios_expansion_gpio_write(0xffffffff, val)?;
        Ok(())
    }
    pub fn xb300_enable(&mut self, _enable: bool) -> Result<()> {
        self.require_initialized()?;
        let val = BLADERF_XB_CS | BLADERF_XB_CSEL | BLADERF_XB_LNA_EN;
        self.nios.nios_expansion_gpio_write(0xffffffff, val)?;
        let _pwr = self.xb300_get_output_power()?;
        Ok(())
    }
    pub fn xb300_init(&mut self) -> Result<()> {
        self.require_initialized()?;
        log::debug!("Setting TRX path to TX");
        self.xb300_set_trx(BladeRfXb300Trx::Tx)
    }
    pub fn xb300_set_trx(&mut self, trx: BladeRfXb300Trx) -> Result<()> {
        self.require_initialized()?;
        let mut val = self.nios.nios_expansion_gpio_read()?;
        val &= !BLADERF_XB_TRX_MASK;
        match trx {
            BladeRfXb300Trx::Rx => val |= BLADERF_XB_TRX_RXN,
            BladeRfXb300Trx::Tx => val |= BLADERF_XB_TRX_TXN,
            BladeRfXb300Trx::Unset => {}
        }
        self.nios.nios_expansion_gpio_write(0xffffffff, val)
    }
    pub fn xb300_get_trx(&mut self) -> Result<BladeRfXb300Trx> {
        self.require_initialized()?;
        let mut val = self.nios.nios_expansion_gpio_read()?;
        val &= BLADERF_XB_TRX_MASK;
        let trx = if val == 0 {
            BladeRfXb300Trx::Unset
        } else if (val & BLADERF_XB_TRX_RXN) != 0 {
            BladeRfXb300Trx::Rx
        } else {
            BladeRfXb300Trx::Tx
        };
        Ok(trx)
    }
    pub fn xb300_set_amplifier_enable(
        &mut self,
        amp: BladeRfXb300Amplifier,
        enable: bool,
    ) -> Result<()> {
        self.require_initialized()?;
        let mut val = self.nios.nios_expansion_gpio_read()?;
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
        self.nios.nios_expansion_gpio_write(0xffffffff, val)
    }
    pub fn xb300_get_amplifier_enable(&mut self, amp: BladeRfXb300Amplifier) -> Result<bool> {
        self.require_initialized()?;
        let val = self.nios.nios_expansion_gpio_read()?;
        match amp {
            BladeRfXb300Amplifier::Pa => Ok((val & BLADERF_XB_PA_EN) != 0),
            BladeRfXb300Amplifier::Lna => Ok((val & BLADERF_XB_LNA_EN) != 0),
            BladeRfXb300Amplifier::Aux => Ok((val & BLADERF_XB_AUX_EN) != 0),
        }
    }
    pub fn xb300_get_output_power(&mut self) -> Result<f32> {
        self.require_initialized()?;
        let mut ret = 0;
        let mut val = self.nios.nios_expansion_gpio_read()?;
        val &= !(BLADERF_XB_CS | BLADERF_XB_SCLK | BLADERF_XB_CSEL);
        self.nios
            .nios_expansion_gpio_write(0xffffffff, BLADERF_XB_SCLK | val)?;
        self.nios
            .nios_expansion_gpio_write(0xffffffff, BLADERF_XB_CS | BLADERF_XB_SCLK | val)?;
        for i in 1u32..=14u32 {
            self.nios.nios_expansion_gpio_write(0xffffffff, val)?;
            self.nios
                .nios_expansion_gpio_write(0xffffffff, BLADERF_XB_SCLK | val)?;
            let rval = self.nios.nios_expansion_gpio_read()?;
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
    }
}
