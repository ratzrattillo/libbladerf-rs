// TODO: find a place where to put generic checks for e.g. type sizes
// assert_eq!(size_of::<u64>(), 8);

pub mod bladerf1;
mod bladerf2;

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

#[derive(Debug)]
pub struct SdrRange<T> {
    pub min: T,
    pub max: T,
    pub step: T,
    pub scale: T,
}

/**
 * Stream direction
 */
#[derive(PartialEq, Debug)]
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
#[repr(u8)]
pub enum BladerfLoopback {
    /** Disables loopback and returns to normal operation. */
    None = 0,

    /** Firmware loopback inside of the FX3 */
    Firmware,

    /** Baseband loopback. TXLPF output is connected to the RXVGA2 input. */
    BbTxlpfRxvga2,

    /** Baseband loopback. TXVGA1 output is connected to the RXVGA2 input. */
    BbTxvga1Rxvga2,

    /** Baseband loopback. TXLPF output is connected to the RXLPF input. */
    BbTxlpfRxlpf,

    /** Baseband loopback. TXVGA1 output is connected to RXLPF input. */
    BbTxvga1Rxlpf,

    /**
     * RF loopback. The TXMIX output, through the AUX PA, is connected to the
     * output of LNA1.
     */
    Lna1,

    /**
     * RF loopback. The TXMIX output, through the AUX PA, is connected to the
     * output of LNA2.
     */
    Lna2,

    /**
     * RF loopback. The TXMIX output, through the AUX PA, is connected to the
     * output of LNA3.
     */
    Lna3,

    /** RFIC digital loopback (built-in self-test) */
    RficBist,
}

// impl TryFrom<u8> for BladerfLoopback {
//     type Error = ();
//
//     fn try_from(value: u8) -> Result<Self, Self::Error> {
//         match value {
//             0 => Ok(BladerfLoopback::None),
//             1 => Ok(BladerfLoopback::Firmware),
//             2 => Ok(BladerfLoopback::BbTxlpfRxvga2),
//             3 => Ok(BladerfLoopback::BbTxvga1Rxvga2),
//             4 => Ok(BladerfLoopback::BbTxlpfRxlpf),
//             5 => Ok(BladerfLoopback::BbTxvga1Rxlpf),
//             6 => Ok(BladerfLoopback::Lna1),
//             7 => Ok(BladerfLoopback::Lna2),
//             8 => Ok(BladerfLoopback::Lna3),
//             9 => Ok(BladerfLoopback::RficBist),
//             _ => Err(()),
//         }
//     }
// }

/**
 * Low-Pass Filter (LPF) mode
 */
#[derive(PartialEq)]
pub enum BladerfLpfMode {
    /**< LPF connected and enabled */
    Normal,
    /**< LPF bypassed */
    Bypassed,
    /**< LPF disabled */
    Disabled,
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

/* String descriptor indices */
pub const BLADE_USB_STR_INDEX_MFR: u8 = 1; /* Manufacturer */
pub const BLADE_USB_STR_INDEX_PRODUCT: u8 = 2; /* Product */
pub const BLADE_USB_STR_INDEX_SERIAL: u8 = 3; /* Serial number */
pub const BLADE_USB_STR_INDEX_FW_VER: u8 = 4; /* Firmware version */

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
//     unsigned char *ptr;
// };
//
// struct bladeRF_sector {
//     unsigned int idx;
//     unsigned int len;
//     unsigned char *ptr;
// };
//
// /**
//  * FPGA configuration source
//  *
//  * Note: the numbering of this enum must match bladerf_fpga_source in
//  * libbladeRF.h
//  */
// typedef enum {
//     NUAND_FPGA_CONFIG_SOURCE_INVALID = 0, /**< Uninitialized/invalid */
//     NUAND_FPGA_CONFIG_SOURCE_FLASH   = 1, /**< Last FPGA load was from flash */
//     NUAND_FPGA_CONFIG_SOURCE_HOST    = 2  /**< Last FPGA load was from host */
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

/* Interface numbers */
pub const USB_IF_LEGACY_CONFIG: u8 = 0;
pub const USB_IF_NULL: u8 = 0;
pub const USB_IF_RF_LINK: u8 = 1;
pub const USB_IF_SPI_FLASH: u8 = 2;
pub const USB_IF_CONFIG: u8 = 3;
