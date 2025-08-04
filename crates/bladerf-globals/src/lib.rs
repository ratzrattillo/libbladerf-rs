// TODO: find a place where to put generic checks for e.g. type sizes
// assert_eq!(size_of::<u64>(), 8);

pub mod bladerf1;
mod bladerf2;
pub mod range;

use std::time::Duration;

#[macro_export]
macro_rules! bladerf_channel_rx {
    ($ch:expr) => {
        ((($ch) << 1) | 0x0) as u8
    };
}
#[macro_export]
macro_rules! bladerf_channel_tx {
    ($ch:expr) => {
        ((($ch) << 1) | 0x1) as u8
    };
}

#[macro_export]
macro_rules! khz {
    ($value:expr) => {
        ($value * 1000u32)
    };
}
#[macro_export]
macro_rules! mhz {
    ($value:expr) => {
        ($value * 1000000u32)
    };
}
#[macro_export]
macro_rules! ghz {
    ($value:expr) => {
        ($value * 1000000000u32)
    };
}

#[derive(Clone)]
pub enum TuningMode {
    Host,
    Fpga,
}

///  Stream direction
#[derive(PartialEq, Debug, Clone)]
#[repr(u8)]
pub enum BladeRf1Direction {
    Rx = 0, // Receive direction
    Tx = 1, // Transmit direction
}

///  Convenience macro: true if argument is a TX channel
#[macro_export]
macro_rules! bladerf_channel_is_tx {
    ($ch:expr) => {
        (($ch) & BladeRf1Direction::Tx as u8) != 0
    };
}

///  Mapping of human-readable names to loopback modes
pub struct BladeRf1LoopbackModes {
    /// Name of loopback mode
    _name: String,
    /// Loopback mode enumeration
    _mode: BladeRf1Loopback,
}

///  Sample format
#[derive(PartialEq)]
pub enum BladeRf1Format {
    ///  Signed, Complex 16-bit Q11. This is the native format of the DAC data.
    ///
    ///  Values in the range [-2048, 2048) are used to represent [-1.0, 1.0).
    ///  Note that the lower bound here is inclusive, and the upper bound is
    ///  exclusive. Ensure that provided samples stay within [-2048, 2047].
    ///
    ///  Samples consist of interleaved IQ value pairs, with I being the first
    ///  value in the pair. Each value in the pair is a right-aligned,
    ///  little-endian int16_t. The FPGA ensures that these values are
    ///  sign-extended.
    ///
    ///  <pre>
    ///   .--------------.--------------.
    ///   | Bits 31...16 | Bits 15...0  |
    ///   +--------------+--------------+
    ///   |   Q[15..0]   |   I[15..0]   |
    ///   `--------------`--------------`
    ///  </pre>
    ///
    ///  When using this format the minimum required buffer size, in bytes, is:
    ///
    ///  \f$
    ///   buffer\_size\_min = (2 \times num\_samples \times num\_channels \times
    ///                       sizeof(int16\_t))
    ///  \f$
    ///
    ///  For example, to hold 2048 samples for one channel, a buffer must be at
    ///  least 8192 bytes large.
    ///
    ///  When a multi-channel ::bladerf_channel_layout is selected, samples
    ///  will be interleaved per channel. For example, with ::BLADERF_RX_X2
    ///  or ::BLADERF_TX_X2 (x2 MIMO), the buffer is structured like:
    ///
    ///  <pre>
    ///   .-------------.--------------.--------------.------------------.
    ///   | Byte offset | Bits 31...16 | Bits 15...0  |    Description   |
    ///   +-------------+--------------+--------------+------------------+
    ///   |    0x00     |     Q0[0]    |     I0[0]    |  Ch 0, sample 0  |
    ///   |    0x04     |     Q1[0]    |     I1[0]    |  Ch 1, sample 0  |
    ///   |    0x08     |     Q0[1]    |     I0[1]    |  Ch 0, sample 1  |
    ///   |    0x0c     |     Q1[1]    |     I1[1]    |  Ch 1, sample 1  |
    ///   |    ...      |      ...     |      ...     |        ...       |
    ///   |    0xxx     |     Q0[n]    |     I0[n]    |  Ch 0, sample n  |
    ///   |    0xxx     |     Q1[n]    |     I1[n]    |  Ch 1, sample n  |
    ///   `-------------`--------------`--------------`------------------`
    ///  </pre>
    ///
    ///  Per the `buffer_size_min` formula above, 2048 samples for two channels
    ///  will generate 4096 total samples, and require at least 16384 bytes.
    ///
    ///  Implementors may use the interleaved buffers directly, or may use
    ///  bladerf_deinterleave_stream_buffer() / bladerf_interleave_stream_buffer()
    ///  if contiguous blocks of samples are desired.
    Sc16Q11 = 0,

