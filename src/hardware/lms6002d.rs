#![allow(dead_code)]

use crate::bladerf::{BLADERF_MODULE_RX, BLADERF_MODULE_TX, bladerf_channel_rx, khz, mhz};
use crate::bladerf1::BladeRf1;
use crate::nios::{NIOS_PKT_8X8_TARGET_LMS6, Nios};
use crate::{Error, Result};
use nusb::Interface;
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::Duration;

#[repr(u8)]
#[derive(PartialEq, Debug)]
pub enum Band {
    Low = 0,
    High = 1,
}
#[repr(u8)]
#[derive(PartialEq, Debug)]
pub enum Tune {
    Normal = 0,
    Quick = 1,
}

///  Low-Pass Filter (LPF) mode
#[derive(PartialEq)]
pub enum LpfMode {
    /// LPF connected and enabled
    Normal,
    /// LPF bypassed
    Bypassed,
    /// LPF disabled
    Disabled,
}

/// RX gain offset
pub const BLADERF1_RX_GAIN_OFFSET: f32 = -6.0;

/// TX gain offset: 60 dB system gain ~= 0 dBm output
pub const BLADERF1_TX_GAIN_OFFSET: f32 = 52.0;

/// Minimum bandwidth, in Hz
///
/// \deprecated Use bladerf_get_bandwidth_range()
pub const BLADERF_BANDWIDTH_MIN: u32 = 1500000;

/// Maximum bandwidth, in Hz
///
/// \deprecated Use bladerf_get_bandwidth_range()
pub const BLADERF_BANDWIDTH_MAX: u32 = 28000000;

/// Minimum tunable frequency (with an XB-200 attached), in Hz.
///
/// While this value is the lowest permitted, note that the components on the
/// XB-200 are only rated down to 50 MHz. Be aware that performance will likely
/// degrade as you tune to lower frequencies.
///
/// \deprecated Call bladerf_expansion_attach(), then use
///             bladerf_get_frequency_range() to get the frequency range.
pub const BLADERF_FREQUENCY_MIN_XB200: u32 = 0;

/// Minimum tunable frequency (without an XB-200 attached), in Hz
///
/// \deprecated Use bladerf_get_frequency_range()
pub const BLADERF_FREQUENCY_MIN: u32 = 237500000;

/// Maximum tunable frequency, in Hz
///
/// \deprecated Use bladerf_get_frequency_range()
pub const BLADERF_FREQUENCY_MAX: u32 = 3800000000;

const LMS_REFERENCE_HZ: u32 = 38400000;

//
// /// In general, the gains should be incremented in the following order (and
// /// decremented in the reverse order).
// ///
// /// <b>TX:</b> `TXVGA1`, `TXVGA2`
// ///
// /// <b>RX:</b> `LNA`, `RXVGA`, `RXVGA2`
// ///
//
/// Minimum RXVGA1 gain, in dB
///
/// \deprecated Use bladerf_get_gain_stage_range()
pub const BLADERF_RXVGA1_GAIN_MIN: i8 = 5;

/// Maximum RXVGA1 gain, in dB
///
/// \deprecated Use bladerf_get_gain_stage_range()
pub const BLADERF_RXVGA1_GAIN_MAX: i8 = 30;

/// Minimum RXVGA2 gain, in dB
///
/// \deprecated Use bladerf_get_gain_stage_range()
pub const BLADERF_RXVGA2_GAIN_MIN: i8 = 0;

/// Maximum RXVGA2 gain, in dB
///
/// \deprecated Use bladerf_get_gain_stage_range()
pub const BLADERF_RXVGA2_GAIN_MAX: i8 = 30;

/// Minimum TXVGA1 gain, in dB
///
/// \deprecated Use bladerf_get_gain_stage_range()
pub const BLADERF_TXVGA1_GAIN_MIN: i8 = -35;

/// Maximum TXVGA1 gain, in dB
///
/// \deprecated Use bladerf_get_gain_stage_range()
pub const BLADERF_TXVGA1_GAIN_MAX: i8 = -4;

/// Minimum TXVGA2 gain, in dB
///
///\deprecated Use bladerf_get_gain_stage_range()
pub const BLADERF_TXVGA2_GAIN_MIN: i8 = 0;

/// Maximum TXVGA2 gain, in dB
///
/// \deprecated Use bladerf_get_gain_stage_range()
pub const BLADERF_TXVGA2_GAIN_MAX: i8 = 25;

/// Gain in dB of the LNA at mid setting
///
/// \deprecated Use bladerf_get_gain_stage_range()
pub const BLADERF_LNA_GAIN_MID_DB: i8 = 3;

/// Gain in db of the LNA at max setting
///
/// \deprecated Use bladerf_get_gain_stage_range()
pub const BLADERF_LNA_GAIN_MAX_DB: i8 = 6;

/// Gain in dB
pub struct GainDb {
    pub db: i8,
}

/// LNA gain options
///
/// \deprecated Use bladerf_get_gain_stage_range()
#[derive(PartialEq)]
pub enum LnaGainCode {
    /// Invalid LNA gain
    // UnsupportedMaxLna3 = 0x0,
    /// LNA bypassed - 0dB gain
    BypassLna1Lna2 = 0x1,
    /// LNA Mid Gain (MAX-6dB)
    MidAllLnas,
    /// LNA Max Gain
    MaxAllLnas,
}

impl From<LnaGainCode> for u8 {
    fn from(value: LnaGainCode) -> Self {
        match value {
            // LnaGainCode::UnsupportedMaxLna3 => 0,
            LnaGainCode::BypassLna1Lna2 => 1,
            LnaGainCode::MidAllLnas => 2,
            LnaGainCode::MaxAllLnas => 3,
        }
    }
}

impl TryFrom<u8> for LnaGainCode {
    type Error = ();

    fn try_from(value: u8) -> std::result::Result<Self, Self::Error> {
        match value {
            // 0 => Ok(LnaGainCode::UnsupportedMaxLna3),
            1 => Ok(LnaGainCode::BypassLna1Lna2),
            2 => Ok(LnaGainCode::MidAllLnas),
            3 => Ok(LnaGainCode::MaxAllLnas),
            _ => {
                log::error!("Unsupported Gain Code {value}");
                Err(())
            }
        }
    }
}

impl From<LnaGainCode> for GainDb {
    fn from(value: LnaGainCode) -> Self {
        GainDb {
            db: match value {
                LnaGainCode::MaxAllLnas => BLADERF_LNA_GAIN_MAX_DB,
                LnaGainCode::MidAllLnas => BLADERF_LNA_GAIN_MID_DB,
                LnaGainCode::BypassLna1Lna2 => 0i8,
            },
        }
    }
}

// impl TryFrom<LnaGainCode> for GainDb {
//     type Error = ();
//
//     fn try_from(value: LnaGainCode) -> Result<Self, Self::Error> {
//         match value {
//             LnaGainCode::MaxAllLnas => Ok(GainDb {
//                 db: BLADERF_LNA_GAIN_MAX_DB,
//             }),
//             LnaGainCode::MidAllLnas => Ok(GainDb {
//                 db: BLADERF_LNA_GAIN_MID_DB,
//             }),
//             LnaGainCode::BypassLna1Lna2 => Ok(GainDb { db: 0i8 }),
//             _ => {
//                 log::error!("Unsupported Gain Code!");
//                 Err(())
//             }
//         }
//     }
// }

impl From<GainDb> for LnaGainCode {
    fn from(value: GainDb) -> Self {
        if value.db >= BLADERF_LNA_GAIN_MAX_DB {
            LnaGainCode::MaxAllLnas
        } else if value.db >= BLADERF_LNA_GAIN_MID_DB {
            LnaGainCode::MidAllLnas
        } else {
            LnaGainCode::BypassLna1Lna2
        }
    }
}

pub struct Rxvga1GainCode {
    pub code: u8,
}

impl From<Rxvga1GainCode> for GainDb {
    fn from(value: Rxvga1GainCode) -> Self {
        let gain_db = (BLADERF_RXVGA1_GAIN_MIN as f32
            + (20.0 * (127.0 / (127.0 - value.code as f32)).log10()))
        .round() as i8;
        GainDb {
            db: gain_db.clamp(BLADERF_RXVGA1_GAIN_MIN, BLADERF_RXVGA1_GAIN_MAX),
        }
    }
}

impl From<GainDb> for Rxvga1GainCode {
    fn from(value: GainDb) -> Self {
        let gain_db = value
            .db
            .clamp(BLADERF_RXVGA1_GAIN_MIN, BLADERF_RXVGA1_GAIN_MAX);
        Rxvga1GainCode {
            code: (127.0
                - 127.0 / (10.0f32.powf((gain_db - BLADERF_RXVGA1_GAIN_MIN) as f32 / 20.0)))
            .round() as u8,
        }
    }
}

pub struct Rxvga2GainCode {
    pub code: u8,
}

impl From<Rxvga2GainCode> for GainDb {
    fn from(value: Rxvga2GainCode) -> Self {
        // log::trace!("rxvga2_gain_code: {}", value.code);
        let gain_db = (value.code * 3) as i8;
        GainDb {
            db: gain_db.clamp(BLADERF_RXVGA2_GAIN_MIN, BLADERF_RXVGA2_GAIN_MAX),
        }
    }
}

impl From<GainDb> for Rxvga2GainCode {
    fn from(value: GainDb) -> Self {
        let gain_db = value
            .db
            .clamp(BLADERF_RXVGA2_GAIN_MIN, BLADERF_RXVGA2_GAIN_MAX);
        Rxvga2GainCode {
            code: (gain_db as f32 / 3.0).round() as u8,
        }
    }
}

pub struct Txvga1GainCode {
    pub code: u8,
}

impl From<Txvga1GainCode> for GainDb {
    fn from(value: Txvga1GainCode) -> Self {
        // Clamp to max value
        let clamped = value.code & 0x1f;
        GainDb {
            // Convert table index to value
            db: clamped as i8 - 35,
        }
    }
}

impl From<GainDb> for Txvga1GainCode {
    fn from(value: GainDb) -> Self {
        // Clamp within recommended thresholds
        let clamped = value
            .db
            .clamp(BLADERF_TXVGA1_GAIN_MIN, BLADERF_TXVGA1_GAIN_MAX);
        Txvga1GainCode {
            // Apply offset to convert gain to register table index
            code: (clamped + 35) as u8,
        }
    }
}

pub struct Txvga2GainCode {
    pub code: u8,
}

impl From<Txvga2GainCode> for GainDb {
    fn from(value: Txvga2GainCode) -> Self {
        // Clamp to max value
        let clamped = (value.code >> 3) & 0x1f;
        GainDb {
            // Register values of 25-31 all correspond to 25 dB
            db: clamped.min(25) as i8,
        }
    }
}

impl From<GainDb> for Txvga2GainCode {
    fn from(value: GainDb) -> Self {
        // Clamp within recommended thresholds
        let clamped = value
            .db
            .clamp(BLADERF_TXVGA2_GAIN_MIN, BLADERF_TXVGA2_GAIN_MAX);
        Txvga2GainCode {
            // Mask and shift to VGA2GAIN bits
            code: ((clamped & 0x1f) << 3) as u8,
        }
    }
}

