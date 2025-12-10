use crate::bladerf::{BLADERF_MODULE_RX, BLADERF_MODULE_TX, Direction};
use crate::board::bladerf1::{BladeRf1, BladeRf1RxStreamer, BladeRf1TxStreamer};
use crate::{Error, Result};
use num_complex::Complex32;
use nusb::MaybeFuture;
use nusb::transfer::{Bulk, ControlIn, ControlType, In, Out, Recipient};
use std::io::{BufRead, Write};
use std::thread::sleep;
use std::time::Duration;

///  Sample format
#[derive(PartialEq)]
pub enum SampleFormat {
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

/// Enable Packet mode
pub const BLADERF_GPIO_PACKET: u32 = 1 << 19;

/// Enable-bit for timestamp counter in the FPGA
pub const BLADERF_GPIO_TIMESTAMP: u32 = 1 << 16;

/// Timestamp 2x divider control.
///
/// @note <b>Important</b>: This bit has no effect and is always enabled (1) in
/// FPGA versions >= v0.3.0.
///
/// @note The remainder of the description of this bit is presented here for
/// historical purposes only. It is only relevant to FPGA versions <= v0.1.2.
///
/// By default, (value = 0), the sample counter is incremented with I and Q,
/// yielding two counts per sample.
///
/// Set this bit to 1 to enable a 2x timestamp divider, effectively achieving 1
/// timestamp count per sample.
pub const BLADERF_GPIO_TIMESTAMP_DIV2: u32 = 1 << 17;

impl BladeRf1RxStreamer {
    /// Create new instance of an RX Streamer to receive I/Q samples.
    pub fn new(
        dev: BladeRf1,
        buffer_size: usize,
        num_transfers: Option<usize>,
        timeout: Option<Duration>,
    ) -> Result<Self> {
        let endpoint = dev.interface.lock().unwrap().endpoint::<Bulk, In>(0x81)?;
        log::trace!(
            "using endpoint 0x81 with buffer_size: {buffer_size}, num_transfers: {num_transfers:?}, timeout: {timeout:?}"
        );
        let mut reader = endpoint.reader(buffer_size);
        if let Some(t) = timeout {
            reader.set_read_timeout(t)
        }
        if let Some(n) = num_transfers {
            reader.set_num_transfers(n)
        }
        Ok(Self {
            dev,
            reader,
            buffer_size,
        })
    }

    pub fn mtu(&self) -> Result<usize> {
        Ok(self.buffer_size)
    }

    /// Activate RX frontend and enable receiving of samples with predefined sample format: `SampleFormat::Sc16Q11`.
    pub fn activate(&mut self) -> Result<()> {
        self.dev
            .perform_format_config(Direction::Rx, SampleFormat::Sc16Q11)?;
        self.dev.enable_module(BLADERF_MODULE_RX, true)?;
        self.dev.experimental_control_urb()
    }

    /// Disable receiving samples and shut down RX frontend.
    pub fn deactivate(&mut self) -> Result<()> {
        self.dev.perform_format_deconfig(Direction::Rx)?;
        self.dev.enable_module(BLADERF_MODULE_RX, false)
    }

