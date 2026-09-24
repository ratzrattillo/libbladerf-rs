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
//!
//! The state machine lives in `StreamCore`, generic over a
//! `BulkEndpoint` and a `StreamHost`, so the lifecycle is tested with
//! mocks (see the `tests` module) and only the USB plumbing needs hardware.
//!
//! All I/O methods return [`MaybeFuture`]. The blocking path (`.wait()`)
//! honors the `timeout` arguments; the awaited path ignores them and
//! consumes at most one USB completion per await, leaving deadline handling
//! to the caller's executor. Awaited reads are cancel-safe.

use crate::bladerf1::board::RfLinkSession;
use crate::channel::Channel;
use crate::error::{Error, Result};
use crate::maybe_future::{NonWasmSend, Op};
use crate::nios_client::streams::{FORMAT_MASK, StreamClaims, StreamFormat, StreamLease};
use crate::usb::BulkEndpoint;
use crate::usb::{BladeRf1DeviceCommands, pending::Pending};
use nusb::MaybeFuture;
use nusb::transfer::{Buffer, Bulk, Completion, In, Out, TransferError};
use std::collections::VecDeque;
use std::future::{Future, IntoFuture};
use std::num::NonZeroUsize;
use std::pin::Pin;
use std::task::{Context, Poll, Waker};
use std::time::Duration;

const DRAIN_TIMEOUT: Duration = Duration::from_secs(5);

/// Device-side operations a stream needs from its session.
///
/// Implemented by [`RfLinkSession`]; the test module provides a recording
/// mock so start/stop/close ordering and stream accounting can be verified
/// without hardware.
pub(crate) trait StreamHost: NonWasmSend {
    fn claims(&mut self) -> &mut StreamClaims;
    fn require_initialized(&mut self) -> impl Future<Output = Result<()>> + NonWasmSend;
    fn enable_module(
        &mut self,
        channel: Channel,
        enable: bool,
    ) -> impl Future<Output = Result<()>> + NonWasmSend;
    fn perform_format_config(
        &mut self,
        format: StreamFormat,
    ) -> impl Future<Output = Result<()>> + NonWasmSend;
    fn perform_format_deconfig(&mut self) -> impl Future<Output = Result<()>> + NonWasmSend;
}

impl StreamHost for RfLinkSession<'_> {
    fn claims(&mut self) -> &mut StreamClaims {
        &mut self.nios.streams
    }
    fn require_initialized(&mut self) -> impl Future<Output = Result<()>> + NonWasmSend {
        RfLinkSession::require_initialized(self).into_future()
    }
    fn enable_module(
        &mut self,
        channel: Channel,
        enable: bool,
    ) -> impl Future<Output = Result<()>> + NonWasmSend {
        self.set_stream_module(channel, enable).into_future()
    }
    fn perform_format_config(
        &mut self,
        format: StreamFormat,
    ) -> impl Future<Output = Result<()>> + NonWasmSend {
        self.apply_stream_format(Some(format)).into_future()
    }
    fn perform_format_deconfig(&mut self) -> impl Future<Output = Result<()>> + NonWasmSend {
        self.apply_stream_format(None).into_future()
    }
}

#[derive(Debug, Clone, Copy)]
struct StreamConfig {
    buffer_size: NonZeroUsize,
    buffer_count: NonZeroUsize,
}

#[derive(Debug, Clone, Copy)]
enum StreamEncoding {
    Samples,
    Timestamps(MetadataLayout),
    Packets,
}

impl StreamEncoding {
    fn hardware_format(self) -> StreamFormat {
        match self {
            Self::Samples => StreamFormat::Samples,
            Self::Timestamps(_) => StreamFormat::Timestamps,
            Self::Packets => StreamFormat::Packets,
        }
    }

    fn alignment(self, packet_size: usize) -> usize {
        match self {
            Self::Timestamps(layout) => layout.message_size(),
            _ => packet_size,
        }
    }

    fn metadata_layout(self) -> Result<MetadataLayout> {
        match self {
            Self::Timestamps(layout) => Ok(layout),
            _ => Err(Error::Unsupported(
                "stream does not use fixed SC16 timestamp messages",
            )),
        }
    }

    fn validate_submission(self, bytes: &[u8]) -> Result<()> {
        match self {
            Self::Samples if !bytes.len().is_multiple_of(4) => Err(Error::Argument(
                "SC16 submissions must contain whole complex samples".into(),
            )),
            Self::Timestamps(layout) => layout.messages(bytes).map(|_| ()),
            Self::Packets => {
                let (packet, remainder) = MetadataPacket::parse_prefix(bytes)?;
                if !remainder.is_empty() || packet.payload().is_empty() {
                    return Err(Error::Argument(
                        "packet submission must match one nonempty declared payload".into(),
                    ));
                }
                Ok(())
            }
            Self::Samples => Ok(()),
        }
    }
}

impl StreamConfig {
    fn new(size: usize, count: usize, max_packet_size: usize) -> Result<Self> {
        let invalid = || Error::Argument("invalid stream buffer size or count".into());
        let buffer_count = NonZeroUsize::new(count).ok_or_else(invalid)?;
        if size == 0 || max_packet_size == 0 {
            return Err(invalid());
        }
        let size = size
            .checked_next_multiple_of(max_packet_size)
            .ok_or_else(invalid)?;
        if size > i32::MAX as usize
            || size
                .checked_mul(count)
                .is_none_or(|total| total > isize::MAX as usize)
        {
            return Err(invalid());
        }
        Ok(Self {
            buffer_size: NonZeroUsize::new(size).ok_or_else(invalid)?,
            buffer_count,
        })
    }
}

/// Zero-copy buffer pool wrapping a bulk endpoint.
///
/// Manages a fixed set of `Buffer` instances that are cycled between
/// available, pending (in-flight), and completed states.
pub(crate) struct BufferPool<E: BulkEndpoint> {
    endpoint: E,
    available: VecDeque<Buffer>,
    buffer_count: usize,
    buffer_size: usize,
    halt: Pending<()>,
}

impl<E: BulkEndpoint> BufferPool<E> {
    fn new(endpoint: E, config: StreamConfig) -> Self {
        let buffer_size = config.buffer_size.get();
        let buffer_count = config.buffer_count.get();
        let mut available = VecDeque::with_capacity(buffer_count);
        for _ in 0..buffer_count {
            available.push_back(endpoint.allocate(buffer_size));
        }
        Self {
            endpoint,
            available,
            buffer_count,
            buffer_size,
            halt: Pending::default(),
        }
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

    #[cfg(not(target_arch = "wasm32"))]
    fn wait_completion(&mut self, timeout: Duration) -> Option<Completion> {
        if self.endpoint.pending() == 0 {
            return None;
        }
        self.endpoint.wait_next_complete(timeout)
    }

    /// Returns a completion that is already available, without waiting.
    fn poll_completion(&mut self) -> Option<Completion> {
        if self.endpoint.pending() == 0 {
            return None;
        }
        let mut cx = Context::from_waker(Waker::noop());
        match self.endpoint.poll_next_complete(&mut cx) {
            Poll::Ready(completion) => Some(completion),
            Poll::Pending => None,
        }
    }

    fn poll_next(&mut self, cx: &mut Context<'_>) -> Poll<Completion> {
        self.endpoint.poll_next_complete(cx)
    }

    fn recycle(&mut self, mut buffer: Buffer) {
        buffer.clear();
        self.available.push_back(buffer);
    }

    fn pop_available(&mut self) -> Option<Buffer> {
        self.available.pop_front()
    }

    fn can_cancel(&self) -> bool {
        self.endpoint.can_cancel()
    }

    fn cancel_all(&mut self) {
        if self.endpoint.pending() > 0 {
            self.endpoint.cancel_all();
        }
    }

    /// Collects all in-flight transfers and returns their buffers to the
    /// pool. Each completion is bounded by the supplied deadline.
    ///
    /// Where cancellation is not available (WebUSB), callers drain while
    /// the module is still streaming so the transfers finish naturally.
    async fn drain(&mut self, timeout: Duration) -> Result<()> {
        while self.pending() > 0 {
            let completion = crate::maybe_future::timeout(timeout, self.endpoint.next_complete())
                .await
                .ok_or_else(|| Error::StreamDrainIncomplete {
                    pending: self.pending(),
                })?;
            self.recycle(completion.buffer);
            match completion.status {
                Ok(()) | Err(TransferError::Cancelled) => {}
                Err(error) => return Err(error.into()),
            }
        }
        Ok(())
    }

    async fn clear_halt(&mut self) -> Result<()> {
        if !self.halt.is_pending() {
            let operation = self.endpoint.clear_halt();
            self.halt
                .begin(async move { operation.await.map_err(Error::from) });
        }
        self.halt.finish(DRAIN_TIMEOUT).await.map(|_| ())
    }

    fn pickup_tx_completed(&mut self) -> Result<()> {
        if let Some(completion) = self.poll_completion() {
            self.recycle(completion.buffer);
            completion.status?;
        }
        Ok(())
    }
}

/// Direction-agnostic stream state machine shared by [`RxStream`] and
/// [`TxStream`].
///
/// `pool` is `None` once the stream is closed; `started` tracks whether
/// the RF module is enabled and the host's active-stream counter holds a
/// reference for this stream.
pub(crate) struct StreamCore<E: BulkEndpoint> {
    channel: Channel,
    encoding: StreamEncoding,
    lease: StreamLease,
    state: StreamState<E>,
}

impl<E: BulkEndpoint> std::fmt::Debug for StreamCore<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut debug = f.debug_struct("StreamCore");
        debug
            .field("channel", &self.channel)
            .field("encoding", &self.encoding);
        match &self.state {
            StreamState::Prepared(_) => {
                debug.field("state", &"prepared");
            }
            StreamState::Running(_) => {
                debug.field("state", &"running");
            }
            StreamState::Closed => {
                debug.field("state", &"closed");
            }
            StreamState::Starting { phase, .. } => {
                debug.field("start_phase", phase);
            }
            StreamState::Stopping { phase, goal, .. } => {
                debug.field("stop_phase", phase).field("goal", goal);
            }
        }
        if let Ok(pool) = self.pool_ref() {
            debug
                .field("buffer_size", &pool.buffer_size)
                .field("buffer_count", &pool.buffer_count)
                .field("available", &pool.available.len())
                .field("pending", &pool.pending());
        }
        debug.finish()
    }
}

