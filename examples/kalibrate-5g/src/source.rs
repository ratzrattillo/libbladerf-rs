use std::time::Duration;

use anyhow::{Context, Result};
use libbladerf_rs::bladerf1::{
    BladeRf1, GainMode, METADATA_HEADER_SIZE, MetadataHeader, RxStream, SampleFormat, TuningMode,
};
use libbladerf_rs::channel::Channel;
use nusb::Speed;
use rustfft::num_complex::Complex32;

use crate::detect::ssb_detect_all;

pub const TUNING_OFFSET_HZ: f64 = 1_500_000.0;
const SETTLE_BLOCKS: usize = 4;

pub struct Source {
    device: BladeRf1,
}

impl Source {
    pub fn open() -> Result<Self> {
        let mut device = BladeRf1::from_first().context("No bladeRF device found")?;
        device
            .initialize(false)
            .context("Device initialization failed")?;

        Ok(Self { device })
    }

    pub fn set_gain(&mut self, gain_db: i8) -> Result<()> {
        self.device
            .set_gain(Channel::Rx, gain_db.into())
            .context("Failed to set gain")
    }

    #[allow(dead_code)]
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

    #[allow(dead_code)]
    pub fn save_dac_trim(&mut self, value: u16) -> Result<()> {
        self.device
            .save_dac_trim(value)
            .with_context(|| format!("Failed to save DAC trim {value} to flash"))
    }

    pub fn serial(&self) -> Result<String> {
        self.device.serial().context("Failed to read serial number")
    }

    #[allow(dead_code)]
    pub fn device(&self) -> &BladeRf1 {
        &self.device
    }

    pub fn streaming_ssb_scan(
        &mut self,
        tune_centers: &[f64],
    ) -> Result<Vec<(f64, Vec<crate::detect::SsbDetectResult>)>> {
        let n_freq = tune_centers.len();
        if n_freq == 0 {
            return Ok(vec![]);
        }

        let speed = self.device.speed().context("Failed to get device speed")?;
        let params = derive_scan_params(speed);
        eprintln!(
            "[*] Scan params: rate={} Ms/s, bw={} MHz, dwell={:.1} ms/freq, usb={:?}",
            params.sample_rate as f64 / 1e6,
            params.bandwidth as f64 / 1e6,
            params.samples_per_freq as f64 / params.sample_rate as f64 * 1_000.0,
            speed
        );

        self.device
            .set_sample_rate(Channel::Rx, params.sample_rate)
            .context("Failed to set sample rate")?;
        self.device
            .set_bandwidth(Channel::Rx, params.bandwidth)
            .context("Failed to set bandwidth")?;
        self.device
            .set_gain_mode(Channel::Rx, GainMode::Mgc)
            .context("Failed to set gain mode")?;

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

        let mut results: Vec<(f64, Vec<crate::detect::SsbDetectResult>)> =
            Vec::with_capacity(n_freq);
        let scan_start = std::time::Instant::now();

        for (idx, &center_freq) in tune_centers.iter().enumerate() {
            let tune_freq = (center_freq + TUNING_OFFSET_HZ) as u64;
            self.device
                .set_frequency(Channel::Rx, tune_freq, TuningMode::Fpga)
                .context("Failed to set frequency")?;

            for _ in 0..SETTLE_BLOCKS {
                if let Ok(buf) = streamer.read(Some(Duration::from_millis(200))) {
                    streamer.recycle(buf);
                }
            }

            let dwell_ms =
                (params.samples_per_freq as f64 / params.sample_rate as f64 * 1_000.0) as u64;
            let deadline = std::time::Instant::now() + Duration::from_millis(dwell_ms + 500);

            let mut samples: Vec<Complex32> = Vec::with_capacity(params.samples_per_freq as usize);
            let mut buf_count: usize = 0;
            let mut total_read_us: u64 = 0;
            let mut total_convert_us: u64 = 0;
            while samples.len() < params.samples_per_freq as usize
                && std::time::Instant::now() < deadline
            {
                let t_read = std::time::Instant::now();
                let buffer = match streamer.read(Some(Duration::from_millis(500))) {
                    Ok(b) => b,
                    Err(_) => break,
                };
                let read_us = t_read.elapsed().as_micros() as u64;
                total_read_us += read_us;

                let t_convert = std::time::Instant::now();
                let raw: &[u8] = &buffer;
                let mut offset = 0;
                while offset + message_block_size <= raw.len() {
                    let block = &raw[offset..offset + message_block_size];
                    if let Some(header) = MetadataHeader::from_bytes(block) {
                        if !header.is_valid_meta_format() {
                            offset += message_block_size;
                            continue;
                        }
                    } else {
                        offset += message_block_size;
                        continue;
                    }

                    let payload = &block[METADATA_HEADER_SIZE..];
                    let payload_samples = payload.len() / 2;
                    let room = (params.samples_per_freq as usize).saturating_sub(samples.len());
                    let to_take = payload_samples.min(room);

                    for j in 0..to_take {
                        let off = j * 2;
                        samples.push(Complex32::new(
                            payload[off] as i8 as f32 / 128.0,
                            payload[off + 1] as i8 as f32 / 128.0,
                        ));
                    }

                    offset += message_block_size;
                }
                streamer.recycle(buffer);
                let convert_us = t_convert.elapsed().as_micros() as u64;
                total_convert_us += convert_us;

                buf_count += 1;
            }

            if buf_count > 0 && buf_count % 20 == 1 {
                eprintln!(
                    "    [perf] freq={center_freq:.0}Hz buf#{buf_count} read={total_read_us}µs convert={total_convert_us}µs samples={}",
                    samples.len()
                );
            }

            let detections: Vec<_> = ssb_detect_all(&samples, params.sample_rate as f64)
                .into_iter()
                .filter(|r| r.detected)
                .collect();

            if detections.is_empty() {
                results.push((
                    center_freq,
                    vec![crate::detect::SsbDetectResult {
                        detected: false,
                        par_db: f64::NEG_INFINITY,
                        peak_ssb_power: 0.0,
                        noise_floor_per_bin: 0.0,
                        freq_offset_hz: 0.0,
                        fill_rate: 0.0,
                        center_freq_hz: 0.0,
                        n_id_2: None,
                        pss_snr_db: None,
                    }],
                ));
            } else {
                results.push((center_freq, detections));
            }

            eprintln!(
                "[*] [{:.1}s] Scan progress: {}/{} ({:.0}%)",
                scan_start.elapsed().as_secs_f64(),
                idx + 1,
                n_freq,
                (idx + 1) as f64 / n_freq as f64 * 100.0
            );
        }

        let elapsed = scan_start.elapsed();
        eprintln!(
            "[*] Scan complete: {n_freq} frequencies in {:.1}s ({:.1} freq/s)",
            elapsed.as_secs_f64(),
            n_freq as f64 / elapsed.as_secs_f64()
        );

        let _ = streamer.close(&mut self.device);
        Ok(results)
    }

