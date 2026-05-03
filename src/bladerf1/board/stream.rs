use crate::bladerf1::board::{BladeRf1, RxStream, TxStream};
use crate::channel::Channel;
use crate::error::{Error, Result};
use nusb::MaybeFuture;
use nusb::transfer::{Buffer, Bulk, Completion, EndpointDirection, TransferError};
use std::collections::VecDeque;
use std::time::Duration;

pub(crate) struct BufferPool<Dir: EndpointDirection> {
    endpoint: nusb::Endpoint<Bulk, Dir>,
    available: VecDeque<Buffer>,
    buffer_count: usize,
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

    fn buffer_size(&self) -> usize {
        self.buffer_size
    }

    fn buffer_count(&self) -> usize {
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

    fn cancel_all(&mut self) {
        if self.endpoint.pending() > 0 {
            self.endpoint.cancel_all();
        }
    }

    fn drain_cancelled(&mut self) {
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

    fn clear_halt(&mut self) -> Result<()> {
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

#[derive(PartialEq, Clone, Copy, Debug)]
pub enum SampleFormat {
    Sc16Q11 = 0,
    Sc16Q11Meta = 1,
    PacketMeta = 2,
    Sc8Q7 = 3,
    Sc8Q7Meta = 4,
    Sc16Q11Packed = 5,
}
pub const BLADERF_GPIO_PACKET: u32 = 1 << 19;
pub const BLADERF_GPIO_TIMESTAMP: u32 = 1 << 16;
pub const BLADERF_GPIO_TIMESTAMP_DIV2: u32 = 1 << 17;
pub const BLADERF_GPIO_8BIT_MODE: u32 = 1 << 20;
pub const BLADERF_GPIO_HIGHLY_PACKED_MODE: u32 = 1 << 21;

pub const METADATA_HEADER_SIZE: usize = 16;

#[repr(C, packed)]
#[derive(Debug, Clone, Copy, Default)]
pub struct MetadataHeader {
    pub reserved_or_length: u16,
    pub flags_or_core: u16,
    pub timestamp: u64,
    pub meta_flags: u32,
}

impl MetadataHeader {
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < METADATA_HEADER_SIZE {
            return None;
        }
        Some(unsafe { std::ptr::read_unaligned(bytes.as_ptr() as *const Self) })
    }

    pub fn timestamp(&self) -> u64 {
        self.timestamp
    }

    pub fn meta_flags(&self) -> u32 {
        self.meta_flags
    }

    pub fn is_valid_meta_format(&self) -> bool {
        let ver = self.flags_or_core as u8;
        ver == 0x00 || ver == 0x34
    }

    pub fn stream_flags(&self) -> u8 {
        (self.flags_or_core >> 8) as u8
    }

    pub fn meta_version(&self) -> u8 {
        (self.flags_or_core & 0xFF) as u8
    }

    pub fn packet_length(&self) -> u16 {
        self.reserved_or_length
    }

    pub fn packet_core_id(&self) -> u8 {
        (self.flags_or_core >> 8) as u8
    }

    pub fn packet_flags(&self) -> u8 {
        self.flags_or_core as u8
    }
}

#[inline(always)]
const fn sign_extend_12(val: u16) -> i16 {
    ((val << 4) as i16) >> 4
}

impl SampleFormat {
    pub fn sample_size(self) -> usize {
        match self {
            Self::Sc16Q11 | Self::Sc16Q11Meta | Self::PacketMeta => 4,
            Self::Sc16Q11Packed => 3,
            Self::Sc8Q7 | Self::Sc8Q7Meta => 2,
        }
    }

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
        for i in 0..num_samples / 2 {
            let si = 6 * i;
            let di = 8 * i;
            let w0 = u16::from_le_bytes([src[si], src[si + 1]]);
            let w1 = u16::from_le_bytes([src[si + 2], src[si + 3]]);
            let w2 = u16::from_le_bytes([src[si + 4], src[si + 5]]);
            let i0 = sign_extend_12(w0 & 0x0FFF);
            let q0 = sign_extend_12((w0 >> 12) | ((w1 & 0x00FF) << 4));
            let i1 = sign_extend_12((w1 >> 8) | ((w2 & 0x000F) << 8));
            let q1 = sign_extend_12(w2 >> 4);
            dst[di] = i0 as u8;
            dst[di + 1] = (i0 >> 8) as u8;
            dst[di + 2] = q0 as u8;
            dst[di + 3] = (q0 >> 8) as u8;
            dst[di + 4] = i1 as u8;
            dst[di + 5] = (i1 >> 8) as u8;
            dst[di + 6] = q1 as u8;
            dst[di + 7] = (q1 >> 8) as u8;
        }
        Ok(())
    }

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
        for i in 0..num_samples / 2 {
            let si = 8 * i;
            let di = 6 * i;
            let v0 = i16::from_le_bytes([src[si], src[si + 1]]) as u16;
            let v1 = i16::from_le_bytes([src[si + 2], src[si + 3]]) as u16;
            let v2 = i16::from_le_bytes([src[si + 4], src[si + 5]]) as u16;
            let v3 = i16::from_le_bytes([src[si + 6], src[si + 7]]) as u16;
            let w0 = (v0 & 0x0FFF) | ((v1 & 0x000F) << 12);
            let w1 = ((v1 >> 4) & 0x00FF) | ((v2 & 0x00FF) << 8);
            let w2 = ((v2 >> 8) & 0x000F) | ((v3 & 0x0FFF) << 4);
            dst[di] = w0 as u8;
            dst[di + 1] = (w0 >> 8) as u8;
            dst[di + 2] = w1 as u8;
            dst[di + 3] = (w1 >> 8) as u8;
            dst[di + 4] = w2 as u8;
            dst[di + 5] = (w2 >> 8) as u8;
        }
        Ok(())
    }
}

fn requires_timestamps(format: SampleFormat) -> bool {
    matches!(
        format,
        SampleFormat::Sc16Q11Meta | SampleFormat::Sc8Q7Meta | SampleFormat::PacketMeta
    )
}

impl BladeRf1 {
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

pub struct RxStreamBuilder<'a> {
    dev: &'a mut BladeRf1,
    buffer_size: usize,
    buffer_count: usize,
    format: SampleFormat,
}

impl<'a> RxStreamBuilder<'a> {
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