    /// Read I/Q samples into a slice of buffers with configurable timeout.
    pub fn read_sync(
        &mut self,
        buffers: &mut [&mut [Complex32]],
        timeout_us: i64,
    ) -> Result<usize> {
        let num_channels = buffers.len();

        if buffers.is_empty() || buffers[0].is_empty() {
            log::debug!("no buffers available, or buffers have a length of zero!");
            return Ok(0);
        }
        if num_channels > 1 {
            log::error!(
                "bladerf1 only supports reading from one RX channel. Please provide a one dimensional buffer!"
            );
            return Err(Error::Invalid);
        }

        self.reader
            .set_read_timeout(Duration::from_micros(timeout_us as u64));

        let buf = self.reader.fill_buf()?;

        let mut received = 0;
        for (dst, src) in buffers[0].iter_mut().zip(
            buf.chunks_exact(2 * size_of::<i16>())
                .map(|buf| buf.split_at(2))
                .map(|(re, im)| {
                    (
                        // i16::from_le_bytes(<[u8; 2]>::try_from(re).unwrap()) as f32 / 2047.5,
                        // i16::from_le_bytes(<[u8; 2]>::try_from(im).unwrap()) as f32 / 2047.5,
                        i16::from_le_bytes(<[u8; 2]>::try_from(re).unwrap()) as f32,
                        i16::from_le_bytes(<[u8; 2]>::try_from(im).unwrap()) as f32,
                    )
                })
                .map(|(re, im)| Complex32::new(re, im)),
        ) {
            *dst = src;
            log::trace!("{src}");
            received += 2 * size_of::<i16>();
        }

        self.reader.consume(received);
        log::trace!("consumed length: {received}");

        Ok(received / (2 * size_of::<i16>()))
    }
}

impl BladeRf1TxStreamer {
    /// Create new instance of an TX Streamer to transmit I/Q samples.
    pub fn new(
        dev: BladeRf1,
        buffer_size: usize,
        num_transfers: Option<usize>,
        timeout: Option<Duration>,
    ) -> Result<Self> {
        let endpoint = dev.interface.lock().unwrap().endpoint::<Bulk, Out>(0x01)?;
        log::trace!(
            "using endpoint 0x01 with buffer_size: {buffer_size}, num_transfers: {num_transfers:?}, timeout: {timeout:?}"
        );
        let mut writer = endpoint.writer(buffer_size);
        if let Some(t) = timeout {
            writer.set_write_timeout(t)
        }
        if let Some(n) = num_transfers {
            writer.set_num_transfers(n)
        }
        Ok(Self {
            dev,
            writer,
            buffer_size,
        })
    }

    pub fn mtu(&self) -> Result<usize> {
        Ok(self.buffer_size)
    }

    // Activate TX frontend.
    pub fn activate(&mut self) -> Result<()> {
        // self.dev.perform_format_config(BladeRfDirection::Rx, Format::Sc16Q11)
        //    ?;
        self.dev.enable_module(BLADERF_MODULE_TX, true)
        // dev.experimental_control_urb()
    }

    /// Shut down TX frontend
    pub fn deactivate(&mut self) -> Result<()> {
        //  self.dev.perform_format_deconfig(BladeRfDirection::Rx)?;
        self.dev.enable_module(BLADERF_MODULE_TX, false)
    }

    /// TODO: https://github.com/FutureSDR/seify/blob/main/src/streamer.rs#L127
    pub fn write(
        &mut self,
        buffers: &[&[Complex32]],
        at_ns: Option<i64>,
        end_burst: bool,
        timeout_us: i64,
    ) -> Result<usize> {
        // TODO: Revisit for correctness
        // https://doc.rust-lang.org/nightly/std/io/trait.Write.html#tymethod.write
        // TODO, find out how to implement write_all
        // https://doc.rust-lang.org/nightly/std/io/trait.Write.html#method.write_all
        self.writer
            .set_write_timeout(Duration::from_micros(timeout_us as u64));
        if let Some(t) = at_ns {
            sleep(Duration::from_nanos(t as u64));
        }
        let mut sent = 0;
        for (n, (re, im)) in buffers[0]
            .iter()
            .enumerate()
            // .map(|c| ((c.re * 2047.5) as i16, (c.im * 2047.5) as i16))
            .map(|(n, c)| (n, (c.re as i16, c.im as i16)))
        {
            let _ = self.writer.write(re.to_le_bytes().as_slice())?;
            let _ = self.writer.write(im.to_le_bytes().as_slice())?;
            sent = n;
        }
        if end_burst {
            self.writer.submit();
        }
        Ok(sent)
        // Ok(())
    }

