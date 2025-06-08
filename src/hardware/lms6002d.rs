#![allow(dead_code)]

use crate::board::bladerf1::{BLADERF_FREQUENCY_MAX, BLADERF_FREQUENCY_MIN, BladerfLnaGain};
use crate::nios::Nios;
use anyhow::{Result, anyhow};
use bladerf_globals::{
    BLADERF_MODULE_RX, BLADERF_MODULE_TX, BladerfLoopback, ENDPOINT_IN, ENDPOINT_OUT,
};
use bladerf_nios::NIOS_PKT_8X8_TARGET_LMS6;
use bladerf_nios::packet_generic::NiosPkt8x8;
use bladerf_nios::packet_retune::{Band, NiosPktRetuneRequest, Tune};
use nusb::Interface;

const LMS_REFERENCE_HZ: u32 = 38400000;

#[macro_export]
macro_rules! khz {
    ($value:expr) => {
        ($value * 1000)
    };
}

macro_rules! mhz {
    ($value:expr) => {
        ($value * 1000000)
    };
}

// macro_rules! ghz {
//     ($value:expr) => {
//         ($value * 1000000000)
//     };
// }

struct DcCalState {
    clk_en: u8, /* Backup of clock enables */

    reg0x72: u8, /* Register backup */

    lna_gain: BladerfLnaGain, /* Backup of gain values */
    rxvga1_gain: i32,
    rxvga2_gain: i32,

    base_addr: u8,       /* Base address of DC cal regs */
    num_submodules: u32, /* # of DC cal submodules to operate on */

    rxvga1_curr_gain: i32, /* Current gains used in retry loops */
    rxvga2_curr_gain: i32,
}

/* LPF conversion table */
pub const UINT_BANDWIDTHS: [u32; 16] = [
    mhz!(28),
    mhz!(20),
    mhz!(14),
    mhz!(12),
    mhz!(10),
    khz!(8750),
    mhz!(7),
    mhz!(6),
    khz!(5500),
    mhz!(5),
    khz!(3840),
    mhz!(3),
    khz!(2750),
    khz!(2500),
    khz!(1750),
    khz!(1500),
];

/* Here we define more conservative band ranges than those in the
 * LMS FAQ (5.24), with the intent of avoiding the use of "edges" that might
 * cause the PLLs to lose lock over temperature changes */
pub const VCO4_LOW: u64 = 3800000000;
pub const VCO4_HIGH: u64 = 4535000000;

pub const VCO3_LOW: u64 = VCO4_HIGH;
pub const VCO3_HIGH: u64 = 5408000000;

pub const VCO2_LOW: u64 = VCO3_HIGH;
pub const VCO2_HIGH: u64 = 6480000000;

pub const VCO1_LOW: u64 = VCO2_HIGH;
pub const VCO1_HIGH: u64 = 7600000000;

// #if VCO4_LOW/16 != BLADERF_FREQUENCY_MIN
// #   error "BLADERF_FREQUENCY_MIN is not actual VCO4_LOW/16 minimum"
// #endif
//
// #if VCO1_HIGH/2 != BLADERF_FREQUENCY_MAX
// #   error "BLADERF_FREQUENCY_MAX is not actual VCO1_HIGH/2 maximum"
// #endif

/* SELVCO values */
pub const VCO4: u8 = 4 << 3;
pub const VCO3: u8 = 5 << 3;
pub const VCO2: u8 = 6 << 3;
pub const VCO1: u8 = 7 << 3;

/* FRANGE values */
pub const DIV2: u8 = 0x4;
pub const DIV4: u8 = 0x5;
pub const DIV8: u8 = 0x6;
pub const DIV16: u8 = 0x7;

/* Frequency Range table. Corresponds to the LMS FREQSEL table.
 * Per feedback from the LMS google group, the last entry, listed as 3.72G
 * in the programming manual, can be applied up to 3.8G */
pub struct FreqRange {
    low: u64,
    high: u64,
    value: u8,
}

pub const BANDS: [FreqRange; 16] = [
    FreqRange {
        low: BLADERF_FREQUENCY_MIN as u64,
        high: VCO4_HIGH / 16,
        value: VCO4 | DIV16,
    },
    FreqRange {
        low: VCO3_LOW / 16,
        high: VCO3_HIGH / 16,
        value: (VCO3 | DIV16),
    },
    FreqRange {
        low: VCO2_LOW / 16,
        high: VCO2_HIGH / 16,
        value: (VCO2 | DIV16),
    },
    FreqRange {
        low: VCO1_LOW / 16,
        high: VCO1_HIGH / 16,
        value: (VCO1 | DIV16),
    },
    FreqRange {
        low: VCO4_LOW / 8,
        high: VCO4_HIGH / 8,
        value: (VCO4 | DIV8),
    },
    FreqRange {
        low: VCO3_LOW / 8,
        high: VCO3_HIGH / 8,
        value: (VCO3 | DIV8),
    },
    FreqRange {
        low: VCO2_LOW / 8,
        high: VCO2_HIGH / 8,
        value: (VCO2 | DIV8),
    },
    FreqRange {
        low: VCO1_LOW / 8,
        high: VCO1_HIGH / 8,
        value: (VCO1 | DIV8),
    },
    FreqRange {
        low: VCO4_LOW / 4,
        high: VCO4_HIGH / 4,
        value: (VCO4 | DIV4),
    },
    FreqRange {
        low: VCO3_LOW / 4,
        high: VCO3_HIGH / 4,
        value: (VCO3 | DIV4),
    },
    FreqRange {
        low: VCO2_LOW / 4,
        high: VCO2_HIGH / 4,
        value: (VCO2 | DIV4),
    },
    FreqRange {
        low: VCO1_LOW / 4,
        high: VCO1_HIGH / 4,
        value: (VCO1 | DIV4),
    },
    FreqRange {
        low: VCO4_LOW / 2,
        high: VCO4_HIGH / 2,
        value: (VCO4 | DIV2),
    },
    FreqRange {
        low: VCO3_LOW / 2,
        high: VCO3_HIGH / 2,
        value: (VCO3 | DIV2),
    },
    FreqRange {
        low: VCO2_LOW / 2,
        high: VCO2_HIGH / 2,
        value: (VCO2 | DIV2),
    },
    FreqRange {
        low: VCO1_LOW / 2,
        high: BLADERF_FREQUENCY_MAX as u64,
        value: (VCO1 | DIV2),
    },
];

