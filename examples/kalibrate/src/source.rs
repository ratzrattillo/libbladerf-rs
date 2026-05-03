use std::collections::VecDeque;
use std::time::Duration;

use anyhow::{Context, Result};
use libbladerf_rs::bladerf1::hardware::lms6002d::frequency::LmsFreq;
use libbladerf_rs::bladerf1::{
    BladeRf1, GainMode, METADATA_HEADER_SIZE, MetadataHeader, RxStream, SampleFormat,
};
use libbladerf_rs::channel::Channel;
use nusb::Speed;
use rustfft::num_complex::Complex32;

const GSM_RATE: f64 = 1_625_000.0 / 6.0;

pub const SAMPLE_RATE: u32 = 4 * 13_000_000 / 48;

pub const BANDWIDTH: u32 = 1_500_000;

const BUFFER_SIZE: usize = 2_048 * 4;
const BUFFER_COUNT: usize = 64;

pub struct Source {
    device: BladeRf1,
    streamer: Option<RxStream>,
}

impl Source {
    pub fn open() -> Result<Self> {
        let mut device = BladeRf1::from_first().context("No bladeRF device found")?;
        device
            .initialize(false)
            .context("Device initialization failed")?;

        device
            .set_sample_rate(Channel::Rx, SAMPLE_RATE)
            .context("Failed to set sample rate")?;
        device
            .set_bandwidth(Channel::Rx, BANDWIDTH)
            .context("Failed to set bandwidth")?;
        device
            .set_gain_mode(Channel::Rx, GainMode::Mgc)
            .context("Failed to set gain mode")?;

        Ok(Self {
            device,
            streamer: None,
        })
    }

    pub fn sample_rate(&mut self) -> f64 {
        self.device
            .get_sample_rate(Channel::Rx)
            .map(|r| r as f64)
            .unwrap_or(SAMPLE_RATE as f64)
    }

    pub fn set_gain(&mut self, gain_db: i8) -> Result<()> {
        self.device
            .set_gain(Channel::Rx, gain_db.into())
            .context("Failed to set gain")
    }

    pub fn set_dac_trim(&mut self, value: u16) -> Result<()> {
        self.device
            .set_dac_trim(value)
            .with_context(|| format!("Failed to set DAC trim to {value}"))
    }

    pub fn get_dac_trim(&mut self) -> Result<u16> {
        self.device
            .get_dac_trim()
            .context("Failed to read DAC trim")
    }

    pub fn save_dac_trim(&mut self, value: u16) -> Result<()> {
        self.device
            .save_dac_trim(value)
            .with_context(|| format!("Failed to save DAC trim {value} to flash"))
    }

    pub fn start(&mut self) -> Result<()> {
        let streamer = RxStream::builder(&mut self.device)
            .buffer_size(BUFFER_SIZE)
            .buffer_count(BUFFER_COUNT)
            .format(SampleFormat::Sc16Q11)
            .build()
            .context("Failed to create RX streamer")?;

        self.streamer = Some(streamer);
        Ok(())
    }

    pub fn stop(&mut self) -> Result<()> {
        if let Some(mut streamer) = self.streamer.take() {
            streamer.close(&mut self.device).ok();
        }
        Ok(())
    }

    pub fn read_samples(&mut self, num_samples: usize) -> Result<Vec<Complex32>> {
        let streamer = self
            .streamer
            .as_mut()
            .context("Streamer not active — call start() first")?;

        let bytes_per_sample = 4;
        let mut result = Vec::with_capacity(num_samples);

        while result.len() < num_samples {
            let buffer = streamer
                .read(Some(Duration::from_secs(2)))
                .context("RX read timeout")?;

            let raw: &[u8] = &buffer;
            let available = raw.len() / bytes_per_sample;
            let needed = num_samples - result.len();
            let to_take = available.min(needed);

            for i in 0..to_take {
                let offset = i * bytes_per_sample;
                let i_val = i16::from_le_bytes([raw[offset], raw[offset + 1]]);
                let q_val = i16::from_le_bytes([raw[offset + 2], raw[offset + 3]]);
                result.push(Complex32::new(
                    i_val as f32 / 2_048.0,
                    q_val as f32 / 2_048.0,
                ));
            }
            streamer.recycle(buffer);
        }

        result.truncate(num_samples);
        Ok(result)
    }