enum StreamState<E: BulkEndpoint> {
    Prepared(BufferPool<E>),
    Starting {
        pool: BufferPool<E>,
        phase: StartPhase,
    },
    Running(BufferPool<E>),
    Stopping {
        pool: BufferPool<E>,
        phase: StopPhase,
        goal: StopGoal,
    },
    Closed,
}

#[derive(Debug, Clone, Copy)]
enum StartPhase {
    Format,
    Module,
    Submit,
}

#[derive(Debug, Clone, Copy)]
enum StopPhase {
    Cancel,
    DrainBeforeDisable,
    Module,
    Drain,
    ClearHalt,
    Format,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StopGoal {
    Prepared,
    Closed,
}

impl<E: BulkEndpoint> StreamCore<E> {
    /// Creates the pool. `buffer_size` is rounded up to the endpoint's max
    /// packet size.
    fn new(
        channel: Channel,
        encoding: StreamEncoding,
        lease: StreamLease,
        endpoint: E,
        buffer_size: usize,
        buffer_count: usize,
    ) -> Result<Self> {
        let config = StreamConfig::new(
            buffer_size,
            buffer_count,
            encoding.alignment(endpoint.max_packet_size()),
        )?;
        log::trace!(
            "Creating {channel:?} stream: buffer_size={buffer_size}, buffer_count={buffer_count}, encoding={encoding:?}"
        );
        Ok(Self {
            channel,
            encoding,
            lease,
            state: StreamState::Prepared(BufferPool::new(endpoint, config)),
        })
    }

    /// Second half of `build()`: checks the board state, configures the
    /// format GPIO bits and clears the endpoint halt.
    #[cfg(test)]
    pub(crate) async fn configure<H: StreamHost>(&mut self, host: &mut H) -> Result<()> {
        host.require_initialized().await?;
        self.pool_mut()?.clear_halt().await
    }

    fn pool_mut(&mut self) -> Result<&mut BufferPool<E>> {
        match &mut self.state {
            StreamState::Prepared(pool)
            | StreamState::Running(pool)
            | StreamState::Starting { pool, .. }
            | StreamState::Stopping { pool, .. } => Ok(pool),
            StreamState::Closed => Err(Error::StreamClosed),
        }
    }

    fn pool_ref(&self) -> Result<&BufferPool<E>> {
        match &self.state {
            StreamState::Prepared(pool)
            | StreamState::Running(pool)
            | StreamState::Starting { pool, .. }
            | StreamState::Stopping { pool, .. } => Ok(pool),
            StreamState::Closed => Err(Error::StreamClosed),
        }
    }

    fn started_pool_mut(&mut self) -> Result<&mut BufferPool<E>> {
        match &mut self.state {
            StreamState::Running(pool) => Ok(pool),
            StreamState::Prepared(_) => Err(Error::StreamNotStarted),
            StreamState::Closed => Err(Error::StreamClosed),
            _ => Err(Error::StreamTransition),
        }
    }

    pub(crate) fn buffer_size(&self) -> Result<usize> {
        Ok(self.pool_ref()?.buffer_size)
    }

    pub(crate) fn buffer_count(&self) -> Result<usize> {
        Ok(self.pool_ref()?.buffer_count)
    }

    fn pending_transfers(&self) -> Result<usize> {
        Ok(self.pool_ref()?.pending())
    }

    pub(crate) fn recycle(&mut self, buf: Buffer) {
        if let Ok(pool) = self.pool_mut() {
            pool.recycle(buf);
        }
    }

    pub(crate) fn start<'a, H: StreamHost>(
        &'a mut self,
        host: &'a mut H,
    ) -> impl MaybeFuture<Output = Result<()>> + 'a {
        Op::new(async move {
            if matches!(self.state, StreamState::Closed) {
                return Err(Error::StreamClosed);
            }
            host.claims().check(&self.lease)?;
            match self.state {
                StreamState::Prepared(_) => {
                    host.require_initialized().await?;
                    host.claims()
                        .reserve_format(&self.lease, self.encoding.hardware_format())?;
                    let StreamState::Prepared(pool) =
                        std::mem::replace(&mut self.state, StreamState::Closed)
                    else {
                        unreachable!()
                    };
                    self.state = StreamState::Starting {
                        pool,
                        phase: StartPhase::Format,
                    };
                }
                StreamState::Starting { .. } => {}
                StreamState::Running(_) => return Err(Error::StreamAlreadyStarted),
                _ => return Err(Error::StreamTransition),
            }
            loop {
                let StreamState::Starting { pool, phase } = &mut self.state else {
                    unreachable!()
                };
                match phase {
                    StartPhase::Format => {
                        host.perform_format_config(self.encoding.hardware_format())
                            .await?;
                        *phase = StartPhase::Module;
                    }
                    StartPhase::Module => {
                        host.enable_module(self.channel, true).await?;
                        *phase = StartPhase::Submit;
                    }
                    StartPhase::Submit => {
                        if self.channel.is_rx() {
                            pool.submit_all_available();
                        }
                        let StreamState::Starting { pool, .. } =
                            std::mem::replace(&mut self.state, StreamState::Closed)
                        else {
                            unreachable!()
                        };
                        self.state = StreamState::Running(pool);
                        return Ok(());
                    }
                }
            }
        })
    }

    pub(crate) fn stop<'a, H: StreamHost>(
        &'a mut self,
        host: &'a mut H,
    ) -> impl MaybeFuture<Output = Result<()>> + 'a {
        Op::new(async move {
            self.begin_teardown(host, StopGoal::Prepared)?;
            self.teardown(host, DRAIN_TIMEOUT).await
        })
    }

    pub(crate) fn close<'a, H: StreamHost>(
        &'a mut self,
        host: &'a mut H,
    ) -> impl MaybeFuture<Output = Result<()>> + 'a {
        Op::new(async move {
            self.begin_teardown(host, StopGoal::Closed)?;
            if matches!(self.state, StreamState::Closed) {
                return Ok(());
            }
            self.teardown(host, DRAIN_TIMEOUT).await
        })
    }

    fn begin_teardown<H: StreamHost>(&mut self, host: &mut H, goal: StopGoal) -> Result<()> {
        if matches!(self.state, StreamState::Closed) {
            return Err(Error::StreamClosed);
        }
        host.claims().check(&self.lease)?;
        match &mut self.state {
            StreamState::Prepared(_) if goal == StopGoal::Closed => {
                self.state = StreamState::Closed;
                host.claims().release(&self.lease);
            }
            StreamState::Prepared(_) => return Err(Error::StreamNotStarted),
            StreamState::Stopping { goal: pending, .. } => {
                if goal == StopGoal::Closed {
                    *pending = goal;
                } else if *pending == StopGoal::Closed {
                    return Err(Error::StreamTransition);
                }
            }
            _ => {
                let old = std::mem::replace(&mut self.state, StreamState::Closed);
                let pool = match old {
                    StreamState::Running(pool) | StreamState::Starting { pool, .. } => pool,
                    _ => unreachable!(),
                };
                self.state = StreamState::Stopping {
                    pool,
                    phase: StopPhase::Cancel,
                    goal,
                };
            }
        }
        Ok(())
    }

    /// Disables the module and returns the endpoint to an idle state.
    ///
    /// Where cancellation is available, in-flight transfers are cancelled
    /// before the module is disabled and the cancelled buffers are
    /// collected afterwards. WebUSB transfers cannot be cancelled and their
    /// promises never settle once the device stops streaming, so they are
    /// collected while the module is still active, then the module is
    /// disabled.
    async fn teardown<H: StreamHost>(&mut self, host: &mut H, timeout: Duration) -> Result<()> {
        loop {
            let StreamState::Stopping { pool, phase, goal } = &mut self.state else {
                unreachable!()
            };
            match phase {
                StopPhase::Cancel => {
                    *phase = if pool.can_cancel() {
                        pool.cancel_all();
                        StopPhase::Module
                    } else {
                        StopPhase::DrainBeforeDisable
                    };
                }
                StopPhase::DrainBeforeDisable => {
                    pool.drain(timeout).await?;
                    *phase = StopPhase::Module;
                }
                StopPhase::Module => {
                    host.enable_module(self.channel, false).await?;
                    *phase = StopPhase::Drain;
                }
                StopPhase::Drain => {
                    pool.drain(timeout).await?;
                    *phase = StopPhase::ClearHalt;
                }
                StopPhase::ClearHalt => {
                    pool.clear_halt().await?;
                    *phase = StopPhase::Format;
                }
                StopPhase::Format => {
                    if host.claims().last_format_user(&self.lease) {
                        host.perform_format_deconfig().await?;
                    }
                    host.claims().release_format(&self.lease);
                    let goal = *goal;
                    let StreamState::Stopping { pool, .. } =
                        std::mem::replace(&mut self.state, StreamState::Closed)
                    else {
                        unreachable!()
                    };
                    if goal == StopGoal::Prepared {
                        self.state = StreamState::Prepared(pool);
                    } else {
                        drop(pool);
                        host.claims().release(&self.lease);
                    }
                    return Ok(());
                }
            }
        }
    }

    pub(crate) fn read(&mut self, timeout: Option<Duration>) -> RxRead<'_, E> {
        RxRead {
            core: self,
            timeout,
            submitted: false,
        }
    }

    pub(crate) fn try_read(&mut self) -> Result<Buffer> {
        let pool = self.started_pool_mut()?;
        pool.submit_all_available();
        let completion = pool.poll_completion().ok_or(Error::WouldBlock)?;
        if let Err(TransferError::Cancelled) = completion.status {
            pool.recycle(completion.buffer);
            return Err(Error::WouldBlock);
        }
        if let Err(e) = completion.status {
            pool.recycle(completion.buffer);
            return Err(e.into());
        }
        Ok(completion.buffer)
    }

    pub(crate) fn get_buffer(&mut self, timeout: Option<Duration>) -> TxGetBuffer<'_, E> {
        TxGetBuffer {
            core: self,
            timeout,
        }
    }

    pub(crate) fn try_get_buffer(&mut self) -> Result<Buffer> {
        let pool = self.started_pool_mut()?;
        pool.pickup_tx_completed()?;
        pool.pop_available().ok_or(Error::WouldBlock)
    }

    pub(crate) fn submit(&mut self, buf: Buffer, len: usize) -> Result<()> {
        if let Err(error) = self.started_pool_mut() {
            self.recycle(buf);
            return Err(error);
        }
        let encoding = self.encoding;
        let pool = self.started_pool_mut()?;
        if len > pool.buffer_size {
            pool.recycle(buf);
            return Err(Error::Argument("submit length exceeds buffer_size".into()));
        }
        if len != buf.len() {
            pool.recycle(buf);
            return Err(Error::Argument(
                "submit length does not match the bytes written into the buffer".into(),
            ));
        }
        if let Err(error) = encoding.validate_submission(&buf) {
            pool.recycle(buf);
            return Err(error);
        }
        pool.submit(buf);
        Ok(())
    }

    pub(crate) fn wait_completion(&mut self, timeout: Option<Duration>) -> TxWaitCompletion<'_, E> {
        TxWaitCompletion {
            core: self,
            timeout,
        }
    }

    pub(crate) fn try_get_completed(&mut self) -> Result<Buffer> {
        let pool = self.started_pool_mut()?;
        pool.pickup_tx_completed()?;
        pool.pop_available().ok_or(Error::WouldBlock)
    }
}