/*
* The LMS FAQ (Rev 1.0r10, Section 5.20) states that the RXVGA1 codes may be
* converted to dB via:
*      value_db = 20 * log10(127 / (127 - code))
*
* However, an offset of 5 appears to be required, yielding:
*      value_db =  5 + 20 * log10(127 / (127 - code))
*
*/
pub const RXVGA1_LUT_CODE2VAL: [u8; 121] = [
    5, 5, 5, 5, 5, 5, 5, 5, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7,
    8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 9, 9, 9, 9, 9, 9, 9, 9, 9, 10, 10, 10, 10, 10, 10, 10, 10, 11,
    11, 11, 11, 11, 11, 11, 12, 12, 12, 12, 12, 12, 12, 13, 13, 13, 13, 13, 13, 14, 14, 14, 14, 14,
    15, 15, 15, 15, 15, 16, 16, 16, 16, 17, 17, 17, 18, 18, 18, 18, 19, 19, 19, 20, 20, 21, 21, 22,
    22, 22, 23, 24, 24, 25, 25, 26, 27, 28, 29, 30,
];

/* The closest values from the above forumla have been selected.
 * indicides 0 - 4 are clamped to 5dB */
pub const RXVGA1_LUT_VAL2CODE: [u8; 31] = [
    2, 2, 2, 2, 2, 2, 14, 26, 37, 47, 56, 63, 70, 76, 82, 87, 91, 95, 99, 102, 104, 107, 109, 111,
    113, 114, 116, 117, 118, 119, 120,
];

pub const LMS_REG_DUMPSET: [u8; 107] = [
    /* Top level configuration */
    0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0E, 0x0F,
    /* TX PLL Configuration */
    0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x1B, 0x1C, 0x1D, 0x1E, 0x1F,
    /* RX PLL Configuration */
    0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28, 0x29, 0x2A, 0x2B, 0x2C, 0x2D, 0x2E, 0x2F,
    /* TX LPF Modules Configuration */
    0x30, 0x31, 0x32, 0x33, 0x34, 0x35, 0x36, /* TX RF Modules Configuration */
    0x40, 0x41, 0x42, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49, 0x4A, 0x4B, 0x4C, 0x4D, 0x4E, 0x4F,
    /* RX LPF, ADC, and DAC Modules Configuration */
    0x50, 0x51, 0x52, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5A, 0x5B, 0x5C, 0x5D, 0x5E, 0x5F,
    /* RX VGA2 Configuration */
    0x60, 0x61, 0x62, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68,
    /* RX FE Modules Configuration */
    0x70, 0x71, 0x72, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7A, 0x7B, 0x7C,
];

/* Register 0x08:  RF loopback config and additional BB config
 *
 * LBRFEN[3:0] @ [3:0]
 *  0000 - RF loopback disabled
 *  0001 - TXMIX output connected to LNA1 path
 *  0010 - TXMIX output connected to LNA2 path
 *  0011 - TXMIX output connected to LNA3 path
 *  else - Reserved
 *
 * LBEN_OPIN @ [4]
 *  0   - Disabled
 *  1   - TX BB loopback signal is connected to RX output pins
 *
 * LBEN_VGA2IN @ [5]
 *  0   - Disabled
 *  1   - TX BB loopback signal is connected to RXVGA2 input
 *
 * LBEN_LPFIN @ [6]
 *  0   - Disabled
 *  1   - TX BB loopback signal is connected to RXLPF input
 *
 */
pub const LBEN_OPIN: u8 = 1 << 4;
pub const LBEN_VGA2IN: u8 = 1 << 5;
pub const LBEN_LPFIN: u8 = 1 << 6;
pub const LBEN_MASK: u8 = LBEN_OPIN | LBEN_VGA2IN | LBEN_LPFIN;

pub const LBRFEN_LNA1: u8 = 1;
pub const LBRFEN_LNA2: u8 = 2;
pub const LBRFEN_LNA3: u8 = 3;
pub const LBRFEN_MASK: u8 = 0xf; /* [3:2] are marked reserved */

/* Register 0x46: Baseband loopback config
 *
 * LOOPBBEN[1:0] @ [3:2]
 *  00 - All Baseband loops opened (default)
 *  01 - TX loopback path connected from TXLPF output
 *  10 - TX loopback path connected from TXVGA1 output
 *  11 - TX loopback path connected from Env/peak detect output
 */
pub const LOOPBBEN_TXLPF: u8 = 1 << 2;
pub const LOOPBBEN_TXVGA: u8 = 2 << 2;
pub const LOOPBBEN_ENVPK: u8 = 3 << 2;
pub const LOOBBBEN_MASK: u8 = 3 << 2;

/* VCOCAP estimation. The MIN/MAX values were determined experimentally by
 * sampling the VCOCAP values over frequency, for each of the VCOs and finding
 * these to be in the "middle" of a linear regression. Although the curve
 * isn't actually linear, the linear approximation yields satisfactory error. */
pub const VCOCAP_MAX_VALUE: u8 = 0x3f;
pub const VCOCAP_EST_MIN: u8 = 15;
pub const VCOCAP_EST_MAX: u8 = 55;
pub const VCOCAP_EST_RANGE: u8 = VCOCAP_EST_MAX - VCOCAP_EST_MIN;
pub const VCOCAP_EST_THRESH: u8 = 7; /* Complain if we're +/- 7 on our guess */

/**
 * If this bit is set, configure PLL output buffers for operation in the
 * bladeRF's "low band." Otherwise, configure the device for operation in the
 * "high band."
 */
pub const LMS_FREQ_FLAGS_LOW_BAND: u8 = 1 << 0;

/**
 * Use VCOCAP value as-is, rather as using it as a starting point hint
 * to the tuning algorithm.  This offers a faster retune, with a potential
 * trade-off in phase noise.
 */
pub const LMS_FREQ_FLAGS_FORCE_VCOCAP: u8 = 1 << 1;

/**
 * This bit indicates whether the quicktune needs to set XB-200 parameters
 */