    pub fn build(self) -> Result<RxStream> {
        let endpoint = self.dev.nios.transport().acquire_streaming_rx_endpoint()?;
        let mps = endpoint.max_packet_size();
        if !self.buffer_size.is_multiple_of(mps) {
            return Err(Error::Argument(format!(
                "buffer_size ({}) must be a multiple of max_packet_size ({})",
                self.buffer_size, mps
            )));
        }
        log::trace!(
            "Creating RxStream: buffer_size={}, buffer_count={}, format={:?}",
            self.buffer_size,
            self.buffer_count,
            self.format
        );
        self.dev.perform_format_config(Channel::Rx, self.format)?;
        let mut pool = BufferPool::new(endpoint, self.buffer_size, self.buffer_count);
        pool.clear_halt()?;
        self.dev.enable_module(Channel::Rx, true)?;
        pool.submit_all_available();
        log::trace!("RxStream activated");
        Ok(RxStream { pool })
    }
}

impl RxStream {
    pub fn builder(dev: &mut BladeRf1) -> RxStreamBuilder<'_> {
        RxStreamBuilder {
            dev,
            buffer_size: 65_536,
            buffer_count: 8,
            format: SampleFormat::Sc16Q11,
        }
    }

    pub fn close(&mut self, dev: &mut BladeRf1) -> Result<()> {
        self.pool.cancel_all();
        dev.enable_module(Channel::Rx, false)?;
        self.pool.drain_cancelled();
        self.pool.clear_halt()
    }

    pub fn read(&mut self, timeout: Option<Duration>) -> Result<Buffer> {
        let timeout = timeout.unwrap_or(Duration::MAX);
        self.pool.submit_all_available();
        let completion = self.pool.wait_completion(timeout).ok_or(Error::Timeout)?;
        if let Err(TransferError::Cancelled) = completion.status {
            return Err(Error::Timeout);
        }
        completion.status?;
        self.pool.drain_extras();
        Ok(completion.buffer)
    }

    pub fn try_read(&mut self) -> Result<Buffer> {
        self.pool.submit_all_available();
        let completion = match self.pool.wait_completion(Duration::ZERO) {
            Some(c) => c,
            None => return Err(Error::WouldBlock),
        };
        if let Err(TransferError::Cancelled) = completion.status {
            return Err(Error::WouldBlock);
        }
        completion.status?;
        self.pool.drain_extras();
        Ok(completion.buffer)
    }

    pub fn buffer_size(&self) -> usize {
        self.pool.buffer_size()
    }

    pub fn buffer_count(&self) -> usize {
        self.pool.buffer_count()
    }

    pub fn recycle(&mut self, buf: Buffer) {
        self.pool.recycle(buf);
    }
}