    pub fn serial(&self) -> Result<String> {
        self.device.serial().context("Failed to read serial number")
    }

    pub fn device(&mut self) -> &mut BladeRf1 {
        &mut self.device
    }

    pub fn flush(&mut self) -> Result<()> {
        let sps = self.sample_rate() / GSM_RATE;
        let flush_len = (10.0 * 128.0 * sps) as usize;
        let _ = self.read_samples(flush_len);
        Ok(())
    }

    pub fn streaming_power_scan(&mut self, frequencies: &[f64]) -> Result<Vec<(f64, f64)>> {
        let n_freq = frequencies.len();
        if n_freq == 0 {
            return Ok(vec![]);
        }

        let speed = self.device.speed().context("Failed to get device speed")?;
        let params = derive_scan_params(speed);
        let ScanParams {
            sample_rate,
            samples_per_freq,
            ..
        } = params;
        eprintln!(
            "[*] Scan params: rate={} Ms/s, samples/freq={}, usb={:?}",
            sample_rate / 1_000_000,
            samples_per_freq,
            speed
        );

        self.device
            .set_sample_rate(Channel::Rx, sample_rate)
            .context("Failed to set sample rate")?;
        self.device
            .set_bandwidth(Channel::Rx, (sample_rate as f64 * 0.875) as u32)
            .context("Failed to set bandwidth")?;

        let lms_freqs: Vec<LmsFreq> = frequencies
            .iter()
            .map(|&f| LmsFreq::try_from(f as u64).context(format!("Invalid frequency {f}")))
            .collect::<Result<Vec<_>>>()?;

        let message_block_size = match speed {
            Speed::High => 2_048,
            _ => 4_096,
        };

        let buffer_size = message_block_size * 4;
        let buffer_count = 64;
        let mut streamer = RxStream::builder(&mut self.device)
            .buffer_size(buffer_size)
            .buffer_count(buffer_count)
            .format(SampleFormat::Sc8Q7Meta)
            .build()
            .context("Failed to create RX streamer")?;

        let base_timestamp = 'discover: {
            let mut prev_ts: u64 = 0;
            let mut valid_count: usize = 0;
            let mut last_valid_ts: u64 = 0;
            for _ in 0..300 {
                let buffer = streamer
                    .read(Some(Duration::from_secs(2)))
                    .context("RX read timeout during timestamp discovery")?;
                let raw: &[u8] = &buffer;
                let mut offset = 0;
                while offset + message_block_size <= raw.len() {
                    if let Some(header) =
                        MetadataHeader::from_bytes(&raw[offset..offset + message_block_size])
                    {
                        if !header.is_valid_meta_format() {
                            offset += message_block_size;
                            continue;
                        }
                        let ts = header.timestamp;
                        if ts > prev_ts {
                            valid_count += 1;
                            last_valid_ts = ts;
                            if valid_count >= 3 {
                                break 'discover last_valid_ts;
                            }
                            prev_ts = ts;
                        } else {
                            valid_count = 0;
                            prev_ts = 0;
                        }
                    }
                    offset += message_block_size;
                }
                streamer.recycle(buffer);
            }
            last_valid_ts
        };

        eprintln!("[*] Base FPGA timestamp: {base_timestamp}");

        let lead_time = sample_rate as u64 / 10;
        let t0 = base_timestamp + lead_time;

        self.device
            .cancel_scheduled_retunes(Channel::Rx)
            .context("Failed to cancel scheduled retunes")?;
        self.device
            .schedule_retune(
                Channel::Rx,
                0,
                frequencies[0] as u64,
                Some(lms_freqs[0].clone()),
            )
            .context("Failed to schedule initial retune")?;

        let mut scheduled: VecDeque<(usize, u64, u64)> = VecDeque::new();
        let mut next_schedule_idx: usize = 1;