/// Future returned by [`RxStream::read`].
pub(crate) struct RxRead<'a, E: BulkEndpoint> {
    core: &'a mut StreamCore<E>,
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    timeout: Option<Duration>,
    submitted: bool,
}

impl<E: BulkEndpoint> Future for RxRead<'_, E> {
    type Output = Result<Buffer>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = &mut *self;
        let pool = match this.core.started_pool_mut() {
            Ok(pool) => pool,
            Err(e) => return Poll::Ready(Err(e)),
        };
        if !this.submitted {
            pool.submit_all_available();
            this.submitted = true;
        }
        if pool.pending() == 0 {
            return Poll::Ready(Err(Error::NoTransfersInFlight));
        }
        let completion = std::task::ready!(pool.poll_next(cx));
        if let Err(e) = completion.status {
            pool.recycle(completion.buffer);
            return Poll::Ready(Err(e.into()));
        }
        Poll::Ready(Ok(completion.buffer))
    }
}

impl<E: BulkEndpoint> MaybeFuture for RxRead<'_, E> {
    #[cfg(not(target_arch = "wasm32"))]
    fn wait(self) -> Result<Buffer> {
        let timeout = self.timeout.unwrap_or(Duration::MAX);
        let pool = self.core.started_pool_mut()?;
        pool.submit_all_available();
        if pool.pending() == 0 {
            return Err(Error::NoTransfersInFlight);
        }
        let completion = pool.wait_completion(timeout).ok_or(Error::Timeout)?;
        if let Err(TransferError::Cancelled) = completion.status {
            pool.recycle(completion.buffer);
            return Err(Error::Timeout);
        }
        if let Err(e) = completion.status {
            pool.recycle(completion.buffer);
            return Err(e.into());
        }
        Ok(completion.buffer)
    }
}

/// Future returned by [`TxStream::get_buffer`].
pub(crate) struct TxGetBuffer<'a, E: BulkEndpoint> {
    core: &'a mut StreamCore<E>,
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    timeout: Option<Duration>,
}

impl<E: BulkEndpoint> Future for TxGetBuffer<'_, E> {
    type Output = Result<Buffer>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let pool = match self.core.started_pool_mut() {
            Ok(pool) => pool,
            Err(e) => return Poll::Ready(Err(e)),
        };
        if let Some(buffer) = pool.pop_available() {
            return Poll::Ready(Ok(buffer));
        }
        if pool.pending() == 0 {
            return Poll::Ready(Err(Error::NoTransfersInFlight));
        }
        let completion = std::task::ready!(pool.poll_next(cx));
        let mut buf = completion.buffer;
        buf.clear();
        match completion.status {
            Ok(()) => Poll::Ready(Ok(buf)),
            Err(e) => {
                pool.available.push_back(buf);
                Poll::Ready(Err(e.into()))
            }
        }
    }
}

impl<E: BulkEndpoint> MaybeFuture for TxGetBuffer<'_, E> {
    #[cfg(not(target_arch = "wasm32"))]
    fn wait(self) -> Result<Buffer> {
        let deadline = self.timeout.map(|t| std::time::Instant::now() + t);
        let pool = self.core.started_pool_mut()?;
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
            if pool.pending() == 0 {
                return Err(Error::NoTransfersInFlight);
            }
            let wait = remaining.min(Duration::from_secs(1));
            if let Some(completion) = pool.wait_completion(wait) {
                let mut buf = completion.buffer;
                buf.clear();
                if let Err(e) = completion.status {
                    pool.available.push_back(buf);
                    return Err(e.into());
                }
                return Ok(buf);
            }
        }
    }
}

/// Future returned by [`TxStream::wait_completion`].
pub(crate) struct TxWaitCompletion<'a, E: BulkEndpoint> {
    core: &'a mut StreamCore<E>,
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    timeout: Option<Duration>,
}

impl<E: BulkEndpoint> Future for TxWaitCompletion<'_, E> {
    type Output = Result<()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let pool = match self.core.started_pool_mut() {
            Ok(pool) => pool,
            Err(e) => return Poll::Ready(Err(e)),
        };
        while pool.pending() > 0 {
            let completion = std::task::ready!(pool.poll_next(cx));
            pool.recycle(completion.buffer);
            completion.status?;
        }
        Poll::Ready(Ok(()))
    }
}

impl<E: BulkEndpoint> MaybeFuture for TxWaitCompletion<'_, E> {
    #[cfg(not(target_arch = "wasm32"))]
    fn wait(self) -> Result<()> {
        let timeout = self.timeout.unwrap_or(Duration::MAX);
        let start = std::time::Instant::now();
        let pool = self.core.started_pool_mut()?;
        while pool.pending() > 0 {
            let remaining = timeout.saturating_sub(start.elapsed());
            if remaining.is_zero() {
                return Err(Error::Timeout);
            }
            let completion = pool.wait_completion(remaining).ok_or(Error::Timeout)?;
            pool.recycle(completion.buffer);
            completion.status?;
        }
        Ok(())
    }
}

/// Receive stream backed by a pool of Bulk-IN buffers.
///
/// Construct via `RxStream::builder()`. The stream follows the
/// build → start → read/recycle → close lifecycle. No `Drop`
/// teardown is performed; call `close()` for clean resource release.
#[derive(Debug)]
pub struct RxStream {
    core: StreamCore<nusb::Endpoint<Bulk, In>>,
}

/// Transmit stream backed by a pool of Bulk-OUT buffers.
///
/// Construct via `TxStream::builder()`. The stream follows the
/// build → start → get_buffer/submit → close lifecycle. No `Drop`
/// teardown is performed; call `close()` for clean resource release.
#[derive(Debug)]
pub struct TxStream {
    core: StreamCore<nusb::Endpoint<Bulk, Out>>,
}

pub use super::metadata::{METADATA_HEADER_SIZE, MetadataHeader};
use super::metadata::{MetadataLayout, MetadataPacket};
pub use super::sample_format::{
    BLADERF_GPIO_8BIT_MODE, BLADERF_GPIO_HIGHLY_PACKED_MODE, BLADERF_GPIO_PACKET,
    BLADERF_GPIO_TIMESTAMP, BLADERF_GPIO_TIMESTAMP_DIV2, SampleFormat,
};

