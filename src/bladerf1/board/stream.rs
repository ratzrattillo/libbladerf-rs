//! BufferPool-based zero-copy streaming over nusb Bulk endpoints.
//!
//! The stream lifecycle has three phases:
//! 1. `build()` — allocates the USB endpoint and configures format GPIO bits.
//! 2. `start()` — enables the RFFE and USB streaming module, then submits
//!    buffers (RX) or begins the send/receive loop.
//! 3. `stop()` or `close()` — tears down the stream: cancels pending
//!    transfers, disables the module, drains cancelled buffers, clears
//!    halt, and deconfigures format GPIO bits.
//!
//! `RxStream` and `TxStream` own a `BufferPool` wrapping an nusb `Endpoint`
//! and a pool of reusable `Buffer` instances. No `Drop` impl is provided on
//! streams; `close()` is the only clean teardown path.

use crate::bladerf1::board::RfLinkSession;
use crate::channel::Channel;
use crate::error::{Error, Result};
use nusb::MaybeFuture;
use nusb::transfer::{Buffer, Bulk, Completion, EndpointDirection, In, Out, TransferError};
use std::collections::VecDeque;
use std::time::Duration;

/// Zero-copy buffer pool wrapping an nusb Bulk `Endpoint`.
///
/// Manages a fixed set of `Buffer` instances that are cycled between
/// available, pending (in-flight), and completed states.
pub(crate) struct BufferPool<Dir: EndpointDirection> {
    /// The underlying nusb Bulk transfer endpoint.
    endpoint: nusb::Endpoint<Bulk, Dir>,
    /// Buffers currently available for submission.
    available: VecDeque<Buffer>,
    /// Total number of buffers in the pool.
    buffer_count: usize,
    /// Size of each buffer in bytes.
    buffer_size: usize,
}

impl<Dir: EndpointDirection> BufferPool<Dir> {
    fn new(endpoint: nusb::Endpoint<Bulk, Dir>, buffer_size: usize, buffer_count: usize) -> Self {
        let mut available = VecDeque::with_capacity(buffer_count);
        for _ in 0..buffer_count {
            let buffer = endpoint.allocate(buffer_size);
            available.push_back(buffer);
        }
        Self {
            endpoint,
            available,
            buffer_count,
            buffer_size,
        }
    }

    /// Returns the size of each buffer in the pool in bytes.
    pub(crate) fn buffer_size(&self) -> usize {
        self.buffer_size
    }

    /// Returns the total number of buffers in the pool.
    pub(crate) fn buffer_count(&self) -> usize {
        self.buffer_count
    }

    fn pending(&self) -> usize {
        self.endpoint.pending()
    }

    fn submit(&mut self, buffer: Buffer) {
        self.endpoint.submit(buffer);
    }

    fn submit_all_available(&mut self) {
        while let Some(mut buffer) = self.available.pop_front() {
            buffer.set_requested_len(self.buffer_size);
            buffer.clear();
            self.endpoint.submit(buffer);
        }
    }

    fn wait_completion(&mut self, timeout: Duration) -> Option<Completion> {
        self.endpoint.wait_next_complete(timeout)
    }

    fn recycle(&mut self, mut buffer: Buffer) {
        buffer.clear();
        self.available.push_back(buffer);
    }

    fn pop_available(&mut self) -> Option<Buffer> {
        self.available.pop_front()
    }

    /// Cancels all pending transfers on the endpoint if any are in-flight.
    pub(crate) fn cancel_all(&mut self) {
        if self.endpoint.pending() > 0 {
            self.endpoint.cancel_all();
        }
    }

