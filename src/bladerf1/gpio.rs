use super::{Band, GainMode, RxMux};
use crate::{Channel, Result};
use nusb::Speed;

const RX_MUX_SHIFT: u32 = 8;
const RX_MUX_MASK: u32 = 7 << RX_MUX_SHIFT;

bitflags::bitflags! {
    /// Flags and lossless raw contents of the config GPIO register.
    ///
    /// All 32 bits are retained, including unnamed bits and encoded selector
    /// fields. Use [`Self::from_bits_retain`] and [`Self::bits`] for raw access.
    /// [`Self::all`] covers the entire register, not just the named flags.
    /// Use the session's high-level operations for validated selector and
    /// sample-format configuration.
    ///
    /// # Examples
    ///
    /// ```
    /// use libbladerf_rs::bladerf1::GpioFlags;
    ///
    /// let mut gpio = GpioFlags::from_bits_retain(0x8000_0057);
    /// gpio.set(GpioFlags::AGC_ENABLE, true);
    /// assert!(gpio.contains(GpioFlags::AGC_ENABLE));
    /// gpio.remove(GpioFlags::AGC_ENABLE);
    /// assert_eq!(gpio.bits(), 0x8000_0057);
    /// ```
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    pub struct GpioFlags: u32 {
        /// Enables small DMA transfers for High-Speed USB.
        const SMALL_DMA_XFER = 1 << 7;
        /// Enables timestamp metadata.
        const TIMESTAMP = 1 << 16;
        /// Divides the timestamp clock for complex sample counting.
        const TIMESTAMP_DIV2 = 1 << 17;
        /// Enables automatic RX gain control.
        const AGC_ENABLE = 1 << 18;
        /// Enables packet metadata.
        const PACKET = 1 << 19;
        /// Enables SC8 on FPGA designs implementing that mode.
        const EIGHT_BIT_MODE = 1 << 20;
        /// Enables packed SC16 on FPGA designs implementing that mode.
        const HIGHLY_PACKED_MODE = 1 << 21;
        const _ = !0;
    }
}

impl GpioFlags {
    pub(crate) fn is_initialized(self) -> bool {
        (self.bits() & 0x7f) != 0
    }

    pub(crate) fn apply_usb_speed(&mut self, speed: Speed) {
        self.set(Self::SMALL_DMA_XFER, speed == Speed::High);
    }

    pub(crate) fn set_gain_mode(&mut self, mode: GainMode) {
        self.set(Self::AGC_ENABLE, mode == GainMode::Default);
    }

    pub(crate) fn gain_mode(self) -> GainMode {
        if self.contains(Self::AGC_ENABLE) {
            GainMode::Default
        } else {
            GainMode::Mgc
        }
    }

    pub(crate) fn set_rx_mux(&mut self, mode: RxMux) {
        *self =
            Self::from_bits_retain((self.bits() & !RX_MUX_MASK) | ((mode as u32) << RX_MUX_SHIFT));
    }

    pub(crate) fn rx_mux(self) -> Result<RxMux> {
        RxMux::try_from((self.bits() & RX_MUX_MASK) >> RX_MUX_SHIFT)
    }

    pub(crate) fn set_band(&mut self, channel: Channel, band: Band) {
        let shift = match channel {
            Channel::Tx => 3,
            Channel::Rx => 5,
        };
        let value = match band {
            Band::Low => 2,
            Band::High => 1,
        };
        *self = Self::from_bits_retain((self.bits() & !(3 << shift)) | (value << shift));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn register_values() -> impl Iterator<Item = u32> {
        [0, 0x57, 0x8000_0057, 0x4000_0700, u32::MAX]
            .into_iter()
            .chain((0..32).flat_map(|bit| [1 << bit, !(1 << bit)]))
    }

    #[test]
    fn gain_and_dma_updates_preserve_other_register_bits() {
        assert_eq!(GpioFlags::all().bits(), u32::MAX);
        for raw in register_values() {
            for (mode, agc) in [(GainMode::Default, 0x0004_0000), (GainMode::Mgc, 0)] {
                let mut gpio = GpioFlags::from_bits_retain(raw);
                gpio.set_gain_mode(mode);
                assert_eq!(gpio.bits(), (raw & !0x0004_0000) | agc);
                assert_eq!(gpio.gain_mode(), mode);
                gpio.set_gain_mode(mode);
                assert_eq!(gpio.bits(), (raw & !0x0004_0000) | agc);
            }
            for (speed, dma) in [
                (Speed::High, 0x80),
                (Speed::Super, 0),
                (Speed::SuperPlus, 0),
            ] {
                let mut gpio = GpioFlags::from_bits_retain(raw);
                gpio.apply_usb_speed(speed);
                assert_eq!(gpio.bits(), (raw & !0x80) | dma);
                gpio.apply_usb_speed(speed);
                assert_eq!(gpio.bits(), (raw & !0x80) | dma);
            }
        }
    }

    #[test]
    fn selector_updates_preserve_all_other_fields() {
        for raw in register_values() {
            for (mode, bits) in [
                (RxMux::MuxBaseband, 0),
                (RxMux::Mux12BitCounter, 0x100),
                (RxMux::Mux32BitCounter, 0x200),
                (RxMux::MuxDigitalLoopback, 0x400),
            ] {
                let mut gpio = GpioFlags::from_bits_retain(raw);
                gpio.set_rx_mux(mode);
                assert_eq!(gpio.bits(), (raw & !0x700) | bits);
                assert_eq!(gpio.rx_mux().unwrap(), mode);
                gpio.set_rx_mux(mode);
                assert_eq!(gpio.bits(), (raw & !0x700) | bits);
            }
            for (channel, band, mask, bits) in [
                (Channel::Tx, Band::Low, 0x18, 0x10),
                (Channel::Tx, Band::High, 0x18, 0x08),
                (Channel::Rx, Band::Low, 0x60, 0x40),
                (Channel::Rx, Band::High, 0x60, 0x20),
            ] {
                let mut gpio = GpioFlags::from_bits_retain(raw);
                gpio.set_band(channel, band);
                assert_eq!(gpio.bits(), (raw & !mask) | bits);
                gpio.set_band(channel, band);
                assert_eq!(gpio.bits(), (raw & !mask) | bits);
            }
        }
        for mux in 0..8 {
            let gpio = GpioFlags::from_bits_retain(0x8000_0057 | (mux << 8));
            assert_eq!(gpio.rx_mux().is_ok(), matches!(mux, 0 | 1 | 2 | 4));
        }
    }

    #[test]
    fn initialization_depends_only_on_the_lower_seven_bits() {
        assert!(GpioFlags::from_bits_retain(0x57).is_initialized());
        assert!(!GpioFlags::empty().is_initialized());
        assert!(!GpioFlags::from_bits_retain(0xffff_ff80).is_initialized());
        for bit in 0..32 {
            let gpio = GpioFlags::from_bits_retain(1 << bit);
            assert_eq!(gpio.is_initialized(), bit < 7);
        }
    }
}