impl RfLinkSession<'_> {
    /// Checks format support against the loaded FPGA and firmware.
    ///
    /// Stock BladeRF1 supports SC16, timestamped SC16, and version-gated packet metadata.
    /// Both directions share the same format capabilities.
    ///
    /// # Errors
    /// Returns USB/protocol errors when version queries fail.
    pub fn supports_format(
        &mut self,
        format: SampleFormat,
        _direction: Channel,
    ) -> impl MaybeFuture<Output = Result<bool>> {
        self.stream_encoding(format).map(|result| match result {
            Ok(_) => Ok(true),
            Err(Error::Unsupported(_)) => Ok(false),
            Err(error) => Err(error),
        })
    }

    /// Queries the fixed-message layout for timestamped SC16 on this device.
    ///
    /// # Errors
    /// Returns an unsupported error for incompatible firmware/FPGA versions,
    /// or the underlying USB/protocol error. A stream retains this layout for
    /// its protected endpoint lifetime.
    pub fn metadata_layout(&mut self) -> impl MaybeFuture<Output = Result<MetadataLayout>> {
        self.stream_encoding(SampleFormat::Sc16Q11Meta)
            .map(|result| result?.metadata_layout())
    }

    fn stream_encoding(
        &mut self,
        format: SampleFormat,
    ) -> impl MaybeFuture<Output = Result<StreamEncoding>> {
        Op::new(async move {
            let format = StreamFormat::try_from(format)?;
            if format == StreamFormat::Samples {
                return Ok(StreamEncoding::Samples);
            }
            let fpga = self.nios.nios_get_fpga_version().await?;
            let firmware = self.device.fx3_firmware_version().await?.parse()?;
            if !format.supports_versions(fpga, firmware) {
                return Err(Error::Unsupported(
                    "sample format is not supported by the loaded FPGA and firmware",
                ));
            }
            match format {
                StreamFormat::Samples => Ok(StreamEncoding::Samples),
                StreamFormat::Packets => Ok(StreamEncoding::Packets),
                StreamFormat::Timestamps => {
                    MetadataLayout::for_versions(self.nios.transport().speed(), fpga, firmware)
                        .map(StreamEncoding::Timestamps)
                }
            }
        })
    }
}

/// Builder for configuring and constructing an `RxStream`.
#[derive(Debug)]
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

    /// Builds the `RxStream`. Acquires the RX streaming endpoint, configures
    /// format GPIO bits, and allocates the buffer pool.
    /// Requires the board to be initialized. Returns `Error` on USB failure.
    pub fn build(self) -> impl MaybeFuture<Output = Result<RxStream>> {
        Op::new(async move {
            StreamConfig::new(self.buffer_size, self.buffer_count, 1)?;
            self.dev.nios.streams.require_unclaimed(Channel::Rx)?;
            self.dev.require_initialized().await?;
            let encoding = self.dev.stream_encoding(self.format).await?;
            let endpoint = self
                .dev
                .nios
                .control()
                .await?
                .acquire_streaming_rx_endpoint()
                .await?;
            let lease = self.dev.nios.streams.claim(Channel::Rx)?;
            let core = StreamCore::new(
                Channel::Rx,
                encoding,
                lease,
                endpoint,
                self.buffer_size,
                self.buffer_count,
            )?;
            Ok(RxStream { core })
        })
    }
}

impl RxStream {
    /// Counts submitted transfers whose completions have not yet been collected.
    ///
    /// During a WebUSB drain, provide enough source data for these existing
    /// reads before retrying stop/close. Each read requests [`Self::buffer_size`]
    /// bytes. No replacement reads are submitted during teardown.
    ///
    /// # Errors
    /// Returns [`Error::StreamClosed`] after close.
    pub fn pending_transfers(&self) -> Result<usize> {
        self.core.pending_transfers()
    }

    /// Returns the validated timestamp-message layout captured when the stream was built.
    ///
    /// # Errors
    /// Returns an unsupported error for plain/packet streams, or
    /// [`Error::StreamClosed`] after close.
    pub fn metadata_layout(&self) -> Result<MetadataLayout> {
        self.core.pool_ref()?;
        self.core.encoding.metadata_layout()
    }

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

    /// Closes RX and releases its endpoint after all transfers have been collected.
    ///
    /// Native backends cancel before disabling; WebUSB drains before disabling.
    /// Keep a trigger or firmware-loopback TX source available until RX drains.
    /// A failed/cancelled close retains the pool and remaining cleanup work.
    /// Shared format bits remain set while a compatible peer uses them.
    ///
    /// # Errors
    /// Returns [`Error::StreamDrainIncomplete`] if a completion takes over five
    /// seconds. Restore the source and retry this method with the same session.
    /// Propagates USB errors and rejects wrong-device sessions. After success,
    /// subsequent calls return [`Error::StreamClosed`].
    pub fn close<'a>(
        &'a mut self,
        dev: &'a mut RfLinkSession<'_>,
    ) -> impl MaybeFuture<Output = Result<()>> + 'a {
        self.core.close(dev)
    }

    /// Enables the RX streaming module and submits all buffers for incoming data.
    /// Returns `Error` if the stream is closed, already started, or the module
    /// fails to enable.
    pub fn start<'a>(
        &'a mut self,
        dev: &'a mut RfLinkSession<'_>,
    ) -> impl MaybeFuture<Output = Result<()>> + 'a {
        self.core.start(dev)
    }

    /// Stops RX, retaining its endpoint claim and buffer pool for restart.
    ///
    /// Uses the same resumable teardown and source requirements as [`Self::close`].
    /// Flash/Config sessions remain unavailable until the stream is closed.
    ///
    /// # Errors
    /// Returns [`Error::StreamDrainIncomplete`] for an unfinished drain and
    /// propagates teardown errors. Keep the stream and retry after recovery.
    pub fn stop<'a>(
        &'a mut self,
        dev: &'a mut RfLinkSession<'_>,
    ) -> impl MaybeFuture<Output = Result<()>> + 'a {
        self.core.stop(dev)
    }

    /// Waits for the next completed transfer buffer.
    ///
    /// Blocking (`.wait()`): returns the filled `Buffer` or `Error::Timeout`
    /// if no buffer arrives within `timeout`; `None` waits indefinitely.
    /// Awaited: `timeout` is ignored; the future resolves with the next
    /// completion and is cancel-safe. The stream must be started.
    pub fn read(&mut self, timeout: Option<Duration>) -> impl MaybeFuture<Output = Result<Buffer>> {
        self.core.read(timeout)
    }

    /// Attempts to retrieve a completed transfer buffer without blocking.
    /// Returns `Error::WouldBlock` if no buffer is immediately available.
    pub fn try_read(&mut self) -> Result<Buffer> {
        self.core.try_read()
    }

    /// Returns the configured buffer size in bytes.
    pub fn buffer_size(&self) -> Result<usize> {
        self.core.buffer_size()
    }

    /// Returns the number of buffers in the pool.
    pub fn buffer_count(&self) -> Result<usize> {
        self.core.buffer_count()
    }

    /// Returns a used buffer to the available pool for reuse.
    pub fn recycle(&mut self, buf: Buffer) {
        self.core.recycle(buf);
    }
}

/// Builder for configuring and constructing a `TxStream`.
#[derive(Debug)]
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
    pub fn build(self) -> impl MaybeFuture<Output = Result<TxStream>> {
        Op::new(async move {
            StreamConfig::new(self.buffer_size, self.buffer_count, 1)?;
            self.dev.nios.streams.require_unclaimed(Channel::Tx)?;
            self.dev.require_initialized().await?;
            let encoding = self.dev.stream_encoding(self.format).await?;
            let endpoint = self
                .dev
                .nios
                .control()
                .await?
                .acquire_streaming_tx_endpoint()
                .await?;
            let lease = self.dev.nios.streams.claim(Channel::Tx)?;
            let core = StreamCore::new(
                Channel::Tx,
                encoding,
                lease,
                endpoint,
                self.buffer_size,
                self.buffer_count,
            )?;
            Ok(TxStream { core })
        })
    }
}

impl TxStream {
    /// Counts submitted transfers whose completions have not yet been collected.
    ///
    /// # Errors
    /// Returns [`Error::StreamClosed`] after close.
    pub fn pending_transfers(&self) -> Result<usize> {
        self.core.pending_transfers()
    }

