use crate::BladeRf1;
use crate::nios::Nios;
use anyhow::{Result, anyhow};
use bladerf_globals::{bladerf_channel_rx, bladerf_channel_tx};

pub const BLADERF_XB_CONFIG_TX_PATH_MIX: u32 = 0x04;
pub const BLADERF_XB_CONFIG_TX_PATH_BYPASS: u32 = 0x08;
pub const BLADERF_XB_CONFIG_TX_BYPASS: u32 = 0x04;
pub const BLADERF_XB_CONFIG_TX_BYPASS_N: u32 = 0x08;
pub const BLADERF_XB_CONFIG_TX_BYPASS_MASK: u32 = 0x0C;
pub const BLADERF_XB_CONFIG_RX_PATH_MIX: u32 = 0x10;
pub const BLADERF_XB_CONFIG_RX_PATH_BYPASS: u32 = 0x20;
pub const BLADERF_XB_CONFIG_RX_BYPASS: u32 = 0x10;
pub const BLADERF_XB_CONFIG_RX_BYPASS_N: u32 = 0x20;
pub const BLADERF_XB_CONFIG_RX_BYPASS_MASK: u32 = 0x30;

pub const BLADERF_XB_RF_ON: u32 = 0x0800;
pub const BLADERF_XB_TX_ENABLE: u32 = 0x1000;
pub const BLADERF_XB_RX_ENABLE: u32 = 0x2000;

pub const BLADERF_XB_TX_RF_SW2: u32 = 0x04000000;
pub const BLADERF_XB_TX_RF_SW1: u32 = 0x08000000;
pub const BLADERF_XB_TX_MASK: u32 = 0x0C000000;
pub const BLADERF_XB_TX_SHIFT: u32 = 26;

pub const BLADERF_XB_RX_RF_SW2: u32 = 0x10000000;
pub const BLADERF_XB_RX_RF_SW1: u32 = 0x20000000;
pub const BLADERF_XB_RX_MASK: u32 = 0x30000000;
pub const BLADERF_XB_RX_SHIFT: u32 = 28;

pub const LMS_RX_SWAP: u8 = 0x40;
pub const LMS_TX_SWAP: u8 = 0x08;

/**
 * XB-200 filter selection options
 */
#[derive(PartialEq, Debug, Clone)]
pub enum BladerfXb200Filter {
    /** 50-54 MHz (6 meter band) filterbank */
    _50M = 0,

    /** 144-148 MHz (2 meter band) filterbank */
    _144M = 1,

    /**
     * 222-225 MHz (1.25 meter band) filterbank.
     *
     * Note that this filter option is technically wider, covering 206-235 MHz.
     */
    _222M = 2,

    /**
     * This option enables the RX/TX channel's custom filter bank path across
     * the associated FILT and FILT-ANT SMA connectors on the XB-200 board.
     *
     * For reception, it is often possible to simply connect the RXFILT and
     * RXFILT-ANT connectors with an SMA cable (effectively, "no filter"). This
     * allows for reception of signals outside of the frequency range of the
     * on-board filters, with some potential trade-off in signal quality.
     *
     * For transmission, <b>always</b> use an appropriate filter on the custom
     * filter path to avoid spurious emissions.
     *
     */
    Custom = 3,

    /**
     * When this option is selected, the other filter options are automatically
     * selected depending on the RX or TX channel's current frequency, based
     * upon the 1dB points of the on-board filters.  For frequencies outside
     * the range of the on-board filters, the custom path is selected.
     */
    Auto1db = 4,

    /**
     * When this option is selected, the other filter options are automatically
     * selected depending on the RX or TX channel's current frequency, based
     * upon the 3dB points of the on-board filters. For frequencies outside the
     * range of the on-board filters, the custom path is selected.
     */
    Auto3db = 5,
}