///  Gain control modes
///
///  In general, the default mode is automatic gain control. This will
///  continuously adjust the gain to maximize dynamic range and minimize clipping.
///
///  @note Implementers are encouraged to simply present a boolean choice between
///        "AGC On" (BladeRf1GainMode::Default) and "AGC Off" (BladeRf1GainMode::Mgc).
///        The remaining choices are for advanced use cases.
#[derive(PartialEq)]
pub enum GainMode {
    /// Device-specific default (automatic, when available)
    ///
    /// On the bladeRF x40 and x115 with FPGA versions >= v0.7.0, this is
    /// automatic gain control.
    ///
    /// On the bladeRF 2.0 Micro, this is BladeRf1GainMode::SlowattackAgc with
    /// reasonable default settings.
    Default,

    /// Manual gain control
    ///
    /// Available on all bladeRF models.
    Mgc,

    /// Automatic gain control, fast attack (advanced)
    ///
    /// Only available on the bladeRF 2.0 Micro. This is an advanced option, and
    /// typically requires additional configuration for ideal performance.
    FastAttackAgc,

    /// Automatic gain control, slow attack (advanced)
    ///
    /// Only available on the bladeRF 2.0 Micro. This is an advanced option, and
    /// typically requires additional configuration for ideal performance.
    SlowAttackAgc,

    /// Automatic gain control, hybrid attack (advanced)
    ///
    /// Only available on the bladeRF 2.0 Micro. This is an advanced option, and
    /// typically requires additional configuration for ideal performance.
    HybridAgc,
}
/// Quick Re-tune parameters.
///
/// @note These parameters, which are associated with the RFIC's register values,
///       are sensitive to changes in the operating environment (e.g.,
///       temperature).
///
/// This structure should be filled in via bladerf_get_quick_tune().
pub struct QuickTune {
    /// Choice of VCO and VCO division factor
    pub freqsel: u8,
    /// VCOCAP value
    pub vcocap: u8,
    /// Integer portion of LO frequency value
    pub nint: u16,
    /// Fractional portion of LO frequency value
    pub nfrac: u32,
    /// Flag bits used internally by libbladeRF
    pub flags: u8,
    /// Flag bits used to configure XB
    pub xb_gpio: u8,
}

struct DcCalState {
    /// Backup of clock enables
    clk_en: u8,
    /// Register backup
    reg0x72: u8,
    ///  Backup of gain values
    lna_gain: LnaGainCode,
    rxvga1_gain: i32,
    rxvga2_gain: i32,

    /// Base address of DC cal regs
    base_addr: u8,
    /// # of DC cal submodules to operate on
    num_submodules: u32,
    /// Current gains used in retry loops
    rxvga1_curr_gain: i32,
    rxvga2_curr_gain: i32,
}

/// Here we define more conservative band ranges than those in the
/// LMS FAQ (5.24), with the intent of avoiding the use of "edges" that might
/// cause the PLLs to lose lock over temperature changes
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

/// SELVCO values
pub const VCO4: u8 = 4 << 3;
pub const VCO3: u8 = 5 << 3;
pub const VCO2: u8 = 6 << 3;
pub const VCO1: u8 = 7 << 3;

/// FRANGE values
pub const DIV2: u8 = 0x4;
pub const DIV4: u8 = 0x5;
pub const DIV8: u8 = 0x6;
pub const DIV16: u8 = 0x7;

/// Frequency Range table. Corresponds to the LMS FREQSEL table.
/// Per feedback from the LMS google group, the last entry, listed as 3.72G
/// in the programming manual, can be applied up to 3.8G
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
        value: VCO3 | DIV16,
    },
    FreqRange {
        low: VCO2_LOW / 16,
        high: VCO2_HIGH / 16,
        value: VCO2 | DIV16,
    },
    FreqRange {
        low: VCO1_LOW / 16,
        high: VCO1_HIGH / 16,
        value: VCO1 | DIV16,
    },
    FreqRange {
        low: VCO4_LOW / 8,
        high: VCO4_HIGH / 8,
        value: VCO4 | DIV8,
    },
    FreqRange {
        low: VCO3_LOW / 8,
        high: VCO3_HIGH / 8,
        value: VCO3 | DIV8,
    },
    FreqRange {
        low: VCO2_LOW / 8,
        high: VCO2_HIGH / 8,
        value: VCO2 | DIV8,
    },
    FreqRange {
        low: VCO1_LOW / 8,
        high: VCO1_HIGH / 8,
        value: VCO1 | DIV8,
    },
    FreqRange {
        low: VCO4_LOW / 4,
        high: VCO4_HIGH / 4,
        value: VCO4 | DIV4,
    },
    FreqRange {
        low: VCO3_LOW / 4,
        high: VCO3_HIGH / 4,
        value: VCO3 | DIV4,
    },
    FreqRange {
        low: VCO2_LOW / 4,
        high: VCO2_HIGH / 4,
        value: VCO2 | DIV4,
    },
    FreqRange {
        low: VCO1_LOW / 4,
        high: VCO1_HIGH / 4,
        value: VCO1 | DIV4,
    },
    FreqRange {
        low: VCO4_LOW / 2,
        high: VCO4_HIGH / 2,
        value: VCO4 | DIV2,
    },
    FreqRange {
        low: VCO3_LOW / 2,
        high: VCO3_HIGH / 2,
        value: VCO3 | DIV2,
    },
    FreqRange {
        low: VCO2_LOW / 2,
        high: VCO2_HIGH / 2,
        value: VCO2 | DIV2,
    },
    FreqRange {
        low: VCO1_LOW / 2,
        high: BLADERF_FREQUENCY_MAX as u64,
        value: VCO1 | DIV2,
    },
];

// /// The LMS FAQ (Rev 1.0r10, Section 5.20) states that the RXVGA1 codes may be
// /// converted to dB via:
// ///      `value_db = 20 * log10(127 / (127 - code))`
// ///
// /// However, an offset of 5 appears to be required, yielding:
// ///     `value_db =  5 + 20 * log10(127 / (127 - code))`
// ///
// /// let gain_db = (BLADERF_RXVGA1_GAIN_MIN as f32 + (20.0 * (127.0 / (127.0 - code)).log10())).round() as i8;
// pub const RXVGA1_LUT_CODE2VAL: [u8; 121] = [
//     5, 5, 5, 5, 5, 5, 5, 5, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7,
//     8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 9, 9, 9, 9, 9, 9, 9, 9, 9, 10, 10, 10, 10, 10, 10, 10, 10, 11,
//     11, 11, 11, 11, 11, 11, 12, 12, 12, 12, 12, 12, 12, 13, 13, 13, 13, 13, 13, 14, 14, 14, 14, 14,
//     15, 15, 15, 15, 15, 16, 16, 16, 16, 17, 17, 17, 18, 18, 18, 18, 19, 19, 19, 20, 20, 21, 21, 22,
//     22, 22, 23, 24, 24, 25, 25, 26, 27, 28, 29, 30,
// ];

// /// The closest values from the above formula have been selected.
// /// indicides 0 - 4 are clamped to 5dB
// ///
// /// let code = 127.0 - 127.0 / (10.0f32.powf((val as f32 - BLADERF_RXVGA1_GAIN_MIN as f32) / 20.0));
// pub const RXVGA1_LUT_VAL2CODE: [u8; 31] = [
//     2, 2, 2, 2, 2, 2, 14, 26, 37, 47, 56, 63, 70, 76, 82, 87, 91, 95, 99, 102, 104, 107, 109, 111,
//     113, 114, 116, 117, 118, 119, 120,
// ];

pub const LMS_REG_DUMPSET: [u8; 107] = [
    // Top level configuration
    0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0E, 0x0F,
    // TX PLL Configuration
    0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x1B, 0x1C, 0x1D, 0x1E, 0x1F,
    // RX PLL Configuration
    0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28, 0x29, 0x2A, 0x2B, 0x2C, 0x2D, 0x2E, 0x2F,
    // TX LPF Modules Configuration
    0x30, 0x31, 0x32, 0x33, 0x34, 0x35, 0x36, // TX RF Modules Configuration
    0x40, 0x41, 0x42, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49, 0x4A, 0x4B, 0x4C, 0x4D, 0x4E, 0x4F,
    // RX LPF, ADC, and DAC Modules Configuration
    0x50, 0x51, 0x52, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5A, 0x5B, 0x5C, 0x5D, 0x5E, 0x5F,
    // RX VGA2 Configuration
    0x60, 0x61, 0x62, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, // RX FE Modules Configuration
    0x70, 0x71, 0x72, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7A, 0x7B, 0x7C,
];

/// Register 0x08:  RF loopback config and additional BB config
///
/// LBRFEN\[3:0\] @ \[3:0\]
///  0000 - RF loopback disabled
///  0001 - TXMIX output connected to LNA1 path
///  0010 - TXMIX output connected to LNA2 path
///  0011 - TXMIX output connected to LNA3 path
///  else - Reserved
///
/// LBEN_OPIN @ \[4\]
///  0   - Disabled
///  1   - TX BB loopback signal is connected to RX output pins
///
/// LBEN_VGA2IN @ \[5\]
///  0   - Disabled
///  1   - TX BB loopback signal is connected to RXVGA2 input
///
///  LBEN_LPFIN @ \[6\]
///   0   - Disabled
///  1   - TX BB loopback signal is connected to RXLPF input
pub const LBEN_OPIN: u8 = 1 << 4;
pub const LBEN_VGA2IN: u8 = 1 << 5;
pub const LBEN_LPFIN: u8 = 1 << 6;
pub const LBEN_MASK: u8 = LBEN_OPIN | LBEN_VGA2IN | LBEN_LPFIN;

pub const LBRFEN_LNA1: u8 = 1;
pub const LBRFEN_LNA2: u8 = 2;
pub const LBRFEN_LNA3: u8 = 3;
pub const LBRFEN_MASK: u8 = 0xf; // [3:2] are marked reserved

/// Register 0x46: Baseband loopback config
///
/// LOOPBBEN\[1:0\] @ \[3:2\]
///  00 - All Baseband loops opened (default)
///  01 - TX loopback path connected from TXLPF output
///  10 - TX loopback path connected from TXVGA1 output
///  11 - TX loopback path connected from Env/peak detect output
pub const LOOPBBEN_TXLPF: u8 = 1 << 2;
pub const LOOPBBEN_TXVGA: u8 = 2 << 2;
pub const LOOPBBEN_ENVPK: u8 = 3 << 2;
pub const LOOBBBEN_MASK: u8 = 3 << 2;

/// VCOCAP estimation. The MIN/MAX values were determined experimentally by
/// sampling the VCOCAP values over frequency, for each of the VCOs and finding
/// these to be in the "middle" of a linear regression. Although the curve
/// isn't actually linear, the linear approximation yields satisfactory error.
pub const VCOCAP_MAX_VALUE: u8 = 0x3f;
pub const VCOCAP_EST_MIN: u8 = 15;
pub const VCOCAP_EST_MAX: u8 = 55;
pub const VCOCAP_EST_RANGE: u8 = VCOCAP_EST_MAX - VCOCAP_EST_MIN;
pub const VCOCAP_EST_THRESH: u8 = 7; // Complain if we're +/- 7 on our guess

/// If this bit is set, configure PLL output buffers for operation in the
/// bladeRF's "low band." Otherwise, configure the device for operation in the
/// "high band."
pub const LMS_FREQ_FLAGS_LOW_BAND: u8 = 1 << 0;