    /// Get I/Q samples from a slice of buffers with configurable timeout and submit them to the BladeRF1 for transmission.
    /// A delay can be defined in the form of a timestamp when transmission should start.
    pub fn write_all(
        &mut self,
        buffers: &[&[Complex32]],
        at_ns: Option<i64>,
        end_burst: bool,
        timeout_us: i64,
    ) -> Result<()> {
        self.write(&buffers, at_ns, end_burst, timeout_us)?;
        Ok(())
    }
}

impl BladeRf1 {
    /// Perform the necessary device configuration for the specified format
    /// (e.g., enabling/disabling timestamp support), first checking that the
    /// requested format would not conflict with the other stream direction.
    ///
    /// dev: Device handle
    /// dir: Direction that is currently being configured
    /// format: Format the channel is being configured for
    ///
    /// @return 0 on success, BLADERF_ERR_* on failure
    pub fn perform_format_config(&self, dir: Direction, format: SampleFormat) -> Result<()> {
        // BladeRf1Format::PacketMeta
        // struct bladerf1_board_data *board_data = dev->board_data;

        // int status = 0;
        let mut use_timestamps: bool = false;
        let _other_using_timestamps: bool = false;

        // status = requires_timestamps(format, &use_timestamps);
        // if (status != 0) {
        //     log_debug("%s: Invalid format: %d", __FUNCTION__, format);
        //     return status;
        // }

        let _other = match dir {
            Direction::Rx => Direction::Tx,
            Direction::Tx => Direction::Rx,
        };

        // status = requires_timestamps(board_data->module_format[other],
        //     &other_using_timestamps);

        // if ((status == 0) && (other_using_timestamps != use_timestamps)) {
        //     log_debug("Format conflict detected: RX=%d, TX=%d");
        //     return BLADERF_ERR_INVAL;
        // }

        let mut gpio_val = self.config_gpio_read()?;

        log::debug!("gpio_val {gpio_val:#08x}");
        if format == SampleFormat::PacketMeta {
            gpio_val |= BLADERF_GPIO_PACKET;
            use_timestamps = true;
            log::debug!("BladeRf1Format::PacketMeta");
        } else {
            gpio_val &= !BLADERF_GPIO_PACKET;
            log::debug!("else");
        }
        log::debug!("gpio_val {gpio_val:#08x}");

        if use_timestamps {
            log::debug!("use_timestamps");
            gpio_val |= BLADERF_GPIO_TIMESTAMP | BLADERF_GPIO_TIMESTAMP_DIV2;
        } else {
            log::debug!("dont use_timestamps");
            gpio_val &= !(BLADERF_GPIO_TIMESTAMP | BLADERF_GPIO_TIMESTAMP_DIV2);
        }

        log::debug!("gpio_val {gpio_val:#08x}");

        self.config_gpio_write(gpio_val)?;
        // if (status == 0) {
        //     board_data->module_format[dir] = format;
        // }

        // return status;
        Ok(())
    }

    /// Deconfigure and update any state pertaining what a format that a stream
    /// direction is no longer using.
    ///
    ///    dev     Device handle
    ///    dir     Direction that is currently being deconfigured
    ///
    /// @return 0 on success, BLADERF_ERR_* on failure
    pub fn perform_format_deconfig(&self, direction: Direction) -> Result<()> {
        // struct bladerf1_board_data *board_data = dev->board_data;

        match direction {
            Direction::Rx | Direction::Tx => {
                // We'll reconfigure the HW when we call perform_format_config, so
                // we just need to update our stored information
                // board_data -> module_format[dir] = - 1;
            }
        }

        Ok(())
    }

    /// Investigate: This is required to set the BladeRF1 into a mode where receiving and transmitting I/Q samples is possible.
    pub fn experimental_control_urb(&self) -> Result<()> {
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
            .lock()
            .unwrap()
            .control_in(pkt, Duration::from_secs(5))
            .wait()?;
        log::debug!("Control Response Data: {vec:?}");
        Ok(())
    }