        while next_schedule_idx < n_freq && scheduled.len() < RETUNE_QUEUE_DEPTH {
            let i = next_schedule_idx;
            let timestamp = t0 + i as u64 * samples_per_freq;
            match self.device.schedule_retune_with_duration(
                Channel::Rx,
                timestamp,
                frequencies[i] as u64,
                Some(lms_freqs[i].clone()),
            ) {
                Ok((_, duration)) => {
                    scheduled.push_back((i, timestamp, duration));
                    next_schedule_idx += 1;
                }
                Err(libbladerf_rs::Error::RetuneQueueFull) => {
                    eprintln!(
                        "[!] Retune queue full during pre-fill at index {i}, will fill later"
                    );
                    break;
                }
                Err(e) => return Err(e).context("Failed to schedule retune"),
            }
        }

        let mut power_accum: Vec<f64> = vec![0.0; n_freq];
        let mut sample_count: Vec<usize> = vec![0; n_freq];
        let mut gap_end_timestamp: u64 = 0;

        let total_samples = n_freq as u64 * samples_per_freq;
        let mut samples_collected: u64 = 0;

        let mut found_start = false;
        while !found_start {
            let buffer = streamer
                .read(Some(Duration::from_secs(2)))
                .context("RX read timeout during lead time")?;
            let raw: &[u8] = &buffer;
            let mut offset = 0;
            while offset + message_block_size <= raw.len() {
                let block = &raw[offset..offset + message_block_size];
                if let Some(header) = MetadataHeader::from_bytes(block)
                    && header.timestamp >= t0
                {
                    found_start = true;
                    break;
                }
                offset += message_block_size;
            }
            streamer.recycle(buffer);
        }

        while samples_collected < total_samples {
            let buffer = streamer
                .read(Some(Duration::from_secs(2)))
                .context("RX read timeout")?;

            let raw: &[u8] = &buffer;

            let mut offset = 0;
            while offset + message_block_size <= raw.len() {
                let block = &raw[offset..offset + message_block_size];

                let header = match MetadataHeader::from_bytes(block) {
                    Some(h) => h,
                    None => {
                        offset += message_block_size;
                        continue;
                    }
                };
                let timestamp = header.timestamp;

                if timestamp < t0 {
                    offset += message_block_size;
                    continue;
                }

                let payload = &block[METADATA_HEADER_SIZE..];

                let freq_idx = current_freq_index(&scheduled, timestamp);

                if timestamp < gap_end_timestamp {
                    offset += message_block_size;
                    continue;
                }

                if let Some(front) = scheduled.front()
                    && timestamp >= front.1
                    && front.2 > 0
                    && gap_end_timestamp == 0
                {
                    gap_end_timestamp = front.1 + front.2;
                    if timestamp < gap_end_timestamp {
                        offset += message_block_size;
                        continue;
                    }
                    gap_end_timestamp = 0;
                }

                if let Some(idx) = freq_idx {
                    let (sum_sq, count) = compute_power_raw(payload);
                    power_accum[idx] += sum_sq;
                    sample_count[idx] += count;
                    samples_collected += count as u64;
                }

                while let Some(front) = scheduled.front() {
                    if timestamp >= front.1 + samples_per_freq && scheduled.len() > 1 {
                        let (_consumed_idx, _, _) = scheduled.pop_front().unwrap();

                        if next_schedule_idx < n_freq && scheduled.len() < RETUNE_QUEUE_DEPTH {
                            let new_timestamp = t0 + next_schedule_idx as u64 * samples_per_freq;
                            match self.device.schedule_retune_with_duration(
                                Channel::Rx,
                                new_timestamp,
                                frequencies[next_schedule_idx] as u64,
                                Some(lms_freqs[next_schedule_idx].clone()),
                            ) {
                                Ok((_, duration)) => {
                                    scheduled.push_back((
                                        next_schedule_idx,
                                        new_timestamp,
                                        duration,
                                    ));
                                    next_schedule_idx += 1;
                                }
                                Err(libbladerf_rs::Error::RetuneQueueFull) => {}
                                Err(e) => return Err(e).context("Failed to schedule next retune"),
                            }
                        }

                        if let Some(next) = scheduled.front()
                            && next.2 > 0
                        {
                            gap_end_timestamp = next.1 + next.2;
                        }
                    } else {
                        break;
                    }
                }

                while next_schedule_idx < n_freq && scheduled.len() < RETUNE_QUEUE_DEPTH {
                    let new_timestamp = t0 + next_schedule_idx as u64 * samples_per_freq;
                    match self.device.schedule_retune_with_duration(
                        Channel::Rx,
                        new_timestamp,
                        frequencies[next_schedule_idx] as u64,
                        Some(lms_freqs[next_schedule_idx].clone()),
                    ) {
                        Ok((_, duration)) => {
                            scheduled.push_back((next_schedule_idx, new_timestamp, duration));
                            next_schedule_idx += 1;
                        }
                        Err(libbladerf_rs::Error::RetuneQueueFull) => break,
                        Err(e) => return Err(e).context("Failed to schedule next retune"),
                    }
                }

                offset += message_block_size;
            }
            streamer.recycle(buffer);
        }

