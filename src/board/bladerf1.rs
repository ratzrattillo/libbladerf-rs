#![allow(private_interfaces, dead_code)]

use crate::hardware::dac161s055::DAC161S055;
use crate::hardware::lms6002d::{BLADERF1_BAND_HIGH, LMS6002D};
use crate::hardware::si5338::SI5338;
use crate::nios::Nios;
use anyhow::{Result, anyhow};
use bladerf_globals::BladerfGainMode::{BladerfGainDefault, BladerfGainMgc};
use bladerf_globals::{
    BLADERF_MODULE_RX, BLADERF_MODULE_TX, BLADERF1_USB_PID, BLADERF1_USB_VID, BladerfGainMode,
    DescriptorTypes, ENDPOINT_IN, ENDPOINT_OUT, StringDescriptors, TIMEOUT, bladerf_channel_rx,
    bladerf_channel_tx,
};
use bladerf_nios::NIOS_PKT_8X32_TARGET_CONTROL;
use bladerf_nios::packet_generic::NiosPkt8x32;
use bladerf_nios::packet_retune::{Band, NiosPktRetuneRequest};
use nusb::descriptors::ConfigurationDescriptor;
use nusb::transfer::{Bulk, ControlIn, ControlOut, ControlType, In, Recipient};
use nusb::{Device, DeviceInfo, Interface};
use std::cmp::PartialEq;
use std::num::NonZero;
use std::time::Duration;

#[derive(thiserror::Error, Debug)]
pub enum BladeRfError {
    /// Device not found.
    #[error("NotFound")]
    NotFound,
}

/**
 * Enable LMS receive
 *
 * @note This bit is set/cleared by bladerf_enable_module()
 */
const BLADERF_GPIO_LMS_RX_ENABLE: u8 = 1 << 1;

/**
 * Enable LMS transmit
 *
 * @note This bit is set/cleared by bladerf_enable_module()
 */
const BLADERF_GPIO_LMS_TX_ENABLE: u8 = 1 << 2;

/**
 * Switch to use TX low band (300MHz - 1.5GHz)
 *
 * @note This is set using bladerf_set_frequency().
 */
const BLADERF_GPIO_TX_LB_ENABLE: u8 = 2 << 3;

/**
 * Switch to use TX high band (1.5GHz - 3.8GHz)
 *
 * @note This is set using bladerf_set_frequency().
 */
const BLADERF_GPIO_TX_HB_ENABLE: u8 = 1 << 3;

/**
 * Counter mode enable
 *
 * Setting this bit to 1 instructs the FPGA to replace the (I, Q) pair in sample
 * data with an incrementing, little-endian, 32-bit counter value. A 0 in bit
 * specifies that sample data should be sent (as normally done).
 *
 * This feature is useful when debugging issues involving dropped samples.
 */
const BLADERF_GPIO_COUNTER_ENABLE: u16 = 1 << 9;

/**
 * Bit mask representing the rx mux selection
 *
 * @note These bits are set using bladerf_set_rx_mux()
 */
const BLADERF_GPIO_RX_MUX_MASK: u16 = 7 << BLADERF_GPIO_RX_MUX_SHIFT;

/**
 * Starting bit index of the RX mux values in FX3 <-> FPGA GPIO bank
 */
const BLADERF_GPIO_RX_MUX_SHIFT: u16 = 8;

/**
 * Switch to use RX low band (300M - 1.5GHz)
 *
 * @note This is set using bladerf_set_frequency().
 */
const BLADERF_GPIO_RX_LB_ENABLE: u16 = 2 << 5;

/**
 * Switch to use RX high band (1.5GHz - 3.8GHz)
 *
 * @note This is set using bladerf_set_frequency().
 */
const BLADERF_GPIO_RX_HB_ENABLE: u16 = 1 << 5;

/**
 * This GPIO bit configures the FPGA to use smaller DMA transfers (256 cycles
 * instead of 512). This is required when the device is not connected at Super
 * Speed (i.e., when it is connected at High Speed).
 *
 * However, the caller need not set this in bladerf_config_gpio_write() calls.
 * The library will set this as needed; callers generally do not need to be
 * concerned with setting/clearing this bit.
 */
const BLADERF_GPIO_FEATURE_SMALL_DMA_XFER: u16 = 1 << 7;

/**
 * Enable Packet mode
 */
const BLADERF_GPIO_PACKET: u32 = 1 << 19;

/**
 * Enable 8bit sample mode
 */
const BLADERF_GPIO_8BIT_MODE: u32 = 1 << 20;

/**
 * AGC enable control bit
 *
 * @note This is set using bladerf_set_gain_mode().
 */
const BLADERF_GPIO_AGC_ENABLE: u32 = 1 << 18;

/**
 * Enable-bit for timestamp counter in the FPGA
 */
const BLADERF_GPIO_TIMESTAMP: u32 = 1 << 16;

/**
 * Timestamp 2x divider control.
 *
 * @note <b>Important</b>: This bit has no effect and is always enabled (1) in
 * FPGA versions >= v0.3.0.
 *
 * @note The remainder of the description of this bit is presented here for
 * historical purposes only. It is only relevant to FPGA versions <= v0.1.2.
 *
 * By default, (value = 0), the sample counter is incremented with I and Q,
 * yielding two counts per sample.
 *
 * Set this bit to 1 to enable a 2x timestamp divider, effectively achieving 1
 * timestamp count per sample.
 * */
