use crate::bladerf::{bladerf_channel_rx, bladerf_channel_tx};
use crate::bladerf1::BladeRf1;
use crate::nios::Nios;
use crate::{Error, Result};
use nusb::Interface;
use std::sync::{Arc, Mutex};

pub(crate) const BLADERF_XB_CONFIG_TX_PATH_MIX: u32 = 0x04;
pub(crate) const BLADERF_XB_CONFIG_TX_PATH_BYPASS: u32 = 0x08;
pub(crate) const BLADERF_XB_CONFIG_TX_BYPASS: u32 = 0x04;
// pub (crate) const BLADERF_XB_CONFIG_TX_BYPASS_N: u32 = 0x08;
pub(crate) const BLADERF_XB_CONFIG_TX_BYPASS_MASK: u32 = 0x0C;
pub(crate) const BLADERF_XB_CONFIG_RX_PATH_MIX: u32 = 0x10;
pub(crate) const BLADERF_XB_CONFIG_RX_PATH_BYPASS: u32 = 0x20;
pub(crate) const BLADERF_XB_CONFIG_RX_BYPASS: u32 = 0x10;
// pub (crate) const BLADERF_XB_CONFIG_RX_BYPASS_N: u32 = 0x20;
pub(crate) const BLADERF_XB_CONFIG_RX_BYPASS_MASK: u32 = 0x30;

pub(crate) const BLADERF_XB_RF_ON: u32 = 0x0800;
pub(crate) const BLADERF_XB_TX_ENABLE: u32 = 0x1000;
pub(crate) const BLADERF_XB_RX_ENABLE: u32 = 0x2000;

// pub (crate) const BLADERF_XB_TX_RF_SW2: u32 = 0x04000000;
// pub (crate) const BLADERF_XB_TX_RF_SW1: u32 = 0x08000000;
pub(crate) const BLADERF_XB_TX_MASK: u32 = 0x0C000000;
pub(crate) const BLADERF_XB_TX_SHIFT: u32 = 26;

// pub (crate) const BLADERF_XB_RX_RF_SW2: u32 = 0x10000000;
// pub (crate) const BLADERF_XB_RX_RF_SW1: u32 = 0x20000000;
pub(crate) const BLADERF_XB_RX_MASK: u32 = 0x30000000;
pub(crate) const BLADERF_XB_RX_SHIFT: u32 = 28;

pub(crate) const LMS_RX_SWAP: u8 = 0x40;
pub(crate) const LMS_TX_SWAP: u8 = 0x08;

/// XB-200 filter selection options
#[derive(PartialEq, Debug, Clone)]
pub enum Xb200Filter {
    /// 50-54 MHz (6 meter band) filterbank
    _50M = 0,

    /// 144-148 MHz (2 meter band) filterbank
    _144M = 1,

    /// 222-225 MHz (1.25 meter band) filterbank.
    ///
    /// Note that this filter option is technically wider, covering 206-235 MHz.
    _222M = 2,

    /// This option enables the RX/TX channel's custom filter bank path across
    /// the associated FILT and FILT-ANT SMA connectors on the XB-200 board.
    ///
    /// For reception, it is often possible to simply connect the RXFILT and
    /// RXFILT-ANT connectors with an SMA cable (effectively, "no filter"). This
    /// allows receiving signals outside the frequency range of the
    /// on-board filters, with some potential trade-off in signal quality.
    ///
    /// For transmission, <b>always</b> use an appropriate filter on the custom
    /// filter path to avoid spurious emissions.
    Custom = 3,

    /// When this option is selected, the other filter options are automatically
    /// selected depending on the RX or TX channel's current frequency, based
    /// upon the 1dB points of the on-board filters.  For frequencies outside
    /// the range of the on-board filters, the custom path is selected.
    Auto1db = 4,