/// Use VCOCAP value as-is, rather as using it as a starting point hint
/// to the tuning algorithm.  This offers a faster retune, with a potential
/// trade-off in phase noise.
pub const LMS_FREQ_FLAGS_FORCE_VCOCAP: u8 = 1 << 1;

/// This bit indicates whether the quicktune needs to set XB-200 parameters
pub const LMS_FREQ_XB_200_ENABLE: u8 = 1 << 7;

/// This bit indicates the quicktune is for the RX module, not setting this bit
/// indicates the quicktune is for the TX module.
pub const LMS_FREQ_XB_200_MODULE_RX: u8 = 1 << 6;

/// This is the bit mask for the filter switch configuration for the XB-200.
pub const LMS_FREQ_XB_200_FILTER_SW: u8 = 3 << 4;

/// Macro that indicates the number of bitshifts necessary to get to the filter
/// switch field
pub const LMS_FREQ_XB_200_FILTER_SW_SHIFT: u8 = 4;

/// This is the bit mask for the path configuration for the XB-200.
pub const LMS_FREQ_XB_200_PATH: u8 = 3 << 2;

/// Macro that indicates the number of bitshifts necessary to get to the path
/// field
pub const LMS_FREQ_XB_200_PATH_SHIFT: u8 = 2;

pub const VTUNE_DELAY_LARGE: u8 = 50;
pub const VTUNE_DELAY_SMALL: u8 = 25;
pub const VTUNE_MAX_ITERATIONS: u8 = 20;

pub const VCO_HIGH: u8 = 0x02;
pub const VCO_NORM: u8 = 0x00;
pub const VCO_LOW: u8 = 0x01;

/// These values are the max counts we've seen (experimentally) between
/// VCOCAP values that converged
pub const VCOCAP_MAX_LOW_HIGH: u8 = 12;

#[derive(Debug, Default)]
pub struct LmsFreq {
    /// Choice of VCO and dision ratio
    pub freqsel: u8,
    /// VCOCAP hint
    pub vcocap: u8,
    /// Integer portion of f_LO given f_REF
    pub nint: u16,
    /// Fractional portion of f_LO given nint and f_REF
    pub nfrac: u32,
    /// Additional parameters defining the tuning configuration. See LMFS_FREQ_FLAGS_* values
    pub flags: u8,
    /// Store XB-200 switch settings
    pub xb_gpio: u8,
    /// VCO division ratio
    pub x: u8,
    /// Filled in by retune operation to denote which VCOCAP value was used
    pub vcocap_result: u8,
}

// pub struct FrequencyHz {
//     pub hz: u64,
// }

impl From<&LmsFreq> for u64 {
    fn from(value: &LmsFreq) -> Self {
        let pll_coeff = ((value.nint as u64) << 23) + value.nfrac as u64;
        let div = (value.x as u64) << 23;

        // FrequencyHz {
        //     hz: ((LMS_REFERENCE_HZ as u64 * pll_coeff) + (div >> 1)) / div,
        // }
        ((LMS_REFERENCE_HZ as u64 * pll_coeff) + (div >> 1)) / div
    }
}

impl TryFrom<u64> for LmsFreq {
    type Error = Error;

    fn try_from(value: u64) -> std::result::Result<Self, Self::Error> {
        /// This is a linear interpolation of our experimentally identified
        /// mean VCOCAP min and VCOCAP max values.
        ///
        /// The MIN/MAX values were determined experimentally by
        /// sampling the VCOCAP values over frequency, for each of the VCOs and finding
        /// these to be in the "middle" of a linear regression. Although the curve
        /// isn't actually linear, the linear approximation yields satisfactory error.
        fn estimate_vcocap(f_target: u32, f_low: u32, f_high: u32) -> u8 {
            let denom: f32 = (f_high - f_low) as f32;
            let num: f32 = VCOCAP_EST_RANGE as f32;
            let f_diff: f32 = (f_target - f_low) as f32;

            let vcocap = (num / denom * f_diff) + 0.5 + VCOCAP_EST_MIN as f32;

            if vcocap > VCOCAP_MAX_VALUE as f32 {
                log::debug!("Clamping VCOCAP estimate from {vcocap} to {VCOCAP_MAX_VALUE}");
                VCOCAP_MAX_VALUE
            } else {
                log::debug!("VCOCAP estimate: {vcocap}");
                vcocap as u8
            }
        }

        // /// Several parameters are required to tune the LMS to a specific frequency.
        // /// These parameters are being calculated in this function.
        let mut f: LmsFreq = LmsFreq::default();

        // Clamp out of range values
        let freq = value.clamp(BLADERF_FREQUENCY_MIN as u64, BLADERF_FREQUENCY_MAX as u64);

        // Figure out freqsel
        let freq_range = BANDS
            .iter()
            .find(|freq_range| (freq >= freq_range.low) && (freq <= freq_range.high))
            .ok_or(Error::Argument("Could not determine frequency range"))?;

        f.freqsel = freq_range.value;
        log::trace!("f.freqsel: {}", f.freqsel);

        // Estimate our target VCOCAP value.
        f.vcocap = estimate_vcocap(freq as u32, freq_range.low as u32, freq_range.high as u32);
        log::trace!("f.vcocap: {}", f.vcocap);

        // Calculate the integer portion of the frequency value
        let vco_x = 1 << ((f.freqsel & 7) - 3);
        log::trace!("vco_x: {vco_x}");
        assert!(vco_x <= u8::MAX as u64);
        f.x = vco_x as u8;
        log::trace!("f.x: {}", f.x);
        let mut temp = (vco_x * freq) / LMS_REFERENCE_HZ as u64;
        assert!(temp <= u16::MAX as u64);
        f.nint = temp as u16;
        log::trace!("f.nint: {}", f.nint);

        temp = (1 << 23) * (vco_x * freq - f.nint as u64 * LMS_REFERENCE_HZ as u64);
        temp = (temp + LMS_REFERENCE_HZ as u64 / 2) / LMS_REFERENCE_HZ as u64;
        assert!(temp <= u32::MAX as u64);
        f.nfrac = temp as u32;
        log::trace!("f.nfrac: {}", f.nfrac);

        assert!(LMS_REFERENCE_HZ as u64 <= u32::MAX as u64);

        if freq < BLADERF1_BAND_HIGH as u64 {
            f.flags |= LMS_FREQ_FLAGS_LOW_BAND;
        }
        log::trace!("f.flags: {}", f.flags);
        Ok(f)
    }
}

/// For >= 1.5 GHz uses the high band should be used. Otherwise, the low
/// band should be selected
pub const BLADERF1_BAND_HIGH: u32 = 1500000000;

/// LPF conversion table
/// This table can be indexed into.
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

/// Internal low-pass filter bandwidth selection
pub enum LmsBw {
    /// 28MHz bandwidth, 14MHz LPF
    Bw28mhz,
    /// 20MHz bandwidth, 10MHz LPF
    Bw20mhz,
    /// 14MHz bandwidth, 7MHz LPF
    Bw14mhz,
    /// 12MHz bandwidth, 6MHz LPF
    Bw12mhz,
    /// 10MHz bandwidth, 5MHz LPF
    Bw10mhz,
    /// 8.75MHz bandwidth, 4.375MHz LPF
    Bw8p75mhz,
    /// 7MHz bandwidth, 3.5MHz LPF
    Bw7mhz,
    /// 6MHz bandwidth, 3MHz LPF
    Bw6mhz,
    /// 5.5MHz bandwidth, 2.75MHz LPF
    Bw5p5mhz,
    /// 5MHz bandwidth, 2.5MHz LPF
    Bw5mhz,
    /// 3.84MHz bandwidth, 1.92MHz LPF
    Bw3p84mhz,
    /// 3MHz bandwidth, 1.5MHz LPF
    Bw3mhz,
    /// 2.75MHz bandwidth, 1.375MHz LPF
    Bw2p75mhz,
    /// 2.5MHz bandwidth, 1.25MHz LPF
    Bw2p5mhz,
    /// 1.75MHz bandwidth, 0.875MHz LPF
    Bw1p75mhz,
    /// 1.5MHz bandwidth, 0.75MHz LPF
    Bw1p5mhz,
}

impl LmsBw {
    /// The LMS requires the different bandwidths being translated to indices
    /// This index is then written to a specific register to set the LPF.
    fn from_index(index: u8) -> Self {
        match index {
            1 => LmsBw::Bw20mhz,
            2 => LmsBw::Bw14mhz,
            3 => LmsBw::Bw12mhz,
            4 => LmsBw::Bw10mhz,
            5 => LmsBw::Bw8p75mhz,
            6 => LmsBw::Bw7mhz,
            7 => LmsBw::Bw6mhz,
            8 => LmsBw::Bw5p5mhz,
            9 => LmsBw::Bw5mhz,
            10 => LmsBw::Bw3p84mhz,
            11 => LmsBw::Bw3mhz,
            12 => LmsBw::Bw2p75mhz,
            13 => LmsBw::Bw2p5mhz,
            14 => LmsBw::Bw1p75mhz,
            15 => LmsBw::Bw1p5mhz,
            _ => LmsBw::Bw28mhz,
        }
    }

    fn to_index(&self) -> u8 {
        match self {
            LmsBw::Bw28mhz => 0,
            LmsBw::Bw20mhz => 1,
            LmsBw::Bw14mhz => 2,
            LmsBw::Bw12mhz => 3,
            LmsBw::Bw10mhz => 4,
            LmsBw::Bw8p75mhz => 5,
            LmsBw::Bw7mhz => 6,
            LmsBw::Bw6mhz => 7,
            LmsBw::Bw5p5mhz => 8,
            LmsBw::Bw5mhz => 9,
            LmsBw::Bw3p84mhz => 10,
            LmsBw::Bw3mhz => 11,
            LmsBw::Bw2p75mhz => 12,
            LmsBw::Bw2p5mhz => 13,
            LmsBw::Bw1p75mhz => 14,
            LmsBw::Bw1p5mhz => 15,
        }
    }
}

impl From<LmsBw> for u32 {
    fn from(value: LmsBw) -> Self {
        match value {
            LmsBw::Bw28mhz => mhz!(28),
            LmsBw::Bw20mhz => mhz!(20),
            LmsBw::Bw14mhz => mhz!(14),
            LmsBw::Bw12mhz => mhz!(12),
            LmsBw::Bw10mhz => mhz!(10),
            LmsBw::Bw8p75mhz => khz!(8750),
            LmsBw::Bw7mhz => mhz!(7),
            LmsBw::Bw6mhz => mhz!(6),
            LmsBw::Bw5p5mhz => khz!(5500),
            LmsBw::Bw5mhz => mhz!(5),
            LmsBw::Bw3p84mhz => khz!(3840),
            LmsBw::Bw3mhz => mhz!(3),
            LmsBw::Bw2p75mhz => khz!(2750),
            LmsBw::Bw2p5mhz => khz!(2500),
            LmsBw::Bw1p75mhz => khz!(1750),
            LmsBw::Bw1p5mhz => khz!(1500),
        }
    }
}
impl From<u32> for LmsBw {
    fn from(value: u32) -> Self {
        if value <= khz!(1500) {
            LmsBw::Bw1p5mhz
        } else if value <= khz!(1750) {
            LmsBw::Bw1p75mhz
        } else if value <= khz!(2500) {
            LmsBw::Bw2p5mhz
        } else if value <= khz!(2750) {
            LmsBw::Bw2p75mhz
        } else if value <= mhz!(3) {
            LmsBw::Bw3mhz
        } else if value <= khz!(3840) {
            LmsBw::Bw3p84mhz
        } else if value <= mhz!(5) {
            LmsBw::Bw5mhz
        } else if value <= khz!(5500) {
            LmsBw::Bw5p5mhz
        } else if value <= mhz!(6) {
            LmsBw::Bw6mhz
        } else if value <= mhz!(7) {
            LmsBw::Bw7mhz
        } else if value <= khz!(8750) {
            LmsBw::Bw8p75mhz
        } else if value <= mhz!(10) {
            LmsBw::Bw10mhz
        } else if value <= mhz!(12) {
            LmsBw::Bw12mhz
        } else if value <= mhz!(14) {
            LmsBw::Bw14mhz
        } else if value <= mhz!(20) {
            LmsBw::Bw20mhz
        } else {
            LmsBw::Bw28mhz
        }
    }
}