impl TryFrom<u32> for BladerfXb200Filter {
    type Error = anyhow::Error;
    fn try_from(value: u32) -> Result<Self> {
        match value {
            0 => Ok(BladerfXb200Filter::_50M),
            1 => Ok(BladerfXb200Filter::_144M),
            2 => Ok(BladerfXb200Filter::_222M),
            3 => Ok(BladerfXb200Filter::Custom),
            4 => Ok(BladerfXb200Filter::Auto1db),
            5 => Ok(BladerfXb200Filter::Auto3db),
            _ => Err(anyhow!("invalid filter selection!")),
        }
    }
}
/**
 * XB-200 signal paths
 */
#[derive(PartialEq, Debug)]
pub enum BladerfXb200Path {
    /**< Bypass the XB-200 mixer */
    Bypass = 0,
    /**< Pass signals through the XB-200 mixer */
    Mix = 1,
}

// struct xb200_xb_data {
//     /* Track filterbank selection for RX and TX auto-selection */
//     bladerf_xb200_filter auto_filter[2];
// };

pub struct Xb200 {
    rx_filterbank: Option<BladerfXb200Filter>,
    tx_filterbank: Option<BladerfXb200Filter>,
}

impl Xb200 {
    pub fn set_filterbank(&mut self, ch: u8, filter: Option<BladerfXb200Filter>) {
        if bladerf_channel_rx!(ch) != 0 {
            self.rx_filterbank = filter;
        } else {
            self.tx_filterbank = filter;
        }
    }

    pub fn get_filterbank(&self, ch: u8) -> &Option<BladerfXb200Filter> {
        if bladerf_channel_rx!(ch) != 0 {
            &self.rx_filterbank
        } else {
            &self.tx_filterbank
        }
    }
}

impl BladeRf1 {
    pub async fn xb200_attach(&mut self) -> Result<()> {
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

        log::debug!("Attaching transverter board");
        let mut val8 = self.si5338.read(39).await?;

        val8 |= 2;

        self.si5338.write(39, val8).await?;
        self.si5338.write(34, 0x22).await?;

        let mut val = self.config_gpio_read().await?;

        val |= 0x80000000;

        self.config_gpio_write(val).await?;
        // val = self.interface.nios_expansion_gpio_read().await?;

        self.interface
            .nios_expansion_gpio_dir_write(0xffffffff, 0x3C00383E)
            .await?;
        self.interface
            .nios_expansion_gpio_write(0xffffffff, 0x800)
            .await?;

        // Load ADF4351 registers via SPI
        // Refer to ADF4351 reference manual for register set
        // The LO is set to a Int-N 1248MHz +3dBm tone
        // Registers are written in order from 5 downto 0
        self.interface.nios_xb200_synth_write(0x580005).await?;
        self.interface.nios_xb200_synth_write(0x99A16C).await?;
        self.interface.nios_xb200_synth_write(0xC004B3).await?;

        log::debug!("MUXOUT: {}", mux_lut[muxout]);

        self.interface
            .nios_xb200_synth_write((0x60008E42 | (1 << 8) | (muxout << 26)) as u32)
            .await?;
        self.interface.nios_xb200_synth_write(0x08008011).await?;
        self.interface.nios_xb200_synth_write(0x00410000).await?;

        val = self.interface.nios_expansion_gpio_read().await?;
        if val & 0x1 != 0 {
            log::debug!("MUXOUT Bit set: OK")
        } else {
            log::debug!("MUXOUT Bit not set: FAIL");
        }

        self.interface
            .nios_expansion_gpio_write(0xffffffff, 0x3C000800)
            .await?;

        self.xb200 = Some(Xb200 {
            rx_filterbank: None,
            tx_filterbank: None,
        });

        Ok(())
    }

    pub fn xb200_detach(&mut self) {
        self.xb200 = None;
    }

    pub async fn xb200_enable(&self, enable: bool) -> Result<()> {
        let orig = self.interface.nios_expansion_gpio_read().await?;
        let mut val = orig;

        if enable {
            val |= BLADERF_XB_RF_ON;
        } else {
            val &= !BLADERF_XB_RF_ON;
        }

        if val == orig {
            Ok(())
        } else {
            self.interface
                .nios_expansion_gpio_write(0xffffffff, val)
                .await
        }
    }