    /// When this option is selected, the other filter options are automatically
    /// selected depending on the RX or TX channel's current frequency, based
    /// upon the 3dB points of the on-board filters. For frequencies outside the
    /// range of the on-board filters, the custom path is selected.
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

// impl From<Xb200Filter> for u32 {
//     fn from(value: Xb200Filter) -> Self {
//         match value {
//             Xb200Filter::_50M => 0,
//             Xb200Filter::_144M => 1,
//             Xb200Filter::_222M => 2,
//             Xb200Filter::Custom => 3,
//             Xb200Filter::Auto1db => 4,
//             Xb200Filter::Auto3db => 5,
//         }
//     }
// }

/// XB-200 signal paths
#[derive(PartialEq, Debug)]
pub enum Xb200Path {
    /// Bypass the XB-200 mixer
    Bypass = 0,
    /// Pass signals through the XB-200 mixer
    Mix = 1,
}

impl BladeRf1 {
    /// Trying to detect if XB200 is enabled by reading the BLADERF_XB_RF_ON gpio Flag,
    /// which is set in xb200_enable(). Might be not the best, or correct way.
    pub fn xb200_is_enabled(interface: &Arc<Mutex<Interface>>) -> Result<bool> {
        // The original libbladerf from Nuand saves the state of attached boards in a
        // separate structure. We try to determine the attached XB200 board ONLY by reading
        // the NIOS_PKT_32X32_TARGET_EXP register. It seems like this register is
        // initialized to 0xffffffff when no board is attached at all. Thus, we return
        // "false", if the register is 0xffffffff.
        // TODO: Verify, if this is really the case, as for now it is an assumption.
        let xb_gpio = interface.lock().unwrap().nios_expansion_gpio_read()?;
        if xb_gpio == 0xffffffff {
            return Ok(false);
        }
        Ok((xb_gpio & BLADERF_XB_RF_ON) != 0)
        // Ok((interface.lock().unwrap().nios_expansion_gpio_read()? & BLADERF_XB_RF_ON) != 0)
    }

    /// Attach the XB200 expansion board.
    /// Note: Verified
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
        // Out: 41010000270000000000000000000000
        let mut val8 = self.si5338.read(39)?;
        log::trace!("[xb200_attach] si5338_read: {val8}");

        val8 |= 2;

        // Out: 41010100270200000000000000000000
        self.si5338.write(39, val8)?;
        // Out: 41010100222200000000000000000000
        self.si5338.write(34, 0x22)?;

        // Out: 43010000000000000000000000000000
        let mut val = self.config_gpio_read()?;

        val |= 0x80000000;

        // Out: 43010100002f00000000000000000000 in this library
        // Out: 43010100002f00008000000000000000 in the original library!
        self.config_gpio_write(val)?;

        let interface = self.interface.lock().unwrap();

        interface.nios_expansion_gpio_dir_write(0xffffffff, 0x3C00383E)?;
        interface.nios_expansion_gpio_write(0xffffffff, 0x800)?;

        // Load ADF4351 registers via SPI
        // Refer to ADF4351 reference manual for register set
        // The LO is set to a Int-N 1248MHz +3dBm tone
        // Registers are written in order from 5 down to 0
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

    /// This method does not do anything. Detach-operations are not required/supported for XB200.
    pub fn xb200_detach(&self) -> Result<()> {
        Ok(())
    }