/// LNA options
#[derive(Clone)]
pub enum LmsLna {
    /// Disable all LNAs
    LnaNone,
    /// Enable LNA1 (300MHz - 2.8GHz)
    Lna1,
    /// Enable LNA2 (1.5GHz - 3.8GHz)
    Lna2,
    /// Enable LNA3 (Unused on the bladeRF)
    Lna3,
}

impl From<LmsLna> for u8 {
    fn from(value: LmsLna) -> Self {
        match value {
            LmsLna::LnaNone => 0,
            LmsLna::Lna1 => 1,
            LmsLna::Lna2 => 2,
            LmsLna::Lna3 => 3,
        }
    }
}

impl TryFrom<u8> for LmsLna {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self> {
        match value {
            0 => Ok(LmsLna::LnaNone),
            1 => Ok(LmsLna::Lna1),
            2 => Ok(LmsLna::Lna2),
            3 => Ok(LmsLna::Lna3),
            _ => Err(Error::Invalid),
        }
    }
}

/// Loopback paths
pub enum LmsLbp {
    ///  Baseband loopback path
    LbpBb,
    ///  RF Loopback path
    LbpRf,
}

/// PA Selection
pub enum LmsPa {
    /// AUX PA Enable (for RF Loopback)
    PaAux,
    /// PA1 Enable (300MHz - 2.8GHz)
    Pa1,
    /// PA2 Enable (1.5GHz - 3.8GHz)
    Pa2,
    /// All PAs disabled
    PaNone,
}

/// SDR (Software-Defined Radio) loopback modes are configurations that route a signal from
/// the transmitter back to the receiver for testing and self-diagnosis.
/// This can be done physically by cabling a transmit port to a receive port, or digitally by
/// routing signals between the digital transmit and receive stages within the device.
/// These modes allow users to test the radio's functionality, like validating signal processing
/// or calibrating components, without using external antennas or an over-the-air connection.
#[derive(PartialEq, Debug, Clone)]
#[repr(u8)]
pub enum Loopback {
    /// Disables loopback and returns to normal operation.
    None = 0,

    /// Firmware loopback inside of the FX3
    Firmware,

    /// Baseband loopback. TXLPF output is connected to the RXVGA2 input.
    BbTxlpfRxvga2,

    /// Baseband loopback. TXVGA1 output is connected to the RXVGA2 input.
    BbTxvga1Rxvga2,

    /// Baseband loopback. TXLPF output is connected to the RXLPF input.
    BbTxlpfRxlpf,

    /// Baseband loopback. TXVGA1 output is connected to RXLPF input.
    BbTxvga1Rxlpf,

    ///  RF loopback. The TXMIX output, through the AUX PA, is connected to the
    ///  output of LNA1.
    Lna1,

    ///  RF loopback. The TXMIX output, through the AUX PA, is connected to the
    ///  output of LNA2.
    Lna2,

    ///  RF loopback. The TXMIX output, through the AUX PA, is connected to the
    ///  output of LNA3.
    Lna3,

    /// RFIC digital loopback (built-in self-test)
    RficBist,
}

// impl TryFrom<u8> for BladeRf1Loopback {
//     type Error = ();
//
//     fn try_from(value: u8) -> Result<Self, Self::Error> {
//         match value {
//             0 => Ok(BladeRf1Loopback::None),
//             1 => Ok(BladeRf1Loopback::Firmware),
//             2 => Ok(BladeRf1Loopback::BbTxlpfRxvga2),
//             3 => Ok(BladeRf1Loopback::BbTxvga1Rxvga2),
//             4 => Ok(BladeRf1Loopback::BbTxlpfRxlpf),
//             5 => Ok(BladeRf1Loopback::BbTxvga1Rxlpf),
//             6 => Ok(BladeRf1Loopback::Lna1),
//             7 => Ok(BladeRf1Loopback::Lna2),
//             8 => Ok(BladeRf1Loopback::Lna3),
//             9 => Ok(BladeRf1Loopback::RficBist),
//             _ => Err(()),
//         }
//     }
// }

///  Mapping of human-readable names to loopback modes
pub struct BladeRf1LoopbackModes {
    /// Name of loopback mode
    _name: String,
    /// Loopback mode enumeration
    _mode: Loopback,
}

/// LMS6002D Transceiver configuration
pub struct LmsXcvrConfig {
    /// Transmit frequency in Hz
    tx_freq_hz: u32,
    /// Receive frequency in Hz
    rx_freq_hz: u32,
    /// Loopback Mode
    loopback_mode: Loopback,
    /// LNA Selection
    lna: LmsLna,
    /// PA Selection
    pa: LmsPa,
    /// Transmit Bandwidth
    tx_bw: LmsBw,
    /// Receive Bandwidth
    rx_bw: LmsBw,
}

/// Representation of the programmable LMS6002D Transceiver
///
/// The LMS6002D can be digitally configured to operate on any mobile communications frequency
/// bands (300MHz to 3.8GHz) and be used on any 2G, 3G or 4G mobile communications standard.
/// Additionally, users can easily configure the device to run with 16 bandwidths up to 28MHz.
///
/// The chip incorporates a multiplicity of RF inputs and outputs to enable a wide range
/// of features to be implemented. Its 12-bit ADC and DAC blocks allow it to directly
/// interface with virtually any baseband, DSP and FPGA ICs.
///
/// The LMS6002D has a standard Serial Port Interface (SPI) for programming and includes
/// provision for a full RF calibration. The device combines LNA, PA driver, RX/TX mixers,
/// RX/TX filters, synthesizers, RX gain control, and TX power control with very few external
/// components.
///   - Single chip transceiver
///   - Covers 300MHz to 3.8GHz
///   - Fully differential baseband signals
///   - Few external components
///   - Programmable modulation bandwidth: 1.5, 1.75, 2.5, 2.75, 3, 3.84, 5, 5.5, 6, 7, 8.75, 10, 12, 14, 20 and 28MHz
///   - Supports both FDD and TDD full duplex
///   - Integrated high performance 12-bit ADC and DAC
///   - Low voltage operation, 1.8V and 3.3V
///   - Standby current less than 1mA
///   - Tx RF output +6dBm, continuous wave
///   - 120 pin DQFN package
///   - Provision for Full Calibration
///   - Power down
///   - Serial interface
#[derive(Clone)]
pub struct LMS6002D {
    /// The communication with the LMS6002D is done over an NUSB interface
    interface: Arc<Mutex<Interface>>,
}

impl LMS6002D {
    /// Create a new instance of an LMS6002D Transceiver
    ///
    /// Expects a handle to an NUSB interface to the BladeRF1.
    // # Examples
    //
    // ```no_run
    // use std::sync::{Arc, Mutex};
    // use libbladerf_rs::{Result, Error};
    // use libbladerf_rs::hardware::lms6002d::LMS6002D;
    // use nusb::MaybeFuture;
    // use libbladerf_rs::bladerf1::BladeRf1;
    //
    // let device = BladeRf1::list_bladerf1()?.next().ok_or(Error::NotFound)?.open().wait()?;
    // let interface = Arc::new(Mutex::new(device.detach_and_claim_interface(0).wait()?));
    // let lms = LMS6002D::new(interface);
    // # Ok::<(), Error>(())
    // ```
    pub fn new(interface: Arc<Mutex<Interface>>) -> Self {
        Self { interface }
    }

    /// Read the LMS6002D configuration by specifying the address
    /// of the configuration value to be read from.
    pub fn read(&self, addr: u8) -> Result<u8> {
        self.interface
            .lock()
            .unwrap()
            .nios_read::<u8, u8>(NIOS_PKT_8X8_TARGET_LMS6, addr)
    }

    /// Write the LMS6002D configuration by specifying the address
    /// of the configuration value to write to.
    pub fn write(&self, addr: u8, data: u8) -> Result<()> {
        self.interface
            .lock()
            .unwrap()
            .nios_write::<u8, u8>(NIOS_PKT_8X8_TARGET_LMS6, addr, data)
    }

    /// Set a specific Bit in the LMS6002D configuration specified
    /// by the address of the value that should be changed. The bits in the supplied mask parameter
    /// will be set in the selected configuration.
    pub fn set(&self, addr: u8, mask: u8) -> Result<()> {
        let data = self.read(addr)?;
        self.write(addr, data | mask)
    }

    /// Soft reset of the LMS
    pub fn soft_reset(&self) -> Result<()> {
        self.write(0x05, 0x12)?;
        self.write(0x05, 0x32)
    }

    /// Get the values of the Voltage Tuning Comparators (VTUNE comparators)
    /// The base parameter defines, which comparator to select.
    /// The state of the comparators can be obtained by powering them up
    /// (register 0x1B for TXPLL or 0x2B for RXPLL, bit 3) and reading the register
    /// 0x1A for TXPPLL or 0x2A for RXPLL, bits 7-6.
    /// Details can be found in the LMS6002 Programming and Calibration Guide.
    pub fn get_vtune(&self, base: u8, delay: u8) -> Result<u8> {
        if delay != 0 {
            sleep(Duration::from_micros(delay as u64));
        }

        let vtune = self.read(base + 10)?;
        Ok(vtune >> 6)
    }

    /// Enable or disable RX or TX RF Frontend of the LMS6002D
    pub fn enable_rffe(&self, module: u8, enable: bool) -> Result<()> {
        let (addr, shift) = if module == BLADERF_MODULE_TX {
            (0x40u8, 1u8)
        } else {
            (0x70u8, 0u8)
        };
        let mut data = self.read(addr)?;

        if enable {
            data |= 1 << shift;
        } else {
            data &= !(1 << shift);
        }
        self.write(addr, data)
    }