pub const LMS_FREQ_XB_200_ENABLE: u8 = 1 << 7;

/**
 * This bit indicates the quicktune is for the RX module, not setting this bit
 * indicates the quicktune is for the TX module.
 */
pub const LMS_FREQ_XB_200_MODULE_RX: u8 = 1 << 6;

/**
 * This is the bit mask for the filter switch configuration for the XB-200.
 */
pub const LMS_FREQ_XB_200_FILTER_SW: u8 = 3 << 4;

/**
 * Macro that indicates the number of bitshifts necessary to get to the filter
 * switch field
 */
pub const LMS_FREQ_XB_200_FILTER_SW_SHIFT: u8 = 4;

/**
 * This is the bit mask for the path configuration for the XB-200.
 */
pub const LMS_FREQ_XB_200_PATH: u8 = 3 << 2;

/**
 * Macro that indicates the number of bitshifts necessary to get to the path
 * field
 */
pub const LMS_FREQ_XB_200_PATH_SHIFT: u8 = 2;

pub const VTUNE_DELAY_LARGE: u8 = 50;
pub const VTUNE_DELAY_SMALL: u8 = 25;
pub const VTUNE_MAX_ITERATIONS: u8 = 20;

pub const VCO_HIGH: u8 = 0x02;
pub const VCO_NORM: u8 = 0x00;
pub const VCO_LOW: u8 = 0x01;

/* These values are the max counts we've seen (experimentally) between
 * VCOCAP values that converged */
pub const VCOCAP_MAX_LOW_HIGH: u8 = 12;

#[derive(Debug, Default)]
pub struct LmsFreq {
    pub freqsel: u8,       // Choice of VCO and dision ratio
    pub vcocap: u8,        // VCOCAP hint
    pub nint: u16,         // Integer portion of f_LO given f_REF
    pub nfrac: u32,        // Fractional portion of f_LO given nint and f_REF
    pub flags: u8, // Additional parameters defining the tuning configuration. See LMFS_FREQ_FLAGS_* values
    pub xb_gpio: u8, // Store XB-200 switch settings
    pub x: u8,     //VCO division ratio
    pub vcocap_result: u8, //Filled in by retune operation to denote which VCOCAP value was used
}

/* For >= 1.5 GHz uses the high band should be used. Otherwise, the low
 * band should be selected */
pub const BLADERF1_BAND_HIGH: u32 = 1500000000;

/**
 * Internal low-pass filter bandwidth selection
 */
pub enum LmsBw {
    /**< 28MHz bandwidth, 14MHz LPF */
    Bw28mhz,
    /**< 20MHz bandwidth, 10MHz LPF */
    Bw20mhz,
    /**< 14MHz bandwidth, 7MHz LPF */
    Bw14mhz,
    /**< 12MHz bandwidth, 6MHz LPF */
    Bw12mhz,
    /**< 10MHz bandwidth, 5MHz LPF */
    Bw10mhz,
    /**< 8.75MHz bandwidth, 4.375MHz LPF */
    Bw8p75mhz,
    /**< 7MHz bandwidth, 3.5MHz LPF */
    Bw7mhz,
    /**< 6MHz bandwidth, 3MHz LPF */
    Bw6mhz,
    /**< 5.5MHz bandwidth, 2.75MHz LPF */
    Bw5p5mhz,
    /**< 5MHz bandwidth, 2.5MHz LPF */
    Bw5mhz,
    /**< 3.84MHz bandwidth, 1.92MHz LPF */
    Bw3p84mhz,
    /**< 3MHz bandwidth, 1.5MHz LPF */
    Bw3mhz,
    /**< 2.75MHz bandwidth, 1.375MHz LPF */
    Bw2p75mhz,
    /**< 2.75MHz bandwidth, 1.375MHz LPF */
    /**< 2.5MHz bandwidth, 1.25MHz LPF */
    Bw2p5mhz,
    /**< 1.75MHz bandwidth, 0.875MHz LPF */
    Bw1p75mhz,
    /**< 1.5MHz bandwidth, 0.75MHz LPF */
    Bw1p5mhz,
}

/**
 * LNA options
 */
pub enum LmsLna {
    /**< Disable all LNAs */
    LnaNone,
    /**< Enable LNA1 (300MHz - 2.8GHz) */
    Lna1,
    /**< Enable LNA2 (1.5GHz - 3.8GHz) */
    Lna2,
    /**< Enable LNA3 (Unused on the bladeRF) */
    Lna3,
}

/**
 * Loopback paths
 */
pub enum LmsLbp {
    /**< Baseband loopback path */
    LbpBb,
    /**< RF Loopback path */
    LbpRf,
}

/**
 * PA Selection
 */
pub enum LmsPa {
    /**< AUX PA Enable (for RF Loopback) */
    PaAux,
    /**< PA1 Enable (300MHz - 2.8GHz) */
    Pa1,
    /**< PA2 Enable (1.5GHz - 3.8GHz) */
    Pa2,
    /**< All PAs disabled */
    PaNone,
}

/**
 * LMS6002D Transceiver configuration
 */
pub struct LmsXcvrConfig {
    /**< Transmit frequency in Hz */
    tx_freq_hz: u32,
    /**< Receive frequency in Hz */
    rx_freq_hz: u32,
    /**< Loopback Mode */
    loopback_mode: BladerfLoopback,
    /**< LNA Selection */
    lna: LmsLna,
    /**< PA Selection */
    pa: LmsPa,
    /**< Transmit Bandwidth */
    tx_bw: LmsBw,
    /**< Receive Bandwidth */
    rx_bw: LmsBw,
}

pub struct LMS6002D {
    interface: Interface,
    // ep_bulk_out: &'a Endpoint<Bulk, Out>,
    // ep_bulk_in: &'a Endpoint<Bulk, In>,
}

impl LMS6002D {
    pub fn new(interface: Interface) -> Self {
        Self { interface }
    }

    pub async fn read(&self, addr: u8) -> Result<u8> {
        type NiosPkt = NiosPkt8x8;

        let request = NiosPkt::new(NIOS_PKT_8X8_TARGET_LMS6, NiosPkt::FLAG_READ, addr, 0x0);

        let response = self
            .interface
            .nios_send(ENDPOINT_OUT, ENDPOINT_IN, request.into())
            .await?;
        Ok(NiosPkt::from(response).data())
    }