        self.device
            .cancel_scheduled_retunes(Channel::Rx)
            .context("Failed to cancel scheduled retunes")?;

        let results: Vec<(f64, f64)> = frequencies
            .iter()
            .enumerate()
            .map(|(i, &freq)| {
                let mean_sq = if sample_count[i] > 0 {
                    power_accum[i] / sample_count[i] as f64
                } else {
                    0.0
                };
                (freq, mean_sq)
            })
            .collect();

        streamer.close(&mut self.device).ok();
        Ok(results)
    }

    #[allow(dead_code)]
    pub fn streaming_fcch_scan(
        &mut self,
        frequencies: &[f64],
        samples_per_freq: usize,
    ) -> Result<Vec<(f64, Vec<Complex32>)>> {
        let n_freq = frequencies.len();
        if n_freq == 0 {
            return Ok(vec![]);
        }

        self.device
            .set_sample_rate(Channel::Rx, SAMPLE_RATE)
            .context("Failed to set sample rate")?;
        self.device
            .set_bandwidth(Channel::Rx, BANDWIDTH)
            .context("Failed to set bandwidth")?;

        let lms_freqs: Vec<LmsFreq> = frequencies
            .iter()
            .map(|&f| LmsFreq::try_from(f as u64).context(format!("Invalid frequency {f}")))
            .collect::<Result<Vec<_>>>()?;

        let effective_rate = self
            .device
            .get_sample_rate(Channel::Rx)
            .unwrap_or(SAMPLE_RATE);

        let mut streamer = RxStream::builder(&mut self.device)
            .buffer_size(BUFFER_SIZE)
            .buffer_count(BUFFER_COUNT)
            .format(SampleFormat::Sc16Q11Meta)
            .build()
            .context("Failed to create RX streamer")?;

        let base_timestamp = 'discover: {
            let mut prev_ts: u64 = 0;
            let mut valid_count: usize = 0;
            let mut last_valid_ts: u64 = 0;
            for _ in 0..300 {
                let buffer = streamer
                    .read(Some(Duration::from_secs(2)))
                    .context("RX read timeout during timestamp discovery")?;
                let raw: &[u8] = &buffer;
                let mut offset = 0;
                while offset + BUFFER_SIZE <= raw.len() {
                    if let Some(header) =
                        MetadataHeader::from_bytes(&raw[offset..offset + BUFFER_SIZE])
                    {
                        if !header.is_valid_meta_format() {
                            offset += BUFFER_SIZE;
                            continue;
                        }
                        let ts = header.timestamp;
                        if ts > prev_ts {
                            valid_count += 1;
                            last_valid_ts = ts;
                            if valid_count >= 3 {
                                break 'discover last_valid_ts;
                            }
                            prev_ts = ts;
                        } else {
                            valid_count = 0;
                            prev_ts = 0;
                        }
                    }
                    offset += BUFFER_SIZE;
                }
                streamer.recycle(buffer);
            }
            last_valid_ts
        };

        let lead_time = effective_rate as u64 / 10;
        let t0 = base_timestamp + lead_time;
        let spf_u64 = samples_per_freq as u64;

        self.device
            .cancel_scheduled_retunes(Channel::Rx)
            .context("Failed to cancel scheduled retunes")?;
        self.device
            .schedule_retune(
                Channel::Rx,
                0,
                frequencies[0] as u64,
                Some(lms_freqs[0].clone()),
            )
            .context("Failed to schedule initial retune")?;

        let mut scheduled: VecDeque<(usize, u64, u64)> = VecDeque::new();
        let mut next_schedule_idx: usize = 1;

        while next_schedule_idx < n_freq && scheduled.len() < RETUNE_QUEUE_DEPTH {
            let i = next_schedule_idx;
            let timestamp = t0 + i as u64 * spf_u64;
            match self.device.schedule_retune_with_duration(
                Channel::Rx,
                timestamp,
                frequencies[i] as u64,
                Some(lms_freqs[i].clone()),
            ) {
                Ok((_, duration)) => {
                    scheduled.push_back((i, timestamp, duration));
                    next_schedule_idx += 1;
                }
                Err(libbladerf_rs::Error::RetuneQueueFull) => break,
                Err(e) => return Err(e).context("Failed to schedule retune"),
            }
        }

        let mut buckets: Vec<Vec<Complex32>> = vec![Vec::new(); n_freq];
        let mut gap_end_timestamp: u64 = 0;
        let total_samples = n_freq as u64 * spf_u64;
        let mut samples_collected: u64 = 0;

        let mut found_start = false;
        while !found_start {
            let buffer = streamer
                .read(Some(Duration::from_secs(2)))
                .context("RX read timeout during lead time")?;
            let raw: &[u8] = &buffer;
            let mut offset = 0;
            while offset + BUFFER_SIZE <= raw.len() {
                if let Some(header) = MetadataHeader::from_bytes(&raw[offset..offset + BUFFER_SIZE])
                    && header.timestamp >= t0
                {
                    found_start = true;
                    break;
                }
                offset += BUFFER_SIZE;
            }
            streamer.recycle(buffer);
        }

        while samples_collected < total_samples {
            let buffer = streamer
                .read(Some(Duration::from_secs(2)))
                .context("RX read timeout")?;
            let raw: &[u8] = &buffer;
            let mut offset = 0;
            while offset + BUFFER_SIZE <= raw.len() {
                let block = &raw[offset..offset + BUFFER_SIZE];
                let header = match MetadataHeader::from_bytes(block) {
                    Some(h) => h,
                    None => {
                        offset += BUFFER_SIZE;
                        continue;
                    }
                };
                let timestamp = header.timestamp;
                if timestamp < t0 {
                    offset += BUFFER_SIZE;
                    continue;
                }

                let payload = &block[METADATA_HEADER_SIZE..];
                let freq_idx = current_freq_index(&scheduled, timestamp);

                if timestamp < gap_end_timestamp {
                    offset += BUFFER_SIZE;
                    continue;
                }

                if let Some(front) = scheduled.front()
                    && timestamp >= front.1
                    && front.2 > 0
                    && gap_end_timestamp == 0
                {
                    gap_end_timestamp = front.1 + front.2;
                    if timestamp < gap_end_timestamp {
                        offset += BUFFER_SIZE;
                        continue;
                    }
                    gap_end_timestamp = 0;
                }

                if let Some(idx) = freq_idx
                    && buckets[idx].len() < samples_per_freq
                {
                    let room = samples_per_freq - buckets[idx].len();
                    let n = (payload.len() / 4).min(room);
                    for j in 0..n {
                        let off = j * 4;
                        let i_val = i16::from_le_bytes([payload[off], payload[off + 1]]);
                        let q_val = i16::from_le_bytes([payload[off + 2], payload[off + 3]]);
                        buckets[idx].push(Complex32::new(
                            i_val as f32 / 2_048.0,
                            q_val as f32 / 2_048.0,
                        ));
                    }
                    samples_collected += n as u64;
                }

                while let Some(front) = scheduled.front() {
                    if timestamp >= front.1 + spf_u64 && scheduled.len() > 1 {
                        let (_, _, _) = scheduled.pop_front().unwrap();
                        if next_schedule_idx < n_freq && scheduled.len() < RETUNE_QUEUE_DEPTH {
                            let new_ts = t0 + next_schedule_idx as u64 * spf_u64;
                            match self.device.schedule_retune_with_duration(
                                Channel::Rx,
                                new_ts,
                                frequencies[next_schedule_idx] as u64,
                                Some(lms_freqs[next_schedule_idx].clone()),
                            ) {
                                Ok((_, duration)) => {
                                    scheduled.push_back((next_schedule_idx, new_ts, duration));
                                    next_schedule_idx += 1;
                                }
                                Err(libbladerf_rs::Error::RetuneQueueFull) => {}
                                Err(e) => return Err(e).context("Failed to schedule next retune"),
                            }
                        }
                        if let Some(next) = scheduled.front()
                            && next.2 > 0
                        {
                            gap_end_timestamp = next.1 + next.2;
                        }
                    } else {
                        break;
                    }
                }

                while next_schedule_idx < n_freq && scheduled.len() < RETUNE_QUEUE_DEPTH {
                    let new_ts = t0 + next_schedule_idx as u64 * spf_u64;
                    match self.device.schedule_retune_with_duration(
                        Channel::Rx,
                        new_ts,
                        frequencies[next_schedule_idx] as u64,
                        Some(lms_freqs[next_schedule_idx].clone()),
                    ) {
                        Ok((_, duration)) => {
                            scheduled.push_back((next_schedule_idx, new_ts, duration));
                            next_schedule_idx += 1;
                        }
                        Err(libbladerf_rs::Error::RetuneQueueFull) => break,
                        Err(e) => return Err(e).context("Failed to schedule next retune"),
                    }
                }

                offset += BUFFER_SIZE;
            }
            streamer.recycle(buffer);
        }

        self.device
            .cancel_scheduled_retunes(Channel::Rx)
            .context("Failed to cancel scheduled retunes")?;

        let results: Vec<(f64, Vec<Complex32>)> = frequencies
            .iter()
            .zip(buckets)
            .map(|(&f, b)| (f, b))
            .collect();

        streamer.close(&mut self.device).ok();
        Ok(results)
    }
}