    /// Cancels all pending transfers and drains their completions back to the pool.
    /// Waits up to 5 seconds total for all cancellations to complete.
    pub(crate) fn drain_cancelled(&mut self) {
        self.cancel_all();
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while self.endpoint.pending() > 0 {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            let timeout = remaining.min(Duration::from_secs(1));
            if timeout.is_zero() {
                log::warn!(
                    "Timeout collecting cancelled transfers, {} remain",
                    self.endpoint.pending()
                );
                break;
            }
            if let Some(completion) = self.endpoint.wait_next_complete(timeout) {
                match completion.status {
                    Ok(()) | Err(nusb::transfer::TransferError::Cancelled) => {}
                    Err(e) => {
                        log::warn!("Transfer error during deactivation: {e}");
                    }
                }
                let mut buf = completion.buffer;
                buf.clear();
                self.available.push_back(buf);
            }
        }
    }

    /// Clears the halt condition on the endpoint.
    /// Returns an error if the clear-halt request fails.
    pub(crate) fn clear_halt(&mut self) -> Result<()> {
        self.endpoint.clear_halt().wait().map_err(Error::from)
    }

    fn pickup_tx_completed(&mut self, timeout: Duration) -> Result<()> {
        if self.endpoint.pending() == 0 {
            return Ok(());
        }
        if let Some(completion) = self.endpoint.wait_next_complete(timeout) {
            completion.status?;
            let mut buf = completion.buffer;
            buf.clear();
            self.available.push_back(buf);
        }
        Ok(())
    }

    fn drain_extras(&mut self) {
        while let Some(extra) = self.wait_completion(Duration::ZERO) {
            let mut b = extra.buffer;
            b.clear();
            b.set_requested_len(self.buffer_size);
            if extra.status.is_err() {
                self.available.push_back(b);
            } else {
                self.submit(b);
            }
        }
    }
}

/// Receive stream backed by a pool of Bulk-IN buffers.
///
/// Construct via `RxStream::builder()`. The stream follows the
/// build → start → read/recycle → close lifecycle. No `Drop`
/// teardown is performed; call `close()` for clean resource release.
pub struct RxStream {
    pool: Option<BufferPool<In>>,
}

/// Transmit stream backed by a pool of Bulk-OUT buffers.
///
/// Construct via `TxStream::builder()`. The stream follows the
/// build → start → get_buffer/submit → close lifecycle. No `Drop`
/// teardown is performed; call `close()` for clean resource release.
pub struct TxStream {
    pool: Option<BufferPool<Out>>,
}

/// I/Q sample format for streaming.
///
/// Determines the layout of sample data within transfer buffers and
/// which format GPIO bits are configured on the FPGA.
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum SampleFormat {
    /// 16-bit complex samples, 12 bits of data per I/Q component (4 bytes/sample).
    Sc16Q11 = 0,
    /// Sc16Q11 with 16-byte metadata headers prepended to each transfer.
    Sc16Q11Meta = 1,
    /// Packet-mode metadata (CMD/RSP headers) prepended to each transfer.
    PacketMeta = 2,
    /// 8-bit complex samples, 8 bits of data per I/Q component (2 bytes/sample).
    Sc8Q7 = 3,
    /// Sc8Q7 with 16-byte metadata headers prepended to each transfer.
    Sc8Q7Meta = 4,
    /// Highly-packed Sc16Q11: 12 bits per component packed at 6 bytes per 2 samples (3 bytes/sample).
    Sc16Q11Packed = 5,
}
/// GPIO bit that enables packet-mode metadata headers.
pub const BLADERF_GPIO_PACKET: u32 = 1 << 19;
/// GPIO bit that enables per-transfer timestamp metadata.
pub const BLADERF_GPIO_TIMESTAMP: u32 = 1 << 16;
/// GPIO bit that halves the timestamp counter rate.
pub const BLADERF_GPIO_TIMESTAMP_DIV2: u32 = 1 << 17;
/// GPIO bit that enables 8-bit sample mode (Sc8Q7).
pub const BLADERF_GPIO_8BIT_MODE: u32 = 1 << 20;
/// GPIO bit that enables highly-packed Sc16Q11 mode.
pub const BLADERF_GPIO_HIGHLY_PACKED_MODE: u32 = 1 << 21;