    /// The XB200 expansion board has to be enabled after attaching in order to be used.
    /// Note: Verified
    pub fn xb200_enable(&self, enable: bool) -> Result<()> {
        let interface = self.interface.lock().unwrap();

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

    /// Initialize the XB200 to defaults for filters and paths
    /// Note: Verified
    pub fn xb200_init(&self) -> Result<()> {
        log::trace!("Setting RX path");
        self.xb200_set_path(bladerf_channel_rx!(0), Xb200Path::Bypass)?;

        log::trace!("Setting TX path");
        self.xb200_set_path(bladerf_channel_tx!(0), Xb200Path::Bypass)?;

        log::trace!("Setting RX filter");
        self.xb200_set_filterbank(bladerf_channel_rx!(0), Xb200Filter::Auto1db)?;

        log::trace!("Setting TX filter");
        self.xb200_set_filterbank(bladerf_channel_tx!(0), Xb200Filter::Auto1db)
    }

    /// Validate XB-200 path selection
    pub fn xb200_get_filterbank(&self, ch: u8) -> Result<Xb200Filter> {
        if ch != bladerf_channel_rx!(0) && ch != bladerf_channel_tx!(0) {
            log::error!("invalid channel");
            return Err(Error::Invalid);
        }

        let val = self.interface.lock().unwrap().nios_expansion_gpio_read()?;
        log::trace!("[xb200_get_filterbank] expansion_gpio_read: {val}");

        let shift = if ch == bladerf_channel_rx!(0) {
            BLADERF_XB_RX_SHIFT
        } else {
            BLADERF_XB_TX_SHIFT
        };

        Xb200Filter::try_from((val >> shift) & 3)
    }

    /// Set the mux filterbank to be used for a specific channel.
    /// Note: Verified
    pub fn set_filterbank_mux(&self, ch: u8, filter: Xb200Filter) -> Result<()> {
        let (mask, shift) = if ch == bladerf_channel_rx!(0) {
            (BLADERF_XB_RX_MASK, BLADERF_XB_RX_SHIFT)
        } else {
            (BLADERF_XB_TX_MASK, BLADERF_XB_TX_SHIFT)
        };

        let interface = self.interface.lock().unwrap();
        let orig = interface.nios_expansion_gpio_read()?;
        log::trace!("[set_filterbank_mux] expansion_gpio_read: {orig}");

        let mut val = orig & !mask;
        val |= (filter.clone() as u32) << shift;

        if orig != val {
            let dir = if mask == BLADERF_XB_TX_MASK {
                "TX"
            } else {
                "RX"
            };
            log::trace!("Engaging {filter:?} band XB-200 {dir:?} filter");

            interface.nios_expansion_gpio_write(0xffffffff, val)?;
        }

        Ok(())
    }

    /// Set the filterbank to be used for a specific channel.
    /// Note: Verified
    pub fn xb200_set_filterbank(&self, ch: u8, filter: Xb200Filter) -> Result<()> {
        if ch != bladerf_channel_rx!(0) && ch != bladerf_channel_tx!(0) {
            log::error!("invalid channel");
            return Err(Error::Invalid);
        }

        if !BladeRf1::xb200_is_enabled(&self.interface)? {
            log::error!("xb_200 not enabled! need to enable?");
            return Err(Error::Invalid);
        }

        if filter == Xb200Filter::Auto1db || filter == Xb200Filter::Auto3db {
            // Save which soft auto filter mode we're in
            // (Just saves the state, but does not communicate with the board)
            // xb_data->auto_filter[ch] = filter;
            // self.xb200
            //     .as_mut()
            //     .unwrap()
            //     .set_filterbank(ch, Some(filter));

            // TODO: Check substraction here if expansion board is attached
            let frequency = self.get_frequency(ch)?;
            log::trace!("[xb200_set_filterbank] get_frequency {frequency}");
            self.xb200_auto_filter_selection(ch, frequency)
        } else {
            // Invalidate the soft auto filter mode entry
            // xb_data->auto_filter[ch] = -1;
            // self.xb200.as_mut().unwrap().set_filterbank(ch, None);

            self.set_filterbank_mux(ch, filter)
        }
    }

    /// Automatically select the appropriate filter for a specific channel and frequency.
    /// Note: Verified
    pub fn xb200_auto_filter_selection(&self, ch: u8, frequency: u64) -> Result<()> {
        if frequency >= 300000000 {
            return Ok(());
        }

        if ch != bladerf_channel_rx!(0) && ch != bladerf_channel_tx!(0) {
            log::error!("invalid channel");
            return Err(Error::Invalid);
        }

        if !BladeRf1::xb200_is_enabled(&self.interface)? {
            log::error!("xb_200 not enabled! need to enable?");
            return Err(Error::Invalid);
        }

        let fb = self.xb200_get_filterbank(ch)?;
        let filter = if fb == Xb200Filter::Auto1db {
            if (37774405..=59535436).contains(&frequency) {
                Xb200Filter::_50M
            } else if (128326173..=166711171).contains(&frequency) {
                Xb200Filter::_144M
            } else if (187593160..=245346403).contains(&frequency) {
                Xb200Filter::_222M
            } else {
                Xb200Filter::Custom
            }
        } else if fb == Xb200Filter::Auto3db {
            if (34782924..=61899260).contains(&frequency) {
                Xb200Filter::_50M
            } else if (121956957..=178444099).contains(&frequency) {
                Xb200Filter::_144M
            } else if (177522675..=260140935).contains(&frequency) {
                Xb200Filter::_222M
            } else {
                Xb200Filter::Custom
            }
        } else {
            log::debug!("not setting filterbank! current value: {fb:?}!");
            return Ok(());
        };

        self.set_filterbank_mux(ch, filter)
    }

    /// Set the signal path to be taken within the XB200.
    /// Note: Verified
    pub fn xb200_set_path(&self, ch: u8, path: Xb200Path) -> Result<()> {
        if ch != bladerf_channel_rx!(0) && ch != bladerf_channel_tx!(0) {
            log::error!("invalid channel!");
            return Err(Error::Invalid);
        }

        let mut lval = self.lms.read(0x5A)?;

        if path == Xb200Path::Mix {
            lval |= if ch == bladerf_channel_rx!(0) {
                LMS_RX_SWAP
            } else {
                LMS_TX_SWAP
            };
        } else {
            lval &= !(if ch == bladerf_channel_rx!(0) {
                LMS_RX_SWAP
            } else {
                LMS_TX_SWAP
            });
        }

        self.lms.write(0x5A, lval)?;

        let mut val = self.interface.lock().unwrap().nios_expansion_gpio_read()?;
        log::trace!("[xb200_set_path] expansion_gpio_read: {val}");

        if (val & BLADERF_XB_RF_ON) == 0 {
            self.xb200_attach()?;
        }

        let mask = if ch == bladerf_channel_rx!(0) {
            BLADERF_XB_CONFIG_RX_BYPASS_MASK | BLADERF_XB_RX_ENABLE
        } else {
            BLADERF_XB_CONFIG_TX_BYPASS_MASK | BLADERF_XB_TX_ENABLE
        };

        val |= BLADERF_XB_RF_ON;
        val &= !mask;

        if ch == bladerf_channel_rx!(0) {
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

    /// Get the currently configured Path on a specific channel.
    /// Note: Verified
    pub fn xb200_get_path(&self, ch: u8) -> Result<Xb200Path> {
        if ch != bladerf_channel_rx!(0) && ch != bladerf_channel_tx!(0) {
            log::error!("invalid channel!");
            return Err(Error::Invalid);
        }

        let val = self.interface.lock().unwrap().nios_expansion_gpio_read()?;
        log::trace!("[xb200_get_path] expansion_gpio_read: {val}");

        if ch == bladerf_channel_rx!(0) {
            if val & BLADERF_XB_CONFIG_RX_BYPASS != 0 {
                Ok(Xb200Path::Mix)
            } else {
                Ok(Xb200Path::Bypass)
            }
        } else if ch == bladerf_channel_tx!(0) {
            if val & BLADERF_XB_CONFIG_TX_BYPASS != 0 {
                Ok(Xb200Path::Mix)
            } else {
                Ok(Xb200Path::Bypass)
            }
        } else {
            log::error!("invalid channel!");
            Err(Error::Invalid)
        }
    }
}