const BLADERF_GPIO_TIMESTAMP_DIV2: u32 = 1 << 17;

/**
 * Packet capable core present bit.
 *
 * @note This is a read-only bit. The FPGA sets its value, and uses it to inform
 *  host that there is a core capable of using packets in the FPGA.
 */
const BLADERF_GPIO_PACKET_CORE_PRESENT: u32 = 1 << 28;

pub const BLADERF_SAMPLERATE_MIN: u64 = 80000;

/** Minimum tunable frequency (without an XB-200 attached), in Hz
*
* \deprecated Use bladerf_get_frequency_range()
 */
pub const BLADERF_FREQUENCY_MIN: u32 = 237500000;

/** Maximum tunable frequency, in Hz
*
* \deprecated Use bladerf_get_frequency_range()
 */
pub const BLADERF_FREQUENCY_MAX: u32 = 3800000000;

/**
 * Maximum output frequency on SMB connector, if no expansion board attached.
 */
pub const BLADERF_SMB_FREQUENCY_MAX: u32 = 200000000;

/**
 * Minimum output frequency on SMB connector, if no expansion board attached.
 */
pub const BLADERF_SMB_FREQUENCY_MIN: u32 = (38400000 * 66) / (32 * 567);

pub const BLADERF_DIRECTION_MASK: u8 = 0x1;
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
    BladerfFormatSc16Q11 = 0,

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
    BladerfFormatSc16Q11Meta = 1,

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
    BladerfFormatPacketMeta = 2,

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
    BladerfFormatSc8Q7 = 3,

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
    BladerfFormatSc8Q7Meta = 4,
}

/**
 * Stream direction
 */
#[derive(PartialEq)]
pub enum BladeRfDirection {
    BladerfRx = 0, // Receive direction
    BladerfTx = 1, // Transmit direction
}

/**
 * Stream channel layout
 */
#[derive(PartialEq)]
pub enum BladeRfChannelLayout {
    BladerfRxX1 = 0, // x1 RX (SISO)
    BladerfTxX1 = 1, // x1 TX (SISO)
    BladerfRxX2 = 2, // x2 RX (MIMO)
    BladerfTxX2 = 3, // x2 TX (MIMO)
}

/**
 * LNA gain options
 *
 * \deprecated Use bladerf_get_gain_stage_range()
 */
#[derive(PartialEq)]
pub enum BladerfLnaGain {
    /**< Invalid LNA gain */
    BladerfLnaGainUnknown,
    /**< LNA bypassed - 0dB gain */
    BladerfLnaGainBypass,
    /**< LNA Mid Gain (MAX-6dB) */
    BladerfLnaGainMid,
    /**< LNA Max Gain */
    BladerfLnaGainMax,
}

pub struct BladeRf1 {
    device: Device,
    interface: Interface,
    lms: LMS6002D,
    si5338: SI5338,
    dac: DAC161S055,
    // xb200: Option<XB200>,
}

impl BladeRf1 {
    async fn list_bladerf1() -> Result<impl Iterator<Item = DeviceInfo>> {
        Ok(nusb::list_devices().await?.filter(|dev| {
            dev.vendor_id() == BLADERF1_USB_VID && dev.product_id() == BLADERF1_USB_PID
        }))
    }

    async fn build(device: Device) -> Result<Box<Self>> {
        let interface = device.detach_and_claim_interface(0).await?;
        // TODO Have a reference to a backend instance that holds the endpoints needed
        // TODO Give this reference to the individual Hardware...
        // TODO: Fix this with RefCell<BackendTest> with interior mutability or Mutex?.
        // Question:: Is it better to claim an endpoint from an interface in each method,
        // where we need to write data or is it better to have the whole Backend behind a mutex?
        let lms = LMS6002D::new(interface.clone());
        let si5338 = SI5338::new(interface.clone());
        let dac = DAC161S055::new(interface.clone());

        Ok(Box::new(Self {
            device,
            interface,
            lms,
            si5338,
            dac,
        }))
    }

    pub async fn from_first() -> Result<Box<Self>> {
        let device = Self::list_bladerf1()
            .await?
            .next()
            .ok_or(BladeRfError::NotFound)?
            .open()
            .await?;
        Self::build(device).await
    }
    pub async fn from_serial(serial: &str) -> Result<Box<Self>> {
        let device = Self::list_bladerf1()
            .await?
            .find(|dev| dev.serial_number() == Some(serial))
            .ok_or(BladeRfError::NotFound)?
            .open()
            .await?;
        Self::build(device).await
    }

    pub async fn from_bus_addr(bus_number: u8, bus_addr: u8) -> Result<Box<Self>> {
        let device = Self::list_bladerf1()
            .await?
            .find(|dev| dev.busnum() == bus_number && dev.device_address() == bus_addr)
            .ok_or(BladeRfError::NotFound)?
            .open()
            .await?;
        Self::build(device).await
    }