    /// Configure the LMS6002D charge pumps
    ///
    /// A voltage-controlled oscillator charge pump (VCO CP) is a circuit used in
    /// phase-locked loops (PLLs) to translate digital signals from a phase-frequency detector (PFD)
    /// into an analog control voltage that regulates the VCO's output frequency.
    ///
    /// It essentially acts as a converter, transforming the PFD's digital pulse outputs into a
    /// stable analog voltage that tunes the VCO. This analog voltage is crucial for maintaining
    /// synchronization and achieving frequency locking within the PLL.
    pub fn config_charge_pumps(&self, module: u8) -> Result<()> {
        let base: u8 = if module == BLADERF_MODULE_RX {
            0x20
        } else {
            0x10
        };

        // Set PLL Ichp (Charge Pump) current
        let mut data = self.read(base + 6)?;
        data &= !0x1f;
        data |= 0x0c;
        self.write(base + 6, data)?;

        // Set Iup (Charge pump UP offset) current
        data = self.read(base + 7)?;
        data &= !0x1f;
        data |= 0x03;
        self.write(base + 7, data)?;

        // Set Idn (Charge pump DOWN offset) current
        data = self.read(base + 8)?;
        data &= !0x1f;
        data |= 0x03;
        self.write(base + 8, data)
    }

    pub fn write_vcocap(&self, base: u8, vcocap: u8, vcocap_reg_state: u8) -> Result<()> {
        assert!(vcocap <= VCOCAP_MAX_VALUE);
        log::trace!("Writing VCOCAP={vcocap}");

        self.write(base + 9, vcocap | vcocap_reg_state)
    }

    /// Set the loopback path.
    pub fn loopback_path(&self, mode: &Loopback) -> Result<()> {
        let mut loopbben = self.read(0x46)?;
        let mut lben_lbrf = self.read(0x08)?;

        // Default to baseband loopback being disabled
        loopbben &= !LOOBBBEN_MASK;

        // Default to RF and BB loopback options being disabled
        lben_lbrf &= !(LBRFEN_MASK | LBEN_MASK);

        match mode {
            Loopback::None => {}
            Loopback::BbTxlpfRxvga2 => {
                loopbben |= LOOPBBEN_TXLPF;
                lben_lbrf |= LBEN_VGA2IN;
            }
            Loopback::BbTxvga1Rxvga2 => {
                loopbben |= LOOPBBEN_TXVGA;
                lben_lbrf |= LBEN_VGA2IN;
            }
            Loopback::BbTxlpfRxlpf => {
                loopbben |= LOOPBBEN_TXLPF;
                lben_lbrf |= LBEN_LPFIN;
            }
            Loopback::BbTxvga1Rxlpf => {
                loopbben |= LOOPBBEN_TXVGA;
                lben_lbrf |= LBEN_LPFIN;
            }
            Loopback::Lna1 => {
                lben_lbrf |= LBRFEN_LNA1;
            }
            Loopback::Lna2 => {
                lben_lbrf |= LBRFEN_LNA2;
            }
            Loopback::Lna3 => {
                lben_lbrf |= LBRFEN_LNA3;
            }
            _ => Err(Error::Argument("Loopback mode not supported"))?,
        }

        self.write(0x46, loopbben)?;
        self.write(0x08, lben_lbrf)
    }

    /// Get the LowPassFilter mode for a specific channel.
    pub fn lpf_get_mode(&self, channel: u8) -> Result<LpfMode> {
        let reg: u8 = if channel == BLADERF_MODULE_RX {
            0x54
        } else {
            0x34
        };

        let data_l = self.read(reg)?;
        let data_h = self.read(reg + 1)?;

        let lpf_enabled = (data_l & (1 << 1)) != 0;
        let lpf_bypassed = (data_h & (1 << 6)) != 0;

        if lpf_enabled && !lpf_bypassed {
            Ok(LpfMode::Normal)
        } else if !lpf_enabled && lpf_bypassed {
            Ok(LpfMode::Bypassed)
        } else if !lpf_enabled && !lpf_bypassed {
            Ok(LpfMode::Disabled)
        } else {
            log::error!("Invalid LPF configuration: {data_l:x}, {data_h:x}");
            Err(Error::Invalid)
        }
    }

    /// Set the LowPassFilter mode for a specific channel.
    pub fn lpf_set_mode(&self, channel: u8, mode: LpfMode) -> Result<()> {
        let reg: u8 = if channel == BLADERF_MODULE_RX {
            0x54
        } else {
            0x34
        };

        let mut data_l = self.read(reg)?;
        let mut data_h = self.read(reg + 1)?;

        match mode {
            LpfMode::Normal => {
                // Enable LPF
                data_l |= 1 << 1;
                // Disable LPF bypass
                data_h &= !(1 << 6);
            }
            LpfMode::Bypassed => {
                // Power down LPF
                data_l &= !(1 << 1);
                // Enable LPF bypass
                data_h |= 1 << 6;
            }
            LpfMode::Disabled => {
                // Power down LPF
                data_l &= !(1 << 1);
                // Disable LPF bypass
                data_h &= !(1 << 6);
            }
        }

        self.write(reg, data_l)?;
        self.write(reg + 1, data_h)
    }

    /// Power up/down RF loopback switch
    pub fn enable_rf_loopback_switch(&self, enable: bool) -> Result<()> {
        let mut regval = self.read(0x0b)?;

        if enable {
            regval |= 1;
        } else {
            regval &= !1;
        }

        self.write(0x0b, regval)
    }

    /// Configure RX-side of loopback
    pub fn loopback_rx(&self, mode: &Loopback) -> Result<()> {
        let lpf_mode = self.lpf_get_mode(BLADERF_MODULE_RX)?;
        match mode {
            Loopback::None => {
                // Ensure all RX blocks are enabled
                self.rxvga1_enable(true)?;

                if lpf_mode == LpfMode::Disabled {
                    self.lpf_set_mode(BLADERF_MODULE_RX, LpfMode::Disabled)?;
                }

                self.rxvga2_enable(true)?;

                // Disable RF loopback switch
                self.enable_rf_loopback_switch(false)?;

                // Power up LNAs
                self.enable_lna_power(true)?;

                // Restore proper settings (LNA, RX PLL) for this frequency
                let f = &self.get_frequency(BLADERF_MODULE_RX)?;
                self.set_frequency(BLADERF_MODULE_RX, f.into())?;
                let f_hz: u64 = f.into();
                let band = if f_hz < BLADERF1_BAND_HIGH as u64 {
                    Band::Low
                } else {
                    Band::High
                };
                self.select_band(BLADERF_MODULE_RX, band)
            }
            Loopback::BbTxvga1Rxvga2 | Loopback::BbTxlpfRxvga2 => {
                // Ensure RXVGA2 is enabled
                self.rxvga2_enable(true)?;
                // RXLPF must be disabled
                self.lpf_set_mode(BLADERF_MODULE_RX, LpfMode::Disabled)
            }
            Loopback::BbTxlpfRxlpf | Loopback::BbTxvga1Rxlpf => {
                // RXVGA1 must be disabled
                self.rxvga1_enable(false)?;

                // Enable the RXLPF if needed
                if lpf_mode == LpfMode::Disabled {
                    self.lpf_set_mode(BLADERF_MODULE_RX, LpfMode::Disabled)?;
                }

                // Ensure RXVGA2 is enabled
                self.rxvga2_enable(true)
            }
            Loopback::Lna1 | Loopback::Lna2 | Loopback::Lna3 => {
                let lms_lna = match mode {
                    Loopback::Lna1 => LmsLna::Lna1,
                    Loopback::Lna2 => LmsLna::Lna2,
                    Loopback::Lna3 => LmsLna::Lna3,
                    _ => return Err(Error::Argument("Could not convert LNA mode.")),
                };

                // Power down LNAs
                self.enable_lna_power(false)?;

                // Ensure RXVGA1 is enabled
                self.rxvga1_enable(true)?;

                // Enable the RXLPF if needed
                if lpf_mode == LpfMode::Disabled {
                    self.lpf_set_mode(BLADERF_MODULE_RX, LpfMode::Disabled)?;
                }

                // Ensure RXVGA2 is enabled
                self.rxvga2_enable(true)?;

                // Select output buffer in RX PLL and select the desired LNA
                let mut regval = self.read(0x25)?;
                regval &= !0x03;
                // regval |= lna;
                regval |= u8::from(lms_lna.clone());

                self.write(0x25, regval)?;

                self.select_lna(lms_lna)?;

                // Enable RF loopback switch
                self.enable_rf_loopback_switch(true)
            }
            _ => Err(Error::Argument("Could not convert LNA mode.")),
        }
    }

    /// Configure TX-side of loopback
    pub fn loopback_tx(&self, mode: &Loopback) -> Result<()> {
        match mode {
            Loopback::None => {
                // Restore proper settings (PA) for this frequency
                let f = &self.get_frequency(BLADERF_MODULE_TX)?;
                self.set_frequency(BLADERF_MODULE_TX, f.into())?;

                let f_hz: u64 = f.into();
                let band = if f_hz < BLADERF1_BAND_HIGH as u64 {
                    Band::Low
                } else {
                    Band::High
                };
                self.select_band(BLADERF_MODULE_TX, band)
            }
            Loopback::BbTxlpfRxvga2
            | Loopback::BbTxvga1Rxvga2
            | Loopback::BbTxlpfRxlpf
            | Loopback::BbTxvga1Rxlpf => Ok(()),
            Loopback::Lna1 | Loopback::Lna2 | Loopback::Lna3 => self.select_pa(LmsPa::PaAux),
            _ => Err(Error::Argument("Invalid loopback mode encountered")),
        }
    }

    /// Set the Loopback mode.
    pub fn set_loopback_mode(&self, mode: Loopback) -> Result<()> {
        // Verify a valid mode is provided before shutting anything down
        match mode {
            Loopback::None => {}
            Loopback::BbTxlpfRxvga2 => {}
            Loopback::BbTxvga1Rxvga2 => {}
            Loopback::BbTxlpfRxlpf => {}
            Loopback::BbTxvga1Rxlpf => {}
            Loopback::Lna1 => {}
            Loopback::Lna2 => {}
            Loopback::Lna3 => {}
            _ => return Err(Error::Argument("Unsupported loopback mode")),
        }

        // Disable all PA/LNAs while entering loopback mode or making changes
        self.select_pa(LmsPa::PaNone)?;
        self.select_lna(LmsLna::LnaNone)?;

        // Disconnect loopback paths while we re-configure blocks
        self.loopback_path(&Loopback::None)?;

        // Configure the RX side of the loopback path
        self.loopback_rx(&mode)?;

        // Configure the TX side of the path
        self.loopback_tx(&mode)?;

        // Configure "switches" along the loopback path
        self.loopback_path(&mode)
    }

    /// Get the currently selected Loopback mode.
    pub fn get_loopback_mode(&self) -> Result<Loopback> {
        let mut loopback = Loopback::None;

        let lben_lbrfen = self.read(0x08)?;
        let loopbben = self.read(0x46)?;

        match lben_lbrfen & 0x7 {
            LBRFEN_LNA1 => {
                loopback = Loopback::Lna1;
            }
            LBRFEN_LNA2 => {
                loopback = Loopback::Lna2;
            }
            LBRFEN_LNA3 => {
                loopback = Loopback::Lna3;
            }
            _ => {}
        }

        match lben_lbrfen & LBEN_MASK {
            LBEN_VGA2IN => {
                if (loopbben & LOOPBBEN_TXLPF) != 0 {
                    loopback = Loopback::BbTxlpfRxvga2;
                } else if (loopbben & LOOPBBEN_TXVGA) != 0 {
                    loopback = Loopback::BbTxvga1Rxvga2;
                }
            }
            LBEN_LPFIN => {
                if (loopbben & LOOPBBEN_TXLPF) != 0 {
                    loopback = Loopback::BbTxlpfRxlpf;
                } else if (loopbben & LOOPBBEN_TXVGA) != 0 {
                    loopback = Loopback::BbTxvga1Rxlpf;
                }
            }
            _ => {}
        }

        Ok(loopback)
    }