    ///  This format is the same as the ::BLADERF_FORMAT_SC16_Q11 format, except
    ///  the first 4 samples in every <i>block*</i> of samples are replaced with
    ///  metadata organized as follows. All fields are little-endian byte order.
    ///
    ///  <pre>
    ///   .-------------.------------.----------------------------------.
    ///   | Byte offset |   Type     | Description                      |
    ///   +-------------+------------+----------------------------------+
    ///   |    0x00     | uint16_t   | Reserved                         |
    ///   |    0x02     |  uint8_t   | Stream flags                     |
    ///   |    0x03     |  uint8_t   | Meta version ID                  |
    ///   |    0x04     | uint64_t   | 64-bit Timestamp                 |
    ///   |    0x0c     | uint32_t   | BLADERF_META_FLAG_* flags        |
    ///   |  0x10..end  |            | Payload                          |
    ///   `-------------`------------`----------------------------------`
    ///  </pre>
    ///
    ///  For IQ sample meta mode, the Meta version ID and Stream flags should
    ///  currently be set to values 0x00 and 0x00, respectively.
    ///
    ///  <i>*</i>The number of samples in a <i>block</i> is dependent upon
    ///  the USB speed being used:
    ///   - USB 2.0 Hi-Speed: 256 samples
    ///   - USB 3.0 SuperSpeed: 512 samples
    ///
    ///  When using the bladerf_sync_rx() and bladerf_sync_tx() functions, the
    ///  above details are entirely transparent; the caller need not be concerned
    ///  with these details. These functions take care of packing/unpacking the
    ///  metadata into/from the underlying stream and convey this information
    ///  through the ::bladerf_metadata structure.
    ///
    ///  However, when using the \ref FN_STREAMING_ASYNC interface, the user is
    ///  responsible for manually packing/unpacking the above metadata into/from
    ///  their samples.
    ///
    ///  @see STREAMING_FORMAT_METADATA
    ///  @see The `src/streaming/metadata.h` header in the libbladeRF codebase.
    Sc16Q11Meta = 1,

    ///  This format is for exchanging packets containing digital payloads with
    ///  the FPGA. A packet is generall a digital payload, that the FPGA then
    ///  processes to either modulate, demodulate, filter, etc.
    ///
    ///  All fields are little-endian byte order.
    ///
    ///  <pre>
    ///   .-------------.------------.----------------------------------.
    ///   | Byte offset |   Type     | Description                      |
    ///   +-------------+------------+----------------------------------+
    ///   |    0x00     | uint16_t   | Packet length (in 32bit DWORDs)  |
    ///   |    0x02     |  uint8_t   | Packet flags                     |
    ///   |    0x03     |  uint8_t   | Packet core ID                   |
    ///   |    0x04     | uint64_t   | 64-bit Timestamp                 |
    ///   |    0x0c     | uint32_t   | BLADERF_META_FLAG_* flags        |
    ///   |  0x10..end  |            | Payload                          |
    ///   `-------------`------------`----------------------------------`
    ///  </pre>
    ///
    ///  A target core (for example a modem) must be specified when calling the
    ///  bladerf_sync_rx() and bladerf_sync_tx() functions.
    ///
    ///  When in packet mode, lengths for all functions and data formats are
    ///  expressed in number of 32-bit DWORDs. As an example, a 12 byte packet
    ///  is considered to be 3 32-bit DWORDs long.
    ///
    ///  This packet format does not send or receive raw IQ samples. The digital
    ///  payloads contain configurations, and digital payloads that are specific
    ///  to the digital core to which they are addressed. It is the FPGA core
    ///  that should generate, interpret, and process the digital payloads.
    ///
    ///  With the exception of packet lenghts, no difference should exist between
    ///  USB 2.0 Hi-Speed or USB 3.0 SuperSpeed for packets for this streaming
    ///  format.
    ///
    ///  @see STREAMING_FORMAT_METADATA
    ///  @see The `src/streaming/metadata.h` header in the libbladeRF codebase.
    PacketMeta = 2,