const RETUNE_QUEUE_DEPTH: usize = 16;

#[derive(Debug, Clone, Copy)]
pub struct ScanParams {
    pub sample_rate: u32,
    pub samples_per_freq: u64,
}

pub fn derive_scan_params(usb_speed: Speed) -> ScanParams {
    let sample_rate = 4_000_000u32;

    let samples_per_freq = match usb_speed {
        Speed::High => 4_000,
        Speed::Super | Speed::SuperPlus => 40_000,
        _ => 1_000,
    };

    ScanParams {
        sample_rate,
        samples_per_freq,
    }
}

const BYTES_PER_SAMPLE: usize = 2;

fn compute_power_raw(payload: &[u8]) -> (f64, usize) {
    let n_samples = payload.len() / BYTES_PER_SAMPLE;
    let mut sum_sq: f64 = 0.0;
    for i in 0..n_samples {
        let off = i * BYTES_PER_SAMPLE;
        let i_val = payload[off] as i8 as f64;
        let q_val = payload[off + 1] as i8 as f64;
        sum_sq += i_val * i_val + q_val * q_val;
    }
    (sum_sq, n_samples)
}

fn current_freq_index(scheduled: &VecDeque<(usize, u64, u64)>, timestamp: u64) -> Option<usize> {
    for &(idx, start_ts, _) in scheduled.iter().rev() {
        if timestamp >= start_ts {
            return Some(idx);
        }
    }
    scheduled.front().map(|(idx, _, _)| *idx)
}