    pub fn measure_pss_offset(
        &mut self,
        ssb_abs_freq_hz: f64,
    ) -> Result<Option<crate::pss::PssCorrelationResult>> {
        let narrow_offset_hz = 1_000_000.0;
        let tune_freq = (ssb_abs_freq_hz + narrow_offset_hz) as u64;
        let narrow_rate: u32 = 4_000_000;
        let narrow_bw: u32 = 3_000_000;
        let n_samples: usize = 4_000_000;

        self.device
            .set_sample_rate(Channel::Rx, narrow_rate)
            .context("Failed to set narrow sample rate")?;
        self.device
            .set_bandwidth(Channel::Rx, narrow_bw)
            .context("Failed to set narrow bandwidth")?;
        self.device
            .set_frequency(Channel::Rx, tune_freq, TuningMode::Fpga)
            .context("Failed to set narrow frequency")?;

        let speed = self.device.speed().context("Failed to get device speed")?;
        let message_block_size = match speed {
            Speed::High => 2_048,
            _ => 4_096,
        };

        let buffer_size = message_block_size * 4;
        let buffer_count = 32;
        let mut streamer = RxStream::builder(&mut self.device)
            .buffer_size(buffer_size)
            .buffer_count(buffer_count)
            .format(SampleFormat::Sc8Q7Meta)
            .build()
            .context("Failed to create narrow RX streamer")?;

        for _ in 0..4 {
            if let Ok(buf) = streamer.read(Some(Duration::from_millis(200))) {
                streamer.recycle(buf);
            }
        }

        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        let mut samples: Vec<Complex32> = Vec::with_capacity(n_samples);
        while samples.len() < n_samples && std::time::Instant::now() < deadline {
            let buffer = match streamer.read(Some(Duration::from_millis(500))) {
                Ok(b) => b,
                Err(_) => break,
            };

            let raw: &[u8] = &buffer;
            let mut offset = 0;
            while offset + message_block_size <= raw.len() {
                let block = &raw[offset..offset + message_block_size];
                if let Some(header) = MetadataHeader::from_bytes(block) {
                    if !header.is_valid_meta_format() {
                        offset += message_block_size;
                        continue;
                    }
                } else {
                    offset += message_block_size;
                    continue;
                }

                let payload = &block[METADATA_HEADER_SIZE..];
                let payload_samples = payload.len() / 2;
                let room = n_samples.saturating_sub(samples.len());
                let to_take = payload_samples.min(room);

                for j in 0..to_take {
                    let off = j * 2;
                    samples.push(Complex32::new(
                        payload[off] as i8 as f32 / 128.0,
                        payload[off + 1] as i8 as f32 / 128.0,
                    ));
                }

                offset += message_block_size;
            }
            streamer.recycle(buffer);
        }

        let _ = streamer.close(&mut self.device);

        if samples.len() < 65_536 {
            eprintln!("[!] Not enough narrow samples ({})", samples.len());
            return Ok(None);
        }

        crate::pss::cancel_dominant_tone(&mut samples, narrow_rate as f64);

        let search_center_hz = -narrow_offset_hz;
        let result =
            crate::pss::pss_correlate(&samples, narrow_rate as f64, search_center_hz, 300_000.0);

        Ok(result)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ScanParams {
    pub sample_rate: u32,
    pub bandwidth: u32,
    pub samples_per_freq: u64,
}

pub fn derive_scan_params(usb_speed: Speed) -> ScanParams {
    let (sample_rate, bandwidth) = match usb_speed {
        Speed::High => (20_000_000u32, 14_000_000u32),
        _ => (40_000_000u32, 28_000_000u32),
    };

    let samples_per_freq = match usb_speed {
        Speed::High => 8_000_000,
        Speed::Super | Speed::SuperPlus => 16_000_000,
        _ => 1_600_000,
    };

    ScanParams {
        sample_rate,
        bandwidth,
        samples_per_freq,
    }
}
