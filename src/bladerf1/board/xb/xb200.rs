use crate::bladerf1::BladeRf1;
use crate::bladerf1::board::xb::detect_xb_board;
use crate::bladerf1::nios_client::NiosInterface;
use crate::channel::Channel;
use crate::error::{Error, Result};
use std::ops::RangeInclusive;
use std::sync::{Arc, Mutex};
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
const AUTO_1DB_FILTERS: &[FilterEntry] = &[
    (37_774_405..=59_535_436, Xb200Filter::_50M),
    (128_326_173..=166_711_171, Xb200Filter::_144M),
    (187_593_160..=245_346_403, Xb200Filter::_222M),
];
const AUTO_3DB_FILTERS: &[FilterEntry] = &[
    (34_782_924..=61_899_260, Xb200Filter::_50M),
    (121_956_957..=178_444_099, Xb200Filter::_144M),
    (177_522_675..=260_140_935, Xb200Filter::_222M),
];
#[derive(PartialEq, Debug, Clone, Copy)]
pub enum Xb200Filter {
    _50M = 0,
    _144M = 1,
    _222M = 2,
    Custom = 3,
    Auto1db = 4,
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
                Err(Error::Invalid)
            }
        }
    }
}
#[derive(PartialEq, Debug)]
pub enum Xb200Path {
    Bypass = 0,
    Mix = 1,
}
impl BladeRf1 {
    pub fn xb200_is_enabled(interface: &Arc<Mutex<NiosInterface>>) -> Result<bool> {
        detect_xb_board(interface, BLADERF_XB_RF_ON)
    }
    pub fn xb200_attach(&self) -> Result<()> {
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
        let mut val8 = self.si5338.read(39)?;
        log::trace!("[xb200_attach] si5338_read: {val8}");
        val8 |= 2;
        self.si5338.write(39, val8)?;
        self.si5338.write(34, 0x22)?;
        let mut val = self.config_gpio_read()?;
        val |= 0x80000000;
        self.config_gpio_write(val)?;
        let mut interface = self.interface.lock().unwrap();
        interface.nios_expansion_gpio_dir_write(0xffffffff, 0x3C00383E)?;
        interface.nios_expansion_gpio_write(0xffffffff, 0x800)?;
        interface.nios_xb200_synth_write(0x580005)?;
        interface.nios_xb200_synth_write(0x99A16C)?;
        interface.nios_xb200_synth_write(0xC004B3)?;
        log::trace!("MUXOUT: {}", mux_lut[muxout]);
        let value = 0x60008E42 | (1 << 8) | ((muxout as u32) << 26);
        interface.nios_xb200_synth_write(value)?;
        interface.nios_xb200_synth_write(0x08008011)?;
        interface.nios_xb200_synth_write(0x00410000)?;
        val = interface.nios_expansion_gpio_read()?;
        log::trace!("[xb200_attach] expansion_gpio_read: {val}");
        if (val & 0x1) != 0 {
            log::debug!("MUXOUT Bit set: OK")
        } else {
            log::debug!("MUXOUT Bit not set: FAIL");
        }
        interface.nios_expansion_gpio_write(0xffffffff, 0x3C000800)?;
        Ok(())
    }
    pub fn xb200_enable(&self, enable: bool) -> Result<()> {
        let mut interface = self.interface.lock().unwrap();
        let orig = interface.nios_expansion_gpio_read()?;
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
            interface.nios_expansion_gpio_write(0xffffffff, val)
        }
    }
    pub fn xb200_init(&self) -> Result<()> {
        log::trace!("Setting RX path");
        self.xb200_set_path(Channel::Rx, Xb200Path::Bypass)?;
        log::trace!("Setting TX path");
        self.xb200_set_path(Channel::Tx, Xb200Path::Bypass)?;
        log::trace!("Setting RX filter");
        self.xb200_set_filterbank(Channel::Rx, Xb200Filter::Auto1db)?;
        log::trace!("Setting TX filter");
        self.xb200_set_filterbank(Channel::Tx, Xb200Filter::Auto1db)
    }
    pub fn xb200_get_filterbank(&self, ch: Channel) -> Result<Xb200Filter> {
        if ch != Channel::Rx && ch != Channel::Tx {
            log::error!("invalid channel");
            return Err(Error::Invalid);
        }
        let val = self.interface.lock().unwrap().nios_expansion_gpio_read()?;
        log::trace!("[xb200_get_filterbank] expansion_gpio_read: {val}");
        let shift = if ch == Channel::Rx {
            BLADERF_XB_RX_SHIFT
        } else {
            BLADERF_XB_TX_SHIFT
        };
        Xb200Filter::try_from((val >> shift) & 3)
    }
    pub fn set_filterbank_mux(&self, ch: Channel, filter: Xb200Filter) -> Result<()> {
        let (mask, shift) = if ch == Channel::Rx {
            (BLADERF_XB_RX_MASK, BLADERF_XB_RX_SHIFT)
        } else {
            (BLADERF_XB_TX_MASK, BLADERF_XB_TX_SHIFT)
        };
        let mut interface = self.interface.lock().unwrap();
        let orig = interface.nios_expansion_gpio_read()?;
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
            interface.nios_expansion_gpio_write(0xffffffff, val)?;
        }
        Ok(())
    }
    pub fn xb200_set_filterbank(&self, ch: Channel, filter: Xb200Filter) -> Result<()> {
        if ch != Channel::Rx && ch != Channel::Tx {
            log::error!("invalid channel");
            return Err(Error::Invalid);
        }
        if !BladeRf1::xb200_is_enabled(&self.interface)? {
            log::error!("xb_200 not enabled! need to enable?");
            return Err(Error::Invalid);
        }
        if filter == Xb200Filter::Auto1db || filter == Xb200Filter::Auto3db {
            let frequency = self.get_frequency(ch)?;
            log::trace!("[xb200_set_filterbank] get_frequency {frequency}");
            self.xb200_auto_filter_selection(ch, frequency)
        } else {
            self.set_filterbank_mux(ch, filter)
        }
    }
    pub fn xb200_auto_filter_selection(&self, channel: Channel, frequency: u64) -> Result<()> {
        if frequency >= 300000000 {
            return Ok(());
        }
        if channel != Channel::Rx && channel != Channel::Tx {
            log::error!("invalid channel");
            return Err(Error::Invalid);
        }
        if !BladeRf1::xb200_is_enabled(&self.interface)? {
            log::error!("xb_200 not enabled! need to enable?");
            return Err(Error::Invalid);
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
    pub fn xb200_set_path(&self, ch: Channel, path: Xb200Path) -> Result<()> {
        if ch != Channel::Rx && ch != Channel::Tx {
            log::error!("invalid channel");
            return Err(Error::Invalid);
        }
        let mut lval = self.lms.read(0x5A)?;
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
        self.lms.write(0x5A, lval)?;
        let mut val = self.interface.lock().unwrap().nios_expansion_gpio_read()?;
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
        self.interface
            .lock()
            .unwrap()
            .nios_expansion_gpio_write(0xffffffff, val)
    }
    pub fn xb200_get_path(&self, ch: Channel) -> Result<Xb200Path> {
        if ch != Channel::Rx && ch != Channel::Tx {
            log::error!("invalid channel");
            return Err(Error::Invalid);
        }
        let val = self.interface.lock().unwrap().nios_expansion_gpio_read()?;
        log::trace!("[xb200_get_path] expansion_gpio_read: {val}");
        let bypass_bit = if ch == Channel::Rx {
            BLADERF_XB_CONFIG_RX_BYPASS
        } else {
            BLADERF_XB_CONFIG_TX_BYPASS
        };
        if val & bypass_bit != 0 {
            Ok(Xb200Path::Mix)
        } else {
            Ok(Xb200Path::Bypass)
        }
    }
}
fn select_filter_from_table(frequency: u64, table: &[FilterEntry]) -> Xb200Filter {
    for (range, filter) in table {
        if range.contains(&frequency) {
            return *filter;
        }
    }
    Xb200Filter::Custom
}
