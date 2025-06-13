// TODO: find a place where to put generic checks for e.g. type sizes
// assert_eq!(size_of::<u64>(), 8);

pub mod bladerf1;

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

pub struct SdrRange {
    pub min: i8,
    pub max: i8,
    pub step: u8,
    pub scale: u8,
}

/**
 * Stream direction
 */
#[derive(PartialEq)]
#[repr(u8)]
pub enum BladeRfDirection {
    Rx = 0, // Receive direction
    Tx = 1, // Transmit direction
}

/**
 * Convenience macro: true if argument is a TX channel
 */
#[macro_export]
macro_rules! bladerf_channel_is_tx {
    ($ch:expr) => {
        (($ch) & BladeRfDirection::Tx as u8) != 0
    };
}

/**
 * Mapping of human-readable names to loopback modes
 */
pub struct BladerfLoopbackModes {
    /**< Name of loopback mode */
    _name: String,
    /**< Loopback mode enumeration */
    _mode: BladerfLoopback,
}

/**
 * Sample format
 */
#[derive(PartialEq)]
pub enum BladerfFormat {
    /**
     * Signed, Complex 16-bit Q11. This is the native format of the DAC data.
     *
     * Values in the range [-2048, 2048) are used to represent [-1.0, 1.0).
     * Note that the lower bound here is inclusive, and the upper bound is
     * exclusive. Ensure that provided samples stay within [-2048, 2047].
     *
     * Samples consist of interleaved IQ value pairs, with I being the first
     * value in the pair. Each value in the pair is a right-aligned,
     * little-endian int16_t. The FPGA ensures that these values are
     * sign-extended.
     *
     * <pre>
     *  .--------------.--------------.
     *  | Bits 31...16 | Bits 15...0  |
     *  +--------------+--------------+
     *  |   Q[15..0]   |   I[15..0]   |
     *  `--------------`--------------`
     * </pre>
     *
     * When using this format the minimum required buffer size, in bytes, is:
     *
     * \f$
     *  buffer\_size\_min = (2 \times num\_samples \times num\_channels \times
     *                      sizeof(int16\_t))
     * \f$
     *
     * For example, to hold 2048 samples for one channel, a buffer must be at
     * least 8192 bytes large.
     *
     * When a multi-channel ::bladerf_channel_layout is selected, samples
     * will be interleaved per channel. For example, with ::BLADERF_RX_X2
     * or ::BLADERF_TX_X2 (x2 MIMO), the buffer is structured like:
     *
     * <pre>
     *  .-------------.--------------.--------------.------------------.
     *  | Byte offset | Bits 31...16 | Bits 15...0  |    Description   |
     *  +-------------+--------------+--------------+------------------+
     *  |    0x00     |     Q0[0]    |     I0[0]    |  Ch 0, sample 0  |
     *  |    0x04     |     Q1[0]    |     I1[0]    |  Ch 1, sample 0  |
     *  |    0x08     |     Q0[1]    |     I0[1]    |  Ch 0, sample 1  |
     *  |    0x0c     |     Q1[1]    |     I1[1]    |  Ch 1, sample 1  |
     *  |    ...      |      ...     |      ...     |        ...       |
     *  |    0xxx     |     Q0[n]    |     I0[n]    |  Ch 0, sample n  |
     *  |    0xxx     |     Q1[n]    |     I1[n]    |  Ch 1, sample n  |
     *  `-------------`--------------`--------------`------------------`
     * </pre>
     *
     * Per the `buffer_size_min` formula above, 2048 samples for two channels
     * will generate 4096 total samples, and require at least 16384 bytes.
     *
     * Implementors may use the interleaved buffers directly, or may use
     * bladerf_deinterleave_stream_buffer() / bladerf_interleave_stream_buffer()
     * if contiguous blocks of samples are desired.
     */
    Sc16Q11 = 0,

