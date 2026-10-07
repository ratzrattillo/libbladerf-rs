//! XB-200 transverter board support.
//!
//! The XB-200 extends the BladeRF1 to cover lower HF/VHF bands (down to
//! ~30 MHz) using a 1248 MHz ADF4351-based local oscillator with high-side
//! injection: the desired RF = 1248 MHz - LO. The board includes:
//!
//! * Filter banks for 6 m (50 MHz), 2 m (144 MHz), 1.25 m (222 MHz), and custom bands.
//! * Automatic filter selection based on target frequency and loss threshold (1 dB or 3 dB).
//! * Spectral inversion correction via I/Q swap on the LMS6002D (Mix path).
//! * Bypass mode for direct passthrough without downconversion.

use crate::bladerf1::GpioFlags;
use crate::bladerf1::board::RfLinkSession;
use crate::bladerf1::hardware::si5338::OutputFlags;
use crate::channel::Channel;
use crate::error::{Error, Result};
use crate::maybe_future::Op;
use nusb::MaybeFuture;
use std::ops::RangeInclusive;

const TX_FILTER_SHIFT: u32 = 26;
const RX_FILTER_SHIFT: u32 = 28;
const SYNTH_REGISTER2_CONFIG: u32 = 0x6000_8e42;
const SYNTH_MUXOUT_DIGITAL_LOCK_DETECT: u32 = 0b110 << 26;

bitflags::bitflags! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    pub(crate) struct Xb200GpioFlags: u32 {
        const MUXOUT = 1 << 0;
        const SYNTH_CS = 1 << 1;
        const TX_PATH_MIX = 1 << 2;
        const TX_PATH_BYPASS = 1 << 3;
        const TX_PATH = Self::TX_PATH_MIX.bits() | Self::TX_PATH_BYPASS.bits();
        const RX_PATH_MIX = 1 << 4;
        const RX_PATH_BYPASS = 1 << 5;
        const RX_PATH = Self::RX_PATH_MIX.bits() | Self::RX_PATH_BYPASS.bits();
        const RF_ON = 1 << 11;
        const TX_ENABLE = 1 << 12;
        const RX_ENABLE = 1 << 13;
        const TX_RF_SW2 = 1 << TX_FILTER_SHIFT;
        const TX_RF_SW1 = 1 << (TX_FILTER_SHIFT + 1);
        const TX_FILTER = Self::TX_RF_SW1.bits() | Self::TX_RF_SW2.bits();
        const RX_RF_SW2 = 1 << RX_FILTER_SHIFT;
        const RX_RF_SW1 = 1 << (RX_FILTER_SHIFT + 1);
        const RX_FILTER = Self::RX_RF_SW1.bits() | Self::RX_RF_SW2.bits();
        const OUTPUTS = Self::SYNTH_CS.bits() | Self::TX_PATH.bits() | Self::RX_PATH.bits()
            | Self::RF_ON.bits() | Self::TX_ENABLE.bits() | Self::RX_ENABLE.bits()
            | Self::TX_FILTER.bits() | Self::RX_FILTER.bits();
        const _ = !0;
    }

    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    struct IqSwapFlags: u8 {
        const RX = 1 << 6;
        const TX = 1 << 3;
        const _ = !0;
    }

    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    struct SynthControlFlags: u32 {
        const INTEGER_N_LOCK_DETECT = 1 << 8;
        const _ = !0;
    }
}

impl Xb200GpioFlags {
    fn filter(self, channel: Channel) -> Result<Xb200Filter> {
        let (mask, shift) = match channel {
            Channel::Rx => (Self::RX_FILTER, RX_FILTER_SHIFT),
            Channel::Tx => (Self::TX_FILTER, TX_FILTER_SHIFT),
        };
        Xb200Filter::try_from((self & mask).bits() >> shift)
    }

    fn set_filter(&mut self, channel: Channel, filter: Xb200Filter) -> Result<()> {
        if matches!(filter, Xb200Filter::Auto1db | Xb200Filter::Auto3db) {
            return Err(Error::Argument(
                "automatic XB200 filter mode is not a mux selection".into(),
            ));
        }
        let (mask, shift) = match channel {
            Channel::Rx => (Self::RX_FILTER, RX_FILTER_SHIFT),
            Channel::Tx => (Self::TX_FILTER, TX_FILTER_SHIFT),
        };
        self.remove(mask);
        self.insert(Self::from_bits_retain((filter as u32) << shift));
        Ok(())
    }

