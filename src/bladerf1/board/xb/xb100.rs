//! XB-100 LED expansion board GPIO control.
//!
//! The XB-100 provides 8 user LEDs (D1–D8) and one tri-color LED (TLED
//! with red, green, blue components), all accessible through the
//! expansion GPIO.

use crate::bladerf1::board::RfLinkSession;
use crate::error::Result;
use crate::maybe_future::Op;
use nusb::MaybeFuture;

bitflags::bitflags! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    pub(crate) struct Xb100GpioFlags: u32 {
        const LED_D1 = 1 << 23;
        const LED_D2 = 1 << 31;
        const LED_D3 = 1 << 29;
        const LED_D4 = 1 << 27;
        const LED_D5 = 1 << 22;
        const LED_D6 = 1 << 24;
        const LED_D7 = 1 << 30;
        const LED_D8 = 1 << 28;
        const TLED_RED = 1 << 21;
        const TLED_GREEN = 1 << 20;
        const TLED_BLUE = 1 << 19;
    }
}

impl Xb100GpioFlags {
    pub(crate) fn is_enabled(self) -> bool {
        self.bits() != u32::MAX && self.contains(Self::all())
    }
}

impl RfLinkSession<'_> {
    /// Prepares the XB-100 board. Currently a no-op placeholder.
    pub fn xb100_attach(&mut self) -> impl MaybeFuture<Output = Result<()>> {
        Op::new(async move {
            self.require_initialized().await?;
            Ok(())
        })
    }
    /// Enables the XB-100 board. When enabled, configures all LED GPIO pins
    /// as outputs and turns the LEDs on. Requires the board to be initialized.
    pub fn xb100_enable(&mut self, enable: bool) -> impl MaybeFuture<Output = Result<()>> {
        Op::new(async move {
            self.require_initialized().await?;
            if enable {
                let leds = Xb100GpioFlags::all();
                self.nios
                    .nios_expansion_gpio_dir_write(leds.bits(), leds.bits())
                    .await?;
                self.nios
                    .nios_expansion_gpio_write(leds.bits(), leds.bits())
                    .await?;
            }
            Ok(())
        })
    }
    /// Initializes the XB-100 board. Currently a no-op placeholder.
    pub fn xb100_init(&mut self) -> impl MaybeFuture<Output = Result<()>> {
        Op::new(async move {
            self.require_initialized().await?;
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn led_mask_covers_all_eleven_upper_gpio_pins() {
        assert_eq!(Xb100GpioFlags::all().bits(), 0xf9f8_0000);
        assert_eq!(Xb100GpioFlags::LED_D2.bits(), 0x8000_0000);
    }

    #[test]
    fn detection_requires_the_full_led_pattern() {
        assert!(Xb100GpioFlags::from_bits_retain(0xf9f8_0000).is_enabled());
        for raw in [0, u32::MAX, 0x3c00_0800, 0x3c00_383e, 0x0005_0400] {
            assert!(!Xb100GpioFlags::from_bits_retain(raw).is_enabled());
        }
    }
}