impl Drop for RxStream {
    fn drop(&mut self) {
        self.pool.cancel_all();
        self.pool.drain_cancelled();
        if let Err(e) = self.pool.clear_halt() {
            log::warn!("RxStream Drop: clear_halt failed: {e:#}");
        }
    }
}

pub struct TxStreamBuilder<'a> {
    dev: &'a mut BladeRf1,
    buffer_size: usize,
    buffer_count: usize,
    format: SampleFormat,
}

impl<'a> TxStreamBuilder<'a> {
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

    pub fn build(self) -> Result<TxStream> {
        let endpoint = self.dev.nios.transport().acquire_streaming_tx_endpoint()?;
        let mps = endpoint.max_packet_size();
        if !self.buffer_size.is_multiple_of(mps) {
            return Err(Error::Argument(format!(
                "buffer_size ({}) must be a multiple of max_packet_size ({})",
                self.buffer_size, mps
            )));
        }
        log::trace!(
            "Creating TxStream: buffer_size={}, buffer_count={}, format={:?}",
            self.buffer_size,
            self.buffer_count,
            self.format
        );
        self.dev.perform_format_config(Channel::Tx, self.format)?;
        let mut pool = BufferPool::new(endpoint, self.buffer_size, self.buffer_count);
        pool.clear_halt()?;
        self.dev.enable_module(Channel::Tx, true)?;
        log::trace!("TxStream activated");
        Ok(TxStream { pool })
    }
}

impl TxStream {
    pub fn builder(dev: &mut BladeRf1) -> TxStreamBuilder<'_> {
        TxStreamBuilder {
            dev,
            buffer_size: 65_536,
            buffer_count: 8,
            format: SampleFormat::Sc16Q11,
        }
    }

    pub fn close(&mut self, dev: &mut BladeRf1) -> Result<()> {
        self.pool.cancel_all();
        dev.enable_module(Channel::Tx, false)?;
        self.pool.drain_cancelled();
        self.pool.clear_halt()?;
        dev.perform_format_deconfig(Channel::Tx)
    }

    pub fn get_buffer(&mut self, timeout: Option<Duration>) -> Result<Buffer> {
        let deadline = timeout.map(|t| std::time::Instant::now() + t);
        loop {
            if let Some(buffer) = self.pool.pop_available() {
                return Ok(buffer);
            }
            let remaining = deadline.map_or(Duration::MAX, |d| {
                d.saturating_duration_since(std::time::Instant::now())
            });
            if remaining.is_zero() {
                return Err(Error::Timeout);
            }
            let wait = remaining.min(Duration::from_secs(1));
            if let Some(completion) = self.pool.wait_completion(wait) {
                completion.status?;
                let mut buf = completion.buffer;
                buf.clear();
                return Ok(buf);
            }
        }
    }

    pub fn try_get_buffer(&mut self) -> Result<Buffer> {
        self.pool.pickup_tx_completed(Duration::ZERO)?;
        match self.pool.pop_available() {
            Some(buffer) => Ok(buffer),
            None => Err(Error::WouldBlock),
        }
    }

    pub fn submit(&mut self, buf: Buffer, len: usize) -> Result<()> {
        if len > self.pool.buffer_size {
            return Err(Error::Argument("submit length exceeds buffer_size".into()));
        }
        self.pool.submit(buf);
        Ok(())
    }

    pub fn wait_completion(&mut self, timeout: Option<Duration>) -> Result<()> {
        let timeout = timeout.unwrap_or(Duration::MAX);
        let start = std::time::Instant::now();
        while self.pool.pending() > 0 {
            let remaining = timeout.saturating_sub(start.elapsed());
            let completion = self
                .pool
                .wait_completion(if remaining.is_zero() {
                    Duration::from_secs(1)
                } else {
                    remaining
                })
                .ok_or(Error::Timeout)?;
            completion.status?;
            let mut buf = completion.buffer;
            buf.clear();
            self.pool.recycle(buf);
        }
        Ok(())
    }

    pub fn try_get_completed(&mut self) -> Result<Buffer> {
        if self.pool.pending() > 0
            && let Some(completion) = self.pool.wait_completion(Duration::ZERO)
        {
            completion.status?;
            let mut buf = completion.buffer;
            buf.clear();
            self.pool.recycle(buf);
        }
        self.pool.pickup_tx_completed(Duration::ZERO)?;
        match self.pool.pop_available() {
            Some(buffer) => Ok(buffer),
            None => Err(Error::WouldBlock),
        }
    }

    pub fn buffer_size(&self) -> usize {
        self.pool.buffer_size()
    }

    pub fn buffer_count(&self) -> usize {
        self.pool.buffer_count()
    }

    pub fn recycle(&mut self, buf: Buffer) {
        self.pool.recycle(buf);
    }
}