/// Size of the metadata header in bytes for *-Meta formats.
pub const METADATA_HEADER_SIZE: usize = 16;

/// Metadata header prepended to transfers using *-Meta sample formats.
///
/// Each field serves a dual purpose depending on whether the format
/// uses timestamp metadata or packet metadata.
#[repr(C, packed)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct MetadataHeader {
    reserved_or_length: u16,
    flags_or_core: u16,
    timestamp: u64,
    meta_flags: u32,
}

impl MetadataHeader {
    /// Creates a new metadata header from raw field values.
    pub fn new(
        reserved_or_length: u16,
        flags_or_core: u16,
        timestamp: u64,
        meta_flags: u32,
    ) -> Self {
        Self {
            reserved_or_length,
            flags_or_core,
            timestamp,
            meta_flags,
        }
    }

    /// Parses a `MetadataHeader` from a byte slice.
    /// Returns `None` if the slice is shorter than `METADATA_HEADER_SIZE`.
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < METADATA_HEADER_SIZE {
            return None;
        }
        Some(unsafe { std::ptr::read_unaligned(bytes.as_ptr() as *const Self) })
    }

    /// Returns the 40-bit hardware timestamp from the header.
    pub fn timestamp(&self) -> u64 {
        self.timestamp
    }

    /// Returns the metadata flags (overflow/underflow indicators).
    pub fn meta_flags(&self) -> u32 {
        self.meta_flags
    }

    /// Returns `true` if the metadata version byte matches a known format (0x00 or 0x34).
    pub fn is_valid_meta_format(&self) -> bool {
        let ver = self.flags_or_core as u8;
        ver == 0x00 || ver == 0x34
    }

    /// Returns the stream flags (high byte of `flags_or_core`).
    pub fn stream_flags(&self) -> u8 {
        (self.flags_or_core >> 8) as u8
    }

    /// Returns the metadata version (low byte of `flags_or_core`).
    pub fn meta_version(&self) -> u8 {
        (self.flags_or_core & 0xFF) as u8
    }

    /// Returns the packet length (valid for PacketMeta format).
    pub fn packet_length(&self) -> u16 {
        self.reserved_or_length
    }

    /// Returns the source core ID (high byte of `flags_or_core`, valid for PacketMeta format).
    pub fn packet_core_id(&self) -> u8 {
        (self.flags_or_core >> 8) as u8
    }

    /// Returns the packet flags (low byte of `flags_or_core`, valid for PacketMeta format).
    pub fn packet_flags(&self) -> u8 {
        self.flags_or_core as u8
    }
}

#[inline(always)]
const fn sign_extend_12(val: u16) -> i16 {
    ((val << 4) as i16) >> 4
}

impl SampleFormat {
    /// Returns the size of a single I/Q sample in bytes for this format.
    pub fn sample_size(self) -> usize {
        match self {
            Self::Sc16Q11 | Self::Sc16Q11Meta | Self::PacketMeta => 4,
            Self::Sc16Q11Packed => 3,
            Self::Sc8Q7 | Self::Sc8Q7Meta => 2,
        }
    }