    pub async fn write(&self, addr: u8, data: u8) -> Result<u8> {
        type NiosPkt = NiosPkt8x8;

        let request = NiosPkt::new(NIOS_PKT_8X8_TARGET_LMS6, NiosPkt::FLAG_WRITE, addr, data);

        let response = self
            .interface
            .nios_send(ENDPOINT_OUT, ENDPOINT_IN, request.into())
            .await?;
        Ok(NiosPkt::from(response).data())
    }

    pub async fn set(&self, addr: u8, mask: u8) -> Result<u8> {
        let mut data = self.read(addr).await?;
        data |= mask;
        self.write(addr, data).await
    }

    pub async fn get_vtune(&self, base: u8, _delay: u8) -> Result<u8> {
        // if (delay != 0) {
        //     VTUNE_BUSY_WAIT(delay);
        // }

        let mut vtune = self.read(base + 10).await?;
        vtune >>= 6;
        Ok(vtune)
    }

    pub async fn enable_rffe(&self, module: u8, enable: bool) -> Result<u8> {
        let (addr, shift) = if module == BLADERF_MODULE_TX {
            (0x40u8, 1u8)
        } else {
            (0x70u8, 0u8)
        };
        let mut data = self.read(addr).await?;

        if enable {
            data |= 1 << shift;
        } else {
            data &= !(1 << shift);
        }
        self.write(addr, data).await
    }

    pub async fn config_charge_pumps(&self, module: u8) -> Result<u8> {
        let base: u8 = if module == BLADERF_MODULE_RX {
            0x20
        } else {
            0x10
        };

        // Set PLL Ichp current
        let mut data = self.read(base + 6).await?;
        data &= !0x1f;
        data |= 0x0c;
        self.write(base + 6, data).await?;

        // Set Iup current
        data = self.read(base + 7).await?;
        data &= !0x1f;
        data |= 0x03;
        self.write(base + 7, data).await?;

        // Set Idn current
        data = self.read(base + 8).await?;
        data &= !0x1f;
        data |= 0x03;
        self.write(base + 8, data).await
    }

    /* This is a linear interpolation of our experimentally identified
     * mean VCOCAP min and VCOCAP max values:
     */
    pub fn estimate_vcocap(f_target: u32, f_low: u32, f_high: u32) -> u8 {
        let denom: f32 = (f_high - f_low) as f32;
        let num: f32 = VCOCAP_EST_RANGE as f32;
        let f_diff: f32 = (f_target - f_low) as f32;

        let vcocap = (num / denom * f_diff) + 0.5 + VCOCAP_EST_MIN as f32;

        if vcocap > VCOCAP_MAX_VALUE as f32 {
            println!("Clamping VCOCAP estimate from {vcocap} to {VCOCAP_MAX_VALUE}");
            VCOCAP_MAX_VALUE
        } else {
            println!("VCOCAP estimate: {vcocap}");
            // vcocap.round() as u8 // .round() added by ratzrattillo
            vcocap as u8
        }
        // vcocap.clamp(0u8, VCOCAP_MAX_VALUE) as u8
    }

    pub fn calculate_tuning_params(mut freq: u32) -> Result<LmsFreq> {
        // let vco_x: u64;
        // let mut temp: u64;
        // let nint: u16;
        // let nfrac: u32;
        // let mut freqsel: u8 = BANDS[0].value;
        // let i: u8 = 0;
        let mut f: LmsFreq = LmsFreq::default();

        /* Clamp out of range values */
        freq = freq.clamp(BLADERF_FREQUENCY_MIN, BLADERF_FREQUENCY_MAX);
        println!("freq: {freq}");

        /* Figure out freqsel */
        let freq_range = BANDS
            .iter()
            .find(|freq_range| (freq >= freq_range.low as u32) && (freq <= freq_range.high as u32))
            .expect("Could not determine frequency range");

        // freqsel = freq_range.value;
        f.freqsel = freq_range.value;
        println!("f.freqsel: {}", f.freqsel);

        /* Estimate our target VCOCAP value. */
        f.vcocap = Self::estimate_vcocap(freq, freq_range.low as u32, freq_range.high as u32);
        println!("f.vcocap: {}", f.vcocap);

        /* Calculate the integer portion of the frequency value */
        let vco_x = 1 << ((f.freqsel & 7) - 3);
        println!("vco_x: {vco_x}");
        assert!(vco_x <= u8::MAX as u64);
        f.x = vco_x as u8;
        println!("f.x: {}", f.x);
        let mut temp = (vco_x * freq as u64) / LMS_REFERENCE_HZ as u64;
        println!("temp: {temp}");
        assert!(temp <= u16::MAX as u64);
        f.nint = temp as u16;
        println!("f.nint: {}", f.nint);

        temp = (1 << 23) * (vco_x * freq as u64 - f.nint as u64 * LMS_REFERENCE_HZ as u64);
        println!("temp: {temp}");
        temp = (temp + LMS_REFERENCE_HZ as u64 / 2) / LMS_REFERENCE_HZ as u64;
        println!("temp: {temp}");
        assert!(temp <= u32::MAX as u64);
        f.nfrac = temp as u32;
        println!("f.nfrac: {}", f.nfrac);

        // f.x = vco_x as u8;
        // f.nint = nint;
        // f.nfrac = nfrac;
        // f.freqsel = freqsel;
        // f.xb_gpio = 0;
        assert!(LMS_REFERENCE_HZ as u64 <= u32::MAX as u64);

        // f.flags = 0;

        if freq < BLADERF1_BAND_HIGH {
            f.flags |= LMS_FREQ_FLAGS_LOW_BAND;
        }
        println!("f.flags: {}", f.flags);

        println!("{f:?}");
        Ok(f)
    }

    pub async fn write_vcocap(&self, base: u8, vcocap: u8, vcocap_reg_state: u8) -> Result<u8> {
        assert!(vcocap <= VCOCAP_MAX_VALUE);
        // println!("Writing VCOCAP=%u\n", vcocap);

        self.write(base + 9, vcocap | vcocap_reg_state).await

        // if (status != 0) {
        // log_debug("VCOCAP write failed: %d\n", status);
        // }
        //
        // return status;
    }