    /// Returns the validated timestamp-message layout captured when the stream was built.
    ///
    /// # Errors
    /// Returns an unsupported error for plain/packet streams, or
    /// [`Error::StreamClosed`] after close.
    pub fn metadata_layout(&self) -> Result<MetadataLayout> {
        self.core.pool_ref()?;
        self.core.encoding.metadata_layout()
    }

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
    pub fn close<'a>(
        &'a mut self,
        dev: &'a mut RfLinkSession<'_>,
    ) -> impl MaybeFuture<Output = Result<()>> + 'a {
        self.core.close(dev)
    }

    /// Enables the TX streaming module. Unlike RX, no automatic buffer submission occurs.
    /// Returns `Error` if the stream is closed, already started, or the module
    /// fails to enable.
    pub fn start<'a>(
        &'a mut self,
        dev: &'a mut RfLinkSession<'_>,
    ) -> impl MaybeFuture<Output = Result<()>> + 'a {
        self.core.start(dev)
    }

    /// Stops the TX stream: disables the module and tears down transfers,
    /// but retains the buffer pool so the stream can be restarted.
    pub fn stop<'a>(
        &'a mut self,
        dev: &'a mut RfLinkSession<'_>,
    ) -> impl MaybeFuture<Output = Result<()>> + 'a {
        self.core.stop(dev)
    }

    /// Gets a buffer from the pool for filling with TX data.
    ///
    /// Blocking (`.wait()`): waits up to `timeout` for a buffer to become
    /// available (from the pool or a completed transfer) and returns
    /// `Error::Timeout` otherwise. Awaited: `timeout` is ignored; resolves
    /// as soon as a buffer is available or the next transfer completes.
    /// The stream must be started.
    pub fn get_buffer(
        &mut self,
        timeout: Option<Duration>,
    ) -> impl MaybeFuture<Output = Result<Buffer>> {
        self.core.get_buffer(timeout)
    }

    /// Tries to get a buffer without blocking. Returns `Error::WouldBlock`
    /// if no buffer is immediately available in the pool.
    pub fn try_get_buffer(&mut self) -> Result<Buffer> {
        self.core.try_get_buffer()
    }

    /// Submits a filled buffer for transmission.
    ///
    /// Exactly `buf.len()` bytes are sent, so `len` must equal the number of
    /// bytes written into `buf` and must not exceed the buffer size. Returns
    /// `Error::Argument` otherwise and returns the buffer to the pool.
    pub fn submit(&mut self, buf: Buffer, len: usize) -> Result<()> {
        self.core.submit(buf, len)
    }

    /// Waits for all pending TX transfers to complete, recycling each
    /// completed buffer back to the pool.
    ///
    /// Blocking (`.wait()`): returns `Error::Timeout` if the transfers do
    /// not complete within `timeout`. Awaited: `timeout` is ignored.
    pub fn wait_completion(
        &mut self,
        timeout: Option<Duration>,
    ) -> impl MaybeFuture<Output = Result<()>> {
        self.core.wait_completion(timeout)
    }

    /// Tries to process a completed TX transfer and return a reusable buffer without blocking.
    /// Returns `Error::WouldBlock` if no completed transfer is immediately available.
    pub fn try_get_completed(&mut self) -> Result<Buffer> {
        self.core.try_get_completed()
    }

    /// Returns the configured buffer size in bytes.
    pub fn buffer_size(&self) -> Result<usize> {
        self.core.buffer_size()
    }

    /// Returns the number of buffers in the pool.
    pub fn buffer_count(&self) -> Result<usize> {
        self.core.buffer_count()
    }

    /// Returns a used buffer to the available pool for reuse.
    pub fn recycle(&mut self, buf: Buffer) {
        self.core.recycle(buf);
    }
}

