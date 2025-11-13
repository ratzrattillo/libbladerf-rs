use crate::bladerf1::BladeRf1;
use crate::nios::Nios;
use crate::{Error, Result};
use nusb::Interface;
use std::sync::{Arc, Mutex};

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

/// XB-300 TRX setting
#[derive(Debug)]
pub enum BladeRfXb300Trx {
    /// Invalid TRX selection
    Inval = -1,
    /// TRX antenna operates as TX
    Tx = 0,
    /// TRX antenna operates as RX
    Rx,
    /// TRX antenna unset
    Unset,
}

/// XB-300 Amplifier selection
#[derive(Debug)]
pub enum BladeRfXb300Amplifier {
    ///  Invalid amplifier selection
    Inval = -1,
    ///  TX Power amplifier
    Pa = 0,
    ///  RX LNA
    Lna,
    /// Auxillary Power amplifier
    Aux,
}

impl BladeRf1 {
    /// Trying to detect if XB300 is enabled by reading the LNA enablement status Flag,
    /// which is set in xb300_enable(). Might be not the best, or correct way.
    pub fn xb300_is_enabled(interface: &Arc<Mutex<Interface>>) -> Result<bool> {
        Ok(interface.lock().unwrap().nios_expansion_gpio_dir_read()?
            & (BLADERF_XB_CS | BLADERF_XB_CSEL | BLADERF_XB_LNA_EN)
            != 0)
    }

    /// Attach the XB300 expansion board
    pub fn xb300_attach(&self) -> Result<()> {
        let mut val = BLADERF_XB_TX_LED
            | BLADERF_XB_RX_LED
            | BLADERF_XB_TRX_MASK
            | BLADERF_XB_PA_EN
            | BLADERF_XB_LNA_EN
            | BLADERF_XB_CSEL
            | BLADERF_XB_SCLK
            | BLADERF_XB_CS;

        let interface = self.interface.lock().unwrap();
        interface.nios_expansion_gpio_dir_write(0xffffffff, val)?;

        val = BLADERF_XB_CS | BLADERF_XB_LNA_EN;
        interface.nios_expansion_gpio_write(0xffffffff, val)?;

        Ok(())
    }

    /// This method does not do anything. Detach-operations are not required/possible for XB200.
    pub fn xb300_detach(&mut self) -> Result<()> {
        Ok(())
    }

    /// The XB300 expansion board has to be enabled after attaching in order to be used.
    pub fn xb300_enable(&self, _enable: bool) -> Result<()> {
        // TODO: Why? These values are already being set in xb300_attach()
        let val = BLADERF_XB_CS | BLADERF_XB_CSEL | BLADERF_XB_LNA_EN;
        self.interface
            .lock()
            .unwrap()
            .nios_expansion_gpio_write(0xffffffff, val)?;

        // The value is discarded, but xb300_get_output_power
        // also contains some initialization code.
        let _pwr = self.xb300_get_output_power()?;

        Ok(())
    }

    /// XB300 init routine sets the TRX path by default to TX.
    pub fn xb300_init(&self) -> Result<()> {
        log::debug!("Setting TRX path to TX");
        self.xb300_set_trx(BladeRfXb300Trx::Tx)
    }

    /// TRX switch for receiving and transmitting from single antenna port
    pub fn xb300_set_trx(&self, trx: BladeRfXb300Trx) -> Result<()> {
        let mut val = self.interface.lock().unwrap().nios_expansion_gpio_read()?;
        val &= !BLADERF_XB_TRX_MASK;

        match trx {
            BladeRfXb300Trx::Rx => val |= BLADERF_XB_TRX_RXN,

            BladeRfXb300Trx::Tx => val |= BLADERF_XB_TRX_TXN,

            BladeRfXb300Trx::Unset => {}

            _ => {
                log::error!("Invalid TRX option: {trx:?}");
                return Err(Error::Invalid);
            }
        }

        self.interface
            .lock()
            .unwrap()
            .nios_expansion_gpio_write(0xffffffff, val)
    }