    pub async fn get_loopback_mode(&self) -> Result<BladerfLoopback> {
        let mut loopback = BladerfLoopback::BladerfLbNone;

        let lben_lbrfen = self.read(0x08).await?;
        let loopbben = self.read(0x46).await?;

        match lben_lbrfen & 0x7 {
            LBRFEN_LNA1 => {
                loopback = BladerfLoopback::BladerfLbRfLna1;
            }
            LBRFEN_LNA2 => {
                loopback = BladerfLoopback::BladerfLbRfLna2;
            }
            LBRFEN_LNA3 => {
                loopback = BladerfLoopback::BladerfLbRfLna3;
            }
            _ => {}
        }

        match lben_lbrfen & LBEN_MASK {
            LBEN_VGA2IN => {
                if (loopbben & LOOPBBEN_TXLPF) != 0 {
                    loopback = BladerfLoopback::BladerfLbBbTxlpfRxvga2;
                } else if (loopbben & LOOPBBEN_TXVGA) != 0 {
                    loopback = BladerfLoopback::BladerfLbBbTxvga1Rxvga2;
                }
            }
            LBEN_LPFIN => {
                if (loopbben & LOOPBBEN_TXLPF) != 0 {
                    loopback = BladerfLoopback::BladerfLbBbTxlpfRxlpf;
                } else if (loopbben & LOOPBBEN_TXVGA) != 0 {
                    loopback = BladerfLoopback::BladerfLbBbTxvga1Rxlpf;
                }
            }
            _ => {}
        }

        Ok(loopback)
    }

    pub async fn is_loopback_enabled(&self) -> Result<bool> {
        let loopback = self.get_loopback_mode().await?;

        Ok(loopback != BladerfLoopback::BladerfLbNone)
    }

    pub async fn write_pll_config(&self, module: u8, freqsel: u8, low_band: bool) -> Result<u8> {
        // let mut regval: u8 = 0;
        // let mut selout: u8 = 0;
        // let mut addr: u8 = 0;

        let addr = if module == BLADERF_MODULE_TX {
            0x15
        } else {
            0x25
        };

        let mut regval = self.read(addr).await?;

        let lb_enabled: bool = self.is_loopback_enabled().await?;

        if !lb_enabled {
            /* Loopback not enabled - update the PLL output buffer. */
            let selout = if low_band { 1 } else { 2 };
            regval = (freqsel << 2) | selout;
        } else {
            /* Loopback is enabled - don't touch PLL output buffer. */
            regval = (regval & !0xfc) | (freqsel << 2);
        }

        self.write(addr, regval).await
    }

    pub async fn vtune_high_to_norm(
        &self,
        base: u8,
        mut vcocap: u8,
        vcocap_reg_state: u8,
    ) -> Result<u8> {
        // let mut vtune: u8 = 0xff;

        for _ in 0..VTUNE_MAX_ITERATIONS {
            if vcocap >= VCOCAP_MAX_VALUE {
                println!("vtune_high_to_norm: VCOCAP hit max value.");
                return Ok(VCOCAP_MAX_VALUE);
            }

            vcocap += 1;

            self.write_vcocap(base, vcocap, vcocap_reg_state).await?;

            let vtune = self.get_vtune(base, VTUNE_DELAY_SMALL).await?;

            if vtune == VCO_NORM {
                println!("VTUNE NORM @ VCOCAP={vcocap}");
                // println!("VTUNE HIGH @ VCOCAP={}", *vtune_high_limit);
                return Ok(vcocap - 1);
            }
        }

        // assert!("VTUNE High->Norm loop failed to converge.\n");
        // return BLADERF_ERR_UNEXPECTED;
        // TODO: Throw error!
        Err(anyhow!("VTUNE High->Norm loop failed to converge."))
        // Ok(vcocap)
    }

    pub async fn vtune_norm_to_high(
        &self,
        base: u8,
        mut vcocap: u8,
        vcocap_reg_state: u8,
    ) -> Result<u8> {
        // let mut vtune: u8 = 0xff;

        for _ in 0..VTUNE_MAX_ITERATIONS {
            println!("base: {base}, vcocap: {vcocap}, vcocap_reg_state: {vcocap_reg_state}");

            if vcocap == 0 {
                println!("vtune_norm_to_high: VCOCAP hit min value.");
                return Ok(0);
            }

            vcocap -= 1;

            self.write_vcocap(base, vcocap, vcocap_reg_state).await?;

            let vtune = self.get_vtune(base, VTUNE_DELAY_SMALL).await?;
            println!("vtune: {vtune}");

            if vtune == VCO_HIGH {
                println!("VTUNE HIGH @ VCOCAP={vcocap}");
                return Ok(vcocap);
            }
        }

        // assert!("VTUNE Norm->High loop failed to converge.\n");
        // return BLADERF_ERR_UNEXPECTED;

        // TODO: Throw error!
        Err(anyhow!("VTUNE Norm->High loop failed to converge."))
        //Ok(vcocap)
    }

    pub async fn vtune_low_to_norm(
        &self,
        base: u8,
        mut vcocap: u8,
        vcocap_reg_state: u8,
    ) -> Result<u8> {
        // let mut vtune: u8 = 0xff;

        for _ in 0..VTUNE_MAX_ITERATIONS {
            if vcocap == 0 {
                println!("vtune_low_to_norm: VCOCAP hit min value.");
                return Ok(0);
            }

            vcocap -= 1;

            self.write_vcocap(base, vcocap, vcocap_reg_state).await?;

            let vtune = self.get_vtune(base, VTUNE_DELAY_SMALL).await?;

            if vtune == VCO_NORM {
                println!("VTUNE NORM @ VCOCAP={vcocap}");
                return Ok(vcocap + 1);
            }
        }

        // assert!("VTUNE Low->Norm loop failed to converge.\n");
        // return BLADERF_ERR_UNEXPECTED;
        // TODO: Throw error!
        Err(anyhow!("VTUNE Low->Norm loop failed to converge."))
        //Ok(vcocap)
    }