    fn set_path(&mut self, channel: Channel, path: Xb200Path) {
        let (paths, enable, mix, bypass) = match channel {
            Channel::Rx => (
                Self::RX_PATH,
                Self::RX_ENABLE,
                Self::RX_PATH_MIX,
                Self::RX_PATH_BYPASS,
            ),
            Channel::Tx => (
                Self::TX_PATH,
                Self::TX_ENABLE,
                Self::TX_PATH_MIX,
                Self::TX_PATH_BYPASS,
            ),
        };
        self.insert(Self::RF_ON);
        self.remove(paths | enable);
        self.insert(match path {
            Xb200Path::Mix => enable | mix,
            Xb200Path::Bypass => bypass,
        });
    }

    fn path(self, channel: Channel) -> Xb200Path {
        let mix = match channel {
            Channel::Rx => Self::RX_PATH_MIX,
            Channel::Tx => Self::TX_PATH_MIX,
        };
        if self.contains(mix) {
            Xb200Path::Mix
        } else {
            Xb200Path::Bypass
        }
    }
}

type FilterEntry = (RangeInclusive<u64>, Xb200Filter);
/// Frequency ranges mapped to filter banks for automatic 1 dB loss selection.
pub(crate) const AUTO_1DB_FILTERS: &[FilterEntry] = &[
    (37_774_405..=59_535_436, Xb200Filter::_50M),
    (128_326_173..=166_711_171, Xb200Filter::_144M),
    (187_593_160..=245_346_403, Xb200Filter::_222M),
];
/// Frequency ranges mapped to filter banks for automatic 3 dB loss selection.
pub(crate) const AUTO_3DB_FILTERS: &[FilterEntry] = &[
    (34_782_924..=61_899_260, Xb200Filter::_50M),
    (121_956_957..=178_444_099, Xb200Filter::_144M),
    (177_522_675..=260_140_935, Xb200Filter::_222M),
];
/// XB-200 filter bank selection.
///
/// The board contains discrete filter banks for 6 m (50 MHz), 2 m (144 MHz),
/// and 1.25 m (222 MHz) bands, plus a custom passthrough and two automatic
/// modes that select based on frequency and loss threshold.
#[derive(PartialEq, Eq, Debug, Clone, Copy)]
#[repr(u8)]
pub enum Xb200Filter {
    /// 6 m band filter (~50 MHz center).
    _50M = 0,
    /// 2 m band filter (~144 MHz center).
    _144M = 1,
    /// 1.25 m band filter (~222 MHz center).
    _222M = 2,
    /// Custom filter passthrough (no band filtering).
    Custom = 3,
    /// Automatic selection using the 1 dB loss threshold table.
    Auto1db = 4,
    /// Automatic selection using the 3 dB loss threshold table.
    Auto3db = 5,
}
impl TryFrom<u32> for Xb200Filter {
    type Error = Error;
    fn try_from(value: u32) -> Result<Self> {
        match value {
            0 => Ok(Xb200Filter::_50M),
            1 => Ok(Xb200Filter::_144M),
            2 => Ok(Xb200Filter::_222M),
            3 => Ok(Xb200Filter::Custom),
            4 => Ok(Xb200Filter::Auto1db),
            5 => Ok(Xb200Filter::Auto3db),
            _ => {
                log::error!("invalid filter selection!");
                Err(Error::Argument("invalid XB200 filter value".into()))
            }
        }
    }
}
/// XB-200 signal path mode.
///
/// In `Mix` mode the ADF4351 down-converts (RX) or up-converts (TX) the
/// signal, requiring I/Q swap on the LMS6002D for spectral inversion
/// correction. `Bypass` passes the signal through directly without
/// frequency conversion.
#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub enum Xb200Path {
    /// Direct passthrough without mixing.
    Bypass = 0,
    /// Mixed path with ADF4351 frequency conversion (includes I/Q swap).
    Mix = 1,
}
impl RfLinkSession<'_> {
    /// Attaches the XB-200 board: configures Si5338 MUXOUT, sets up the
    /// ADF4351 synthesizer, and programs expansion GPIO direction/pin values.
    pub fn xb200_attach(&mut self) -> impl MaybeFuture<Output = Result<()>> {
        Op::new(async move {
            self.require_initialized().await?;
            log::trace!("Attaching XB200 transverter board");
            let mut val8 = OutputFlags::from_bits_retain(self.si().read(39).await?);
            log::trace!("[xb200_attach] si5338_read: {val8:?}");
            val8.insert(OutputFlags::B);
            self.si().write(39, val8.bits()).await?;
            self.si().write(34, 0x22).await?;
            self.config_gpio_modify(GpioFlags::set_xb200_mode).await?;
            self.nios
                .nios_expansion_gpio_dir_write(u32::MAX, Xb200GpioFlags::OUTPUTS.bits())
                .await?;
            self.nios
                .nios_expansion_gpio_write(0xffffffff, Xb200GpioFlags::RF_ON.bits())
                .await?;
            self.nios.nios_xb200_synth_write(0x580005).await?;
            self.nios.nios_xb200_synth_write(0x99A16C).await?;
            self.nios.nios_xb200_synth_write(0xC004B3).await?;
            log::trace!("MUXOUT: DIGITAL LOCK DETECT");
            let mut value = SynthControlFlags::from_bits_retain(
                SYNTH_REGISTER2_CONFIG | SYNTH_MUXOUT_DIGITAL_LOCK_DETECT,
            );
            value.insert(SynthControlFlags::INTEGER_N_LOCK_DETECT);
            self.nios.nios_xb200_synth_write(value.bits()).await?;
            self.nios.nios_xb200_synth_write(0x08008011).await?;
            self.nios.nios_xb200_synth_write(0x00410000).await?;
            let val = Xb200GpioFlags::from_bits_retain(self.nios.nios_expansion_gpio_read().await?);
            log::trace!("[xb200_attach] expansion_gpio_read: {val:?}");
            if val.contains(Xb200GpioFlags::MUXOUT) {
                log::debug!("MUXOUT Bit set: OK")
            } else {
                log::debug!("MUXOUT Bit not set: FAIL");
            }
            self.nios
                .nios_expansion_gpio_write(
                    u32::MAX,
                    (Xb200GpioFlags::RF_ON | Xb200GpioFlags::TX_FILTER | Xb200GpioFlags::RX_FILTER)
                        .bits(),
                )
                .await?;
            Ok(())
        })
    }

    /// Enables or disables the XB-200 RF circuitry via the RF_ON GPIO bit.
    pub fn xb200_enable(&mut self, enable: bool) -> impl MaybeFuture<Output = Result<()>> {
        Op::new(async move {
            self.require_initialized().await?;
            let orig =
                Xb200GpioFlags::from_bits_retain(self.nios.nios_expansion_gpio_read().await?);
            log::trace!("[xb200_enable] expansion_gpio_read: {orig:?}");
            let mut val = orig;
            val.set(Xb200GpioFlags::RF_ON, enable);
            if val == orig {
                Ok(())
            } else {
                self.nios
                    .nios_expansion_gpio_write(0xffffffff, val.bits())
                    .await
            }
        })
    }
    /// Initializes the XB-200: sets both RX and TX paths to bypass mode
    /// and both filter banks to Auto1db.
    pub fn xb200_init(&mut self) -> impl MaybeFuture<Output = Result<()>> {
        Op::new(async move {
            self.require_initialized().await?;
            log::trace!("Setting RX path");
            self.xb200_set_path(Channel::Rx, Xb200Path::Bypass).await?;
            log::trace!("Setting TX path");
            self.xb200_set_path(Channel::Tx, Xb200Path::Bypass).await?;
            log::trace!("Setting RX filter");
            self.xb200_set_filterbank(Channel::Rx, Xb200Filter::Auto1db)
                .await?;
            log::trace!("Setting TX filter");
            self.xb200_set_filterbank(Channel::Tx, Xb200Filter::Auto1db)
                .await
        })
    }
    /// Returns the currently selected filter bank for the given channel.
    pub fn xb200_get_filterbank(
        &mut self,
        ch: Channel,
    ) -> impl MaybeFuture<Output = Result<Xb200Filter>> {
        Op::new(async move {
            self.require_initialized().await?;
            let val = Xb200GpioFlags::from_bits_retain(self.nios.nios_expansion_gpio_read().await?);
            log::trace!("[xb200_get_filterbank] expansion_gpio_read: {val:?}");
            val.filter(ch)
        })
    }
    /// Directly sets the filter bank mux for the given channel without auto-selection.
    ///
    /// # Errors
    /// Returns an argument error for automatic filter modes. Device state and
    /// USB errors are propagated.
    pub fn set_filterbank_mux(
        &mut self,
        ch: Channel,
        filter: Xb200Filter,
    ) -> impl MaybeFuture<Output = Result<()>> {
        Op::new(async move {
            self.require_initialized().await?;
            let orig =
                Xb200GpioFlags::from_bits_retain(self.nios.nios_expansion_gpio_read().await?);
            log::trace!("[set_filterbank_mux] expansion_gpio_read: {orig:?}");
            let mut val = orig;
            val.set_filter(ch, filter)?;
            if orig != val {
                let dir = if ch == Channel::Tx { "TX" } else { "RX" };
                log::trace!("Engaging {filter:?} band XB-200 {dir} filter");
                self.nios
                    .nios_expansion_gpio_write(u32::MAX, val.bits())
                    .await?;
            }
            Ok(())
        })
    }
    /// Sets the filter bank for the given channel. For `Auto1db` or `Auto3db`,
    /// reads the current frequency and selects the appropriate filter from the
    /// corresponding frequency-range table. For other variants, sets directly.
    pub fn xb200_set_filterbank(
        &mut self,
        ch: Channel,
        filter: Xb200Filter,
    ) -> impl MaybeFuture<Output = Result<()>> {
        Op::new(async move {
            self.require_initialized().await?;
            if !self.nios.xb200_is_enabled().await? {
                log::error!("xb_200 not enabled! need to enable?");
                return Err(Error::Unsupported("XB200 not enabled"));
            }
            if filter == Xb200Filter::Auto1db || filter == Xb200Filter::Auto3db {
                let frequency = self.get_frequency(ch).await?;
                log::trace!("[xb200_set_filterbank] get_frequency {frequency}");
                self.xb200_auto_filter_selection(ch, frequency).await
            } else {
                self.set_filterbank_mux(ch, filter).await
            }
        })
    }
    /// Selects the filter bank automatically based on frequency and the currently
    /// configured auto mode (1 dB or 3 dB threshold). For frequencies above 300 MHz,
    /// returns immediately without changing the filter (band is outside XB-200 range).
    pub fn xb200_auto_filter_selection(
        &mut self,
        channel: Channel,
        frequency: u64,
    ) -> impl MaybeFuture<Output = Result<()>> {
        Op::new(async move {
            self.require_initialized().await?;
            if frequency >= 300_000_000 {
                return Ok(());
            }
            if !self.nios.xb200_is_enabled().await? {
                log::error!("xb_200 not enabled! need to enable?");
                return Err(Error::Unsupported("XB200 not enabled"));
            }
            let fb = self.xb200_get_filterbank(channel).await?;
            log::trace!("xb_200 current filterbank: {fb:?}");
            let filter = match fb {
                Xb200Filter::Auto1db => select_filter_from_table(frequency, AUTO_1DB_FILTERS),
                Xb200Filter::Auto3db => select_filter_from_table(frequency, AUTO_3DB_FILTERS),
                _ => {
                    log::debug!("not setting filterbank! current value: {fb:?}!");
                    return Ok(());
                }
            };
            self.set_filterbank_mux(channel, filter).await
        })
    }
    /// Sets the XB-200 signal path (Mix or Bypass) for the given channel.
    /// In Mix mode, enables I/Q swap on the LMS6002D to correct spectral inversion.
    pub fn xb200_set_path(
        &mut self,
        ch: Channel,
        path: Xb200Path,
    ) -> impl MaybeFuture<Output = Result<()>> {
        Op::new(async move {
            self.require_initialized().await?;
            let mut lval = IqSwapFlags::from_bits_retain(self.lms().read(0x5A).await?);
            let swap_mask = if ch == Channel::Rx {
                IqSwapFlags::RX
            } else {
                IqSwapFlags::TX
            };
            lval.set(swap_mask, path == Xb200Path::Mix);
            self.lms().write(0x5A, lval.bits()).await?;
            let mut val =
                Xb200GpioFlags::from_bits_retain(self.nios.nios_expansion_gpio_read().await?);
            log::trace!("[xb200_set_path] expansion_gpio_read: {val:?}");
            if !val.contains(Xb200GpioFlags::RF_ON) {
                self.xb200_attach().await?;
            }
            val.set_path(ch, path);
            self.nios
                .nios_expansion_gpio_write(0xffffffff, val.bits())
                .await
        })
    }
    /// Writes a raw SPI register value to the ADF4351 synthesizer on the XB-200.
    pub fn xb_spi_write(&mut self, value: u32) -> impl MaybeFuture<Output = Result<()>> {
        Op::new(async move {
            self.require_initialized().await?;
            self.nios.nios_xb200_synth_write(value).await
        })
    }
    /// Returns the currently configured signal path (Mix or Bypass) for the given channel.
    pub fn xb200_get_path(&mut self, ch: Channel) -> impl MaybeFuture<Output = Result<Xb200Path>> {
        Op::new(async move {
            self.require_initialized().await?;
            let val = Xb200GpioFlags::from_bits_retain(self.nios.nios_expansion_gpio_read().await?);
            log::trace!("[xb200_get_path] expansion_gpio_read: {val:#010x}");
            let path = val.path(ch);
            log::trace!("[xb200_get_path] returning {path:?}");
            Ok(path)
        })
    }
}
/// Looks up the appropriate `Xb200Filter` for the given frequency from the
/// provided table. Returns `Xb200Filter::Custom` if no range matches.
pub(crate) fn select_filter_from_table(frequency: u64, table: &[FilterEntry]) -> Xb200Filter {
    for (range, filter) in table {
        if range.contains(&frequency) {
            return *filter;
        }
    }
    Xb200Filter::Custom
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_and_filters_preserve_other_gpio_fields() {
        assert_eq!(Xb200GpioFlags::OUTPUTS.bits(), 0x3c00_383e);
        for raw in [0, u32::MAX, 0xc345_6789, 0x3c00_0800] {
            for (channel, path, mask, expected) in [
                (Channel::Rx, Xb200Path::Mix, 0x2830, 0x2810),
                (Channel::Rx, Xb200Path::Bypass, 0x2830, 0x0820),
                (Channel::Tx, Xb200Path::Mix, 0x180c, 0x1804),
                (Channel::Tx, Xb200Path::Bypass, 0x180c, 0x0808),
            ] {
                let mut gpio = Xb200GpioFlags::from_bits_retain(raw);
                gpio.set_path(channel, path);
                assert_eq!(gpio.bits(), (raw & !mask) | expected);
                assert_eq!(gpio.path(channel), path);
            }
            for (channel, mask, selections) in [
                (
                    Channel::Rx,
                    0x3000_0000,
                    [0, 0x1000_0000, 0x2000_0000, 0x3000_0000],
                ),
                (
                    Channel::Tx,
                    0x0c00_0000,
                    [0, 0x0400_0000, 0x0800_0000, 0x0c00_0000],
                ),
            ] {
                for (filter, expected) in [
                    Xb200Filter::_50M,
                    Xb200Filter::_144M,
                    Xb200Filter::_222M,
                    Xb200Filter::Custom,
                ]
                .into_iter()
                .zip(selections)
                {
                    let mut gpio = Xb200GpioFlags::from_bits_retain(raw);
                    gpio.set_filter(channel, filter).unwrap();
                    assert_eq!(gpio.bits(), (raw & !mask) | expected);
                    assert_eq!(gpio.filter(channel).unwrap(), filter);
                }
                for filter in [Xb200Filter::Auto1db, Xb200Filter::Auto3db] {
                    let mut gpio = Xb200GpioFlags::from_bits_retain(raw);
                    assert!(matches!(
                        gpio.set_filter(channel, filter),
                        Err(Error::Argument(_))
                    ));
                    assert_eq!(gpio.bits(), raw);
                }
            }
        }
    }
}