    pub async fn from_fd(fd: std::os::fd::OwnedFd) -> Result<Box<Self>> {
        let device = Device::from_fd(fd).await?;
        // TODO: Do check on device, if it really is a bladerf
        Self::build(device).await
    }

    pub async fn serial(&self) -> Result<String> {
        self.get_string_descriptor(NonZero::try_from(StringDescriptors::Serial as u8)?)
            .await
    }

    pub async fn manufacturer(&self) -> Result<String> {
        self.get_string_descriptor(NonZero::try_from(StringDescriptors::Manufacturer as u8)?)
            .await
    }

    pub async fn fx3_firmware(&self) -> Result<String> {
        self.get_string_descriptor(NonZero::try_from(StringDescriptors::Fx3Firmware as u8)?)
            .await
    }

    pub async fn product(&self) -> Result<String> {
        self.get_string_descriptor(NonZero::try_from(StringDescriptors::Product as u8)?)
            .await
    }

    async fn config_gpio_read(&self) -> Result<u32> {
        type NiosPkt = NiosPkt8x32;

        let request = NiosPkt::new(NIOS_PKT_8X32_TARGET_CONTROL, NiosPkt::FLAG_READ, 0x0, 0x0);
        let response = self
            .interface
            .nios_send(ENDPOINT_OUT, ENDPOINT_IN, request.into())
            .await?;
        Ok(NiosPkt::from(response).data())
    }

    async fn config_gpio_write(&self, mut data: u32) -> Result<u32> {
        type NiosPkt = NiosPkt8x32;

        enum DeviceSpeed {
            Unknown,
            High,
            Super,
        }

        // TODO: Get usb speed dynamically
        let device_speed: DeviceSpeed = DeviceSpeed::Super;
        match device_speed {
            DeviceSpeed::Unknown => {
                println!("DeviceSpeed::Unknown");
            }
            DeviceSpeed::High => {
                println!("DeviceSpeed::High");
                data |= BLADERF_GPIO_FEATURE_SMALL_DMA_XFER as u32;
            }
            DeviceSpeed::Super => {
                println!("DeviceSpeed::Super");
                data &= !BLADERF_GPIO_FEATURE_SMALL_DMA_XFER as u32;
            }
        }

        let request = NiosPkt::new(NIOS_PKT_8X32_TARGET_CONTROL, NiosPkt::FLAG_WRITE, 0x0, data);
        let response_vec = self
            .interface
            .nios_send(ENDPOINT_OUT, ENDPOINT_IN, request.into())
            .await?;
        let response = NiosPkt::from(response_vec);
        Ok(response.data())
    }