    /// Unpacks Sc16Q11Packed data (3 bytes per sample) into standard Sc16Q11 (4 bytes per sample).
    /// `num_samples` must be a multiple of 2. Returns `Error::Argument` if buffers are too small.
    pub fn unpack_sc16q11_packed(src: &[u8], dst: &mut [u8], num_samples: usize) -> Result<()> {
        if !num_samples.is_multiple_of(2) {
            return Err(Error::Argument(
                "num_samples must be a multiple of 2".into(),
            ));
        }
        let src_needed = 3usize.saturating_mul(num_samples);
        let dst_needed = 4usize.saturating_mul(num_samples);
        if src.len() < src_needed {
            return Err(Error::Argument("source buffer too small".into()));
        }
        if dst.len() < dst_needed {
            return Err(Error::Argument("destination buffer too small".into()));
        }
        let pairs = num_samples / 2;
        let src_chunks = src[..src_needed].chunks_exact(6);
        let dst_chunks = dst[..dst_needed].chunks_exact_mut(8);
        for (s, d) in src_chunks.zip(dst_chunks).take(pairs) {
            let w0 = u16::from_le_bytes([s[0], s[1]]);
            let w1 = u16::from_le_bytes([s[2], s[3]]);
            let w2 = u16::from_le_bytes([s[4], s[5]]);
            let i0 = sign_extend_12(w0 & 0x0FFF);
            let q0 = sign_extend_12((w0 >> 12) | ((w1 & 0x00FF) << 4));
            let i1 = sign_extend_12((w1 >> 8) | ((w2 & 0x000F) << 8));
            let q1 = sign_extend_12(w2 >> 4);
            d[0] = i0 as u8;
            d[1] = (i0 >> 8) as u8;
            d[2] = q0 as u8;
            d[3] = (q0 >> 8) as u8;
            d[4] = i1 as u8;
            d[5] = (i1 >> 8) as u8;
            d[6] = q1 as u8;
            d[7] = (q1 >> 8) as u8;
        }
        Ok(())
    }

    /// Packs standard Sc16Q11 data (4 bytes per sample) into Sc16Q11Packed (3 bytes per sample).
    /// `num_samples` must be a multiple of 2. Returns `Error::Argument` if buffers are too small.
    pub fn pack_sc16q11_packed(src: &[u8], dst: &mut [u8], num_samples: usize) -> Result<()> {
        if !num_samples.is_multiple_of(2) {
            return Err(Error::Argument(
                "num_samples must be a multiple of 2".into(),
            ));
        }
        let src_needed = 4usize.saturating_mul(num_samples);
        let dst_needed = 3usize.saturating_mul(num_samples);
        if src.len() < src_needed {
            return Err(Error::Argument("source buffer too small".into()));
        }
        if dst.len() < dst_needed {
            return Err(Error::Argument("destination buffer too small".into()));
        }
        let pairs = num_samples / 2;
        let src_chunks = src[..src_needed].chunks_exact(8);
        let dst_chunks = dst[..dst_needed].chunks_exact_mut(6);
        for (s, d) in src_chunks.zip(dst_chunks).take(pairs) {
            let v0 = i16::from_le_bytes([s[0], s[1]]) as u16;
            let v1 = i16::from_le_bytes([s[2], s[3]]) as u16;
            let v2 = i16::from_le_bytes([s[4], s[5]]) as u16;
            let v3 = i16::from_le_bytes([s[6], s[7]]) as u16;
            let w0 = (v0 & 0x0FFF) | ((v1 & 0x000F) << 12);
            let w1 = ((v1 >> 4) & 0x00FF) | ((v2 & 0x00FF) << 8);
            let w2 = ((v2 >> 8) & 0x000F) | ((v3 & 0x0FFF) << 4);
            d[0] = w0 as u8;
            d[1] = (w0 >> 8) as u8;
            d[2] = w1 as u8;
            d[3] = (w1 >> 8) as u8;
            d[4] = w2 as u8;
            d[5] = (w2 >> 8) as u8;
        }
        Ok(())
    }
}

impl SampleFormat {
    /// Returns `true` if this format requires timestamp metadata headers.
    pub fn requires_timestamps(self) -> bool {
        matches!(
            self,
            SampleFormat::Sc16Q11Meta | SampleFormat::Sc8Q7Meta | SampleFormat::PacketMeta
        )
    }
}

