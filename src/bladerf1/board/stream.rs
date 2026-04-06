use crate::bladerf1::board::{BladeRf1, BladeRf1RxStreamer, BladeRf1TxStreamer};
use crate::channel::Channel;
use crate::error::{Error, Result};
use nusb::transfer::Buffer;
use std::collections::VecDeque;
use std::thread::sleep;
use std::time::Duration;
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
impl BladeRf1 {
    pub fn supports_format(&self, format: SampleFormat, direction: Channel) -> bool {
        match direction {
            Channel::Rx => matches!(
                format,
                SampleFormat::Sc16Q11 | SampleFormat::Sc16Q11Meta | SampleFormat::PacketMeta
            ),
            Channel::Tx => matches!(
                format,
                SampleFormat::Sc16Q11 | SampleFormat::Sc16Q11Meta | SampleFormat::PacketMeta
            ),
        }
    }
}
impl BladeRf1RxStreamer {
    pub fn new(
        dev: BladeRf1,
        buffer_size: usize,
        buffer_count: usize,
        format: SampleFormat,
    ) -> Result<Self> {
        let endpoint = dev
            .interface
            .lock()
            .unwrap()
            .transport()
            .acquire_streaming_rx_endpoint()?;
        let max_packet_size = endpoint.max_packet_size();
        if buffer_size % max_packet_size != 0 {
            return Err(Error::Invalid);
        }
        log::trace!(
            "RX streamer created with buffer_size: {}, buffer_count: {}, format: {:?}",
            buffer_size,
            buffer_count,
            format
        );
        let mut available = VecDeque::with_capacity(buffer_count);
        for _ in 0..buffer_count {
            let buffer = endpoint.allocate(buffer_size);
            available.push_back(buffer);
        }
        Ok(Self {
            device: dev,
            endpoint,
            available,
            completed: VecDeque::new(),
            in_flight_count: 0,
            buffer_size,
            format,
            is_active: false,
        })
    }
    pub fn activate(&mut self) -> Result<()> {
        self.device
            .perform_format_config(Channel::Rx, self.format)?;
        self.device.enable_module(Channel::Rx, true)?;
        log::trace!(
            "Activating RX streamer, submitting {} buffers",
            self.available.len()
        );
        self.in_flight_count = 0;
        while let Some(buffer) = self.available.pop_front() {
            let mut buf = buffer;
            buf.set_requested_len(self.buffer_size);
            buf.clear();
            self.endpoint.submit(buf);
            self.in_flight_count += 1;
        }
        self.is_active = true;
        Ok(())
    }
    pub fn deactivate(&mut self) -> Result<()> {
        log::trace!("Deactivating RX streamer");
        self.device.enable_module(Channel::Rx, false)?;
        self.device.perform_format_deconfig(Channel::Rx)?;
        while self.in_flight_count > 0 {
            if let Some(completion) = self.endpoint.wait_next_complete(Duration::from_secs(1)) {
                completion.status?;
                self.in_flight_count -= 1;
                let mut buf = completion.buffer;
                buf.clear();
                self.available.push_back(buf);
            }
        }
        while let Some(mut buffer) = self.completed.pop_front() {
            buffer.clear();
            self.available.push_back(buffer);
        }
        self.is_active = false;
        Ok(())
    }
    pub fn read(&mut self, timeout: Option<Duration>) -> Result<Buffer> {
        let timeout = timeout.unwrap_or(Duration::MAX);
        self.refill_pipeline()?;
        if let Some(buffer) = self.completed.pop_front() {
            return Ok(buffer);
        }
        let completion = self
            .endpoint
            .wait_next_complete(timeout)
            .ok_or(Error::Timeout)?;
        completion.status?;
        self.in_flight_count -= 1;
        Ok(completion.buffer)
    }
    pub fn try_read(&mut self) -> Result<Option<Buffer>> {
        self.refill_pipeline()?;
        if let Some(buffer) = self.completed.pop_front() {
            return Ok(Some(buffer));
        }
        if let Some(completion) = self.endpoint.wait_next_complete(Duration::ZERO) {
            completion.status?;
            self.in_flight_count -= 1;
            return Ok(Some(completion.buffer));
        }
        Ok(None)
    }
    pub fn recycle(&mut self, mut buffer: Buffer) -> Result<()> {
        buffer.clear();
        self.available.push_back(buffer);
        Ok(())
    }
    pub fn buffer_size(&self) -> usize {
        self.buffer_size
    }
    pub fn buffer_count(&self) -> usize {
        self.available.len() + self.in_flight_count + self.completed.len()
    }
    pub fn completed_count(&self) -> usize {
        self.completed.len()
    }
    pub fn is_active(&self) -> bool {
        self.is_active
    }
    fn refill_pipeline(&mut self) -> Result<()> {
        if !self.is_active {
            return Ok(());
        }
        while self.completed.len() > 1 {
            if let Some(mut buffer) = self.completed.pop_front() {
                buffer.clear();
                self.available.push_back(buffer);
            }
        }
        while !self.available.is_empty() && self.in_flight_count < self.buffer_count() {
            if let Some(buffer) = self.available.pop_front() {
                let mut buf = buffer;
                buf.set_requested_len(self.buffer_size);
                buf.clear();
                self.endpoint.submit(buf);
                self.in_flight_count += 1;
            }
        }
        Ok(())
    }
}
impl BladeRf1TxStreamer {
    pub fn new(
        dev: BladeRf1,
        buffer_size: usize,
        buffer_count: usize,
        format: SampleFormat,
    ) -> Result<Self> {
        let endpoint = dev
            .interface
            .lock()
            .unwrap()
            .transport()
            .acquire_streaming_tx_endpoint()?;
        let max_packet_size = endpoint.max_packet_size();
        if buffer_size % max_packet_size != 0 {
            return Err(Error::Invalid);
        }
        log::trace!(
            "TX streamer created with buffer_size: {}, buffer_count: {}, format: {:?}",
            buffer_size,
            buffer_count,
            format
        );
        let mut available = VecDeque::with_capacity(buffer_count);
        for _ in 0..buffer_count {
            let buffer = endpoint.allocate(buffer_size);
            available.push_back(buffer);
        }
        Ok(Self {
            device: dev,
            endpoint,
            available,
            completed: VecDeque::new(),
            in_flight_count: 0,
            buffer_size,
            format,
            is_active: false,
        })
    }
    pub fn activate(&mut self) -> Result<()> {
        self.device
            .perform_format_config(Channel::Tx, self.format)?;
        self.device.enable_module(Channel::Tx, true)?;
        self.is_active = true;
        Ok(())
    }
    pub fn deactivate(&mut self) -> Result<()> {
        log::trace!("Deactivating TX streamer");
        self.device.enable_module(Channel::Tx, false)?;
        self.device.perform_format_deconfig(Channel::Tx)?;
        while self.in_flight_count > 0 {
            if let Some(completion) = self.endpoint.wait_next_complete(Duration::from_secs(1)) {
                completion.status?;
                self.in_flight_count -= 1;
                let mut buf = completion.buffer;
                buf.clear();
                self.available.push_back(buf);
            }
        }
        while let Some(mut buffer) = self.completed.pop_front() {
            buffer.clear();
            self.available.push_back(buffer);
        }
        self.is_active = false;
        Ok(())
    }
    pub fn get_buffer(&mut self, timeout: Option<Duration>) -> Result<Buffer> {
        let timeout = timeout.unwrap_or(Duration::MAX);
        let start = std::time::Instant::now();
        loop {
            if let Some(buffer) = self.available.pop_front() {
                return Ok(buffer);
            }
            self.pick_up_completed(Duration::ZERO)?;
            if !self.available.is_empty() {
                continue;
            }
            if timeout != Duration::MAX && start.elapsed() > timeout {
                return Err(Error::Timeout);
            }
            sleep(Duration::from_millis(1));
        }
    }
    pub fn try_get_buffer(&mut self) -> Result<Option<Buffer>> {
        self.pick_up_completed(Duration::ZERO)?;
        match self.available.pop_front() {
            Some(buffer) => Ok(Some(buffer)),
            None => Ok(None),
        }
    }
    pub fn submit(&mut self, buffer: Buffer, len: usize) -> Result<()> {
        if len > self.buffer_size {
            return Err(Error::Invalid);
        }
        self.endpoint.submit(buffer);
        self.in_flight_count += 1;
        Ok(())
    }
    pub fn wait_completion(&mut self, timeout: Option<Duration>) -> Result<()> {
        let timeout = timeout.unwrap_or(Duration::MAX);
        let start = std::time::Instant::now();
        while self.in_flight_count > 0 {
            let remaining = timeout.saturating_sub(start.elapsed());
            let completion = self
                .endpoint
                .wait_next_complete(if remaining.is_zero() {
                    Duration::from_secs(1)
                } else {
                    remaining
                })
                .ok_or(Error::Timeout)?;
            completion.status?;
            self.in_flight_count -= 1;
            let mut buf = completion.buffer;
            buf.clear();
            self.completed.push_back(buf);
        }
        Ok(())
    }
    pub fn try_get_completed(&mut self) -> Result<Option<Buffer>> {
        if let Some(completion) = self.endpoint.wait_next_complete(Duration::ZERO) {
            completion.status?;
            if self.in_flight_count > 0 {
                self.in_flight_count -= 1;
            }
            let mut buf = completion.buffer;
            buf.clear();
            self.completed.push_back(buf);
        }
        self.pick_up_completed(Duration::ZERO)?;
        match self.available.pop_front() {
            Some(buffer) => Ok(Some(buffer)),
            None => Ok(None),
        }
    }
    pub fn recycle(&mut self, mut buffer: Buffer) -> Result<()> {
        buffer.clear();
        self.available.push_back(buffer);
        Ok(())
    }
    pub fn buffer_size(&self) -> usize {
        self.buffer_size
    }
    pub fn buffer_count(&self) -> usize {
        self.available.len() + self.in_flight_count + self.completed.len()
    }
    pub fn is_active(&self) -> bool {
        self.is_active
    }
    fn pick_up_completed(&mut self, timeout: Duration) -> Result<()> {
        if let Some(completion) = self.endpoint.wait_next_complete(timeout) {
            completion.status?;
            if self.in_flight_count > 0 {
                self.in_flight_count -= 1;
            }
            let mut buf = completion.buffer;
            buf.clear();
            self.completed.push_back(buf);
        }
        while let Some(mut buffer) = self.completed.pop_front() {
            buffer.clear();
            self.available.push_back(buffer);
        }
        Ok(())
    }
}
impl BladeRf1 {
    pub fn perform_format_config(&self, channel: Channel, format: SampleFormat) -> Result<()> {
        let mut use_timestamps: bool = false;
        let _other_using_timestamps: bool = false;
        let _other = if channel.is_rx() {
            Channel::Tx
        } else {
            Channel::Rx
        };
        let mut gpio_val = self.config_gpio_read()?;
        if format == SampleFormat::PacketMeta {
            gpio_val |= BLADERF_GPIO_PACKET;
            use_timestamps = true;
            log::debug!("BladeRf1Format::PacketMeta");
        } else {
            gpio_val &= !BLADERF_GPIO_PACKET;
        }
        if use_timestamps {
            gpio_val |= BLADERF_GPIO_TIMESTAMP | BLADERF_GPIO_TIMESTAMP_DIV2;
        } else {
            gpio_val &= !(BLADERF_GPIO_TIMESTAMP | BLADERF_GPIO_TIMESTAMP_DIV2);
        }
        self.config_gpio_write(gpio_val)?;
        Ok(())
    }
    pub fn perform_format_deconfig(&self, _channel: Channel) -> Result<()> {
        Ok(())
    }
}