    /*
    bladerf1_initialize is wrapped in bladerf1_open
     */
    pub async fn initialize(&self) -> Result<()> {
        self.interface.set_alt_setting(0x01).await?;
        println!("[*] Init - Set Alt Setting to 0x01");

        // Out: 43010000000000000000000000000000
        // In:  43010200000000000000000000000000
        let cfg = self.config_gpio_read().await?;
        if (cfg & 0x7f) == 0 {
            println!("[*] Init - Default GPIO value \"{cfg}\" found - initializing device");
            /* Set the GPIO pins to enable the LMS and select the low band */
            // Out: 43010100005700000000000000000000
            // In:  43010300005700000000000000000000
            self.config_gpio_write(0x57).await?;

            /* Disable the front ends */
            println!("[*] Init - Disabling RX and TX Frontend");
            // Out: 41000000400000000000000000000000
            // In:  41000200400200000000000000000000
            // Out: 41000100400000000000000000000000
            // In:  41000300400000000000000000000000
            self.lms.enable_rffe(BLADERF_MODULE_TX, false).await?;
            println!("{BLADERF_MODULE_TX}");

            // Out: 41000000700000000000000000000000
            // In:  41000200700200000000000000000000
            // Out: 41000100700000000000000000000000
            // In:  41000300700000000000000000000000
            self.lms.enable_rffe(BLADERF_MODULE_RX, false).await?;
            println!("{BLADERF_MODULE_RX}");

            /* Set the internal LMS register to enable RX and TX */
            println!("[*] Init - Set LMS register to enable RX and TX");
            // Out: 41000100053e00000000000000000000
            // In:  41000300053e00000000000000000000
            self.lms.write(0x05, 0x3e).await?;

            /* LMS FAQ: Improve TX spurious emission performance */
            println!("[*] Init - Set LMS register to enable RX and TX");
            // Out: 41000100474000000000000000000000
            // In:  41000300474000000000000000000000
            self.lms.write(0x47, 0x40).await?;

            /* LMS FAQ: Improve ADC performance */
            println!("[*] Init - Set register to improve ADC performance");
            // Out: 41000100592900000000000000000000
            // In:  41000300592900000000000000000000
            self.lms.write(0x59, 0x29).await?;

            /* LMS FAQ: Common mode voltage for ADC */
            println!("[*] Init - Set Common mode voltage for ADC");
            // Out: 41000100643600000000000000000000
            // In:  41000300643600000000000000000000
            self.lms.write(0x64, 0x36).await?;

            /* LMS FAQ: Higher LNA Gain */
            println!("[*] Init - Set Higher LNA Gain");
            // Out: 41000100793700000000000000000000
            // In:  41000300793700000000000000000000
            self.lms.write(0x79, 0x37).await?;

            /* Power down DC calibration comparators until they are need, as they
             * have been shown to introduce undesirable artifacts into our signals.
             * (This is documented in the LMS6 FAQ). */

            println!("[*] Init - Power down TX LPF DC cal comparator");
            // Out: 410000003f0000000000000000000000
            // In:  410002003f0000000000000000000000
            // Out: 410001003f8000000000000000000000
            // In:  410003003f8000000000000000000000
            self.lms.set(0x3f, 0x80).await?; /* TX LPF DC cal comparator */

            println!("[*] Init - Power down RX LPF DC cal comparator");
            // Out: 410000005f0000000000000000000000
            // In:  410002005f1f00000000000000000000
            // Out: 410001005f9f00000000000000000000
            // In:  410003005f9f00000000000000000000
            self.lms.set(0x5f, 0x80).await?; /* RX LPF DC cal comparator */

            println!("[*] Init - Power down RXVGA2A/B DC cal comparators");
            // Out: 410000006e0000000000000000000000
            // In:  410002006e0000000000000000000000
            // Out: 410001006ec000000000000000000000
            // In:  410003006ec000000000000000000000
            self.lms.set(0x6e, 0xc0).await?; /* RXVGA2A/B DC cal comparators */

            /* Configure charge pump current offsets */
            println!("[*] Init - Configure TX charge pump current offsets");
            // Out: 41000000160000000000000000000000
            // In:  41000200168c00000000000000000000
            // Out: 41000100160000000000000000000000
            // In:  41000300168c00000000000000000000
            // Out: 41000000170000000000000000000000
            // In:  4100020017e000000000000000000000
            // Out: 4100010017e300000000000000000000
            // In:  4100030017e300000000000000000000
            // Out: 41000000180000000000000000000000
            // In:  41000200184000000000000000000000
            // Out: 41000100184300000000000000000000
            // In:  41000300184300000000000000000000
            let _ = self.lms.config_charge_pumps(BLADERF_MODULE_TX).await?;
            println!("[*] Init - Configure RX charge pump current offsets");

            // Out: 41000000260000000000000000000000
            // In:  41000200268c00000000000000000000
            // Out: 41000100260000000000000000000000
            // In:  41000300268c00000000000000000000
            // Out: 41000000270000000000000000000000
            // In:  4100020027e000000000000000000000
            // Out: 4100010027e300000000000000000000
            // In:  4100030027e300000000000000000000
            // Out: 41000000280000000000000000000000
            // In:  41000200284000000000000000000000
            // Out: 41000100184300000000000000000000
            // In:  41000300284300000000000000000000
            let _ = self.lms.config_charge_pumps(BLADERF_MODULE_RX).await?;

            println!("[*] Init - Set TX Samplerate");
            // Out: 41010000260000000000000000000000
            // In:  41010200260000000000000000000000
            // Out: 41010100260300000000000000000000
            // In:  41010300260300000000000000000000
            // Out: 410101004b6600000000000000000000
            // In:  410103004b6600000000000000000000
            // Out: 410101004c9c00000000000000000000
            // In:  410103004c9c00000000000000000000
            // Out: 410101004d0800000000000000000000
            // In:  410103004d0800000000000000000000
            // Out: 410101004e0000000000000000000000
            // In:  410103004e0000000000000000000000
            // Out: 410101004f0000000000000000000000
            // In:  410103004f0000000000000000000000
            // Out: 41010100500000000000000000000000
            // In:  41010300500000000000000000000000
            // Out: 41010100510500000000000000000000
            // In:  41010300510500000000000000000000
            // Out: 41010100520000000000000000000000
            // In:  41010300520000000000000000000000
            // Out: 41010100530000000000000000000000
            // In:  41010300530000000000000000000000
            // Out: 41010100540000000000000000000000
            // In:  41010300540000000000000000000000
            // Out: 4101010021c800000000000000000000
            // In : 4101030021c800000000000000000000
            let _actual_tx = self
                .si5338
                .set_sample_rate(bladerf_channel_tx!(0), 1000000)
                .await?;

            println!("[*] Init - Set RX Samplerate");
            // Out: As above but slightly different (Matches original packets)
            // In:  As above but slightly different (Matches original packets)
            let _actual_rx = self
                .si5338
                .set_sample_rate(bladerf_channel_rx!(0), 1000000)
                .await?;

            // SI5338 Packet: Magic: 0x54, 8x 0xff, Channel (int), 4Byte Frequency
            // With TX Channel: {0x54, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x0, 0x0, 0x0, 0x0, 0x40, 0x0, 0x0};
            // With RX Channel: {0x54, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x0, 0x0, 0x0, 0x0, 0x80, 0x0, 0x0};
            // Basically  nios_si5338_read == nios 8x8 read

            // board_data->tuning_mode = tuning_get_default_mode(dev);

            println!("self.set_frequency(bladerf_channel_tx!(0), 2447000000)?;");
            // Out: 5400000000000000003fb95555ac1f00
            // In:  5400000000000000001e030000000000
            self.set_frequency(bladerf_channel_tx!(0), 2447000000)
                .await?;

            println!("self.set_frequency(bladerf_channel_rx!(0), 2484000000)?;");
            // Out: 54000000000000000040b000006c2300
            // In:  54000000000000000021030000000000
            self.set_frequency(bladerf_channel_rx!(0), 2484000000)
                .await?;

            // /* Set the calibrated VCTCXO DAC value */
            // TODO: board_data.dac_trim instead of 0
            // Out: 42000100280000000000000000000000
            // In:  42000300280000000000000000000000
            self.dac.write(0).await?;

            // status = dac161s055_write(dev, board_data->dac_trim);
            // if (status != 0) {
            //     return status;
            // }

            // /* Set the default gain mode */
            // Out expected: 4200010008d1ab000000000000000000
            // Out actual:   42000100080000000000000000000000
            // In: expected: 4200030008d1ab000000000000000000
            // In actual:    42000300080000000000000000000000
            // Todo: Implement AGC table and set mode to BladerfGainDefault
            self.set_gain_mode(bladerf_channel_rx!(0), BladerfGainMgc)
                .await?;
        } else {
            println!("[*] Init - Device already initialized: {cfg:#04x}");
            //board_data->tuning_mode = tuning_get_default_mode(dev);
        }

        // /* Check if we have an expansion board attached */
        // status = dev->board->expansion_get_attached(dev, &dev->xb);
        // if (status != 0) {
        //     return status;
        // }
        //
        // /* Update device state */
        // board_data->state = STATE_INITIALIZED;
        //
        // /* Set up LMS DC offset register calibration and initial IQ settings,
        //  * if any tables have been loaded already.
        //  *
        //  * This is done every time the device is opened (with an FPGA loaded),
        //  * as the user may change/update DC calibration tables without reloading the
        //  * FPGA.
        //  */
        // status = bladerf1_apply_lms_dc_cals(dev);
        // if (status != 0) {
        //     return status;
        // }

        Ok(())
    }