    ///  Signed, Complex 8-bit Q8. This is the native format of the DAC data.
    ///
    ///  Values in the range [-128, 128) are used to represent [-1.0, 1.0).
    ///  Note that the lower bound here is inclusive, and the upper bound is
    ///  exclusive. Ensure that provided samples stay within [-128, 127].
    ///
    ///  Samples consist of interleaved IQ value pairs, with I being the first
    ///  value in the pair. Each value in the pair is a right-aligned int8_t.
    ///  The FPGA ensures that these values are sign-extended.
    ///
    ///  <pre>
    ///   .--------------.--------------.
    ///   | Bits 15...8  | Bits  7...0  |
    ///   +--------------+--------------+
    ///   |    Q[7..0]   |    I[7..0]   |
    ///   `--------------`--------------`
    ///  </pre>
    ///
    ///  When using this format the minimum required buffer size, in bytes, is:
    ///
    ///  \f$
    ///   buffer\_size\_min = (2 \times num\_samples \times num\_channels \times
    ///                       sizeof(int8\_t))
    ///  \f$
    ///
    ///  For example, to hold 2048 samples for one channel, a buffer must be at
    ///  least 4096 bytes large.
    ///
    ///  When a multi-channel ::bladerf_channel_layout is selected, samples
    ///  will be interleaved per channel. For example, with ::BLADERF_RX_X2
    ///  or ::BLADERF_TX_X2 (x2 MIMO), the buffer is structured like:
    ///
    ///  <pre>
    ///   .-------------.--------------.--------------.------------------.
    ///   | Byte offset | Bits 15...8  | Bits  7...0  |    Description   |
    ///   +-------------+--------------+--------------+------------------+
    ///   |    0x00     |     Q0[0]    |     I0[0]    |  Ch 0, sample 0  |
    ///   |    0x02     |     Q1[0]    |     I1[0]    |  Ch 1, sample 0  |
    ///   |    0x04     |     Q0[1]    |     I0[1]    |  Ch 0, sample 1  |
    ///   |    0x06     |     Q1[1]    |     I1[1]    |  Ch 1, sample 1  |
    ///   |    ...      |      ...     |      ...     |        ...       |
    ///   |    0xxx     |     Q0[n]    |     I0[n]    |  Ch 0, sample n  |
    ///   |    0xxx     |     Q1[n]    |     I1[n]    |  Ch 1, sample n  |
    ///   `-------------`--------------`--------------`------------------`
    ///  </pre>
    ///
    ///  Per the `buffer_size_min` formula above, 2048 samples for two channels
    ///  will generate 4096 total samples, and require at least 8192 bytes.
    ///
    ///  Implementors may use the interleaved buffers directly, or may use
    ///  bladerf_deinterleave_stream_buffer() / bladerf_interleave_stream_buffer()
    ///  if contiguous blocks of samples are desired.
    Sc8Q7 = 3,