    /// Check if the looback mode is enabled (Loopback mode is not None)
    pub fn is_loopback_enabled(&self) -> Result<bool> {
        let loopback = self.get_loopback_mode()?;

        Ok(loopback != Loopback::None)
    }

    /// Write PhaseLockedLoop configuration
    pub fn write_pll_config(&self, module: u8, freqsel: u8, low_band: bool) -> Result<()> {
        let addr = if module == BLADERF_MODULE_TX {
            0x15
        } else {
            0x25
        };

        let mut regval = self.read(addr)?;

        let lb_enabled: bool = self.is_loopback_enabled()?;

        if !lb_enabled {
            // Loopback not enabled - update the PLL output buffer.
            let selout = if low_band { 1 } else { 2 };
            regval = (freqsel << 2) | selout;
        } else {
            // Loopback is enabled - don't touch PLL output buffer.
            regval = (regval & !0xfc) | (freqsel << 2);
        }

        self.write(addr, regval)
    }

    pub fn vtune_high_to_norm(&self, base: u8, mut vcocap: u8, vcocap_reg_state: u8) -> Result<u8> {
        for _ in 0..VTUNE_MAX_ITERATIONS {
            if vcocap >= VCOCAP_MAX_VALUE {
                log::trace!("vtune_high_to_norm: VCOCAP hit max value.");
                return Ok(VCOCAP_MAX_VALUE);
            }

            vcocap += 1;

            self.write_vcocap(base, vcocap, vcocap_reg_state)?;

            let vtune = self.get_vtune(base, VTUNE_DELAY_SMALL)?;

            if vtune == VCO_NORM {
                log::trace!("VTUNE NORM @ VCOCAP={vcocap}");
                return Ok(vcocap - 1);
            }
        }

        log::error!("VTUNE High->Norm loop failed to converge.");
        Err(Error::Invalid)
    }

    pub fn vtune_norm_to_high(&self, base: u8, mut vcocap: u8, vcocap_reg_state: u8) -> Result<u8> {
        for _ in 0..VTUNE_MAX_ITERATIONS {
            log::trace!("base: {base}, vcocap: {vcocap}, vcocap_reg_state: {vcocap_reg_state}");

            if vcocap == 0 {
                log::debug!("vtune_norm_to_high: VCOCAP hit min value.");
                return Ok(0);
            }

            vcocap -= 1;

            self.write_vcocap(base, vcocap, vcocap_reg_state)?;

            let vtune = self.get_vtune(base, VTUNE_DELAY_SMALL)?;
            log::trace!("vtune: {vtune}");

            if vtune == VCO_HIGH {
                log::debug!("VTUNE HIGH @ VCOCAP={vcocap}");
                return Ok(vcocap);
            }
        }

        log::error!("VTUNE Norm->High loop failed to converge.");
        Err(Error::Invalid)
    }

    
    pub fn vtune_low_to_norm(&self, base: u8, mut vcocap: u8, vcocap_reg_state: u8) -> Result<u8> {
        for _ in 0..VTUNE_MAX_ITERATIONS {
            if vcocap == 0 {
                log::debug!("vtune_low_to_norm: VCOCAP hit min value.");
                return Ok(0);
            }

            vcocap -= 1;

            self.write_vcocap(base, vcocap, vcocap_reg_state)?;

            let vtune = self.get_vtune(base, VTUNE_DELAY_SMALL)?;

            if vtune == VCO_NORM {
                log::debug!("VTUNE NORM @ VCOCAP={vcocap}");
                return Ok(vcocap + 1);
            }
        }

        log::error!("VTUNE Low->Norm loop failed to converge.");
        Err(Error::Invalid)
    }

    /// Wait for VTUNE to reach HIGH or LOW. NORM is not a valid option here
    pub fn wait_for_vtune_value(
        &self,
        base: u8,
        target_value: u8,
        vcocap: &mut u8,
        vcocap_reg_state: u8,
    ) -> Result<()> {
        const MAX_RETRIES: u32 = 15;
        let limit: u8 = if target_value == VCO_HIGH {
            0
        } else {
            VCOCAP_MAX_VALUE
        };
        let inc: i8 = if target_value == VCO_HIGH { -1 } else { 1 };

        assert!(target_value == VCO_HIGH || target_value == VCO_LOW);

        for i in 0..MAX_RETRIES {
            let vtune = self.get_vtune(base, 0)?;

            if vtune == target_value {
                log::debug!("VTUNE reached {target_value} at iteration {i}");
                return Ok(());
            } else {
                log::trace!("VTUNE was {vtune}. Waiting and retrying...");

                // Unneeded, due to USB transfer duration
                sleep(Duration::from_micros(10));
            }
        }

        log::trace!("Timed out while waiting for VTUNE={target_value}. Walking VCOCAP...");

        while *vcocap != limit {
            *vcocap = (*vcocap as i8 + inc) as u8;

            self.write_vcocap(base, *vcocap, vcocap_reg_state)?;

            let vtune = self.get_vtune(base, VTUNE_DELAY_SMALL)?;
            if vtune == target_value {
                log::debug!("VTUNE={vtune} reached with VCOCAP={vcocap}");
                return Ok(());
            }
        }

        log::debug!("VTUNE did not reach {target_value}. Tuning may not be nominal.");
        Ok(())
    }

    /// This function assumes an initial VCOCAP estimate has already been written.
    ///
    /// Remember, increasing VCOCAP works towards a lower voltage, and vice versa:
    /// From experimental observations, we don't expect to see the "normal" region
    /// extend beyond 16 counts.
    ///
    /// VCOCAP = 0              VCOCAP=63
    /// /                                 \
    /// v                                  v
    /// |----High-----\[ Normal \]----Low----|     VTUNE voltage comparison
    ///
    /// The VTUNE voltage can be found on R263 (RX) or R265 (Tx). (They're under the
    /// can shielding the LMS6002D.) By placing a scope probe on these and retuning,
    /// you should be able to see the relationship between VCOCAP changes and
    /// the voltage changes.
    pub fn tune_vcocap(&self, vcocap_est: u8, base: u8, vcocap_reg_state: u8) -> Result<u8> {
        let mut vcocap: u8 = vcocap_est;
        // Where VCOCAP puts use into VTUNE HIGH region
        let mut vtune_high_limit: u8 = VCOCAP_MAX_VALUE;
        // Where VCOCAP puts use into VTUNE LOW region
        let mut vtune_low_limit: u8 = 0;

        let mut vtune = self.get_vtune(base, VTUNE_DELAY_LARGE)?;

        match vtune {
            VCO_HIGH => {
                log::trace!("Estimate HIGH: Walking down to NORM.");
                vtune_high_limit = self.vtune_high_to_norm(base, vcocap, vcocap_reg_state)?;
            }
            VCO_NORM => {
                log::trace!("Estimate NORM: Walking up to HIGH.");
                vtune_high_limit = self.vtune_norm_to_high(base, vcocap, vcocap_reg_state)?;
            }
            VCO_LOW => {
                log::trace!("Estimate LOW: Walking down to NORM.");
                vtune_low_limit = self.vtune_low_to_norm(base, vcocap, vcocap_reg_state)?;
            }
            _ => {}
        }

        if vtune_high_limit != VCOCAP_MAX_VALUE {
            // We determined our VTUNE HIGH limit. Try to force ourselves to the
            // LOW limit and then walk back up to norm from there.
            //
            // Reminder - There's an inverse relationship between VTUNE and VCOCAP
            match vtune {
                VCO_NORM | VCO_HIGH => {
                    if (vtune_high_limit + VCOCAP_MAX_LOW_HIGH) < VCOCAP_MAX_VALUE {
                        vcocap = vtune_high_limit + VCOCAP_MAX_LOW_HIGH;
                    } else {
                        vcocap = VCOCAP_MAX_VALUE;
                        log::debug!("Clamping VCOCAP to {vcocap}.");
                    }
                }
                _ => {
                    log::error!("Invalid state");
                    return Err(Error::Invalid);
                }
            }

            self.write_vcocap(base, vcocap, vcocap_reg_state)?;

            log::trace!("Waiting for VTUNE LOW @ VCOCAP={vcocap}");
            self.wait_for_vtune_value(base, VCO_LOW, &mut vcocap, vcocap_reg_state)?;

            log::trace!("Walking VTUNE LOW to NORM from VCOCAP={vcocap}");
            vtune_low_limit = self.vtune_low_to_norm(base, vcocap, vcocap_reg_state)?;
        } else {
            // We determined our VTUNE LOW limit. Try to force ourselves up to
            //  the HIGH limit and then walk down to NORM from there
            //
            //  Reminder - There's an inverse relationship between VTUNE and VCOCAP
            match vtune {
                VCO_LOW | VCO_NORM => {
                    if (vtune_low_limit - VCOCAP_MAX_LOW_HIGH) > 0 {
                        vcocap = vtune_low_limit - VCOCAP_MAX_LOW_HIGH;
                    } else {
                        vcocap = 0;
                        log::debug!("Clamping VCOCAP to {vcocap}.");
                    }
                }
                _ => {
                    log::error!("Invalid state");
                    return Err(Error::Invalid);
                }
            }

            self.write_vcocap(base, vcocap, vcocap_reg_state)?;

            log::trace!("Waiting for VTUNE HIGH @ VCOCAP={vcocap}");
            self.wait_for_vtune_value(base, VCO_HIGH, &mut vcocap, vcocap_reg_state)?;

            log::trace!("Walking VTUNE HIGH to NORM from VCOCAP={vcocap}");
            vtune_high_limit = self.vtune_high_to_norm(base, vcocap, vcocap_reg_state)?;
        }

        vcocap = vtune_high_limit + (vtune_low_limit - vtune_high_limit) / 2;

        log::trace!("VTUNE LOW:   {vtune_low_limit}");
        log::trace!("VTUNE NORM:  {vcocap}");
        log::trace!("VTUNE Est:   {vcocap_est}");
        log::trace!("VTUNE HIGH:  {vtune_high_limit}");

        self.write_vcocap(base, vcocap, vcocap_reg_state)?;

        vtune = self.get_vtune(base, VTUNE_DELAY_SMALL)?;

        if vtune != VCO_NORM {
            log::error!("Final VCOCAP={vcocap} is not in VTUNE NORM region.");
            return Err(Error::Invalid);
        }
        // Inform the caller of what we converged to
        Ok(vcocap)
    }