    /**
     * This format is the same as the ::BLADERF_FORMAT_SC16_Q11 format, except
     * the first 4 samples in every <i>block*</i> of samples are replaced with
     * metadata organized as follows. All fields are little-endian byte order.
     *
     * <pre>
     *  .-------------.------------.----------------------------------.
     *  | Byte offset |   Type     | Description                      |
     *  +-------------+------------+----------------------------------+
     *  |    0x00     | uint16_t   | Reserved                         |
     *  |    0x02     |  uint8_t   | Stream flags                     |
     *  |    0x03     |  uint8_t   | Meta version ID                  |
     *  |    0x04     | uint64_t   | 64-bit Timestamp                 |
     *  |    0x0c     | uint32_t   | BLADERF_META_FLAG_* flags        |
     *  |  0x10..end  |            | Payload                          |
     *  `-------------`------------`----------------------------------`
     * </pre>
     *
     * For IQ sample meta mode, the Meta version ID and Stream flags should
     * currently be set to values 0x00 and 0x00, respectively.
     *
     * <i>*</i>The number of samples in a <i>block</i> is dependent upon
     * the USB speed being used:
     *  - USB 2.0 Hi-Speed: 256 samples
     *  - USB 3.0 SuperSpeed: 512 samples
     *
     * When using the bladerf_sync_rx() and bladerf_sync_tx() functions, the
     * above details are entirely transparent; the caller need not be concerned
     * with these details. These functions take care of packing/unpacking the
     * metadata into/from the underlying stream and convey this information
     * through the ::bladerf_metadata structure.
     *
     * However, when using the \ref FN_STREAMING_ASYNC interface, the user is
     * responsible for manually packing/unpacking the above metadata into/from
     * their samples.
     *
     * @see STREAMING_FORMAT_METADATA
     * @see The `src/streaming/metadata.h` header in the libbladeRF codebase.
     */
    Sc16Q11Meta = 1,

    /**
     * This format is for exchanging packets containing digital payloads with
     * the FPGA. A packet is generall a digital payload, that the FPGA then
     * processes to either modulate, demodulate, filter, etc.
     *
     * All fields are little-endian byte order.
     *
     * <pre>
     *  .-------------.------------.----------------------------------.
     *  | Byte offset |   Type     | Description                      |
     *  +-------------+------------+----------------------------------+
     *  |    0x00     | uint16_t   | Packet length (in 32bit DWORDs)  |
     *  |    0x02     |  uint8_t   | Packet flags                     |
     *  |    0x03     |  uint8_t   | Packet core ID                   |
     *  |    0x04     | uint64_t   | 64-bit Timestamp                 |
     *  |    0x0c     | uint32_t   | BLADERF_META_FLAG_* flags        |
     *  |  0x10..end  |            | Payload                          |
     *  `-------------`------------`----------------------------------`
     * </pre>
     *
     * A target core (for example a modem) must be specified when calling the
     * bladerf_sync_rx() and bladerf_sync_tx() functions.
     *
     * When in packet mode, lengths for all functions and data formats are
     * expressed in number of 32-bit DWORDs. As an example, a 12 byte packet
     * is considered to be 3 32-bit DWORDs long.
     *
     * This packet format does not send or receive raw IQ samples. The digital
     * payloads contain configurations, and digital payloads that are specific
     * to the digital core to which they are addressed. It is the FPGA core
     * that should generate, interpret, and process the digital payloads.
     *
     * With the exception of packet lenghts, no difference should exist between
     * USB 2.0 Hi-Speed or USB 3.0 SuperSpeed for packets for this streaming
     * format.
     *
     * @see STREAMING_FORMAT_METADATA
     * @see The `src/streaming/metadata.h` header in the libbladeRF codebase.
     */
    PacketMeta = 2,