impl RfLinkSession<'_> {
    /// Returns `true` if the device supports the given sample format for the specified channel.
    pub fn supports_format(&self, format: SampleFormat, direction: Channel) -> bool {
        match direction {
            Channel::Rx => matches!(
                format,
                SampleFormat::Sc8Q7Meta
                    | SampleFormat::Sc16Q11
                    | SampleFormat::Sc16Q11Meta
                    | SampleFormat::Sc16Q11Packed
                    | SampleFormat::PacketMeta
            ),
            Channel::Tx => matches!(
                format,
                SampleFormat::Sc16Q11
                    | SampleFormat::Sc16Q11Meta
                    | SampleFormat::Sc16Q11Packed
                    | SampleFormat::PacketMeta
            ),
        }
    }
}

/// Builder for configuring and constructing an `RxStream`.
pub struct RxStreamBuilder<'a, 'b> {
    dev: &'a mut RfLinkSession<'b>,
    buffer_size: usize,
    buffer_count: usize,
    format: SampleFormat,
}

impl<'a, 'b> RxStreamBuilder<'a, 'b> {
    /// Sets the buffer size in bytes. Aligned up to the endpoint's max packet size.
    pub fn buffer_size(mut self, size: usize) -> Self {
        self.buffer_size = size;
        self
    }

    pub fn buffer_count(mut self, count: usize) -> Self {
        self.buffer_count = count;
        self
    }

    pub fn format(mut self, format: SampleFormat) -> Self {
        self.format = format;
        self
    }

    /// Builds the `RxStream`. Acquires the RX streaming endpoint, configures
    /// format GPIO bits, and allocates the buffer pool.
    /// Requires the board to be initialized. Returns `Error` on USB failure.
    pub fn build(self) -> Result<RxStream> {
        self.dev.require_initialized()?;
        let endpoint = self.dev.nios.transport().acquire_streaming_rx_endpoint()?;
        let mps = endpoint.max_packet_size();
        let buffer_size = self.buffer_size.next_multiple_of(mps);
        log::trace!(
            "Creating RxStream: buffer_size={}, buffer_count={}, format={:?}",
            buffer_size,
            self.buffer_count,
            self.format
        );
        self.dev.perform_format_config(self.format)?;
        let mut pool = BufferPool::new(endpoint, buffer_size, self.buffer_count);
        pool.clear_halt()?;
        Ok(RxStream { pool: Some(pool) })
    }
}

impl RxStream {
    /// Returns a builder for constructing an `RxStream` with default parameters
    /// (64 KiB buffers, 8 buffers, Sc16Q11 format).
    pub fn builder<'a, 'b>(dev: &'a mut RfLinkSession<'b>) -> RxStreamBuilder<'a, 'b> {
        RxStreamBuilder {
            dev,
            buffer_size: 65_536,
            buffer_count: 8,
            format: SampleFormat::Sc16Q11,
        }
    }

    pub fn close(&mut self, dev: &mut RfLinkSession<'_>) -> Result<()> {
        let mut pool = self.pool.take().ok_or(Error::StreamClosed)?;
        dev.nios.stream_stopped();
        dev.close_stream(Channel::Rx, &mut pool)
    }

    /// Enables the RX streaming module and submits all buffers for incoming data.
    /// Returns `Error` if the stream is already closed or the module fails to enable.
    pub fn start(&mut self, dev: &mut RfLinkSession<'_>) -> Result<()> {
        dev.enable_module(Channel::Rx, true)?;
        dev.nios.stream_started();
        self.pool_mut()?.submit_all_available();
        log::trace!("RxStream started");
        Ok(())
    }

    pub fn stop(&mut self, dev: &mut RfLinkSession<'_>) -> Result<()> {
        let pool = self.pool_mut()?;
        dev.nios.stream_stopped();
        dev.close_stream(Channel::Rx, pool)
    }

    fn pool_mut(&mut self) -> Result<&mut BufferPool<In>> {
        self.pool.as_mut().ok_or(Error::StreamClosed)
    }

    fn pool_ref(&self) -> Result<&BufferPool<In>> {
        self.pool.as_ref().ok_or(Error::StreamClosed)
    }