    ///  This format is the same as the ::BLADERF_FORMAT_SC8_Q7 format, except
    ///  the first 4 samples in every <i>block*</i> of samples are replaced with
    ///  metadata organized as follows. All fields are little-endian byte order.
    ///
    ///  <pre>
    ///   .-------------.------------.----------------------------------.
    ///   | Byte offset |   Type     | Description                      |
    ///   +-------------+------------+----------------------------------+
    ///   |    0x00     | uint16_t   | Reserved                         |
    ///   |    0x02     |  uint8_t   | Stream flags                     |
    ///   |    0x03     |  uint8_t   | Meta version ID                  |
    ///   |    0x04     | uint64_t   | 64-bit Timestamp                 |
    ///   |    0x0c     | uint32_t   | BLADERF_META_FLAG_* flags        |
    ///   |  0x10..end  |            | Payload                          |
    ///   `-------------`------------`----------------------------------`
    ///  </pre>
    ///
    ///  For IQ sample meta mode, the Meta version ID and Stream flags should
    ///  currently be set to values 0x00 and 0x00, respectively.
    ///
    ///  <i>*</i>The number of samples in a <i>block</i> is dependent upon
    ///  the USB speed being used:
    ///   - USB 2.0 Hi-Speed: 256 samples
    ///   - USB 3.0 SuperSpeed: 512 samples
    ///
    ///  When using the bladerf_sync_rx() and bladerf_sync_tx() functions, the
    ///  above details are entirely transparent; the caller need not be concerned
    ///  with these details. These functions take care of packing/unpacking the
    ///  metadata into/from the underlying stream and convey this information
    ///  through the ::bladerf_metadata structure.
    ///
    ///  However, when using the \ref FN_STREAMING_ASYNC interface, the user is
    ///  responsible for manually packing/unpacking the above metadata into/from
    ///  their samples.
    ///
    ///  @see STREAMING_FORMAT_METADATA
    ///  @see The `src/streaming/metadata.h` header in the libbladeRF codebase.
    Sc8Q7Meta = 4,
}

/// Loopback options
#[derive(PartialEq, Debug, Clone)]
#[repr(u8)]
pub enum BladeRf1Loopback {
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

///  Low-Pass Filter (LPF) mode
#[derive(PartialEq)]
pub enum BladeRf1LpfMode {
    /// LPF connected and enabled
    Normal,
    /// LPF bypassed
    Bypassed,
    /// LPF disabled
    Disabled,
}

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