    pub async fn bladerf_enable_module(&self, module: u8, enable: bool) -> Result<u8> {
        self.lms.enable_rffe(module, enable).await
    }

    // Todo: Implement band select for set_frequency
    pub async fn band_select(&self, module: u8, band: Band) -> Result<u32> {
        //const uint32_t band = low_band ? 2 : 1;
        let band_value = match band {
            Band::Low => 2,
            Band::High => 1,
        };

        println!("Selecting %s band. {band:?}");

        self.lms.select_band(module, band).await?;
        // status = lms_select_band(dev, module, low_band);
        // if (status != 0) {
        //     return status;
        // }

        let mut gpio = self.config_gpio_read().await?;
        // #ifndef BLADERF_NIOS_BUILD
        //     status = dev->backend->config_gpio_read(dev, &gpio);
        // #else
        //     status = CONFIG_GPIO_READ(dev, &gpio);
        // #endif
        //     if (status != 0) {
        //         return status;
        //     }

        // gpio &= !(module == BLADERF_MODULE_TX ? (3 << 3) : (3 << 5));
        let shift = if module == BLADERF_MODULE_TX {
            3 << 3
        } else {
            3 << 5
        };
        gpio &= !shift;

        // gpio |= (module == BLADERF_MODULE_TX ? (band << 3) : (band << 5));
        let shift = if module == BLADERF_MODULE_TX {
            band_value << 3
        } else {
            band_value << 5
        };
        gpio |= !shift;

        // #ifndef BLADERF_NIOS_BUILD
        //     return dev->backend->config_gpio_write(dev, gpio);
        // #else
        //     return CONFIG_GPIO_WRITE(dev, gpio);
        // #endif
        self.config_gpio_write(gpio).await
    }

    pub async fn set_frequency(&self, channel: u8, frequency: u64) -> Result<()> {
        //let dc_cal = if channel == bladerf_channel_rx!(0) { cal_dc.rx } else { cal.dc_tx };

        println!("Setting Frequency on channel {channel} to {frequency}Hz");

        // Ommit XB200 settings here

        // TODO: The tuning mode should be read from the board config
        // In the packet captures, this is where the changes happen:
        // -  Packet No. 317 in rx-BladeRFTest-unix-filtered.pcapng
        // -  Packet No. 230 in rx-rusttool-filtered.pcapng
        // This is maybe due to the tuning mode being FPGA and not Host
        enum TuningMode {
            Host,
            Fpga,
        }
        let mode = TuningMode::Fpga;
        // For tuning HOST Tuning Mode:
        match mode {
            TuningMode::Host => {
                self.lms.set_frequency(channel, frequency as u32).await?;
                let band = if frequency < BLADERF1_BAND_HIGH as u64 {
                    Band::Low
                } else {
                    Band::High
                };
                self.band_select(channel, band).await?;
            }
            TuningMode::Fpga => {
                self.lms
                    .schedule_retune(
                        channel,
                        NiosPktRetuneRequest::RETUNE_NOW,
                        frequency as u32,
                        None,
                    )
                    .await?;
            }
        }

        Ok(())
    }