    /// Waits for the next completed transfer buffer with the given timeout.
    /// Returns the filled `Buffer` or `Error::Timeout` if no buffer arrives
    /// within the timeout. `None` timeouts wait indefinitely.
    pub fn read(&mut self, timeout: Option<Duration>) -> Result<Buffer> {
        let timeout = timeout.unwrap_or(Duration::MAX);
        self.pool_mut()?.submit_all_available();
        let completion = self
            .pool_mut()?
            .wait_completion(timeout)
            .ok_or(Error::Timeout)?;
        if let Err(TransferError::Cancelled) = completion.status {
            return Err(Error::Timeout);
        }
        completion.status?;
        self.pool_mut()?.drain_extras();
        Ok(completion.buffer)
    }

    /// Attempts to retrieve a completed transfer buffer without blocking.
    /// Returns `Error::WouldBlock` if no buffer is immediately available.
    pub fn try_read(&mut self) -> Result<Buffer> {
        self.pool_mut()?.submit_all_available();
        let completion = match self.pool_mut()?.wait_completion(Duration::ZERO) {
            Some(c) => c,
            None => return Err(Error::WouldBlock),
        };
        if let Err(TransferError::Cancelled) = completion.status {
            return Err(Error::WouldBlock);
        }
        completion.status?;
        self.pool_mut()?.drain_extras();
        Ok(completion.buffer)
    }

    /// Returns the configured buffer size in bytes.
    pub fn buffer_size(&self) -> Result<usize> {
        Ok(self.pool_ref()?.buffer_size())
    }

    /// Returns the number of buffers in the pool.
    pub fn buffer_count(&self) -> Result<usize> {
        Ok(self.pool_ref()?.buffer_count())
    }

    /// Returns a used buffer to the available pool for reuse.
    pub fn recycle(&mut self, buf: Buffer) {
        if let Some(ref mut pool) = self.pool {
            pool.recycle(buf);
        }
    }
}

/// Builder for configuring and constructing a `TxStream`.
pub struct TxStreamBuilder<'a, 'b> {
    dev: &'a mut RfLinkSession<'b>,
    buffer_size: usize,
    buffer_count: usize,
    format: SampleFormat,
}

impl<'a, 'b> TxStreamBuilder<'a, 'b> {
    /// Sets the buffer size in bytes. Aligned up to the endpoint's max packet size.
    pub fn buffer_size(mut self, size: usize) -> Self {
        self.buffer_size = size;
        self
    }

    /// Sets the number of buffers in the pool.
    pub fn buffer_count(mut self, count: usize) -> Self {
        self.buffer_count = count;
        self
    }

    /// Sets the I/Q sample format.
    pub fn format(mut self, format: SampleFormat) -> Self {
        self.format = format;
        self
    }

    /// Builds the `TxStream`. Acquires the TX streaming endpoint, configures
    /// format GPIO bits, and allocates the buffer pool.
    /// Requires the board to be initialized. Returns `Error` on USB failure.
    pub fn build(self) -> Result<TxStream> {
        self.dev.require_initialized()?;
        let endpoint = self.dev.nios.transport().acquire_streaming_tx_endpoint()?;
        let mps = endpoint.max_packet_size();
        let buffer_size = self.buffer_size.next_multiple_of(mps);
        log::trace!(
            "Creating TxStream: buffer_size={}, buffer_count={}, format={:?}",
            buffer_size,
            self.buffer_count,
            self.format
        );
        self.dev.perform_format_config(self.format)?;
        let mut pool = BufferPool::new(endpoint, buffer_size, self.buffer_count);
        pool.clear_halt()?;
        Ok(TxStream { pool: Some(pool) })
    }
}

impl TxStream {
    /// Returns a builder for constructing a `TxStream` with default parameters
    /// (64 KiB buffers, 8 buffers, Sc16Q11 format).
    pub fn builder<'a, 'b>(dev: &'a mut RfLinkSession<'b>) -> TxStreamBuilder<'a, 'b> {
        TxStreamBuilder {
            dev,
            buffer_size: 65_536,
            buffer_count: 8,
            format: SampleFormat::Sc16Q11,
        }
    }