    /* Wait for VTUNE to reach HIGH or LOW. NORM is not a valid option here */
    pub async fn wait_for_vtune_value(
        &self,
        base: u8,
        target_value: u8,
        vcocap: &mut u8,
        vcocap_reg_state: u8,
    ) -> Result<()> {
        // let mut vtune: u8 = 0;
        const MAX_RETRIES: u32 = 15;
        let limit: u8 = if target_value == VCO_HIGH {
            0
        } else {
            VCOCAP_MAX_VALUE
        };
        let inc: i8 = if target_value == VCO_HIGH { -1 } else { 1 };

        assert!(target_value == VCO_HIGH || target_value == VCO_LOW);

        for i in 0..MAX_RETRIES {
            let vtune = self.get_vtune(base, 0).await?;

            if vtune == target_value {
                println!("VTUNE reached {target_value} at iteration {i}");
                return Ok(());
            } else {
                println!("VTUNE was {vtune}. Waiting and retrying...");

                //VTUNE_BUSY_WAIT(10);
            }
        }

        println!("Timed out while waiting for VTUNE={target_value}. Walking VCOCAP...\n");

        while *vcocap != limit {
            *vcocap = (*vcocap as i8 + inc) as u8;

            self.write_vcocap(base, *vcocap, vcocap_reg_state).await?;

            let vtune = self.get_vtune(base, VTUNE_DELAY_SMALL).await?;
            if vtune == target_value {
                println!("VTUNE={vtune} reached with VCOCAP={vcocap}");
                return Ok(());
            }
        }

        println!("VTUNE did not reach {target_value}. Tuning may not be nominal.");
        Ok(())

        // #   ifdef ERROR_ON_NO_VTUNE_LIMIT
        // return BLADERF_ERR_UNEXPECTED;
        // #   else
        // return 0;
        // #   endif
    }

    /* This function assumes an initial VCOCAP estimate has already been written.
     *
     * Remember, increasing VCOCAP works towards a lower voltage, and vice versa:
     * From experimental observations, we don't expect to see the "normal" region
     * extend beyond 16 counts.
     *
     *  VCOCAP = 0              VCOCAP=63
     * /                                 \
     * v                                  v
     * |----High-----[ Normal ]----Low----|     VTUNE voltage comparison
     *
     * The VTUNE voltage can be found on R263 (RX) or R265 (Tx). (They're under the
     * can shielding the LMS6002D.) By placing a scope probe on these and retuning,
     * you should be able to see the relationship between VCOCAP changes and
     * the voltage changes.
     */
    pub async fn tune_vcocap(&self, vcocap_est: u8, base: u8, vcocap_reg_state: u8) -> Result<u8> {
        // let mut status: i32 = 0;
        let mut vcocap: u8 = vcocap_est;
        // let mut vtune: u8 = 0;
        let mut vtune_high_limit: u8 = VCOCAP_MAX_VALUE; /* Where VCOCAP puts use into VTUNE HIGH region */
        let mut vtune_low_limit: u8 = 0; /* Where VCOCAP puts use into VTUNE LOW region */

        //RESET_BUSY_WAIT_COUNT();

        let mut vtune = self.get_vtune(base, VTUNE_DELAY_LARGE).await?;

        match vtune {
            VCO_HIGH => {
                println!("Estimate HIGH: Walking down to NORM.");
                vtune_high_limit = self
                    .vtune_high_to_norm(base, vcocap, vcocap_reg_state)
                    .await?;
            }
            VCO_NORM => {
                println!("Estimate NORM: Walking up to HIGH.");
                vtune_high_limit = self
                    .vtune_norm_to_high(base, vcocap, vcocap_reg_state)
                    .await?;
            }
            VCO_LOW => {
                println!("Estimate LOW: Walking down to NORM.");
                vtune_low_limit = self
                    .vtune_low_to_norm(base, vcocap, vcocap_reg_state)
                    .await?;
            }
            _ => {}
        }

        if vtune_high_limit != VCOCAP_MAX_VALUE {
            /* We determined our VTUNE HIGH limit. Try to force ourselves to the
             * LOW limit and then walk back up to norm from there.
             *
             * Reminder - There's an inverse relationship between VTUNE and VCOCAP
             */
            match vtune {
                VCO_NORM | VCO_HIGH => {
                    if (vtune_high_limit + VCOCAP_MAX_LOW_HIGH) < VCOCAP_MAX_VALUE {
                        vcocap = vtune_high_limit + VCOCAP_MAX_LOW_HIGH;
                    } else {
                        vcocap = VCOCAP_MAX_VALUE;
                        println!("Clamping VCOCAP to {vcocap}.");
                    }
                }
                _ => {
                    //assert!("Invalid state");
                    // return BLADERF_ERR_UNEXPECTED;
                    return Err(anyhow!("Invalid state"));
                }
            }

            self.write_vcocap(base, vcocap, vcocap_reg_state).await?;

            println!("Waiting for VTUNE LOW @ VCOCAP={vcocap}");
            self.wait_for_vtune_value(base, VCO_LOW, &mut vcocap, vcocap_reg_state)
                .await?;

            println!("Walking VTUNE LOW to NORM from VCOCAP={vcocap}");
            vtune_low_limit = self
                .vtune_low_to_norm(base, vcocap, vcocap_reg_state)
                .await?;
        } else {
            /* We determined our VTUNE LOW limit. Try to force ourselves up to
             * the HIGH limit and then walk down to NORM from there
             *
             * Reminder - There's an inverse relationship between VTUNE and VCOCAP
             */
            match vtune {
                VCO_LOW | VCO_NORM => {
                    if (vtune_low_limit - VCOCAP_MAX_LOW_HIGH) > 0 {
                        vcocap = vtune_low_limit - VCOCAP_MAX_LOW_HIGH;
                    } else {
                        vcocap = 0;
                        println!("Clamping VCOCAP to {vcocap}.");
                    }
                }
                _ => {
                    // assert!("Invalid state");
                    // return BLADERF_ERR_UNEXPECTED;
                    return Err(anyhow!("Invalid state"));
                }
            }

            self.write_vcocap(base, vcocap, vcocap_reg_state).await?;

            println!("Waiting for VTUNE HIGH @ VCOCAP={vcocap}");
            self.wait_for_vtune_value(base, VCO_HIGH, &mut vcocap, vcocap_reg_state)
                .await?;

            println!("Walking VTUNE HIGH to NORM from VCOCAP={vcocap}");
            vtune_high_limit = self
                .vtune_high_to_norm(base, vcocap, vcocap_reg_state)
                .await?;
        }

        vcocap = vtune_high_limit + (vtune_low_limit - vtune_high_limit) / 2;

        println!("VTUNE LOW:   {vtune_low_limit}");
        println!("VTUNE NORM:  {vcocap}");
        println!("VTUNE Est:   {vcocap_est}"); // , vcocap_est - vcocap
        println!("VTUNE HIGH:  {vtune_high_limit}");

        // #       if LMS_COUNT_BUSY_WAITS
        //     println!("Busy waits:  %u\n", busy_wait_count);
        //     println!("Busy us:     %u\n", busy_wait_duration);
        // #       endif

        self.write_vcocap(base, vcocap, vcocap_reg_state).await?;

        /* Inform the caller of what we converged to */
        // *vcocap_result = vcocap;

        vtune = self.get_vtune(base, VTUNE_DELAY_SMALL).await?;

        // PRINT_BUSY_WAIT_INFO();

        if vtune != VCO_NORM {
            // status = BLADERF_ERR_UNEXPECTED;
            // assert!("Final VCOCAP={} is not in VTUNE NORM region.", "{}", vcocap);
            return Err(anyhow!(
                "Final VCOCAP={} is not in VTUNE NORM region.",
                vcocap
            ));
        }
        Ok(vcocap)
    }