    fn try_from(value: u8) -> Result<Self, Self::Error> {
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
pub enum BladeRf1GainMode {
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

#[derive(Clone, Default)]
pub struct BladeRf1RationalRate {
    /// Integer portion
    pub integer: u64,
    /// Numerator in fractional portion
    pub num: u64,
    /// Denominator in fractional portion. This must be greater than 0.
    pub den: u64,
}

#[repr(u8)]
pub enum StringDescriptors {
    /// Don't want to start with 0 as 0 is reserved for the language table
    Manufacturer = 0x1,
    Product,
    Serial,
    Fx3Firmware,
}

#[repr(u8)]
pub enum DescriptorTypes {
    /// Don't want to start with 0 as 0 is reserved for the language table
    Device = 0x01,
    Configuration = 0x2,
    String = 0x03,
    Default = 0x06,
    BOS = 0x0f,
}

#[allow(dead_code)]
pub const BLADERF_MODULE_RX: u8 = bladerf_channel_rx!(0);
#[allow(dead_code)]
pub const BLADERF_MODULE_TX: u8 = bladerf_channel_tx!(0);

pub const ENDPOINT_OUT: u8 = 0x02;
pub const ENDPOINT_IN: u8 = 0x82;

pub const TIMEOUT: Duration = Duration::from_millis(1);

pub const BLADE_USB_CMD_QUERY_VERSION: u8 = 0;
pub const BLADE_USB_CMD_QUERY_FPGA_STATUS: u8 = 1;
pub const BLADE_USB_CMD_BEGIN_PROG: u8 = 2;
pub const BLADE_USB_CMD_END_PROG: u8 = 3;
pub const BLADE_USB_CMD_RF_RX: u8 = 4;
pub const BLADE_USB_CMD_RF_TX: u8 = 5;
pub const BLADE_USB_CMD_QUERY_DEVICE_READY: u8 = 6;
pub const BLADE_USB_CMD_QUERY_FLASH_ID: u8 = 7;
pub const BLADE_USB_CMD_QUERY_FPGA_SOURCE: u8 = 8;
pub const BLADE_USB_CMD_FLASH_READ: u8 = 100;
pub const BLADE_USB_CMD_FLASH_WRITE: u8 = 101;
pub const BLADE_USB_CMD_FLASH_ERASE: u8 = 102;
pub const BLADE_USB_CMD_READ_OTP: u8 = 103;
pub const BLADE_USB_CMD_WRITE_OTP: u8 = 104;
pub const BLADE_USB_CMD_RESET: u8 = 105;
pub const BLADE_USB_CMD_JUMP_TO_BOOTLOADER: u8 = 106;
pub const BLADE_USB_CMD_READ_PAGE_BUFFER: u8 = 107;
pub const BLADE_USB_CMD_WRITE_PAGE_BUFFER: u8 = 108;
pub const BLADE_USB_CMD_LOCK_OTP: u8 = 109;
pub const BLADE_USB_CMD_READ_CAL_CACHE: u8 = 110;
pub const BLADE_USB_CMD_INVALIDATE_CAL_CACHE: u8 = 111;
pub const BLADE_USB_CMD_REFRESH_CAL_CACHE: u8 = 112;
pub const BLADE_USB_CMD_SET_LOOPBACK: u8 = 113;
pub const BLADE_USB_CMD_GET_LOOPBACK: u8 = 114;
pub const BLADE_USB_CMD_READ_LOG_ENTRY: u8 = 115;

/// String descriptor indices
/// Manufacturer
pub const BLADE_USB_STR_INDEX_MFR: u8 = 1;
/// Product
pub const BLADE_USB_STR_INDEX_PRODUCT: u8 = 2;
/// Serial number
pub const BLADE_USB_STR_INDEX_SERIAL: u8 = 3;
/// Firmware version
pub const BLADE_USB_STR_INDEX_FW_VER: u8 = 4;

pub const CAL_BUFFER_SIZE: u16 = 256;
pub const CAL_PAGE: u16 = 768;

pub const AUTOLOAD_BUFFER_SIZE: u16 = 256;
pub const AUTOLOAD_PAGE: u16 = 1024;

// #ifdef _MSC_VER
// #   define PACK(decl_to_pack_) \
// __pragma(pack(push,1)) \
// decl_to_pack_ \
// __pragma(pack(pop))
// #elif defined(__GNUC__)
// #   define PACK(decl_to_pack_) \
// decl_to_pack_ __attribute__((__packed__))
// #else
// #error "Unexpected compiler/environment"
// #endif
//
// PACK(
// struct bladerf_fx3_version {
//     unsigned short major;
//     unsigned short minor;
// });
//
// struct bladeRF_firmware {
//     unsigned int len;
//     unsigned char/// ptr;
// };
//
// struct bladeRF_sector {
//     unsigned int idx;
//     unsigned int len;
//     unsigned char/// ptr;
// };
//
// ///
// ///  FPGA configuration source
// ///
// ///  Note: the numbering of this enum must match bladerf_fpga_source in
// ///  libbladeRF.h
// /// /
// typedef enum {
//     NUAND_FPGA_CONFIG_SOURCE_INVALID = 0, /// < Uninitialized/invalid/// /
//     NUAND_FPGA_CONFIG_SOURCE_FLASH   = 1, /// < Last FPGA load was from flash/// /
//     NUAND_FPGA_CONFIG_SOURCE_HOST    = 2  /// < Last FPGA load was from host/// /
// } NuandFpgaConfigSource;
//
// #define USB_CYPRESS_VENDOR_ID   0x04b4
// #define USB_FX3_PRODUCT_ID      0x00f3
//
// #define BLADE_USB_TYPE_OUT      0x40
// #define BLADE_USB_TYPE_IN       0xC0
// #define BLADE_USB_TIMEOUT_MS    1000
//
// #define USB_NUAND_VENDOR_ID                         0x2cf0
// #define USB_NUAND_BLADERF_PRODUCT_ID                0x5246
// #define USB_NUAND_BLADERF_BOOT_PRODUCT_ID           0x5247
// #define USB_NUAND_BLADERF2_PRODUCT_ID               0x5250
//
// #define USB_NUAND_LEGACY_VENDOR_ID                  0x1d50
// #define USB_NUAND_BLADERF_LEGACY_PRODUCT_ID         0x6066
// #define USB_NUAND_BLADERF_LEGACY_BOOT_PRODUCT_ID    0x6080
//
// #define USB_NUAND_BLADERF_MINOR_BASE 193
// #define NUM_CONCURRENT  8
// #define NUM_DATA_URB    (1024)
// #define DATA_BUF_SZ     (1024*4)

/// Interface numbers
pub const USB_IF_LEGACY_CONFIG: u8 = 0;
pub const USB_IF_NULL: u8 = 0;
pub const USB_IF_RF_LINK: u8 = 1;
pub const USB_IF_SPI_FLASH: u8 = 2;
pub const USB_IF_CONFIG: u8 = 3;