impl Drop for TxStream {
    fn drop(&mut self) {
        self.pool.cancel_all();
        self.pool.drain_cancelled();
        if let Err(e) = self.pool.clear_halt() {
            log::warn!("TxStream Drop: clear_halt failed: {e:#}");
        }
    }
}

impl BladeRf1 {
    pub fn perform_format_config(&mut self, _channel: Channel, format: SampleFormat) -> Result<()> {
        let use_timestamps = requires_timestamps(format);
        let mut gpio_val = self.config_gpio_read()?;

        if format == SampleFormat::PacketMeta {
            gpio_val |= BLADERF_GPIO_PACKET;
            log::debug!("BladeRf1Format::PacketMeta");
        } else {
            gpio_val &= !BLADERF_GPIO_PACKET;
        }

        if use_timestamps {
            gpio_val |= BLADERF_GPIO_TIMESTAMP | BLADERF_GPIO_TIMESTAMP_DIV2;
        } else {
            gpio_val &= !(BLADERF_GPIO_TIMESTAMP | BLADERF_GPIO_TIMESTAMP_DIV2);
        }

        if matches!(format, SampleFormat::Sc8Q7 | SampleFormat::Sc8Q7Meta) {
            gpio_val |= BLADERF_GPIO_8BIT_MODE;
        } else {
            gpio_val &= !BLADERF_GPIO_8BIT_MODE;
        }

        if format == SampleFormat::Sc16Q11Packed {
            gpio_val |= BLADERF_GPIO_HIGHLY_PACKED_MODE;
        } else {
            gpio_val &= !BLADERF_GPIO_HIGHLY_PACKED_MODE;
        }

        self.config_gpio_write(gpio_val)?;
        Ok(())
    }

    pub fn perform_format_deconfig(&mut self, _channel: Channel) -> Result<()> {
        let mut gpio_val = self.config_gpio_read()?;
        gpio_val &= !(BLADERF_GPIO_PACKET
            | BLADERF_GPIO_TIMESTAMP
            | BLADERF_GPIO_TIMESTAMP_DIV2
            | BLADERF_GPIO_8BIT_MODE
            | BLADERF_GPIO_HIGHLY_PACKED_MODE);
        self.config_gpio_write(gpio_val)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pack_i16(value: i16) -> [u8; 2] {
        value.to_le_bytes()
    }

    #[test]
    fn pack_unpack_roundtrip() {
        let samples: [i16; 8] = [0, -1, 2_047, -2048, 1, -100, 0x07FF, -2048i16];
        let num_samples = samples.len();
        let mut src = vec![0u8; 4 * num_samples];
        let mut packed = vec![0u8; 3 * num_samples];
        let mut unpacked = vec![0u8; 4 * num_samples];
        for (i, &s) in samples.iter().enumerate() {
            src[2 * i..2 * i + 2].copy_from_slice(&pack_i16(s));
        }
        SampleFormat::pack_sc16q11_packed(&src, &mut packed, num_samples).unwrap();
        SampleFormat::unpack_sc16q11_packed(&packed, &mut unpacked, num_samples).unwrap();
        for (i, &orig) in samples.iter().enumerate() {
            let got = i16::from_le_bytes([unpacked[2 * i], unpacked[2 * i + 1]]);
            assert_eq!(
                got, orig,
                "Sample {i}: expected {orig:#06x}, got {got:#06x}"
            );
        }
    }

    #[test]
    fn pack_unpack_roundtrip_negative() {
        let samples: [i16; 4] = [-1, -1, -2048, -2048];
        let num_samples = samples.len();
        let mut src = vec![0u8; 4 * num_samples];
        let mut packed = vec![0u8; 3 * num_samples];
        let mut unpacked = vec![0u8; 4 * num_samples];
        for (i, &s) in samples.iter().enumerate() {
            src[2 * i..2 * i + 2].copy_from_slice(&pack_i16(s));
        }
        SampleFormat::pack_sc16q11_packed(&src, &mut packed, num_samples).unwrap();
        SampleFormat::unpack_sc16q11_packed(&packed, &mut unpacked, num_samples).unwrap();
        for (i, &orig) in samples.iter().enumerate() {
            let got = i16::from_le_bytes([unpacked[2 * i], unpacked[2 * i + 1]]);
            assert_eq!(got, orig, "Sample {i}: expected {orig}, got {got}");
        }
    }
}