    /// Performs full stream teardown: disables the TX module, cancels pending
    /// transfers, drains them, clears halt, and deconfigures format GPIO bits.
    /// Consumes the stream pool; subsequent calls return `Error::StreamClosed`.
    pub fn close(&mut self, dev: &mut RfLinkSession<'_>) -> Result<()> {
        let mut pool = self.pool.take().ok_or(Error::StreamClosed)?;
        dev.nios.stream_stopped();
        dev.close_stream(Channel::Tx, &mut pool)
    }

    /// Enables the TX streaming module. Unlike RX, no automatic buffer submission occurs.
    /// Returns `Error` if the stream is already closed or the module fails to enable.
    pub fn start(&mut self, dev: &mut RfLinkSession<'_>) -> Result<()> {
        dev.enable_module(Channel::Tx, true)?;
        dev.nios.stream_started();
        log::trace!("TxStream started");
        Ok(())
    }

    /// Stops the TX stream: disables the module and tears down transfers,
    /// but retains the buffer pool so the stream can be restarted.
    pub fn stop(&mut self, dev: &mut RfLinkSession<'_>) -> Result<()> {
        let pool = self.pool_mut()?;
        dev.nios.stream_stopped();
        dev.close_stream(Channel::Tx, pool)
    }

    fn pool_mut(&mut self) -> Result<&mut BufferPool<Out>> {
        self.pool.as_mut().ok_or(Error::StreamClosed)
    }

    fn pool_ref(&self) -> Result<&BufferPool<Out>> {
        self.pool.as_ref().ok_or(Error::StreamClosed)
    }

    /// Gets a buffer from the pool for filling with TX data. Waits up to `timeout`
    /// for a buffer to become available (either from the pool or a completed transfer).
    /// Returns `Error::Timeout` if no buffer is available within the time limit.
    pub fn get_buffer(&mut self, timeout: Option<Duration>) -> Result<Buffer> {
        let deadline = timeout.map(|t| std::time::Instant::now() + t);
        let pool = self.pool_mut()?;
        loop {
            if let Some(buffer) = pool.pop_available() {
                return Ok(buffer);
            }
            let remaining = deadline.map_or(Duration::MAX, |d| {
                d.saturating_duration_since(std::time::Instant::now())
            });
            if remaining.is_zero() {
                return Err(Error::Timeout);
            }
            let wait = remaining.min(Duration::from_secs(1));
            if let Some(completion) = pool.wait_completion(wait) {
                completion.status?;
                let mut buf = completion.buffer;
                buf.clear();
                return Ok(buf);
            }
        }
    }

    /// Tries to get a buffer without blocking. Returns `Error::WouldBlock`
    /// if no buffer is immediately available in the pool.
    pub fn try_get_buffer(&mut self) -> Result<Buffer> {
        self.pool_mut()?.pickup_tx_completed(Duration::ZERO)?;
        match self.pool_mut()?.pop_available() {
            Some(buffer) => Ok(buffer),
            None => Err(Error::WouldBlock),
        }
    }

    /// Submits a filled buffer for transmission. `len` must not exceed the buffer size.
    /// Returns `Error::Argument` if `len` is too large.
    pub fn submit(&mut self, buf: Buffer, len: usize) -> Result<()> {
        let pool = self.pool_mut()?;
        if len > pool.buffer_size {
            return Err(Error::Argument("submit length exceeds buffer_size".into()));
        }
        pool.submit(buf);
        Ok(())
    }

