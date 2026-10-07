use super::{Band, GainMode, RxMux};
use crate::{Channel, Result};
use nusb::Speed;

const TX_BAND_SHIFT: u32 = 3;
const RX_BAND_SHIFT: u32 = 5;
const BAND_SELECT_MASK: u32 = 0b11;
const TX_BAND_MASK: u32 = BAND_SELECT_MASK << TX_BAND_SHIFT;
const RX_BAND_MASK: u32 = BAND_SELECT_MASK << RX_BAND_SHIFT;
const RF_SWITCH_LOW_BAND: u32 = 0b10;
const RF_SWITCH_HIGH_BAND: u32 = 0b01;
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
    /// let mut gpio = GpioFlags::LMS_RESET_N | GpioFlags::LMS_RX_ENABLE;
    /// gpio.set(GpioFlags::AGC_ENABLE, true);
    /// assert!(gpio.contains(GpioFlags::AGC_ENABLE));
    /// gpio.remove(GpioFlags::AGC_ENABLE);
    /// assert_eq!(gpio, GpioFlags::LMS_RESET_N | GpioFlags::LMS_RX_ENABLE);
    /// ```
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    pub struct GpioFlags: u32 {
        /// Releases the LMS6002D's active-low hardware reset when set.
        const LMS_RESET_N = 1 << 0;
        /// Enables the LMS6002D receive path through its RX enable pin.
        const LMS_RX_ENABLE = 1 << 1;
        /// Enables the LMS6002D transmit path through its TX enable pin.
        const LMS_TX_ENABLE = 1 << 2;
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
    pub(crate) const LMS_CONTROL: Self = Self::LMS_RESET_N
        .union(Self::LMS_RX_ENABLE)
        .union(Self::LMS_TX_ENABLE);

    pub(crate) fn for_initialization() -> Self {
        let mut gpio = Self::LMS_CONTROL;
        gpio.set_band(Channel::Rx, Band::Low);
        gpio.set_band(Channel::Tx, Band::Low);
        gpio
    }

    pub(crate) fn is_initialized(self) -> bool {
        (self.bits() & (Self::LMS_CONTROL.bits() | TX_BAND_MASK | RX_BAND_MASK)) != 0
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
        let (mask, shift) = match channel {
            Channel::Tx => (TX_BAND_MASK, TX_BAND_SHIFT),
            Channel::Rx => (RX_BAND_MASK, RX_BAND_SHIFT),
        };
        let value = match band {
            Band::Low => RF_SWITCH_LOW_BAND,
            Band::High => RF_SWITCH_HIGH_BAND,
        };
        *self = Self::from_bits_retain((self.bits() & !mask) | (value << shift));
    }

    #[cfg(feature = "xb200")]
    pub(crate) fn set_xb200_mode(&mut self) {
        const XB_MODE_SHIFT: u32 = 30;
        const XB_MODE_MASK: u32 = 0b11 << XB_MODE_SHIFT;
        const XB_MODE_TRANSVERTER: u32 = 0b10 << XB_MODE_SHIFT;

        *self = Self::from_bits_retain((self.bits() & !XB_MODE_MASK) | XB_MODE_TRANSVERTER);
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
        assert_eq!(GpioFlags::LMS_CONTROL.bits(), 0x07);
        assert_eq!(GpioFlags::for_initialization().bits(), 0x57);
        assert!(GpioFlags::from_bits_retain(0x57).is_initialized());
        assert!(!GpioFlags::empty().is_initialized());
        assert!(!GpioFlags::from_bits_retain(0xffff_ff80).is_initialized());
        for bit in 0..32 {
            let gpio = GpioFlags::from_bits_retain(1 << bit);
            assert_eq!(gpio.is_initialized(), bit < 7);
        }
    }

    #[cfg(feature = "xb200")]
    #[test]
    fn xb200_mode_replaces_the_selector_without_touching_other_bits() {
        for raw in register_values() {
            let mut gpio = GpioFlags::from_bits_retain(raw);
            gpio.set_xb200_mode();
            assert_eq!(gpio.bits(), (raw & 0x3fff_ffff) | 0x8000_0000);
        }
    }
}