    pub async fn xb200_init(&mut self) -> Result<()> {
        log::debug!("Setting RX path");
        self.xb200_set_path(bladerf_channel_rx!(0), &BladerfXb200Path::Bypass)
            .await?;

        log::debug!("Setting TX path");
        self.xb200_set_path(bladerf_channel_tx!(0), &BladerfXb200Path::Bypass)
            .await?;

        log::debug!("Setting RX filter");
        self.xb200_set_filterbank(bladerf_channel_rx!(0), BladerfXb200Filter::Auto1db)
            .await?;

        log::debug!("Setting TX filter");
        self.xb200_set_filterbank(bladerf_channel_tx!(0), BladerfXb200Filter::Auto1db)
            .await
    }

    /**
     * Validate XB-200 path selection
     *
     * @param[in]   p   Path supplied by API user.
     *
     * @return 0 for a valid enumeration value, BLADERF_ERR_INVAL otherwise.
     */
    pub async fn xb200_get_filterbank(&self, ch: u8) -> Result<BladerfXb200Filter> {
        if ch != bladerf_channel_rx!(0) && ch != bladerf_channel_tx!(0) {
            return Err(anyhow!("invalid channel"));
        }

        let val = self.interface.nios_expansion_gpio_read().await?;

        let shift = if ch == bladerf_channel_rx!(0) {
            BLADERF_XB_RX_SHIFT
        } else {
            BLADERF_XB_TX_SHIFT
        };

        BladerfXb200Filter::try_from((val >> shift) & 3)
    }

    pub async fn set_filterbank_mux(&self, ch: u8, filter: BladerfXb200Filter) -> Result<()> {
        let (mask, shift) = if ch == bladerf_channel_rx!(0) {
            (BLADERF_XB_RX_MASK, BLADERF_XB_RX_SHIFT)
        } else {
            (BLADERF_XB_TX_MASK, BLADERF_XB_TX_SHIFT)
        };

        let orig = self.interface.nios_expansion_gpio_read().await?;

        let mut val = orig & !mask;
        val |= (filter.clone() as u32) << shift;

        if orig != val {
            let dir = if mask == BLADERF_XB_TX_MASK {
                "TX"
            } else {
                "RX"
            };
            log::debug!("Engaging {:?} band XB-200 {:?} filter\n", filter, dir);

            self.interface
                .nios_expansion_gpio_write(0xffffffff, val)
                .await?;
        }

        Ok(())
    }

    pub async fn xb200_set_filterbank(&mut self, ch: u8, filter: BladerfXb200Filter) -> Result<()> {
        if ch != bladerf_channel_rx!(0) && ch != bladerf_channel_tx!(0) {
            return Err(anyhow!("invalid channel"));
        }

        if self.xb200.as_ref().is_none() {
            log::error!("xb_data is null (do you need to xb200_attach?)");
            return Err(anyhow!("xb_200 not attached!"));
        }

        if filter == BladerfXb200Filter::Auto1db || filter == BladerfXb200Filter::Auto3db {
            /* Save which soft auto filter mode we're in */
            //xb_data->auto_filter[ch] = filter;
            self.xb200
                .as_mut()
                .unwrap()
                .set_filterbank(ch, Some(filter));

            let frequency = self.get_frequency(ch).await?;
            self.xb200_auto_filter_selection(ch, frequency).await?;
        } else {
            /* Invalidate the soft auto filter mode entry */
            //xb_data->auto_filter[ch] = -1;
            self.xb200.as_mut().unwrap().set_filterbank(ch, None);

            self.set_filterbank_mux(ch, filter).await?;
        }

        Ok(())
    }