    #[allow(unreachable_code)] // TODO: Only while AGC table is not implemented
    pub async fn set_gain_mode(&self, channel: u8, mode: BladerfGainMode) -> Result<u32> {
        if channel != BLADERF_MODULE_RX {
            return Err(anyhow!("Operation only supported on RX channel"));
        }

        let mut config_gpio = self.config_gpio_read().await?;
        if mode == BladerfGainDefault {
            // Default mode is the same as Automatic mode
            return Err(anyhow!("Todo: Implement AGC Table"));
            // if (!have_cap(board_data->capabilities, BLADERF_CAP_AGC_DC_LUT)) {
            //     log_warning("AGC not supported by FPGA. %s\n", MGC_WARN);
            //     log_info("To enable AGC, %s, then %s\n", FPGA_STR, DCCAL_STR);
            //     log_debug("%s: expected FPGA >= v0.7.0, got v%u.%u.%u\n",
            //               __FUNCTION__, board_data->fpga_version.major,
            //               board_data->fpga_version.minor,
            //               board_data->fpga_version.patch);
            //     return BLADERF_ERR_UNSUPPORTED;
            // }
            //
            // if (!board_data->cal.dc_rx) {
            //     log_warning("RX DC calibration table not found. %s\n", MGC_WARN);
            //     log_info("To enable AGC, %s\n", DCCAL_STR);
            //     return BLADERF_ERR_UNSUPPORTED;
            // }
            //
            // if (board_data->cal.dc_rx->version != TABLE_VERSION) {
            //     log_warning("RX DC calibration table is out-of-date. %s\n",
            //                 MGC_WARN);
            //     log_info("To enable AGC, %s\n", DCCAL_STR);
            //     log_debug("%s: expected version %u, got %u\n", __FUNCTION__,
            //               TABLE_VERSION, board_data->cal.dc_rx->version);
            //
            //     return BLADERF_ERR_UNSUPPORTED;
            // }
            config_gpio |= BLADERF_GPIO_AGC_ENABLE;
        } else if mode == BladerfGainMgc {
            config_gpio &= !BLADERF_GPIO_AGC_ENABLE;
        }

        self.config_gpio_write(config_gpio).await
    }

    // Get BladeRf frequency
    // https://github.com/Nuand/bladeRF/blob/master/host/libraries/libbladeRF/include/libbladeRF.h#L694
    // const BLADERF_CHANNEL_RX(ch) (bladerf_channel)(((ch) << 1) | 0x0)
    // https://github.com/Nuand/bladeRF/blob/master/host/libraries/libbladeRF/include/libbladeRF.h#L694
    // const BLADERF_MODULE_RX BLADERF_CHANNEL_RX(0)
    // https://github.com/Nuand/bladeRF/blob/fe3304d75967c88ab4f17ff37cb5daf8ff53d3e1/host/libraries/libbladeRF/src/board/bladerf1/bladerf1.c#L2121
    // static int bladerf1_get_frequency(struct bladerf *dev, bladerf_channel ch, bladerf_frequency *frequency);
    // https://github.com/Nuand/bladeRF/blob/master/fpga_common/src/lms.c#L1698
    // int lms_get_frequency(struct bladerf *dev, bladerf_module mod, struct lms_freq *f)
    // lms_freq struct: https://github.com/Nuand/bladeRF/blob/master/fpga_common/include/lms.h#L101
    // https://github.com/Nuand/bladeRF/blob/master/fpga_common/src/lms.c#L1698
    // const uint8_t base = (mod == BLADERF_MODULE_RX) ? 0x20 : 0x10;
    // sudo usermod -a -G wireshark jl
    // sudo modprobe usbmon
    // sudo setfacl -m u:jl:r /dev/usbmon*
    // Wireshark Display filter depending on lsusb output: usb.bus_id == 2 and usb.device_address == 6
    // pub fn get_freq(&self, module: u8) -> Result<LmsFreq> {
    //     //self.device.set_configuration(1)?;
    //     // TODO: FPGA must be loaded!
    //     self.interface.set_alt_setting(1)?;
    //
    //     let addr = if module == crate::bladerf::BLADERF_MODULE_RX {
    //         0x20u8
    //     } else {
    //         0x10u8
    //     };
    //
    //     let mut lms_freq = LmsFreq {
    //         freqsel: 0,
    //         vcocap: 0,
    //         nint: 0,
    //         nfrac: 0,
    //         //flags: 0,
    //         //xb_gpio: 0,
    //         x: 0,
    //         //vcocap_result: 0,
    //     };
    //
    //     let mut request = NiosPkt::<u8, u8>::new(
    //         NIOS_PKT_8X8_TARGET_LMS6,
    //         NIOS_PKT_FLAG_READ,
    //         addr | 0x0u8,
    //         0x0,
    //     );
    //
    //     let mut response = self.lms_read(request.into_vec())?;
    //     let mut response_pkt: NiosPkt<u8, u8> = NiosPkt::<u8, u8>::reuse(response);
    //     lms_freq.nint = u16::from(response_pkt.data()) << 1;
    //
    //     response_pkt
    //         .set_flags(NIOS_PKT_FLAG_READ)
    //         .set_addr(addr | 0x1u8)
    //         .set_data(0x0);
    //     request = response_pkt;
    //
    //     response = self.lms_read(request.into_vec())?;
    //     let mut response_pkt: NiosPkt<u8, u8> = NiosPkt::<u8, u8>::reuse(response);
    //
    //     lms_freq.nint = lms_freq.nint | ((u16::from(response_pkt.data()) & 0x80) >> 7);
    //     lms_freq.nfrac = (u32::from(response_pkt.data()) & 0x7f) << 16;
    //
    //     response_pkt
    //         .set_flags(NIOS_PKT_FLAG_READ)
    //         .set_addr(addr | 0x2u8)
    //         .set_data(0x0);
    //     request = response_pkt;
    //
    //     response = self.lms_read(request.into_vec())?;
    //     let mut response_pkt: NiosPkt<u8, u8> = NiosPkt::<u8, u8>::reuse(response);
    //
    //     lms_freq.nfrac = lms_freq.nfrac | (u32::from(response_pkt.data()) << 8);
    //
    //     response_pkt
    //         .set_flags(NIOS_PKT_FLAG_READ)
    //         .set_addr(addr | 0x3u8)
    //         .set_data(0x0);
    //     request = response_pkt;
    //
    //     response = self.lms_read(request.into_vec())?;
    //     let mut response_pkt: NiosPkt<u8, u8> = NiosPkt::<u8, u8>::reuse(response);
    //     lms_freq.nfrac = lms_freq.nfrac | u32::from(response_pkt.data());
    //
    //     response_pkt
    //         .set_flags(NIOS_PKT_FLAG_READ)
    //         .set_addr(addr | 0x5u8)
    //         .set_data(0x0);
    //     request = response_pkt;
    //
    //     response = self.lms_read(request.into_vec())?;
    //     let mut response_pkt: NiosPkt<u8, u8> = NiosPkt::<u8, u8>::reuse(response);
    //
    //     lms_freq.freqsel = response_pkt.data() >> 2;
    //     if lms_freq.freqsel >= 3 {
    //         lms_freq.x = 1 << ((lms_freq.freqsel & 7) - 3);
    //     }
    //
    //     response_pkt
    //         .set_flags(NIOS_PKT_FLAG_READ)
    //         .set_addr(addr | 0x9u8)
    //         .set_data(0x0);
    //     request = response_pkt;
    //
    //     response = self.lms_read(request.into_vec())?;
    //     let mut response_pkt: NiosPkt<u8, u8> = NiosPkt::<u8, u8>::reuse(response);
    //
    //     lms_freq.vcocap = response_pkt.data() & 0x3f;
    //
    //     Ok(lms_freq)
    // }