    /// Waits for all pending TX transfers to complete. Recycles each
    /// completed buffer back to the pool. Returns `Error::Timeout` if
    /// pending transfers do not complete within the time limit.
    pub fn wait_completion(&mut self, timeout: Option<Duration>) -> Result<()> {
        let timeout = timeout.unwrap_or(Duration::MAX);
        let start = std::time::Instant::now();
        let pool = self.pool_mut()?;
        while pool.pending() > 0 {
            let remaining = timeout.saturating_sub(start.elapsed());
            let completion = pool
                .wait_completion(if remaining.is_zero() {
                    Duration::from_secs(1)
                } else {
                    remaining
                })
                .ok_or(Error::Timeout)?;
            completion.status?;
            let mut buf = completion.buffer;
            buf.clear();
            pool.recycle(buf);
        }
        Ok(())
    }

    /// Tries to process a completed TX transfer and return a reusable buffer without blocking.
    /// Returns `Error::WouldBlock` if no completed transfer is immediately available.
    pub fn try_get_completed(&mut self) -> Result<Buffer> {
        let pool = self.pool_mut()?;
        if pool.pending() > 0
            && let Some(completion) = pool.wait_completion(Duration::ZERO)
        {
            completion.status?;
            let mut buf = completion.buffer;
            buf.clear();
            pool.recycle(buf);
        }
        pool.pickup_tx_completed(Duration::ZERO)?;
        match pool.pop_available() {
            Some(buffer) => Ok(buffer),
            None => Err(Error::WouldBlock),
        }
    }

    /// Returns the configured buffer size in bytes.
    pub fn buffer_size(&self) -> Result<usize> {
        Ok(self.pool_ref()?.buffer_size())
    }

    /// Returns the number of buffers in the pool.
    pub fn buffer_count(&self) -> Result<usize> {
        Ok(self.pool_ref()?.buffer_count())
    }

    /// Returns a used buffer to the available pool for reuse.
    pub fn recycle(&mut self, buf: Buffer) {
        if let Some(ref mut pool) = self.pool {
            pool.recycle(buf);
        }
    }
}

impl RfLinkSession<'_> {
    /// Configures the global format GPIO bits for the given `SampleFormat`.
    /// The format GPIO bits (PACKET, TIMESTAMP, 8BIT_MODE, HIGHLY_PACKED)
    /// are global, not per-channel. Requires the board to be initialized.
    pub fn perform_format_config(&mut self, format: SampleFormat) -> Result<()> {
        self.require_initialized()?;
        let use_timestamps = format.requires_timestamps();
        self.config_gpio_modify(|gpio| {
            let mut g = if format == SampleFormat::PacketMeta {
                gpio | BLADERF_GPIO_PACKET
            } else {
                gpio & !BLADERF_GPIO_PACKET
            };
            g = if use_timestamps {
                g | BLADERF_GPIO_TIMESTAMP | BLADERF_GPIO_TIMESTAMP_DIV2
            } else {
                g & !(BLADERF_GPIO_TIMESTAMP | BLADERF_GPIO_TIMESTAMP_DIV2)
            };
            g = if matches!(format, SampleFormat::Sc8Q7 | SampleFormat::Sc8Q7Meta) {
                g | BLADERF_GPIO_8BIT_MODE
            } else {
                g & !BLADERF_GPIO_8BIT_MODE
            };
            if format == SampleFormat::Sc16Q11Packed {
                g | BLADERF_GPIO_HIGHLY_PACKED_MODE
            } else {
                g & !BLADERF_GPIO_HIGHLY_PACKED_MODE
            }
        })
    }

    /// Clears all global format GPIO bits. Requires the board to be initialized.
    pub fn perform_format_deconfig(&mut self) -> Result<()> {
        self.require_initialized()?;
        self.config_gpio_modify(|gpio| {
            gpio & !(BLADERF_GPIO_PACKET
                | BLADERF_GPIO_TIMESTAMP
                | BLADERF_GPIO_TIMESTAMP_DIV2
                | BLADERF_GPIO_8BIT_MODE
                | BLADERF_GPIO_HIGHLY_PACKED_MODE)
        })
    }
}