    pub async fn xb200_auto_filter_selection(&self, ch: u8, frequency: u32) -> Result<()> {
        if frequency >= 300000000 {
            return Ok(());
        }

        if ch != bladerf_channel_rx!(0) && ch != bladerf_channel_tx!(0) {
            return Err(anyhow!("invalid channel"));
        }

        if self.xb200.as_ref().is_none() {
            log::error!("xb_data is null (do you need to xb200_attach?)");
            return Err(anyhow!("xb_200 not attached!"));
        }

        let filter = if self.xb200.as_ref().unwrap().get_filterbank(ch)
            == &Some(BladerfXb200Filter::Auto1db)
        {
            if 37774405 <= frequency && frequency <= 59535436 {
                Ok(BladerfXb200Filter::_50M)
            } else if 128326173 <= frequency && frequency <= 166711171 {
                Ok(BladerfXb200Filter::_144M)
            } else if 187593160 <= frequency && frequency <= 245346403 {
                Ok(BladerfXb200Filter::_222M)
            } else {
                Ok(BladerfXb200Filter::Custom)
            }
        } else if self.xb200.as_ref().unwrap().get_filterbank(ch)
            == &Some(BladerfXb200Filter::Auto3db)
        {
            if 34782924 <= frequency && frequency <= 61899260 {
                Ok(BladerfXb200Filter::_50M)
            } else if 121956957 <= frequency && frequency <= 178444099 {
                Ok(BladerfXb200Filter::_144M)
            } else if 177522675 <= frequency && frequency <= 260140935 {
                Ok(BladerfXb200Filter::_222M)
            } else {
                Ok(BladerfXb200Filter::Custom)
            }
        } else {
            Err(anyhow!("unexpected filterbank"))
        };

        if let Ok(filterbank) = filter {
            self.set_filterbank_mux(ch, filterbank).await?;
        }

        Ok(())
    }

    pub async fn xb200_set_path(&mut self, ch: u8, path: &BladerfXb200Path) -> Result<()> {
        if ch != bladerf_channel_rx!(0) && ch != bladerf_channel_tx!(0) {
            return Err(anyhow!("invalid channel"));
            // Error::from(BladeRfError::Invalid)
        }

        let lorig = self.lms.read(0x5A).await?;
        let mut lval = lorig;

        if path == &BladerfXb200Path::Mix {
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

        self.lms.write(0x5A, lval).await?;

        let mut val = self.interface.nios_expansion_gpio_read().await?;

        if (val & BLADERF_XB_RF_ON) == 0 {
            self.xb200_attach().await?;
        }

        let mask = if ch == bladerf_channel_rx!(0) {
            BLADERF_XB_CONFIG_RX_BYPASS_MASK | BLADERF_XB_RX_ENABLE
        } else {
            BLADERF_XB_CONFIG_TX_BYPASS_MASK | BLADERF_XB_TX_ENABLE
        };

        val |= BLADERF_XB_RF_ON;
        val &= !mask;

        if ch == bladerf_channel_rx!(0) {
            if path == &BladerfXb200Path::Mix {
                val |= BLADERF_XB_RX_ENABLE | BLADERF_XB_CONFIG_RX_PATH_MIX;
            } else {
                val |= BLADERF_XB_CONFIG_RX_PATH_BYPASS;
            }
        } else {
            if path == &BladerfXb200Path::Mix {
                val |= BLADERF_XB_TX_ENABLE | BLADERF_XB_CONFIG_TX_PATH_MIX;
            } else {
                val |= BLADERF_XB_CONFIG_TX_PATH_BYPASS;
            }
        }

        self.interface
            .nios_expansion_gpio_write(0xffffffff, val)
            .await
    }

    pub async fn xb200_get_path(&self, ch: u8) -> Result<BladerfXb200Path> {
        let val = self.interface.nios_expansion_gpio_read().await?;

        if ch == bladerf_channel_rx!(0) {
            if val & BLADERF_XB_CONFIG_RX_BYPASS != 0 {
                Ok(BladerfXb200Path::Mix)
            } else {
                Ok(BladerfXb200Path::Bypass)
            }
        } else if ch == bladerf_channel_tx!(0) {
            if val & BLADERF_XB_CONFIG_TX_BYPASS != 0 {
                Ok(BladerfXb200Path::Mix)
            } else {
                Ok(BladerfXb200Path::Bypass)
            }
        } else {
            Err(anyhow!("invalid channel"))
        }
    }
}
