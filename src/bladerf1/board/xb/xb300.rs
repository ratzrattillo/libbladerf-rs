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

bitflags::bitflags! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    pub(crate) struct Xb300GpioFlags: u32 {
        const AUX_EN = 1 << 1;
        const TX_LED = 1 << 4;
        const RX_LED = 1 << 5;
        const TRX_TXN = 1 << 6;
        const TRX_RXN = 1 << 7;
        const TRX = Self::TRX_TXN.bits() | Self::TRX_RXN.bits();
        const PA_EN = 1 << 9;
        const LNA_EN = 1 << 10;
        const CS = 1 << 16;
        const CSEL = 1 << 18;
        const DOUT = 1 << 20;
        const SCLK = 1 << 22;
        const DETECT = Self::CS.bits() | Self::CSEL.bits() | Self::LNA_EN.bits();
        const _ = !0;
    }
}

impl Xb300GpioFlags {
    fn power_detector_bit(self, clock: u32) -> u32 {
        if (2..=11).contains(&clock) {
            u32::from(self.contains(Self::DOUT)) << (11 - clock)
        } else {
            0
        }
    }
}

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
            let mut val = Xb300GpioFlags::TX_LED
                | Xb300GpioFlags::RX_LED
                | Xb300GpioFlags::TRX
                | Xb300GpioFlags::PA_EN
                | Xb300GpioFlags::LNA_EN
                | Xb300GpioFlags::CSEL
                | Xb300GpioFlags::SCLK
                | Xb300GpioFlags::CS;
            self.nios
                .nios_expansion_gpio_dir_write(0xffffffff, val.bits())
                .await?;
            val = Xb300GpioFlags::CS | Xb300GpioFlags::LNA_EN;
            self.nios
                .nios_expansion_gpio_write(0xffffffff, val.bits())
                .await?;
            Ok(())
        })
    }
    /// Selects the XB-300 on the expansion bus and reads the power detector
    /// once to prime it. `enable` is accepted for API symmetry with the
    /// other boards and currently has no effect, as in libbladeRF.
    pub fn xb300_enable(&mut self, _enable: bool) -> impl MaybeFuture<Output = Result<()>> {
        Op::new(async move {
            self.require_initialized().await?;
            self.nios
                .nios_expansion_gpio_write(0xffffffff, Xb300GpioFlags::DETECT.bits())
                .await?;
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
            let mut val =
                Xb300GpioFlags::from_bits_retain(self.nios.nios_expansion_gpio_read().await?);
            val.remove(Xb300GpioFlags::TRX);
            match trx {
                BladeRfXb300Trx::Rx => val.insert(Xb300GpioFlags::TRX_RXN),
                BladeRfXb300Trx::Tx => val.insert(Xb300GpioFlags::TRX_TXN),
                BladeRfXb300Trx::Unset => {}
            }
            self.nios
                .nios_expansion_gpio_write(0xffffffff, val.bits())
                .await
        })
    }
    /// Reads the transmit/receive switch position.
    pub fn xb300_get_trx(&mut self) -> impl MaybeFuture<Output = Result<BladeRfXb300Trx>> {
        Op::new(async move {
            self.require_initialized().await?;
            let val = Xb300GpioFlags::from_bits_retain(self.nios.nios_expansion_gpio_read().await?)
                & Xb300GpioFlags::TRX;
            let trx = if val.is_empty() {
                BladeRfXb300Trx::Unset
            } else if val.contains(Xb300GpioFlags::TRX_RXN) {
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
            let mut val =
                Xb300GpioFlags::from_bits_retain(self.nios.nios_expansion_gpio_read().await?);
            match amp {
                BladeRfXb300Amplifier::Pa => {
                    val.set(Xb300GpioFlags::TX_LED | Xb300GpioFlags::PA_EN, enable);
                }
                BladeRfXb300Amplifier::Lna => {
                    val.set(Xb300GpioFlags::RX_LED, enable);
                    val.set(Xb300GpioFlags::LNA_EN, !enable);
                }
                BladeRfXb300Amplifier::Aux => {
                    val.set(Xb300GpioFlags::AUX_EN, enable);
                }
            }
            self.nios
                .nios_expansion_gpio_write(0xffffffff, val.bits())
                .await
        })
    }
    /// Returns whether the given amplifier stage is enabled.
    pub fn xb300_get_amplifier_enable(
        &mut self,
        amp: BladeRfXb300Amplifier,
    ) -> impl MaybeFuture<Output = Result<bool>> {
        Op::new(async move {
            self.require_initialized().await?;
            let val = Xb300GpioFlags::from_bits_retain(self.nios.nios_expansion_gpio_read().await?);
            match amp {
                BladeRfXb300Amplifier::Pa => Ok(val.contains(Xb300GpioFlags::PA_EN)),
                BladeRfXb300Amplifier::Lna => Ok(val.contains(Xb300GpioFlags::LNA_EN)),
                BladeRfXb300Amplifier::Aux => Ok(val.contains(Xb300GpioFlags::AUX_EN)),
            }
        })
    }
    /// Reads the RF output power in dBm from the board's power detector.
    pub fn xb300_get_output_power(&mut self) -> impl MaybeFuture<Output = Result<f32>> {
        Op::new(async move {
            self.require_initialized().await?;
            let mut ret = 0;
            let mut val =
                Xb300GpioFlags::from_bits_retain(self.nios.nios_expansion_gpio_read().await?);
            val.remove(Xb300GpioFlags::CS | Xb300GpioFlags::SCLK | Xb300GpioFlags::CSEL);
            self.nios
                .nios_expansion_gpio_write(0xffffffff, (Xb300GpioFlags::SCLK | val).bits())
                .await?;
            self.nios
                .nios_expansion_gpio_write(
                    0xffffffff,
                    (Xb300GpioFlags::CS | Xb300GpioFlags::SCLK | val).bits(),
                )
                .await?;
            for i in 1u32..=14u32 {
                self.nios
                    .nios_expansion_gpio_write(0xffffffff, val.bits())
                    .await?;
                self.nios
                    .nios_expansion_gpio_write(0xffffffff, (Xb300GpioFlags::SCLK | val).bits())
                    .await?;
                let rval =
                    Xb300GpioFlags::from_bits_retain(self.nios.nios_expansion_gpio_read().await?);
                ret |= rval.power_detector_bit(i);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn power_detector_normalizes_gpio_into_a_ten_bit_adc_word() {
        let high = Xb300GpioFlags::from_bits_retain(0xffff_ffff);
        let low = Xb300GpioFlags::from_bits_retain(0xffef_ffff);
        assert_eq!(
            (1..=14)
                .map(|clock| high.power_detector_bit(clock))
                .sum::<u32>(),
            1023
        );
        assert_eq!(
            (1..=14)
                .map(|clock| low.power_detector_bit(clock))
                .sum::<u32>(),
            0
        );
        assert_eq!(high.power_detector_bit(1), 0);
        assert_eq!(high.power_detector_bit(2), 512);
        assert_eq!(high.power_detector_bit(11), 1);
        assert_eq!(high.power_detector_bit(12), 0);
    }
}