    // /// Investigate:
    // pub fn experimental_control_urb2(&self) -> Result<()> {
    //     // TODO: Dont know what this is doing
    //     let pkt = ControlIn {
    //         control_type: ControlType::Vendor,
    //         recipient: Recipient::Device,
    //         request: 0x1,
    //         value: 0x1,
    //         index: 0,
    //         length: 0x4,
    //     };
    //     let vec = self
    //         .interface
    //         .lock()
    //         .unwrap()
    //         .control_in(pkt, Duration::from_secs(5))
    //         .wait()?;
    //     log::debug!("Control Response Data: {vec:?}");
    //     Ok(())
    // }

    // pub fn write_all_sync(
    //     &mut self,
    //     buffers: &[&[Complex32]],
    //     _at_ns: Option<i64>,
    //     _end_burst: bool,
    //     timeout_us: i64,
    // ) -> Result<()> {
    //     let num_buffers = buffers.len();
    //     // This length may not be set in stone for every buffer in buffers.
    //     let buffer_size = buffers[0].len();
    //
    //     if buffers.is_empty() || buffers[0].is_empty() {
    //         return Ok(());
    //     }
    //
    //     let ep_bulk_out = self.interface.endpoint::<Bulk, Out>(0x01)?;
    //     let writer = ep_bulk_out.writer(buffer_size).with_num_transfers(num_buffers).with_write_timeout(Duration::from_micros(timeout_us as u64));
    //     for buffer in buffers {
    //
    //     }
    //
    //     Ok(())
    // }

    // pub  fn _run_stream(&self) -> Result<()> {
    //     // TODO: In_ENDPOINT is 0x81 here, not 0x82
    //     let mut ep_bulk_in = self.interface.endpoint::<Bulk, In>(0x81)?;
    //
    //     let n_transfers = 8;
    //     let factor = 32;
    //     // let factor = match self.device.speed().unwrap_or(Speed::Low) {
    //     //     // TODO: These numbers are completely made up.
    //     //     // TODO: They should be based on real USB Frame sizes depending on the given Speed
    //     //     Speed::Low => 8,
    //     //     Speed::Full => 16,
    //     //     Speed::High => 32,
    //     //     Speed::Super => 32, // This factor is used by the original libusb libbladerf implementation.
    //     //     Speed::SuperPlus => 96,
    //     //     _ => 8,
    //     // };
    //
    //     let max_packet_size = ep_bulk_in.max_packet_size();
    //     let max_frame_size = max_packet_size/// factor;
    //     log::debug!("Max Packet Size: {max_packet_size}");
    //
    //     for _i in 0..n_transfers {
    //         let buffer = ep_bulk_in.allocate(max_frame_size);
    //         ep_bulk_in.submit(buffer);
    //         // log::debug!("submitted_transfers: {i}");
    //     }
    //
    //     loop {
    //         let result = ep_bulk_in.next_complete();
    //         log::debug!("{result:?}");
    //         if result.status.is_err() {
    //             break;
    //         }
    //         ep_bulk_in.submit(result.buffer);
    //     }
    //     Ok(())
    // }

    // pub  fn bladerf1_stream(&self, stream: &bladerf_stream, layout: BladeRfChannelLayout) -> Result<()> {
    //     let dir: BladeRfDirection = layout & BLADERF_DIRECTION_MASK;
    //     let stream_status: i32;
    //
    //     // if layout != BladeRfChannelLayout::BladerfRxX1 && layout != BladeRfChannelLayout::BladerfTxX1 {
    //     //     return Err(anyhow!("Invalid ChannelLayout"));
    //     // }
    //
    //     self.perform_format_config(dir, stream->format)?;
    //
    //     stream_status = self._run_stream(stream, layout);
    //     // TODO: static void LIBUSB_CALL lusb_stream_cb
    //
    //     self.perform_format_deconfig(dir)?;
    // }
}