    // pub fn lms_frequency_to_hz(lms_freq: &LmsFreq) -> u64 {
    //     let pll_coeff = ((lms_freq.nint as u64) << 23) + lms_freq.nfrac as u64;
    //     let div = (lms_freq.x as u64) << 23;
    //
    //     if div > 0 {
    //         ((LMS_REFERENCE_HZ as u64 * pll_coeff) + (div >> 1)) / div
    //     } else {
    //         0
    //     }
    // }

    /// Get BladeRf1 String descriptor
    pub async fn get_string_descriptor(&self, descriptor_index: NonZero<u8>) -> Result<String> {
        let descriptor = self
            .device
            .get_string_descriptor(descriptor_index, 0x409, Duration::from_secs(1))
            .await?;
        Ok(descriptor)
    }

    /// Get BladeRf1 Serial number
    pub async fn get_configuration_descriptor(&self, descriptor_index: u8) -> Result<Vec<u8>> {
        let descriptor = self
            .device
            .get_descriptor(
                DescriptorTypes::Configuration as u8,
                descriptor_index,
                0x00,
                Duration::from_secs(1),
            )
            .await?;
        Ok(descriptor)
    }

    pub async fn get_supported_languages(&self) -> Result<Vec<u16>> {
        let languages = self
            .device
            .get_string_descriptor_supported_languages(Duration::from_secs(1))
            .await?
            .collect();

        Ok(languages)
    }

    pub fn get_configurations(&self) -> Vec<ConfigurationDescriptor> {
        self.device.configurations().collect()
    }

    pub async fn set_configuration(&self, configuration: u16) -> Result<()> {
        //self.device.set_configuration(configuration)?;
        Ok(self
            .interface
            .control_out(
                ControlOut {
                    control_type: ControlType::Standard,
                    recipient: Recipient::Device,
                    request: 0x09, //Request::VersionStringRead as u8,
                    value: configuration,
                    index: 0x00,
                    data: &[],
                },
                TIMEOUT,
            )
            .await?)
    }