    /// Get the currently selected TRX path (Can be Rx or Tx or Unset)
    pub fn xb300_get_trx(&self) -> Result<BladeRfXb300Trx> {
        let mut val = self.interface.lock().unwrap().nios_expansion_gpio_read()?;
        val &= BLADERF_XB_TRX_MASK;

        let trx = if val == 0 {
            BladeRfXb300Trx::Unset
        } else if val & BLADERF_XB_TRX_RXN != 0 {
            BladeRfXb300Trx::Rx
        } else {
            BladeRfXb300Trx::Tx
        };

        // TODO: Probably not required!
        // Sanity check
        // match trx {
        //     case BladeRfXb300Trx::TX:
        //     case BladeRfXb300Trx::RX:
        //     case BladeRfXb300Trx::UNSET:
        //
        //     _:
        //         log::debug!("Read back invalid TRX setting value: %d", *trx);
        //         return Err(anyhow!("Invalid TRX option: %d"));
        // }
        //
        // return status;
        Ok(trx)
    }

    /// Enable one of the XB300 supported amplifiers (Pa, LNA or Aux)
    pub fn xb300_set_amplifier_enable(
        &self,
        amp: BladeRfXb300Amplifier,
        enable: bool,
    ) -> Result<()> {
        let mut val = self.interface.lock().unwrap().nios_expansion_gpio_read()?;

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
            _ => {
                log::error!("Invalid amplifier selection: {amp:?}");
                return Err(Error::Invalid);
            }
        }

        self.interface
            .lock()
            .unwrap()
            .nios_expansion_gpio_write(0xffffffff, val)
    }

    /// Check if a specific XB300 amplifier is currently active.
    pub fn xb300_get_amplifier_enable(&self, amp: BladeRfXb300Amplifier) -> Result<bool> {
        let val = self.interface.lock().unwrap().nios_expansion_gpio_read()?;

        match amp {
            BladeRfXb300Amplifier::Pa => Ok(val & BLADERF_XB_PA_EN != 0),
            BladeRfXb300Amplifier::Lna => Ok(val & BLADERF_XB_LNA_EN != 0),
            BladeRfXb300Amplifier::Aux => Ok(val & BLADERF_XB_AUX_EN != 0),
            _ => {
                log::error!("Read back invalid amplifier setting: {amp:?}");
                Err(Error::Invalid)
            }
        }
    }

    /// Return the output power of the XB300
    pub fn xb300_get_output_power(&self) -> Result<f32> {
        let mut ret = 0;

        let interface = self.interface.lock().unwrap();

        let mut val = interface.nios_expansion_gpio_read()?;

        val &= !(BLADERF_XB_CS | BLADERF_XB_SCLK | BLADERF_XB_CSEL);

        interface.nios_expansion_gpio_write(0xffffffff, BLADERF_XB_SCLK | val)?;
        interface.nios_expansion_gpio_write(0xffffffff, BLADERF_XB_CS | BLADERF_XB_SCLK | val)?;

        for i in 1u32..=14u32 {
            interface.nios_expansion_gpio_write(0xffffffff, val)?;
            interface.nios_expansion_gpio_write(0xffffffff, BLADERF_XB_SCLK | val)?;

            let rval = self.interface.lock().unwrap().nios_expansion_gpio_read()?;

            if (2..=11).contains(&i) {
                ret |= (!!(rval & BLADERF_XB_DOUT)) << (11 - i);
            }
        }

        let volt = (1.8f32 / 1024.0f32) * ret as f32;
        let volt2 = volt * volt;
        let volt3 = volt2 * volt;
        let volt4 = volt3 * volt;

        let pwr = -503.933f32 * volt4 + 1409.489f32 * volt3 - 1487.84f32 * volt2
            + 722.9793f32 * volt
            - 114.7529f32;

        Ok(pwr)
    }
}