    /// Set the frequency using an already existing LmsFreq struct with precalculated values.
    /// This can allow for faster retuning, as the parameters to fill the LmsFreq struct do
    /// not have to be calculated on the fly.
    pub fn set_precalculated_frequency(&self, module: u8, f: &mut LmsFreq) -> Result<()> {
        //  Select the base address based on which PLL we are configuring
        let base: u8 = if module == BLADERF_MODULE_RX {
            0x20
        } else {
            0x10
        };

        // Utilize atomic writes to the PLL registers, if possible. This
        // "multiwrite" is indicated by the MSB being set.
        let pll_base: u8 = base | 0x80;

        f.vcocap_result = 0xff;

        // Turn on the DSMs
        let mut data = self.read(0x09)?;
        data |= 0x05;
        self.write(0x09, data)?;

        // Write the initial vcocap estimate first to allow for adequate time for
        // VTUNE to stabilize. We need to be sure to keep the upper bits of
        // this register and perform a RMW, as bit 7 is VOVCOREG[0].
        let result = self.read(base + 9);
        if result.is_err() {
            self.turn_off_dsms()?;
            log::error!("Failed to read vcocap regstate!");
            return Err(Error::Invalid);
        }
        let mut vcocap_reg_state = result?;

        vcocap_reg_state &= !0x3f;

        let result = self.write_vcocap(base, f.vcocap, vcocap_reg_state);
        if result.is_err() {
            self.turn_off_dsms()?;
            log::error!("Failed to write vcocap_reg_state!");
            return Err(Error::Invalid);
        }

        let low_band = (f.flags & LMS_FREQ_FLAGS_LOW_BAND) != 0;
        let result = self.write_pll_config(module, f.freqsel, low_band);
        if result.is_err() {
            self.turn_off_dsms()?;
            log::error!("Failed to write pll_config!");
            return Err(Error::Invalid);
        }

        let mut freq_data = [0u8; 4];
        freq_data[0] = (f.nint >> 1) as u8;
        freq_data[1] = (((f.nint & 1) << 7) as u32 | ((f.nfrac >> 16) & 0x7f)) as u8;
        freq_data[2] = ((f.nfrac >> 8) & 0xff) as u8;
        freq_data[3] = (f.nfrac & 0xff) as u8;

        for (idx, value) in freq_data.iter().enumerate() {
            let result = self.write(pll_base + idx as u8, *value);
            if result.is_err() {
                self.turn_off_dsms()?;
                log::error!("Failed to write pll {}!", pll_base + idx as u8);
                return Err(Error::Invalid);
            }
        }

        // Perform tuning algorithm unless we've been instructed to just use
        // the VCOCAP hint as-is.
        if (f.flags & LMS_FREQ_FLAGS_FORCE_VCOCAP) != 0 {
            f.vcocap_result = f.vcocap;
        } else {
            // Walk down VCOCAP values find an optimal values
            log::trace!("Tuning VCOCAP...");
            f.vcocap_result = self.tune_vcocap(f.vcocap, base, vcocap_reg_state)?;
        }

        Ok(())
    }

    /// Turn off Delta sigma digital core supply (1.8V)
    pub fn turn_off_dsms(&self) -> Result<()> {
        let mut data = self.read(0x09)?;
        data &= !0x05;
        self.write(0x09, data)
    }

    /// Power up or power down LNAs.
    pub fn enable_lna_power(&self, enable: bool) -> Result<()> {
        // Magic test register to power down LNAs
        let mut regval = self.read(0x7d)?;

        if enable {
            regval &= !(1 << 0);
        } else {
            regval |= 1 << 0;
        }

        self.write(0x7d, regval)?;

        // Decode test registers
        regval = self.read(0x70)?;

        if enable {
            regval &= !(1 << 1);
        } else {
            regval |= 1 << 1;
        }

        self.write(0x70, regval)
    }

    /// Select Power Amplifiers of the LMS
    pub fn select_pa(&self, pa: LmsPa) -> Result<()> {
        let mut data = self.read(0x44)?;

        // Disable PA1, PA2, and AUX PA - we'll enable as requested below.
        data &= !0x1C;

        // AUX PA powered down
        data |= 1 << 1;

        match pa {
            LmsPa::PaAux => {
                // Power up the AUX PA
                data &= !(1 << 1);
            }
            LmsPa::Pa1 => {
                // PA_EN[2:0] = 010 - Enable PA1
                data |= 2 << 2;
            }
            LmsPa::Pa2 => {
                // PA_EN[2:0] = 100 - Enable PA2
                data |= 4 << 2;
            }
            LmsPa::PaNone => {} // _ => {
                                //     log::error!("Invalid PA selection");
                                //     return Err(Error::Invalid);
                                // }
        }

        self.write(0x44, data)
    }

    /// Select which LNA to enable
    pub fn select_lna(&self, lna: LmsLna) -> Result<()> {
        let mut data = self.read(0x75)?;

        data &= !(3 << 4);
        data |= (u8::from(lna) & 3) << 4;

        self.write(0x75, data)
    }

    /// Select which band (Low or High) should be enabled.
    /// Depending on the selected module/channel, the correct LowNoiseAmplifiers or PowerAmplifiers
    /// will be selected.
    pub fn select_band(&self, module: u8, band: Band) -> Result<()> {
        // If loopback mode disabled, avoid changing the PA or LNA selection,
        // as these need to remain powered down or disabled
        if self.is_loopback_enabled()? {
            log::debug!("Loopback enabled!");
            return Ok(());
        }

        if module == BLADERF_MODULE_TX {
            let lms_pa = if band == Band::Low {
                LmsPa::Pa1
            } else {
                LmsPa::Pa2
            };
            self.select_pa(lms_pa)
        } else {
            let lms_lna = if band == Band::Low {
                LmsLna::Lna1
            } else {
                LmsLna::Lna2
            };
            self.select_lna(lms_lna)
        }
    }

    /// Set the frequency on a specific channel. The correct parameters for tuning are calculated
    /// on the fly.
    pub fn set_frequency(&self, channel: u8, frequency: u64) -> Result<()> {
        let mut f = frequency.try_into()?;
        log::trace!("{f:?}");

        self.set_precalculated_frequency(channel, &mut f)
    }

    /// Get teh frequency on a specific channel.
    pub fn get_frequency(&self, module: u8) -> Result<LmsFreq> {
        let mut f = LmsFreq::default();
        let base: u8 = if module == BLADERF_MODULE_RX {
            0x20
        } else {
            0x10
        };

        let mut data = self.read(base)?;
        f.nint = (data as u16) << 1;

        data = self.read(base + 1)?;
        f.nint |= ((data & 0x80) >> 7) as u16;
        f.nfrac = (data as u32 & 0x7f) << 16;

        data = self.read(base + 2)?;
        f.nfrac |= (data as u32) << 8;

        data = self.read(base + 3)?;
        f.nfrac |= data as u32;

        data = self.read(base + 5)?;
        f.freqsel = data >> 2;
        f.x = 1 << ((f.freqsel & 7) - 3);

        data = self.read(base + 9)?;
        f.vcocap = data & 0x3f;

        Ok(f)
    }

    // pub fn frequency_to_hz(lms_freq: &LmsFreq) -> u32 {
    //     let pll_coeff = ((lms_freq.nint as u64) << 23) + lms_freq.nfrac as u64;
    //     let div = (lms_freq.x as u64) << 23;
    //
    //     (((LMS_REFERENCE_HZ as u64 * pll_coeff) + (div >> 1)) / div) as u32
    // }

    /// Set the LNA gain in dB.
    pub fn lna_set_gain(&self, gain: GainDb) -> Result<()> {
        // Set the gain on the LNA
        let mut data = self.read(0x75)?;
        // Clear out previous gain setting
        data &= !(3 << 6);

        let lna_gain_code: LnaGainCode = gain.into();
        let lna_gain_code_u8: u8 = lna_gain_code.into();
        // Update gain value
        data |= (lna_gain_code_u8 & 3) << 6;
        self.write(0x75, data)
    }

    /// Get the LNA gain in dB.
    pub fn lna_get_gain(&self) -> Result<GainDb> {
        let mut data = self.read(0x75)?;
        data >>= 6;
        data &= 3;

        let lna_gain_code: LnaGainCode = data.try_into().map_err(|_| Error::Invalid)?;
        Ok(lna_gain_code.into())
    }

    /// Get the currently selected LNA.
    pub fn get_lna(&self) -> Result<LmsLna> {
        let data = self.read(0x75)?;
        LmsLna::try_from((data >> 4) & 0x3)
    }

    /// Enable the first Variable Gain Amplifier after the antenna.
    pub fn rxvga1_enable(&self, enable: bool) -> Result<()> {
        // Enable bit is in reserved register documented in this thread:
        // https://groups.google.com/forum/#!topic/limemicro-opensource/8iTannzlfzg
        let mut data = self.read(0x7d)?;
        if enable {
            data &= !(1 << 3);
        } else {
            data |= 1 << 3;
        }
        self.write(0x7d, data)
    }

    /// Set the gain of the first Variable Gain Amplifier after the RX antenna.
    pub fn rxvga1_set_gain(&self, gain_db: GainDb) -> Result<()> {
        // Set the RFB_TIA_RXFE mixer gain
        // let gain_db = gain.clamp(BLADERF_RXVGA1_GAIN_MIN, BLADERF_RXVGA1_GAIN_MAX);
        // let code = RXVGA1_LUT_VAL2CODE[gain_db as usize];

        let code: Rxvga1GainCode = gain_db.into();
        self.write(0x76, code.code)
    }

    /// Get the gain of the first Variable Gain Amplifier after the RX antenna.
    pub fn rxvga1_get_gain(&self) -> Result<GainDb> {
        let mut data = self.read(0x76)?;

        data &= 0x7f;
        // https://cdn.sanity.io/files/yv2p7ubm/production/44688b111c3f9bfcfb68c4851d13283f37cdc0e9.pdf#%5B%7B%22num%22%3A99%2C%22gen%22%3A0%7D%2C%7B%22name%22%3A%22XYZ%22%7D%2C68%2C544%2C0%5D
        // The LMS FAQ (Rev 1.0r10, Section 5.20) states that the RXVGA1 codes may be
        // converted to dB via:
        //      `value_db = 20 * log10(127 / (127 - code))`
        //
        // However, an offset of 5 appears to be required, yielding:
        //     `value_db =  5 + 20 * log10(127 / (127 - code))`
        // let code = data.clamp(0, 120) as usize;
        // let gain_db = RXVGA1_LUT_CODE2VAL[code] as i8;
        let rxvga1_gain_code = Rxvga1GainCode {
            code: data.clamp(0, 120),
        };

        Ok(rxvga1_gain_code.into())
    }

    /// Enable the second Variable Gain Amplifier after the RX antenna.
    pub fn rxvga2_enable(&self, enable: bool) -> Result<()> {
        // Enable RXVGA2
        let mut data = self.read(0x64)?;
        if enable {
            data |= 1 << 1;
        } else {
            data &= !(1 << 1);
        }
        self.write(0x64, data)
    }

    /// RXVGA2 gain can only be incremented in 3dB steps.
    /// It is not recommended to use a gain higher than 30.
    /// This is enforced by clamping higher gains to 30 automatically in this method.
    /// For details see section 2.7 RX VGA2 Configuration in Programming and Calibration Guide
    pub fn rxvga2_set_gain(&self, gain_db: GainDb) -> Result<()> {
        // Set the gain on RXVGA2
        let code: Rxvga2GainCode = gain_db.into();
        self.write(0x65, code.code)
    }

    /// Get the gain of the second Variable Gain Amplifier after the RX antenna.
    pub fn rxvga2_get_gain(&self) -> Result<GainDb> {
        let rxvga2_gain_code = Rxvga2GainCode {
            code: self.read(0x65)?,
        };

        Ok(rxvga2_gain_code.into())
    }