    pub async fn set_precalculated_frequency(&self, module: u8, f: &mut LmsFreq) -> Result<()> {
        /* Select the base address based on which PLL we are configuring */
        let base: u8 = if module == BLADERF_MODULE_RX {
            0x20
        } else {
            0x10
        };

        // let mut data: u8 = 0;
        // let mut vcocap_reg_state: u8 = 0;
        // let mut status: u8 = 0;
        // let mut dsm_status: i32 = 0;

        /* Utilize atomic writes to the PLL registers, if possible. This
         * "multiwrite" is indicated by the MSB being set. */
        let pll_base: u8 = base | 0x80;
        // #ifdef BLADERF_NIOS_BUILD
        //     const uint8_t pll_base = base | 0x80;
        // #else
        //     const uint8_t pll_base =
        //     have_cap(dev->board->get_capabilities(dev), BLADERF_CAP_ATOMIC_NINT_NFRAC_WRITE) ? (base | 0x80) : base;
        // #endif

        f.vcocap_result = 0xff;

        /* Turn on the DSMs */
        let mut data = self.read(0x09).await?;
        data |= 0x05;
        self.write(0x09, data)
            .await
            .expect("Failed to turn on DSMs\n");

        /* Write the initial vcocap estimate first to allow for adequate time for
         * VTUNE to stabilize. We need to be sure to keep the upper bits of
         * this register and perform a RMW, as bit 7 is VOVCOREG[0]. */
        let mut result = self.read(base + 9).await;
        if result.is_err() {
            self.turn_off_dsms().await?;
            return Err(anyhow!("Failed to read vcocap regstate!"));
        }
        let mut vcocap_reg_state = result?;

        vcocap_reg_state &= !0x3f;

        result = self.write_vcocap(base, f.vcocap, vcocap_reg_state).await;
        if result.is_err() {
            self.turn_off_dsms().await?;
            return Err(anyhow!("Failed to write vcocap_reg_state!"));
        }

        let low_band = (f.flags & LMS_FREQ_FLAGS_LOW_BAND) != 0;
        result = self.write_pll_config(module, f.freqsel, low_band).await;
        if result.is_err() {
            self.turn_off_dsms().await?;
            return Err(anyhow!("Failed to write pll_config!"));
        }

        let mut freq_data = [0u8; 4];
        freq_data[0] = (f.nint >> 1) as u8;
        freq_data[1] = (((f.nint & 1) << 7) as u32 | ((f.nfrac >> 16) & 0x7f)) as u8;
        freq_data[2] = ((f.nfrac >> 8) & 0xff) as u8;
        freq_data[3] = (f.nfrac & 0xff) as u8;

        for (idx, value) in freq_data.iter().enumerate() {
            result = self.write(pll_base + idx as u8, *value).await;
            if result.is_err() {
                self.turn_off_dsms().await?;
                return Err(anyhow!("Failed to write pll {}!", pll_base + idx as u8));
            }
        }

        // data = (f.nint >> 1) as u8;
        // result = self.write(pll_base, data);
        // if result.is_err() {
        //     self.turn_off_dsms()?;
        //     return Err(anyhow!("Failed to write pll pll_base + 0!"))
        // }
        //
        // data = (((f.nint & 1) << 7) as u32 | ((f.nfrac >> 16) & 0x7f)) as u8;
        // result = self.write(pll_base + 1, data);
        // if result.is_err() {
        //     self.turn_off_dsms()?;
        //     return Err(anyhow!("Failed to write pll pll_base + 1!"))
        // }
        //
        // data = ((f.nfrac >> 8) & 0xff) as u8;
        // result = self.write(pll_base + 2, data);
        // if result.is_err() {
        //     self.turn_off_dsms()?;
        //     return Err(anyhow!("Failed to write pll pll_base + 2!"))
        // }
        //
        // data = (f.nfrac & 0xff) as u8;
        // result = self.write(pll_base + 3, data);
        // if result.is_err() {
        //     self.turn_off_dsms()?;
        //     return Err(anyhow!("Failed to write pll pll_base + 3!"))
        // }

        /* Perform tuning algorithm unless we've been instructed to just use
         * the VCOCAP hint as-is. */
        if (f.flags & LMS_FREQ_FLAGS_FORCE_VCOCAP) != 0 {
            f.vcocap_result = f.vcocap;
        } else {
            /* Walk down VCOCAP values find an optimal values */
            println!("Tuning VCOCAP...");
            f.vcocap_result = self.tune_vcocap(f.vcocap, base, vcocap_reg_state).await?;
        }

        Ok(())
    }

    pub async fn turn_off_dsms(&self) -> Result<u8> {
        let mut data = self.read(0x09).await?;
        data &= !0x05;
        self.write(0x09, data).await
    }