    /**
     * Signed, Complex 8-bit Q8. This is the native format of the DAC data.
     *
     * Values in the range [-128, 128) are used to represent [-1.0, 1.0).
     * Note that the lower bound here is inclusive, and the upper bound is
     * exclusive. Ensure that provided samples stay within [-128, 127].
     *
     * Samples consist of interleaved IQ value pairs, with I being the first
     * value in the pair. Each value in the pair is a right-aligned int8_t.
     * The FPGA ensures that these values are sign-extended.
     *
     * <pre>
     *  .--------------.--------------.
     *  | Bits 15...8  | Bits  7...0  |
     *  +--------------+--------------+
     *  |    Q[7..0]   |    I[7..0]   |
     *  `--------------`--------------`
     * </pre>
     *
     * When using this format the minimum required buffer size, in bytes, is:
     *
     * \f$
     *  buffer\_size\_min = (2 \times num\_samples \times num\_channels \times
     *                      sizeof(int8\_t))
     * \f$
     *
     * For example, to hold 2048 samples for one channel, a buffer must be at
     * least 4096 bytes large.
     *
     * When a multi-channel ::bladerf_channel_layout is selected, samples
     * will be interleaved per channel. For example, with ::BLADERF_RX_X2
     * or ::BLADERF_TX_X2 (x2 MIMO), the buffer is structured like:
     *
     * <pre>
     *  .-------------.--------------.--------------.------------------.
     *  | Byte offset | Bits 15...8  | Bits  7...0  |    Description   |
     *  +-------------+--------------+--------------+------------------+
     *  |    0x00     |     Q0[0]    |     I0[0]    |  Ch 0, sample 0  |
     *  |    0x02     |     Q1[0]    |     I1[0]    |  Ch 1, sample 0  |
     *  |    0x04     |     Q0[1]    |     I0[1]    |  Ch 0, sample 1  |
     *  |    0x06     |     Q1[1]    |     I1[1]    |  Ch 1, sample 1  |
     *  |    ...      |      ...     |      ...     |        ...       |
     *  |    0xxx     |     Q0[n]    |     I0[n]    |  Ch 0, sample n  |
     *  |    0xxx     |     Q1[n]    |     I1[n]    |  Ch 1, sample n  |
     *  `-------------`--------------`--------------`------------------`
     * </pre>
     *
     * Per the `buffer_size_min` formula above, 2048 samples for two channels
     * will generate 4096 total samples, and require at least 8192 bytes.
     *
     * Implementors may use the interleaved buffers directly, or may use
     * bladerf_deinterleave_stream_buffer() / bladerf_interleave_stream_buffer()
     * if contiguous blocks of samples are desired.
     */
    Sc8Q7 = 3,

    /**
     * This format is the same as the ::BLADERF_FORMAT_SC8_Q7 format, except
     * the first 4 samples in every <i>block*</i> of samples are replaced with
     * metadata organized as follows. All fields are little-endian byte order.
     *
     * <pre>
     *  .-------------.------------.----------------------------------.
     *  | Byte offset |   Type     | Description                      |
     *  +-------------+------------+----------------------------------+
     *  |    0x00     | uint16_t   | Reserved                         |
     *  |    0x02     |  uint8_t   | Stream flags                     |
     *  |    0x03     |  uint8_t   | Meta version ID                  |
     *  |    0x04     | uint64_t   | 64-bit Timestamp                 |
     *  |    0x0c     | uint32_t   | BLADERF_META_FLAG_* flags        |
     *  |  0x10..end  |            | Payload                          |
     *  `-------------`------------`----------------------------------`
     * </pre>
     *
     * For IQ sample meta mode, the Meta version ID and Stream flags should
     * currently be set to values 0x00 and 0x00, respectively.
     *
     * <i>*</i>The number of samples in a <i>block</i> is dependent upon
     * the USB speed being used:
     *  - USB 2.0 Hi-Speed: 256 samples
     *  - USB 3.0 SuperSpeed: 512 samples
     *
     * When using the bladerf_sync_rx() and bladerf_sync_tx() functions, the
     * above details are entirely transparent; the caller need not be concerned
     * with these details. These functions take care of packing/unpacking the
     * metadata into/from the underlying stream and convey this information
     * through the ::bladerf_metadata structure.
     *
     * However, when using the \ref FN_STREAMING_ASYNC interface, the user is
     * responsible for manually packing/unpacking the above metadata into/from
     * their samples.
     *
     * @see STREAMING_FORMAT_METADATA
     * @see The `src/streaming/metadata.h` header in the libbladeRF codebase.
     */
    Sc8Q7Meta = 4,
}