    /// Get the gain of the first Variable Gain Amplifier before the TX antenna.
    pub fn txvga1_get_gain(&self) -> Result<GainDb> {
        let txvga1_gain_code = Txvga1GainCode {
            code: self.read(0x41)?,
        };
        Ok(txvga1_gain_code.into())
    }

    /// Get the gain of the second Variable Gain Amplifier before the TX antenna.
    pub fn txvga2_get_gain(&self) -> Result<GainDb> {
        let txvga2_gain_code = Txvga2GainCode {
            code: self.read(0x45)?,
        };
        Ok(txvga2_gain_code.into())
    }

    /// Set the gain of the first Variable Gain Amplifier before the TX antenna.
    pub fn txvga1_set_gain(&self, gain: GainDb) -> Result<()> {
        let txvga1_gain_code: Txvga1GainCode = gain.into();
        // Since 0x41 is only VGA1GAIN, we don't need to RMW
        self.write(0x41, txvga1_gain_code.code)
    }

    /// Set the gain of the second Variable Gain Amplifier before the TX antenna.
    pub fn txvga2_set_gain(&self, gain: GainDb) -> Result<()> {
        // 0x45 is not only VGA2GAIN, thus we have to RMW to not accidentally overwrite ENVD setting
        let mut data = self.read(0x45)?;
        data &= !(0x1f << 3);

        let txvga2_gain_code: Txvga2GainCode = gain.into();
        data |= txvga2_gain_code.code;
        self.write(0x45, data)
    }

    /// Enable peak detection to localize the strongest signals (peak) in the monitored
    /// frequency spectrum.
    pub fn peakdetect_enable(&self, enable: bool) -> Result<()> {
        let mut data = self.read(0x44)?;
        if enable {
            data &= !(1 << 0);
        } else {
            data |= 1;
        }
        self.write(0x44, data)
    }

    /// Get quicktune parameters of the specified channel.
    pub fn get_quick_tune(&self, module: u8) -> Result<QuickTune> {
        let f = &self.get_frequency(module)?;

        let mut quick_tune = QuickTune {
            freqsel: f.freqsel,
            vcocap: f.vcocap,
            nint: f.nint,
            nfrac: f.nfrac,
            flags: 0,
            xb_gpio: 0,
        };

        let val = self.interface.lock().unwrap().nios_expansion_gpio_read()?;

        // TODO: Test if the enablement check really works...
        // if self.xb200.is_some() {
        if BladeRf1::xb200_is_enabled(&self.interface)? {
            quick_tune.xb_gpio |= LMS_FREQ_XB_200_ENABLE;
            if module == bladerf_channel_rx!(0) {
                quick_tune.xb_gpio |= LMS_FREQ_XB_200_MODULE_RX;
                // BLADERF_XB_CONFIG_RX_BYPASS_MASK
                quick_tune.xb_gpio |= (((val & 0x30) >> 4) << LMS_FREQ_XB_200_PATH_SHIFT) as u8;
                // BLADERF_XB_RX_MASK
                quick_tune.xb_gpio |=
                    (((val & 0x30000000) >> 28) << LMS_FREQ_XB_200_FILTER_SW_SHIFT) as u8;
            } else {
                // BLADERF_XB_CONFIG_TX_BYPASS_MASK
                quick_tune.xb_gpio |=
                    (((val & 0x0C) >> 2) << LMS_FREQ_XB_200_FILTER_SW_SHIFT) as u8;
                // BLADERF_XB_TX_MASK
                quick_tune.xb_gpio |=
                    (((val & 0x0C000000) >> 26) << LMS_FREQ_XB_200_PATH_SHIFT) as u8;
            }

            quick_tune.flags = LMS_FREQ_FLAGS_FORCE_VCOCAP;

            let f_hz: u64 = f.into();
            if f_hz < BLADERF1_BAND_HIGH as u64 {
                quick_tune.flags |= LMS_FREQ_FLAGS_LOW_BAND;
            }
        }
        Ok(quick_tune)
    }

    /// Enable or disable LowPassFilter on selected channel.
    pub fn lpf_enable(&self, channel: u8, enable: bool) -> Result<()> {
        let addr = if channel == BLADERF_MODULE_RX {
            0x54
        } else {
            0x34
        };

        let mut data = self.read(addr)?;

        if enable {
            data |= 1 << 1;
        } else {
            data &= !(1 << 1);
        }

        self.write(addr, data)?;

        // Check to see if we are bypassed
        data = self.read(addr + 1)?;
        if data & (1 << 6) != 0 {
            // Bypass is enabled; switch back to normal operation
            data &= !(1 << 6);
            self.write(addr + 1, data)?;
        }

        Ok(())
    }

    /// Set the bandwidth of the LMS6002D Transceiver
    pub fn set_bandwidth(&self, channel: u8, bw: LmsBw) -> Result<()> {
        let addr = if channel == BLADERF_MODULE_RX {
            0x54
        } else {
            0x34
        };

        let mut data = self.read(addr)?;

        // Clear out previous bandwidth setting
        data &= !0x3c;
        // Apply new bandwidth setting
        data |= bw.to_index() << 2;

        self.write(addr, data)
    }

    /// Get the bandwidth of the LMS6002D Transceiver
    pub fn get_bandwidth(&self, channel: u8) -> Result<LmsBw> {
        let addr = if channel == BLADERF_MODULE_RX {
            0x54
        } else {
            0x34
        };

        let mut data = self.read(addr)?;

        // Fetch bandwidth table index from reg[5:2]
        data >>= 2;
        data &= 0xf;

        // Lookup the bandwidth for returned u8 in lookup table
        // and convert u32 bandwidth into Enum
        Ok(LmsBw::from_index(data))
    }

    /// Scale the DC offset on specified channel.
    fn scale_dc_offset(module: u8, mut value: i16) -> Result<u8> {
        match module {
            BLADERF_MODULE_RX => {
                // RX only has 6 bits of scale to work with, remove normalization
                value >>= 5;

                // value = value.clamp(-0x3f, 0x3f);
                // if value < 0 {
                //     value = value.abs();
                //     // This register uses bit 6 to denote a negative value
                //     value |= 1 << 6;
                // }

                if value < 0 {
                    if value <= -64 {
                        // Clamp
                        value = 0x3f;
                    } else {
                        value = (-value) & 0x3f;
                    }

                    // This register uses bit 6 to denote a negative value
                    value |= 1 << 6;
                } else if value >= 64 {
                    // Clamp
                    value = 0x3f;
                } else {
                    value &= 0x3f;
                }

                Ok(value as u8)
            }
            BLADERF_MODULE_TX => {
                // TX only has 7 bits of scale to work with, remove normalization
                value >>= 4;

                // LMS6002D 0x00 = -16, 0x80 = 0, 0xff = 15.9375
                // let clamped = value.clamp(0x00, 0x7f);
                // if value >= 0 {
                //     // Assert bit 7 for positive numbers
                //     Ok((clamped | (1 << 7)) as u8)
                // } else {
                //     Ok(clamped as u8)
                // }

                if value >= 0 {
                    let ret = (if value >= 128 { 0x7f } else { value & 0x7f }) as u8;

                    // Assert bit 7 for positive numbers
                    Ok((1 << 7) | ret)
                } else {
                    Ok((if value <= -128 { 0x00 } else { value & 0x7f }) as u8)
                }
            }
            _ => {
                log::error!("Invalid module selected!");
                Err(Error::Invalid)
            }
        }
    }

    /// Unscale the DC offset on specified channel.
    fn unscale_dc_offset(module: u8, mut regval: u8) -> Result<i16> {
        match module {
            BLADERF_MODULE_RX => {
                // Mask out an unrelated control bit
                regval &= 0x7f;

                // Determine sign
                let value = if regval & (1 << 6) != 0 {
                    -((regval & 0x3f) as i16)
                } else {
                    (regval & 0x3f) as i16
                };

                // Renormalize to 2048
                Ok(value << 5)

                // // Mask out an unrelated control bit
                // regval &= 0x7f;
                // log::error!("{regval:#x} {regval:#b} {regval} [unrelated control bit masked]");
                //
                // let sign_bit = (regval as i16) & (1 << 6);
                // log::error!("{sign_bit:#x} {sign_bit:#b} {sign_bit} [sign_bit mask]");
                //
                // let mut value = if sign_bit != 0 {
                //     !(regval as i16)
                // } else {
                //     regval as i16
                // };
                // log::error!("{value:#x} {value:#b} {value} [bitflip if signed]");
                //
                // value |= sign_bit;
                // log::error!("{value:#x} {value:#b} {value} [transform to signed value if required]");
                //
                // // Renormalize to 2048
                // Ok(value << 5)
            }
            BLADERF_MODULE_TX => {
                // LMS6002D 0x00 = -16, 0x80 = 0, 0xff = 15.9375
                // Allow a range from -128 (-2048) until 127 (2032)
                let value = -(0x80 - regval as i16);
                Ok((value) << 4)
            }
            _ => {
                log::error!("Invalid module selected!");
                Err(Error::Invalid)
            }
        }
    }

    /// Set the DC offset on specified channel and address.
    fn set_dc_offset(&self, module: u8, addr: u8, value: i16) -> Result<()> {
        let regval = match module {
            BLADERF_MODULE_RX => {
                let mut tmp = self.read(addr)?;
                // Bit 7 is unrelated to lms dc correction, save its state
                tmp &= 1 << 7;
                Self::scale_dc_offset(module, value)? | tmp
            }
            BLADERF_MODULE_TX => Self::scale_dc_offset(module, value)?,
            _ => {
                log::error!("Invalid module selected!");
                return Err(Error::Invalid);
            }
        };

        // log::error!("[set_dc_offset] addr: {addr:#x}, regval: {regval:#x}");
        self.write(addr, regval)
    }

    /// Set the DC offset on specified channel for the imaginary part.
    pub fn set_dc_offset_i(&self, module: u8, value: i16) -> Result<()> {
        let addr = if module == BLADERF_MODULE_TX {
            0x42
        } else {
            0x71
        };
        self.set_dc_offset(module, addr, value)
    }

    /// Set the DC offset on specified channel for the real part.
    pub fn set_dc_offset_q(&self, module: u8, value: i16) -> Result<()> {
        let addr = if module == BLADERF_MODULE_TX {
            0x43
        } else {
            0x72
        };
        self.set_dc_offset(module, addr, value)
    }

    /// Get the DC offset on specified channel on the supplied address.
    fn get_dc_offset(&self, module: u8, addr: u8) -> Result<i16> {
        let regval = self.read(addr)?;
        // log::error!("[get_dc_offset] addr: {addr:#x}, regval: {regval:#x}");

        Self::unscale_dc_offset(module, regval)
    }

    /// Get the DC offset on specified channel for the imaginary part.
    pub fn get_dc_offset_i(&self, module: u8) -> Result<i16> {
        let addr = if module == BLADERF_MODULE_TX {
            0x42
        } else {
            0x71
        };
        self.get_dc_offset(module, addr)
    }

    /// Get the DC offset on specified channel for the real part.
    pub fn get_dc_offset_q(&self, module: u8) -> Result<i16> {
        let addr = if module == BLADERF_MODULE_TX {
            0x43
        } else {
            0x72
        };
        self.get_dc_offset(module, addr)
    }
}
