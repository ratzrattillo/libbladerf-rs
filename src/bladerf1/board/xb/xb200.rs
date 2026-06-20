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

use crate::bladerf1::board::RfLinkSession;
use crate::channel::Channel;
use crate::error::{Error, Result};
use std::ops::RangeInclusive;
pub(crate) const BLADERF_XB_CONFIG_TX_PATH_MIX: u32 = 0x04;
pub(crate) const BLADERF_XB_CONFIG_TX_PATH_BYPASS: u32 = 0x08;
pub(crate) const BLADERF_XB_CONFIG_TX_BYPASS: u32 = 0x04;
pub(crate) const BLADERF_XB_CONFIG_TX_BYPASS_MASK: u32 = 0x0C;
pub(crate) const BLADERF_XB_CONFIG_RX_PATH_MIX: u32 = 0x10;
pub(crate) const BLADERF_XB_CONFIG_RX_PATH_BYPASS: u32 = 0x20;
pub(crate) const BLADERF_XB_CONFIG_RX_BYPASS: u32 = 0x10;
pub(crate) const BLADERF_XB_CONFIG_RX_BYPASS_MASK: u32 = 0x30;
pub(crate) const BLADERF_XB_RF_ON: u32 = 0x0800;
pub(crate) const BLADERF_XB_TX_ENABLE: u32 = 0x1000;
pub(crate) const BLADERF_XB_RX_ENABLE: u32 = 0x2000;
pub(crate) const BLADERF_XB_TX_MASK: u32 = 0x0C000000;
pub(crate) const BLADERF_XB_TX_SHIFT: u32 = 26;
pub(crate) const BLADERF_XB_RX_MASK: u32 = 0x30000000;
pub(crate) const BLADERF_XB_RX_SHIFT: u32 = 28;
pub(crate) const LMS_RX_SWAP: u8 = 0x40;
pub(crate) const LMS_TX_SWAP: u8 = 0x08;
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
    pub fn xb200_attach(&mut self) -> Result<()> {
        self.require_initialized()?;
        let muxout: usize = 6;
        let mux_lut = [
            "THREE-STATE OUTPUT",
            "DVdd",
            "DGND",
            "R COUNTER OUTPUT",
            "N DIVIDER OUTPUT",
            "ANALOG LOCK DETECT",
            "DIGITAL LOCK DETECT",
            "RESERVED",
        ];
        log::trace!("Attaching XB200 transverter board");
        let mut val8 = self.si().read(39)?;
        log::trace!("[xb200_attach] si5338_read: {val8}");
        val8 |= 2;
        self.si().write(39, val8)?;
        self.si().write(34, 0x22)?;
        self.config_gpio_modify(|gpio| gpio | 0x80000000)?;
        self.nios
            .nios_expansion_gpio_dir_write(0xffffffff, 0x3C00383E)?;
        self.nios.nios_expansion_gpio_write(0xffffffff, 0x800)?;
        self.nios.nios_xb200_synth_write(0x580005)?;
        self.nios.nios_xb200_synth_write(0x99A16C)?;
        self.nios.nios_xb200_synth_write(0xC004B3)?;
        log::trace!("MUXOUT: {}", mux_lut[muxout]);
        let value = 0x60008E42 | (1 << 8) | ((muxout as u32) << 26);
        self.nios.nios_xb200_synth_write(value)?;
        self.nios.nios_xb200_synth_write(0x08008011)?;
        self.nios.nios_xb200_synth_write(0x00410000)?;
        let val = self.nios.nios_expansion_gpio_read()?;
        log::trace!("[xb200_attach] expansion_gpio_read: {val}");
        if (val & 0x1) != 0 {
            log::debug!("MUXOUT Bit set: OK")
        } else {
            log::debug!("MUXOUT Bit not set: FAIL");
        }
        self.nios
            .nios_expansion_gpio_write(0xffffffff, 0x3C000800)?;
        Ok(())
    }

    /// Enables or disables the XB-200 RF circuitry via the RF_ON GPIO bit.
    pub fn xb200_enable(&mut self, enable: bool) -> Result<()> {
        self.require_initialized()?;
        let orig = self.nios.nios_expansion_gpio_read()?;
        log::trace!("[xb200_enable] expansion_gpio_read: {orig}");
        let mut val = orig;
        if enable {
            val |= BLADERF_XB_RF_ON;
        } else {
            val &= !BLADERF_XB_RF_ON;
        }
        if val == orig {
            Ok(())
        } else {
            self.nios.nios_expansion_gpio_write(0xffffffff, val)
        }
    }
    /// Initializes the XB-200: sets both RX and TX paths to bypass mode
    /// and both filter banks to Auto1db.
    pub fn xb200_init(&mut self) -> Result<()> {
        self.require_initialized()?;
        log::trace!("Setting RX path");
        self.xb200_set_path(Channel::Rx, Xb200Path::Bypass)?;
        log::trace!("Setting TX path");
        self.xb200_set_path(Channel::Tx, Xb200Path::Bypass)?;
        log::trace!("Setting RX filter");
        self.xb200_set_filterbank(Channel::Rx, Xb200Filter::Auto1db)?;
        log::trace!("Setting TX filter");
        self.xb200_set_filterbank(Channel::Tx, Xb200Filter::Auto1db)
    }
    /// Returns the currently selected filter bank for the given channel.
    pub fn xb200_get_filterbank(&mut self, ch: Channel) -> Result<Xb200Filter> {
        self.require_initialized()?;
        let val = self.nios.nios_expansion_gpio_read()?;
        log::trace!("[xb200_get_filterbank] expansion_gpio_read: {val}");
        let shift = if ch == Channel::Rx {
            BLADERF_XB_RX_SHIFT
        } else {
            BLADERF_XB_TX_SHIFT
        };
        Xb200Filter::try_from((val >> shift) & 3)
    }
    /// Directly sets the filter bank mux for the given channel without auto-selection.
    pub fn set_filterbank_mux(&mut self, ch: Channel, filter: Xb200Filter) -> Result<()> {
        self.require_initialized()?;
        let (mask, shift) = if ch == Channel::Rx {
            (BLADERF_XB_RX_MASK, BLADERF_XB_RX_SHIFT)
        } else {
            (BLADERF_XB_TX_MASK, BLADERF_XB_TX_SHIFT)
        };
        let orig = self.nios.nios_expansion_gpio_read()?;
        log::trace!("[set_filterbank_mux] expansion_gpio_read: {orig}");
        let mut val = orig & !mask;
        val |= (filter as u32) << shift;
        if orig != val {
            let dir = if mask == BLADERF_XB_TX_MASK {
                "TX"
            } else {
                "RX"
            };
            log::trace!("Engaging {filter:?} band XB-200 {dir} filter");
            self.nios.nios_expansion_gpio_write(0xffffffff, val)?;
        }
        Ok(())
    }
    /// Sets the filter bank for the given channel. For `Auto1db` or `Auto3db`,
    /// reads the current frequency and selects the appropriate filter from the
    /// corresponding frequency-range table. For other variants, sets directly.
    pub fn xb200_set_filterbank(&mut self, ch: Channel, filter: Xb200Filter) -> Result<()> {
        self.require_initialized()?;
        if !self.nios.xb200_is_enabled()? {
            log::error!("xb_200 not enabled! need to enable?");
            return Err(Error::Unsupported("XB200 not enabled"));
        }
        if filter == Xb200Filter::Auto1db || filter == Xb200Filter::Auto3db {
            let frequency = self.get_frequency(ch)?;
            log::trace!("[xb200_set_filterbank] get_frequency {frequency}");
            self.xb200_auto_filter_selection(ch, frequency)
        } else {
            self.set_filterbank_mux(ch, filter)
        }
    }
    /// Selects the filter bank automatically based on frequency and the currently
    /// configured auto mode (1 dB or 3 dB threshold). For frequencies above 300 MHz,
    /// returns immediately without changing the filter (band is outside XB-200 range).
    pub fn xb200_auto_filter_selection(&mut self, channel: Channel, frequency: u64) -> Result<()> {
        self.require_initialized()?;
        if frequency >= 300_000_000 {
            return Ok(());
        }
        if !self.nios.xb200_is_enabled()? {
            log::error!("xb_200 not enabled! need to enable?");
            return Err(Error::Unsupported("XB200 not enabled"));
        }
        let fb = self.xb200_get_filterbank(channel)?;
        log::trace!("xb_200 current filterbank: {fb:?}");
        let filter = match fb {
            Xb200Filter::Auto1db => select_filter_from_table(frequency, AUTO_1DB_FILTERS),
            Xb200Filter::Auto3db => select_filter_from_table(frequency, AUTO_3DB_FILTERS),
            _ => {
                log::debug!("not setting filterbank! current value: {fb:?}!");
                return Ok(());
            }
        };
        self.set_filterbank_mux(channel, filter)
    }
    /// Sets the XB-200 signal path (Mix or Bypass) for the given channel.
    /// In Mix mode, enables I/Q swap on the LMS6002D to correct spectral inversion.
    pub fn xb200_set_path(&mut self, ch: Channel, path: Xb200Path) -> Result<()> {
        self.require_initialized()?;
        let mut lval = self.lms().read(0x5A)?;
        let swap_mask = if ch == Channel::Rx {
            LMS_RX_SWAP
        } else {
            LMS_TX_SWAP
        };
        if path == Xb200Path::Mix {
            lval |= swap_mask;
        } else {
            lval &= !swap_mask;
        }
        self.lms().write(0x5A, lval)?;
        let mut val = self.nios.nios_expansion_gpio_read()?;
        log::trace!("[xb200_set_path] expansion_gpio_read: {val}");
        if (val & BLADERF_XB_RF_ON) == 0 {
            self.xb200_attach()?;
        }
        let mask = if ch == Channel::Rx {
            BLADERF_XB_CONFIG_RX_BYPASS_MASK | BLADERF_XB_RX_ENABLE
        } else {
            BLADERF_XB_CONFIG_TX_BYPASS_MASK | BLADERF_XB_TX_ENABLE
        };
        val |= BLADERF_XB_RF_ON;
        val &= !mask;
        if ch == Channel::Rx {
            if path == Xb200Path::Mix {
                val |= BLADERF_XB_RX_ENABLE | BLADERF_XB_CONFIG_RX_PATH_MIX;
            } else {
                val |= BLADERF_XB_CONFIG_RX_PATH_BYPASS;
            }
        } else if path == Xb200Path::Mix {
            val |= BLADERF_XB_TX_ENABLE | BLADERF_XB_CONFIG_TX_PATH_MIX;
        } else {
            val |= BLADERF_XB_CONFIG_TX_PATH_BYPASS;
        }
        self.nios.nios_expansion_gpio_write(0xffffffff, val)
    }
    /// Writes a raw SPI register value to the ADF4351 synthesizer on the XB-200.
    pub fn xb_spi_write(&mut self, value: u32) -> Result<()> {
        self.require_initialized()?;
        self.nios.nios_xb200_synth_write(value)
    }
    /// Returns the currently configured signal path (Mix or Bypass) for the given channel.
    pub fn xb200_get_path(&mut self, ch: Channel) -> Result<Xb200Path> {
        self.require_initialized()?;
        let val = self.nios.nios_expansion_gpio_read()?;
        log::trace!("[xb200_get_path] expansion_gpio_read: {val:#010x}");
        let bypass_bit = if ch == Channel::Rx {
            BLADERF_XB_CONFIG_RX_BYPASS
        } else {
            BLADERF_XB_CONFIG_TX_BYPASS
        };
        log::trace!(
            "[xb200_get_path] bypass_bit={bypass_bit:#x}, val & bypass_bit = {:#x}",
            val & bypass_bit
        );
        if (val & bypass_bit) != 0 {
            log::trace!("[xb200_get_path] returning Mix");
            Ok(Xb200Path::Mix)
        } else {
            log::trace!("[xb200_get_path] returning Bypass");
            Ok(Xb200Path::Bypass)
        }
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