impl RfLinkSession<'_> {
    /// Configures the global format GPIO bits for the given `SampleFormat`.
    /// The format GPIO bits (PACKET, TIMESTAMP, 8BIT_MODE, HIGHLY_PACKED)
    /// are global, not per-channel. Requires the board to be initialized.
    pub fn perform_format_config(
        &mut self,
        format: SampleFormat,
    ) -> impl MaybeFuture<Output = Result<()>> {
        Op::new(async move {
            self.nios.streams.require_idle()?;
            self.require_initialized().await?;
            if !self.supports_format(format, Channel::Rx).await? {
                return Err(Error::Unsupported(
                    "sample format is not supported by the loaded FPGA and firmware",
                ));
            }
            self.apply_stream_format(Some(StreamFormat::try_from(format)?))
                .await
        })
    }

    /// Clears all global format GPIO bits. Requires the board to be initialized.
    pub fn perform_format_deconfig(&mut self) -> impl MaybeFuture<Output = Result<()>> {
        Op::new(async move {
            self.nios.streams.require_idle()?;
            self.require_initialized().await?;
            self.apply_stream_format(None).await
        })
    }

    fn apply_stream_format(
        &mut self,
        format: Option<StreamFormat>,
    ) -> impl MaybeFuture<Output = Result<()>> {
        self.nios.nios_config_modify(move |gpio| {
            (gpio & !FORMAT_MASK) | format.map_or(0, StreamFormat::bits)
        })
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use crate::error::ErrorKind;
    use crate::maybe_future::block_on;
    use std::sync::{Arc, Mutex};

    const MPS: usize = 512;
    const BUFFERS: usize = 4;

    fn timestamp_encoding() -> StreamEncoding {
        StreamEncoding::Timestamps(
            MetadataLayout::for_versions(
                nusb::Speed::Super,
                crate::SemanticVersion::new(0, 16, 0),
                crate::SemanticVersion::new(2, 5, 0),
            )
            .unwrap(),
        )
    }

    type Log = Arc<Mutex<Vec<String>>>;

    struct MockState {
        pending: VecDeque<(Buffer, Option<std::result::Result<(), TransferError>>)>,
        auto_complete: bool,
        cancellable: bool,
        fill: usize,
        clear_halts: usize,
        sequence: u64,
        halt_ready: bool,
    }

    #[derive(Clone)]
    struct MockHandle(Arc<Mutex<MockState>>);

    impl MockHandle {
        fn new(auto_complete: bool) -> Self {
            Self(Arc::new(Mutex::new(MockState {
                pending: VecDeque::new(),
                auto_complete,
                cancellable: true,
                fill: MPS,
                clear_halts: 0,
                sequence: 0,
                halt_ready: true,
            })))
        }
        fn cancellable(&self) -> bool {
            self.0.lock().unwrap().cancellable
        }
        fn set_cancellable(&self, cancellable: bool) {
            self.0.lock().unwrap().cancellable = cancellable;
        }
        fn endpoint(&self, log: &Log) -> MockEndpoint {
            MockEndpoint {
                state: self.clone(),
                log: Arc::clone(log),
            }
        }
        fn pending(&self) -> usize {
            self.0.lock().unwrap().pending.len()
        }
        fn clear_halts(&self) -> usize {
            self.0.lock().unwrap().clear_halts
        }
        fn complete_next(&self, status: std::result::Result<(), TransferError>) {
            let mut st = self.0.lock().unwrap();
            let slot = st
                .pending
                .iter_mut()
                .find(|slot| slot.1.is_none())
                .expect("nothing pending");
            slot.1 = Some(status);
        }
    }

    struct MockEndpoint {
        state: MockHandle,
        log: Log,
    }

    impl MockEndpoint {
        fn take_ready(&mut self) -> Option<Completion> {
            let mut st = self.state.0.lock().unwrap();
            let fill = st.fill;
            match st.pending.front() {
                Some((_, Some(_))) => {
                    let (mut buffer, status) = st.pending.pop_front().unwrap();
                    let status = status.unwrap();
                    self.log.lock().unwrap().push("complete".into());
                    if status.is_ok() {
                        buffer.clear();
                        buffer.extend_fill(fill.min(buffer.capacity()), 0xAB);
                        buffer[..8].copy_from_slice(&st.sequence.to_le_bytes());
                        st.sequence += 1;
                    }
                    Some(Completion {
                        actual_len: buffer.len(),
                        buffer,
                        status,
                    })
                }
                _ => None,
            }
        }
    }

    impl BulkEndpoint for MockEndpoint {
        fn max_packet_size(&self) -> usize {
            MPS
        }
        fn allocate(&self, len: usize) -> Buffer {
            Buffer::new(len)
        }
        fn submit(&mut self, buffer: Buffer) {
            let mut st = self.state.0.lock().unwrap();
            let done = st.auto_complete.then_some(Ok(()));
            st.pending.push_back((buffer, done));
            self.log.lock().unwrap().push("submit".into());
        }
        fn pending(&self) -> usize {
            self.state.pending()
        }
        fn poll_next_complete(&mut self, _cx: &mut Context<'_>) -> Poll<Completion> {
            assert!(
                self.pending() > 0,
                "poll_next_complete with nothing pending"
            );
            match self.take_ready() {
                Some(c) => Poll::Ready(c),
                None => Poll::Pending,
            }
        }
        fn wait_next_complete(&mut self, _timeout: Duration) -> Option<Completion> {
            assert!(
                self.pending() > 0,
                "wait_next_complete with nothing pending"
            );
            self.take_ready()
        }
        fn can_cancel(&self) -> bool {
            self.state.cancellable()
        }
        fn cancel_all(&mut self) {
            if self.state.cancellable() {
                let mut st = self.state.0.lock().unwrap();
                for slot in st.pending.iter_mut() {
                    if slot.1.is_none() {
                        slot.1 = Some(Err(TransferError::Cancelled));
                    }
                }
                self.log.lock().unwrap().push("cancel_all".into());
            }
        }
        fn clear_halt(
            &mut self,
        ) -> impl MaybeFuture<Output = std::result::Result<(), nusb::Error>> + 'static {
            self.state.0.lock().unwrap().clear_halts += 1;
            self.log.lock().unwrap().push("clear_halt".into());
            let state = self.state.clone();
            Op::new(std::future::poll_fn(move |_| {
                if state.0.lock().unwrap().halt_ready {
                    Poll::Ready(Ok(()))
                } else {
                    Poll::Pending
                }
            }))
        }
    }

    struct MockHost {
        log: Log,
        claims: StreamClaims,
        initialized: bool,
        module: [bool; 2],
        format: Option<StreamFormat>,
        fail_enable: bool,
        pause: Option<&'static str>,
    }

    impl MockHost {
        fn new(log: &Log) -> Self {
            Self {
                log: Arc::clone(log),
                claims: StreamClaims::default(),
                initialized: true,
                module: [false, false],
                format: None,
                fail_enable: false,
                pause: None,
            }
        }
        fn module(&self, channel: Channel) -> bool {
            self.module[channel as u8 as usize]
        }
    }

    impl StreamHost for MockHost {
        fn claims(&mut self) -> &mut StreamClaims {
            &mut self.claims
        }
        async fn require_initialized(&mut self) -> Result<()> {
            if self.initialized {
                Ok(())
            } else {
                Err(Error::NotInitialized)
            }
        }
        async fn enable_module(&mut self, channel: Channel, enable: bool) -> Result<()> {
            self.log
                .lock()
                .unwrap()
                .push(format!("enable({channel:?},{enable})"));
            if self.pause == Some(if enable { "enable" } else { "disable" }) {
                std::future::pending::<()>().await;
            }
            if self.fail_enable && enable {
                return Err(Error::Timeout);
            }
            self.module[channel as u8 as usize] = enable;
            Ok(())
        }
        async fn perform_format_config(&mut self, format: StreamFormat) -> Result<()> {
            self.log.lock().unwrap().push("config".into());
            if self.pause == Some("config") {
                std::future::pending::<()>().await;
            }
            self.format = Some(format);
            Ok(())
        }
        async fn perform_format_deconfig(&mut self) -> Result<()> {
            self.log.lock().unwrap().push("deconfig".into());
            if self.pause == Some("deconfig") {
                std::future::pending::<()>().await;
            }
            self.format = None;
            Ok(())
        }
    }

    struct Fixture {
        log: Log,
        ep: MockHandle,
        host: MockHost,
        core: StreamCore<MockEndpoint>,
    }

    impl Fixture {
        fn new(channel: Channel, auto_complete: bool) -> Self {
            let log: Log = Arc::default();
            let ep = MockHandle::new(auto_complete);
            let mut host = MockHost::new(&log);
            let mut core = StreamCore::new(
                channel,
                StreamEncoding::Samples,
                host.claims.claim(channel).unwrap(),
                ep.endpoint(&log),
                MPS * 3 + 1,
                BUFFERS,
            )
            .unwrap();
            block_on(core.configure(&mut host)).unwrap();
            log.lock().unwrap().clear();
            Self {
                log,
                ep,
                host,
                core,
            }
        }
        fn rx() -> Self {
            Self::new(Channel::Rx, true)
        }
        fn tx() -> Self {
            Self::new(Channel::Tx, true)
        }
        fn log(&self) -> Vec<String> {
            self.log.lock().unwrap().clone()
        }
        fn available(&self) -> usize {
            self.core.pool_ref().map_or(0, |p| p.available.len())
        }
        /// available + in flight + held by the caller == buffer_count.
        fn assert_pool_invariant(&self, held: usize) {
            if self.core.pool_ref().is_ok() {
                assert_eq!(
                    self.available() + self.ep.pending() + held,
                    BUFFERS,
                    "pool invariant violated (available={}, pending={}, held={held})",
                    self.available(),
                    self.ep.pending()
                );
            }
        }
    }

    fn is_kind(e: &Error, kind: ErrorKind) -> bool {
        e.kind() == kind
    }

    #[test]
    fn stream_configuration_rejects_invalid_sizes_before_allocating() {
        for (size, count, mps) in [
            (0, 4, 512),
            (512, 0, 512),
            (512, 4, 0),
            (usize::MAX, 4, 512),
            (512, usize::MAX, 512),
            (i32::MAX as usize, 2, 512),
        ] {
            assert!(StreamConfig::new(size, count, mps).is_err());
        }
        let config = StreamConfig::new(513, 4, 512).unwrap();
        assert_eq!(config.buffer_size.get(), 1024);
    }

    #[test]
    fn cancelled_start_and_teardown_resume_each_hardware_checkpoint() {
        for pause in ["config", "enable", "disable", "deconfig", "halt"] {
            let mut f = Fixture::new(Channel::Rx, false);
            let starting = matches!(pause, "config" | "enable");
            if !starting {
                f.core.start(&mut f.host).wait().unwrap();
            }
            f.host.pause = Some(pause);
            if pause == "halt" {
                f.ep.0.lock().unwrap().halt_ready = false;
            }
            let halts = f.ep.clear_halts();
            {
                let mut operation: Pin<Box<dyn Future<Output = Result<()>> + '_>> = if starting {
                    Box::pin(f.core.start(&mut f.host).into_future())
                } else {
                    Box::pin(f.core.close(&mut f.host).into_future())
                };
                assert!(
                    operation
                        .as_mut()
                        .poll(&mut Context::from_waker(Waker::noop()))
                        .is_pending()
                );
            }
            f.assert_pool_invariant(0);
            assert!(matches!(f.core.try_read(), Err(Error::StreamTransition)));
            assert!(matches!(
                f.host.claims.require_idle(),
                Err(Error::StreamsActive)
            ));
            f.host.pause = None;
            f.ep.0.lock().unwrap().halt_ready = true;
            if starting {
                f.core.start(&mut f.host).wait().unwrap();
            }
            f.core.close(&mut f.host).wait().unwrap();
            if pause == "halt" {
                assert_eq!(f.ep.clear_halts(), halts + 1);
            }
            assert!(f.host.claims.require_idle().is_ok());
        }
    }

    #[test]
    fn interrupted_noncancellable_drain_returns_each_collected_buffer_to_the_pool() {
        let mut f = Fixture::new(Channel::Rx, false);
        f.ep.set_cancellable(false);
        f.core.start(&mut f.host).wait().unwrap();
        f.ep.complete_next(Ok(()));
        {
            let mut operation = std::pin::pin!(f.core.stop(&mut f.host).into_future());
            assert!(
                operation
                    .as_mut()
                    .poll(&mut Context::from_waker(Waker::noop()))
                    .is_pending()
            );
        }
        f.assert_pool_invariant(0);
        assert_eq!(f.available(), 1);
        assert!(f.host.module(Channel::Rx));
        for _ in 1..BUFFERS {
            f.ep.complete_next(Ok(()));
        }
        f.core.stop(&mut f.host).wait().unwrap();
        f.assert_pool_invariant(0);
        assert!(matches!(
            f.host.claims.require_idle(),
            Err(Error::StreamsActive)
        ));
        f.core.close(&mut f.host).wait().unwrap();
    }

    #[test]
    fn source_limited_drain_times_out_without_disabling_or_losing_ownership() {
        for goal in [StopGoal::Prepared, StopGoal::Closed] {
            let mut f = Fixture::new(Channel::Rx, false);
            f.ep.set_cancellable(false);
            f.core.start(&mut f.host).wait().unwrap();
            f.core.begin_teardown(&mut f.host, goal).unwrap();
            for completed in 0..BUFFERS {
                let error = block_on(f.core.teardown(&mut f.host, Duration::ZERO)).unwrap_err();
                assert!(
                    matches!(error, Error::StreamDrainIncomplete { pending } if pending == BUFFERS - completed)
                );
                assert_eq!(error.kind(), ErrorKind::Timeout);
                assert_eq!(f.available(), completed);
                f.assert_pool_invariant(0);
                assert!(f.host.module(Channel::Rx));
                assert_eq!(f.host.claims.format_users(), 1);
                assert!(matches!(
                    f.host.claims.require_idle(),
                    Err(Error::StreamsActive)
                ));
                assert!(matches!(f.core.try_read(), Err(Error::StreamTransition)));
                assert!(
                    !f.log()
                        .iter()
                        .any(|operation| operation == "cancel" || operation == "deconfig")
                );
                f.ep.complete_next(Ok(()));
            }
            block_on(f.core.teardown(&mut f.host, Duration::ZERO)).unwrap();
            assert_eq!(f.ep.pending(), 0);
            assert!(!f.host.module(Channel::Rx));
            assert_eq!(f.host.claims.format_users(), 0);
            if goal == StopGoal::Prepared {
                f.assert_pool_invariant(0);
                f.core.close(&mut f.host).wait().unwrap();
            }
            assert!(f.host.claims.require_idle().is_ok());
        }
    }

    #[test]
    fn failed_noncancellable_drain_returns_the_failed_buffer_and_resumes_the_rest() {
        let mut f = Fixture::new(Channel::Rx, false);
        f.ep.set_cancellable(false);
        f.core.start(&mut f.host).wait().unwrap();
        f.ep.complete_next(Err(TransferError::Disconnected));
        assert!(matches!(
            f.core.close(&mut f.host).wait(),
            Err(Error::Transfer(TransferError::Disconnected))
        ));
        assert_eq!(f.available(), 1);
        assert_eq!(f.core.pending_transfers().unwrap(), BUFFERS - 1);
        assert!(f.host.module(Channel::Rx));
        f.assert_pool_invariant(0);
        for _ in 1..BUFFERS {
            f.ep.complete_next(Ok(()));
        }
        f.core.close(&mut f.host).wait().unwrap();
        assert!(f.host.claims.require_idle().is_ok());
    }

    #[test]
    fn duplex_formats_survive_either_close_order_and_reject_incompatible_start() {
        for close_rx_first in [false, true] {
            let mut f = Fixture::rx();
            f.core.encoding = timestamp_encoding();
            let tx_ep = MockHandle::new(true);
            let mut tx = StreamCore::new(
                Channel::Tx,
                StreamEncoding::Packets,
                f.host.claims.claim(Channel::Tx).unwrap(),
                tx_ep.endpoint(&f.log),
                MPS,
                BUFFERS,
            )
            .unwrap();
            f.core.start(&mut f.host).wait().unwrap();
            assert!(matches!(
                tx.start(&mut f.host).wait(),
                Err(Error::IncompatibleStreamFormat)
            ));
            tx.encoding = timestamp_encoding();
            tx.start(&mut f.host).wait().unwrap();
            assert_eq!(f.host.claims.format_users(), 2);
            if close_rx_first {
                f.core.close(&mut f.host).wait().unwrap();
            } else {
                tx.close(&mut f.host).wait().unwrap();
            }
            assert_eq!(f.host.format, Some(StreamFormat::Timestamps));
            assert_eq!(f.host.claims.format_users(), 1);
            if close_rx_first {
                tx.close(&mut f.host).wait().unwrap();
            } else {
                let buf = f.core.read(None).wait().unwrap();
                f.core.recycle(buf);
                f.core.close(&mut f.host).wait().unwrap();
            }
            assert_eq!(f.host.format, None);
        }
    }

    #[test]
    fn wrong_device_is_rejected_before_any_module_or_format_change() {
        let mut f = Fixture::rx();
        let mut other = MockHost::new(&f.log);
        assert!(matches!(
            f.core.start(&mut other).wait(),
            Err(Error::WrongDevice)
        ));
        assert!(matches!(
            f.core.close(&mut other).wait(),
            Err(Error::WrongDevice)
        ));
        assert!(f.log().is_empty());
        f.core.close(&mut f.host).wait().unwrap();
    }

    #[test]
    fn stopped_tx_submission_returns_held_buffer_before_error() {
        let mut f = Fixture::tx();
        f.core.start(&mut f.host).wait().unwrap();
        let buffer = f.core.get_buffer(None).wait().unwrap();
        f.core.stop(&mut f.host).wait().unwrap();
        assert!(matches!(
            f.core.submit(buffer, 0),
            Err(Error::StreamNotStarted)
        ));
        f.assert_pool_invariant(0);
    }

    #[test]
    fn buffer_size_rounds_up_to_max_packet_size() {
        let f = Fixture::rx();
        assert_eq!(f.core.buffer_size().unwrap(), MPS * 4);
        assert_eq!(f.core.buffer_count().unwrap(), BUFFERS);
        assert_eq!(f.host.format, None);
        assert_eq!(f.ep.clear_halts(), 1);
    }

    #[test]
    fn timestamp_buffers_align_to_messages_and_rejected_tx_payloads_return_to_the_pool() {
        let log: Log = Arc::default();
        let ep = MockHandle::new(false);
        let mut host = MockHost::new(&log);
        let mut core = StreamCore::new(
            Channel::Tx,
            timestamp_encoding(),
            host.claims.claim(Channel::Tx).unwrap(),
            ep.endpoint(&log),
            8193,
            2,
        )
        .unwrap();
        assert_eq!(core.buffer_size().unwrap(), 16_384);
        core.start(&mut host).wait().unwrap();
        let mut buffer = core.get_buffer(None).wait().unwrap();
        buffer.extend_fill(8191, 0);
        assert!(matches!(
            core.submit(buffer, 8191),
            Err(Error::MetadataLength { .. })
        ));
        assert_eq!(core.pool_ref().unwrap().available.len(), 2);
        assert_eq!(ep.pending(), 0);
        let mut buffer = core.get_buffer(None).wait().unwrap();
        buffer.extend_fill(8192, 0);
        core.submit(buffer, 8192).unwrap();
        assert_eq!(ep.pending(), 1);
        core.close(&mut host).wait().unwrap();
    }

    #[test]
    fn configure_requires_initialized_board() {
        let log: Log = Arc::default();
        let ep = MockHandle::new(true);
        let mut host = MockHost::new(&log);
        host.initialized = false;
        let mut core = StreamCore::new(
            Channel::Rx,
            StreamEncoding::Samples,
            host.claims.claim(Channel::Rx).unwrap(),
            ep.endpoint(&log),
            MPS,
            2,
        )
        .unwrap();
        assert!(matches!(
            block_on(core.configure(&mut host)),
            Err(Error::NotInitialized)
        ));
        assert!(host.format.is_none());
    }

    #[test]
    fn rx_start_read_stop_restart_close() {
        let mut f = Fixture::rx();
        f.core.start(&mut f.host).wait().unwrap();
        assert!(f.host.module(Channel::Rx));
        assert_eq!(f.host.claims.format_users(), 1);
        assert_eq!(f.ep.pending(), BUFFERS, "RX start submits every buffer");

        let buf = f.core.read(None).wait().unwrap();
        assert_eq!(buf.len(), MPS);
        f.assert_pool_invariant(1);
        f.core.recycle(buf);
        f.assert_pool_invariant(0);

        f.core.stop(&mut f.host).wait().unwrap();
        assert!(!f.host.module(Channel::Rx));
        assert_eq!(f.host.claims.format_users(), 0);
        assert_eq!(f.ep.pending(), 0);
        f.assert_pool_invariant(0);

        f.core.start(&mut f.host).wait().unwrap();
        let buf = f.core.read(None).wait().unwrap();
        f.core.recycle(buf);
        f.core.close(&mut f.host).wait().unwrap();
        assert_eq!(
            f.host.claims.format_users(),
            0,
            "stop followed by close must not underflow"
        );
        assert!(matches!(
            f.core.close(&mut f.host).wait(),
            Err(Error::StreamClosed)
        ));
    }

    #[test]
    fn prepared_close_releases_claim_without_touching_hardware() {
        let mut f = Fixture::rx();
        f.core.close(&mut f.host).wait().unwrap();
        assert_eq!(f.host.claims.format_users(), 0);
        assert!(f.host.format.is_none());
        assert!(f.log().is_empty());
        assert!(f.host.claims.require_idle().is_ok());
    }

    #[test]
    fn start_twice_and_stop_when_not_started_are_rejected() {
        let mut f = Fixture::rx();
        assert!(matches!(
            f.core.stop(&mut f.host).wait(),
            Err(Error::StreamNotStarted)
        ));
        f.core.start(&mut f.host).wait().unwrap();
        assert!(matches!(
            f.core.start(&mut f.host).wait(),
            Err(Error::StreamAlreadyStarted)
        ));
        assert_eq!(
            f.host.claims.format_users(),
            1,
            "rejected start must not touch the counter"
        );
        f.core.stop(&mut f.host).wait().unwrap();
        assert!(matches!(
            f.core.stop(&mut f.host).wait(),
            Err(Error::StreamNotStarted)
        ));
        assert_eq!(f.host.claims.format_users(), 0);
    }

    #[test]
    fn failed_module_enable_retains_retryable_transition() {
        let mut f = Fixture::rx();
        f.host.fail_enable = true;
        assert!(matches!(
            f.core.start(&mut f.host).wait(),
            Err(Error::Timeout)
        ));
        assert!(matches!(f.core.state, StreamState::Starting { .. }));
        assert_eq!(f.host.claims.format_users(), 1);
        assert!(matches!(
            f.core.read(None).wait(),
            Err(Error::StreamTransition)
        ));
        assert_eq!(
            f.ep.pending(),
            0,
            "no transfers submitted when enable fails"
        );
        f.host.fail_enable = false;
        f.core.start(&mut f.host).wait().unwrap();
        f.core.close(&mut f.host).wait().unwrap();
    }

    #[test]
    fn io_before_start_is_rejected() {
        let mut f = Fixture::rx();
        assert!(matches!(
            f.core.read(None).wait(),
            Err(Error::StreamNotStarted)
        ));
        assert!(matches!(f.core.try_read(), Err(Error::StreamNotStarted)));
        assert_eq!(
            f.ep.pending(),
            0,
            "nothing may be submitted while the module is off"
        );
        let mut t = Fixture::tx();
        assert!(matches!(
            t.core.get_buffer(None).wait(),
            Err(Error::StreamNotStarted)
        ));
        assert!(matches!(
            t.core.wait_completion(None).wait(),
            Err(Error::StreamNotStarted)
        ));
        let buf = Buffer::new(16);
        assert!(matches!(
            t.core.submit(buf, 0),
            Err(Error::StreamNotStarted)
        ));
        assert!(is_kind(
            &t.core.try_get_buffer().unwrap_err(),
            ErrorKind::State
        ));
    }

    #[test]
    fn read_with_all_buffers_held_reports_no_transfers_in_flight() {
        let mut f = Fixture::rx();
        f.core.start(&mut f.host).wait().unwrap();
        let held: Vec<Buffer> = (0..BUFFERS)
            .map(|_| f.core.read(None).wait().unwrap())
            .collect();
        f.assert_pool_invariant(BUFFERS);
        assert!(matches!(
            f.core.read(None).wait(),
            Err(Error::NoTransfersInFlight)
        ));
        assert!(matches!(
            block_on(f.core.read(None).into_future()),
            Err(Error::NoTransfersInFlight)
        ));
        for b in held {
            f.core.recycle(b);
        }
        assert!(f.core.read(None).wait().is_ok());
    }

    #[test]
    fn error_completion_recycles_the_buffer() {
        let mut f = Fixture::new(Channel::Rx, false);
        f.core.start(&mut f.host).wait().unwrap();
        f.ep.complete_next(Err(TransferError::Stall));
        assert!(matches!(
            f.core.read(None).wait(),
            Err(Error::Transfer(TransferError::Stall))
        ));
        f.assert_pool_invariant(0);
        f.ep.complete_next(Err(TransferError::Cancelled));
        assert!(matches!(f.core.read(None).wait(), Err(Error::Timeout)));
        f.assert_pool_invariant(0);
    }

    #[test]
    fn sync_wait_returns_timeout_when_nothing_completes() {
        let mut f = Fixture::new(Channel::Rx, false);
        f.core.start(&mut f.host).wait().unwrap();
        assert!(matches!(
            f.core.read(Some(Duration::from_millis(1))).wait(),
            Err(Error::Timeout)
        ));
        assert_eq!(
            f.ep.pending(),
            BUFFERS,
            "a timeout leaves the transfers in flight"
        );
    }

    #[test]
    fn async_read_is_cancel_safe() {
        let mut f = Fixture::new(Channel::Rx, false);
        f.core.start(&mut f.host).wait().unwrap();
        let mut cx = Context::from_waker(Waker::noop());
        {
            let mut fut = std::pin::pin!(f.core.read(None).into_future());
            assert!(fut.as_mut().poll(&mut cx).is_pending());
        }
        f.assert_pool_invariant(0);
        f.ep.complete_next(Ok(()));
        let buf = block_on(f.core.read(None).into_future()).unwrap();
        assert_eq!(buf.len(), MPS);
        f.assert_pool_invariant(1);
        f.core.recycle(buf);
    }

    #[test]
    fn async_and_sync_reads_agree() {
        let mut f = Fixture::rx();
        let mut g = Fixture::rx();
        f.core.start(&mut f.host).wait().unwrap();
        g.core.start(&mut g.host).wait().unwrap();
        let a = f.core.read(None).wait().unwrap();
        let b = block_on(g.core.read(None).into_future()).unwrap();
        assert_eq!(&a[..], &b[..]);
        f.assert_pool_invariant(1);
        g.assert_pool_invariant(1);
        f.core.recycle(a);
        g.core.recycle(b);
    }

    #[test]
    fn all_read_paths_deliver_every_ready_completion_in_order() {
        for path in 0..3 {
            let mut f = Fixture::rx();
            f.core.start(&mut f.host).wait().unwrap();
            for expected in 0..(BUFFERS * 8) as u64 {
                let buffer = match path {
                    0 => f.core.read(None).wait(),
                    1 => block_on(f.core.read(None).into_future()),
                    _ => f.core.try_read(),
                }
                .unwrap();
                assert_eq!(
                    u64::from_le_bytes(buffer[..8].try_into().unwrap()),
                    expected
                );
                f.assert_pool_invariant(1);
                f.core.recycle(buffer);
            }
        }
    }

    #[test]
    fn tx_probe_recycles_failed_completions() {
        let mut f = Fixture::new(Channel::Tx, false);
        f.core.start(&mut f.host).wait().unwrap();
        for _ in 0..BUFFERS * 4 {
            let mut buffer = f.core.get_buffer(None).wait().unwrap();
            buffer.extend_from_slice(&[1, 2, 3, 4]);
            f.core.submit(buffer, 4).unwrap();
            f.ep.complete_next(Err(TransferError::Stall));
            assert!(matches!(
                f.core.try_get_buffer(),
                Err(Error::Transfer(TransferError::Stall))
            ));
            f.assert_pool_invariant(0);
        }
    }

    #[test]
    fn tx_round_trip() {
        let mut f = Fixture::tx();
        f.core.start(&mut f.host).wait().unwrap();
        assert_eq!(f.ep.pending(), 0, "TX start submits nothing");
        let mut buf = f.core.get_buffer(None).wait().unwrap();
        buf.extend_from_slice(&[1, 2, 3, 4]);
        f.core.submit(buf, 4).unwrap();
        assert_eq!(f.ep.pending(), 1);
        f.core.wait_completion(None).wait().unwrap();
        f.assert_pool_invariant(0);

        let mut buf = block_on(f.core.get_buffer(None).into_future()).unwrap();
        buf.extend_from_slice(&[5; 8]);
        f.core.submit(buf, 8).unwrap();
        block_on(f.core.wait_completion(None).into_future()).unwrap();
        f.assert_pool_invariant(0);
        f.core.close(&mut f.host).wait().unwrap();
        assert_eq!(f.host.claims.format_users(), 0);
    }

    #[test]
    fn tx_get_buffer_reuses_completed_transfers() {
        let mut f = Fixture::tx();
        f.core.start(&mut f.host).wait().unwrap();
        for _ in 0..BUFFERS * 3 {
            let mut buf = f.core.get_buffer(None).wait().unwrap();
            buf.extend_from_slice(&[0; 4]);
            f.core.submit(buf, 4).unwrap();
        }
        f.assert_pool_invariant(0);
    }

    #[test]
    fn submit_length_mismatch_recycles_and_errors() {
        let mut f = Fixture::tx();
        f.core.start(&mut f.host).wait().unwrap();
        let mut buf = f.core.get_buffer(None).wait().unwrap();
        buf.extend_from_slice(&[0; 8]);
        assert!(matches!(f.core.submit(buf, 4), Err(Error::Argument(_))));
        f.assert_pool_invariant(0);
        let buf = f.core.get_buffer(None).wait().unwrap();
        assert!(matches!(
            f.core.submit(buf, MPS * 4 + 1),
            Err(Error::Argument(_))
        ));
        f.assert_pool_invariant(0);
        let mut buf = f.core.get_buffer(None).wait().unwrap();
        buf.extend_from_slice(&[0; 2]);
        assert!(matches!(f.core.submit(buf, 2), Err(Error::Argument(_))));
        f.assert_pool_invariant(0);
    }

    #[test]
    fn native_teardown_order() {
        let mut f = Fixture::new(Channel::Rx, false);
        f.core.start(&mut f.host).wait().unwrap();
        f.log.lock().unwrap().clear();
        f.core.close(&mut f.host).wait().unwrap();
        let log = f.log();
        let pos = |s: &str| {
            log.iter()
                .position(|l| l == s)
                .unwrap_or_else(|| panic!("{s} missing in {log:?}"))
        };
        assert!(pos("cancel_all") < pos("enable(Rx,false)"));
        assert!(pos("enable(Rx,false)") < pos("clear_halt"));
        assert!(pos("clear_halt") < pos("deconfig"));
        assert_eq!(f.ep.pending(), 0, "cancelled transfers were collected");
    }

    #[test]
    fn non_cancellable_teardown_drains_before_disable() {
        let mut f = Fixture::rx();
        f.ep.set_cancellable(false);
        f.core.start(&mut f.host).wait().unwrap();
        f.log.lock().unwrap().clear();
        f.core.close(&mut f.host).wait().unwrap();
        let log = f.log();
        let pos = |s: &str| {
            log.iter()
                .position(|l| l == s)
                .unwrap_or_else(|| panic!("{s} missing in {log:?}"))
        };
        assert!(
            !log.contains(&"cancel_all".to_string()),
            "a non-cancellable endpoint must not be cancelled"
        );
        let last_complete = log
            .iter()
            .rposition(|l| l == "complete")
            .expect("in-flight transfers must have completed");
        assert!(
            last_complete < pos("enable(Rx,false)"),
            "in-flight transfers must complete before the module is disabled: {log:?}"
        );
        assert!(pos("enable(Rx,false)") < pos("clear_halt"));
        assert!(pos("clear_halt") < pos("deconfig"));
        assert_eq!(f.ep.pending(), 0, "in-flight transfers were collected");
    }

    #[derive(Clone, Copy, Debug)]
    enum Action {
        Start,
        Read,
        Stop,
        Close,
    }

    #[derive(Default)]
    struct Model {
        closed: bool,
        started: bool,
        active: i32,
    }

    impl Model {
        fn apply(&mut self, action: Action) -> bool {
            match action {
                _ if self.closed => false,
                Action::Start if !self.started => {
                    self.started = true;
                    self.active += 1;
                    true
                }
                Action::Start => false,
                Action::Read => self.started,
                Action::Stop if self.started => {
                    self.started = false;
                    self.active -= 1;
                    true
                }
                Action::Stop => false,
                Action::Close => {
                    if self.started {
                        self.active -= 1;
                    }
                    self.started = false;
                    self.closed = true;
                    true
                }
            }
        }
    }

    #[test]
    fn lifecycle_model_holds_for_all_short_sequences() {
        const ACTIONS: [Action; 4] = [Action::Start, Action::Read, Action::Stop, Action::Close];
        let mut sequences: Vec<Vec<Action>> = vec![vec![]];
        for _ in 0..5 {
            let mut next = Vec::new();
            for seq in &sequences {
                for a in ACTIONS {
                    let mut s = seq.clone();
                    s.push(a);
                    next.push(s);
                }
            }
            sequences = next;
        }
        for seq in sequences {
            let mut f = Fixture::rx();
            let mut model = Model::default();
            let mut held = Vec::new();
            for (i, action) in seq.iter().enumerate() {
                let expected_ok = model.apply(*action);
                let actual = match action {
                    Action::Start => f.core.start(&mut f.host).wait().map(drop),
                    Action::Stop => f.core.stop(&mut f.host).wait().map(drop),
                    Action::Close => {
                        held.drain(..).for_each(|b| f.core.recycle(b));
                        f.core.close(&mut f.host).wait().map(drop)
                    }
                    Action::Read => f.core.read(None).wait().map(|b| held.push(b)),
                };
                assert_eq!(
                    actual.is_ok(),
                    expected_ok,
                    "{seq:?} step {i} ({action:?}): got {actual:?}"
                );
                assert_eq!(
                    f.host.claims.format_users(),
                    model.active as usize,
                    "{seq:?} step {i}: format users"
                );
                assert_eq!(
                    f.host.module(Channel::Rx),
                    model.started,
                    "{seq:?} step {i}: module"
                );
                assert_eq!(
                    matches!(f.core.state, StreamState::Running(_)),
                    model.started,
                    "{seq:?} step {i}: started flag"
                );
                f.assert_pool_invariant(held.len());
            }
        }
    }
}