/**
* Loopback options
*/
#[derive(PartialEq)]
pub enum BladerfLoopback {
    /** Disables loopback and returns to normal operation. */
    LbNone = 0,

    /** Firmware loopback inside of the FX3 */
    LbFirmware,

    /** Baseband loopback. TXLPF output is connected to the RXVGA2 input. */
    LbBbTxlpfRxvga2,

    /** Baseband loopback. TXVGA1 output is connected to the RXVGA2 input. */
    LbBbTxvga1Rxvga2,

    /** Baseband loopback. TXLPF output is connected to the RXLPF input. */
    LbBbTxlpfRxlpf,

    /** Baseband loopback. TXVGA1 output is connected to RXLPF input. */
    LbBbTxvga1Rxlpf,

    /**
     * RF loopback. The TXMIX output, through the AUX PA, is connected to the
     * output of LNA1.
     */
    LbLna1,

    /**
     * RF loopback. The TXMIX output, through the AUX PA, is connected to the
     * output of LNA2.
     */
    LbLna2,

    /**
     * RF loopback. The TXMIX output, through the AUX PA, is connected to the
     * output of LNA3.
     */
    LbLna3,

    /** RFIC digital loopback (built-in self-test) */
    LbRficBist,
}

/**
 * Gain control modes
 *
 * In general, the default mode is automatic gain control. This will
 * continuously adjust the gain to maximize dynamic range and minimize clipping.
 *
 * @note Implementers are encouraged to simply present a boolean choice between
 *       "AGC On" (::BladerfGainDefault) and "AGC Off" (::BladerfGainMgc).
 *       The remaining choices are for advanced use cases.
 */
#[derive(PartialEq)]
pub enum BladerfGainMode {
    /** Device-specific default (automatic, when available)
     *
     * On the bladeRF x40 and x115 with FPGA versions >= v0.7.0, this is
     * automatic gain control.
     *
     * On the bladeRF 2.0 Micro, this is BladerfGainSlowattackAgc with
     * reasonable default settings.
     */
    Default,

    /** Manual gain control
     *
     * Available on all bladeRF models.
     */
    Mgc,

    /** Automatic gain control, fast attack (advanced)
     *
     * Only available on the bladeRF 2.0 Micro. This is an advanced option, and
     * typically requires additional configuration for ideal performance.
     */
    FastattackAgc,

    /** Automatic gain control, slow attack (advanced)
     *
     * Only available on the bladeRF 2.0 Micro. This is an advanced option, and
     * typically requires additional configuration for ideal performance.
     */
    SlowattackAgc,

    /** Automatic gain control, hybrid attack (advanced)
     *
     * Only available on the bladeRF 2.0 Micro. This is an advanced option, and
     * typically requires additional configuration for ideal performance.
     */
    HybridAgc,
}

#[derive(Clone, Default)]
pub struct BladerfRationalRate {
    /* Integer portion */
    pub integer: u64,
    /* Numerator in fractional portion */
    pub num: u64,
    /* Denominator in fractional portion. This must be greater than 0. */
    pub den: u64,
}

#[repr(u8)]
pub enum StringDescriptors {
    Manufacturer = 0x1, // Don't want to start with 0 as 0 is reserved for the language table
    Product,
    Serial,
    Fx3Firmware,
}

#[repr(u8)]
pub enum DescriptorTypes {
    Device = 0x01,
    Configuration = 0x2, // Don't want to start with 0 as 0 is reserved for the language table
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