    pub async fn select_pa(&self, pa: LmsPa) -> Result<u8> {
        // status = LMS_READ(dev, 0x44, &data);
        let mut data = self.read(0x44).await?;

        /* Disable PA1, PA2, and AUX PA - we'll enable as requested below. */
        data &= !0x1C;

        /* AUX PA powered down */
        data |= 1 << 1;

        match pa {
            LmsPa::PaAux => {
                data &= !(1 << 1); /* Power up the AUX PA */
            }
            LmsPa::Pa1 => {
                data |= 2 << 2; /* PA_EN[2:0] = 010 - Enable PA1 */
            }
            LmsPa::Pa2 => {
                data |= 4 << 2; /* PA_EN[2:0] = 100 - Enable PA2 */
            }
            LmsPa::PaNone => {}
        }

        // match pa {
        // case PA_AUX:
        // data &= ~(1 << 1);  /* Power up the AUX PA */
        // break;
        //
        // case PA_1:
        // data |= (2 << 2);   /* PA_EN[2:0] = 010 - Enable PA1 */
        // break;
        //
        // case PA_2:
        // data |= (4 << 2);   /* PA_EN[2:0] = 100 - Enable PA2 */
        // break;
        //
        // case PA_NONE:
        // break;
        //
        // default:
        // assert(!"Invalid PA selection");
        // status = BLADERF_ERR_INVAL;
        // }

        // if (status == 0) {
        // status = LMS_WRITE(dev, 0x44, data);
        // }
        self.write(0x44, data).await
    }

    /* Select which LNA to enable */
    pub async fn select_lna(&self, lna: LmsLna) -> Result<u8> {
        let mut data = self.read(0x75).await?;
        // status = LMS_READ(dev, 0x75, &data);
        // if (status != 0) {
        //     return status;
        // }

        data &= !(3 << 4);
        data |= (lna as u8 & 3) << 4;

        // return LMS_WRITE(dev, 0x75, data);
        self.write(0x75, data).await
    }

    pub async fn select_band(&self, module: u8, band: Band) -> Result<u8> {
        /* If loopback mode disabled, avoid changing the PA or LNA selection,
         * as these need to remain are powered down or disabled */
        // status = is_loopback_enabled(dev);
        // if (status < 0) {
        //     return status;
        // } else if (status > 0) {
        //     return 0;
        // }
        if self.is_loopback_enabled().await? {
            println!("Loopback enabled!");
            return Ok(0);
        }

        // if (module == BLADERF_MODULE_TX) {
        //     lms_pa pa = low_band ? PA_1 : PA_2;
        //     status = lms_select_pa(dev, pa);
        // } else {
        //     lms_lna lna = low_band ? LNA_1 : LNA_2;
        //     status = lms_select_lna(dev, lna);
        // }
        if module == BLADERF_MODULE_TX {
            let lms_pa = if band == Band::Low {
                LmsPa::Pa1
            } else {
                LmsPa::Pa2
            };
            self.select_pa(lms_pa).await
        } else {
            let lms_lna = if band == Band::Low {
                LmsLna::Lna1
            } else {
                LmsLna::Lna2
            };
            self.select_lna(lms_lna).await
        }
    }

    pub async fn set_frequency(&self, channel: u8, frequency: u32) -> Result<LmsFreq> {
        let mut f = Self::calculate_tuning_params(frequency)?;
        println!("{f:?}");

        self.set_precalculated_frequency(channel, &mut f).await?;
        Ok(f)
    }

    pub async fn get_frequency(&self, module: u8) -> Result<LmsFreq> {
        let mut f = LmsFreq::default();
        let base: u8 = if module == BLADERF_MODULE_RX {
            0x20
        } else {
            0x10
        };

        let mut data = self.read(base).await?;
        f.nint = (data as u16) << 1;

        data = self.read(base + 1).await?;
        f.nint |= (data as u16 & 0x80) >> 7;
        f.nfrac = (data as u32 & 0x7f) << 16;

        data = self.read(base + 2).await?;
        f.nfrac |= (data as u32) << 8;

        data = self.read(base + 3).await?;
        f.nfrac |= data as u32;

        data = self.read(base + 5).await?;
        f.freqsel = data >> 2;
        f.x = 1 << ((f.freqsel & 7) - 3);

        data = self.read(base + 9).await?;
        f.vcocap = data & 0x3f;

        Ok(f)
    }

    pub fn frequency_to_hz(lms_freq: &LmsFreq) -> u64 {
        let pll_coeff = ((lms_freq.nint as u64) << 23) + lms_freq.nfrac as u64;
        let div = (lms_freq.x as u64) << 23;

        if div > 0 {
            ((LMS_REFERENCE_HZ as u64 * pll_coeff) + (div >> 1)) / div
        } else {
            0
        }
    }

    pub async fn schedule_retune(
        &self,
        channel: u8,
        timestamp: u64,
        frequency: u32,
        quick_tune: Option<LmsFreq>,
    ) -> Result<LmsFreq> {
        let f = if let Some(qt) = quick_tune {
            qt
        } else {
            // if (dev->xb == BLADERF_XB_200) {
            //     log_info("Consider supplying the quick_tune parameter to"
            //              " bladerf_schedule_retune() when the XB-200 is enabled.\n");
            // }
            Self::calculate_tuning_params(frequency)?
        };

        println!("{f:?}");

        let band = if f.flags & LMS_FREQ_FLAGS_LOW_BAND != 0 {
            Band::Low
        } else {
            Band::High
        };

        let tune = if (f.flags & LMS_FREQ_FLAGS_FORCE_VCOCAP) != 0 {
            Tune::Quick
        } else {
            Tune::Normal
        };

        self.interface
            .nios_retune(
                channel, timestamp, f.nint, f.nfrac, f.freqsel, f.vcocap, band, tune, f.xb_gpio,
            )
            .await?;
        Ok(f)
    }

    pub async fn cancel_scheduled_retunes(&self, channel: u8) -> Result<()> {
        self.interface
            .nios_retune(
                channel,
                NiosPktRetuneRequest::CLEAR_QUEUE,
                0,
                0,
                0,
                0,
                Band::Low,
                Tune::Normal,
                0,
            )
            .await
    }
}