    /**
     * Perform the neccessary device configuration for the specified format
     * (e.g., enabling/disabling timestamp support), first checking that the
     * requested format would not conflict with the other stream direction.
     *
     * @param           dev     Device handle
     * @param[in]       dir     Direction that is currently being configured
     * @param[in]       format  Format the channel is being configured for
     *
     * @return 0 on success, BLADERF_ERR_* on failure
     */
    pub async fn perform_format_config(
        &self,
        dir: BladeRfDirection,
        format: BladerfFormat,
    ) -> Result<()> {
        // BladerfFormatPacketMeta
        //struct bladerf1_board_data *board_data = dev->board_data;

        //int status = 0;
        let mut use_timestamps: bool = false;
        let _other_using_timestamps: bool = false;

        // status = requires_timestamps(format, &use_timestamps);
        // if (status != 0) {
        //     log_debug("%s: Invalid format: %d\n", __FUNCTION__, format);
        //     return status;
        // }

        let _other = match dir {
            BladeRfDirection::BladerfRx => BladeRfDirection::BladerfTx,
            BladeRfDirection::BladerfTx => BladeRfDirection::BladerfRx,
        };

        // status = requires_timestamps(board_data->module_format[other],
        //     &other_using_timestamps);

        // if ((status == 0) && (other_using_timestamps != use_timestamps)) {
        //     log_debug("Format conflict detected: RX=%d, TX=%d\n");
        //     return BLADERF_ERR_INVAL;
        // }

        let mut gpio_val = self.config_gpio_read().await?;

        println!("gpio_val {gpio_val:#08x}");
        if format == BladerfFormat::BladerfFormatPacketMeta {
            gpio_val |= BLADERF_GPIO_PACKET;
            use_timestamps = true;
            println!("BladerfFormat::BladerfFormatPacketMeta");
        } else {
            gpio_val &= !BLADERF_GPIO_PACKET;
            println!("else");
        }
        println!("gpio_val {gpio_val:#08x}");

        if use_timestamps {
            println!("use_timestamps");
            gpio_val |= BLADERF_GPIO_TIMESTAMP | BLADERF_GPIO_TIMESTAMP_DIV2;
        } else {
            println!("dont use_timestamps");
            gpio_val &= !(BLADERF_GPIO_TIMESTAMP | BLADERF_GPIO_TIMESTAMP_DIV2);
        }

        println!("gpio_val {gpio_val:#08x}");

        self.config_gpio_write(gpio_val).await?;
        // if (status == 0) {
        //     board_data->module_format[dir] = format;
        // }

        //return status;
        Ok(())
    }

    /**
     * Deconfigure and update any state pertaining what a format that a stream
     * direction is no longer using.
     *
     * @param       dev     Device handle
     * @param[in]   dir     Direction that is currently being deconfigured
     *
     * @return 0 on success, BLADERF_ERR_* on failure
     */
    pub fn perform_format_deconfig(&self, dir: BladeRfDirection) -> Result<()> {
        //struct bladerf1_board_data *board_data = dev->board_data;

        // match dir {
        //     BladeRfDirection::BladerfRx => {
        //         BladeRfDirection::BladerfTx
        //     }
        //     BladeRfDirection::BladerfTx => {
        //         BladeRfDirection::BladerfRx
        //     }
        // };

        match dir {
            BladeRfDirection::BladerfRx | BladeRfDirection::BladerfTx => {
                /* We'll reconfigure the HW when we call perform_format_config, so
                 * we just need to update our stored information */
                //board_data -> module_format[dir] = - 1;
            }
        }

        Ok(())
    }

    pub async fn experimental_control_urb(&self) -> Result<()> {
        // TODO: Dont know what this is doing
        let pkt = ControlIn {
            control_type: ControlType::Vendor,
            recipient: Recipient::Device,
            request: 0x4,
            value: 0x1,
            index: 0,
            length: 0x4,
        };
        let vec = self
            .interface
            .control_in(pkt, Duration::from_secs(5))
            .await?;
        println!("Control Response Data: {vec:?}");
        Ok(())
    }

    pub async fn async_run_stream(&self) -> Result<()> {
        // TODO: In_ENDPOINT is 0x81 here, not 0x82
        let mut ep_bulk_in = self.interface.endpoint::<Bulk, In>(0x81)?;

        let n_transfers = 8;

        // let max_packet_size = ep_bulk_in.max_packet_size();
        let max_packet_size = 32768;
        println!("Max Packet Size: {max_packet_size}");

        for _i in 0..n_transfers {
            let buffer = ep_bulk_in.allocate(max_packet_size);
            ep_bulk_in.submit(buffer);
            // println!("submitted_transfers: {i}");
        }

        loop {
            let result = ep_bulk_in.next_complete().await;
            // println!("{result:?}");
            if result.status.is_err() {
                break;
            }
            ep_bulk_in.submit(result.buffer);
        }
        Ok(())
    }

    // pub async fn bladerf1_stream(&self, stream: &bladerf_stream, layout: BladeRfChannelLayout) -> Result<()> {
    //     let dir: BladeRfDirection = layout & BLADERF_DIRECTION_MASK;
    //     let stream_status: i32;
    //
    //     // if layout != BladeRfChannelLayout::BladerfRxX1 && layout != BladeRfChannelLayout::BladerfTxX1 {
    //     //     return Err(anyhow!("Invalid ChannelLayout"));
    //     // }
    //
    //     self.perform_format_config(dir, stream->format)?;
    //
    //     stream_status = self.async_run_stream(stream, layout).await;
    //     // TODO: static void LIBUSB_CALL lusb_stream_cb
    //
    //     self.perform_format_deconfig(dir)?;
    // }

    pub async fn reset(&self) -> Result<()> {
        //self.check_api_version(UsbVersion::from_bcd(0x0102))?;
        //self.write_control(Request::Reset, 0, 0, &[])?;
        self.device.set_configuration(0).await?;

        Ok(())
    }
}
