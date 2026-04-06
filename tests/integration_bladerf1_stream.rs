#[path = "common/mod.rs"]
mod common;

use common::*;

use libbladerf_rs::Result;
use libbladerf_rs::bladerf1::board::{BladeRf1RxStreamer, BladeRf1TxStreamer, SampleFormat};
use std::time::Duration;

#[test]
fn rx_stream() -> Result<()> {
    logging_init("bladerf1_stream");

    let mut streamer = BladeRf1RxStreamer::new(
        BLADERF.clone(),
        65536, // buffer_size (must be multiple of 512)
        8,     // buffer_count
        SampleFormat::Sc16Q11,
    )?;
    streamer.activate()?;

    // Read a buffer using the new ownership-based API
    let buffer = streamer.read(None)?;
    let n = buffer.len();

    assert!(n > 0);
    log::trace!("Read {n} bytes!");

    // Recycle the buffer back to the pool
    streamer.recycle(buffer)?;

    Ok(())
}

#[test]
fn tx_stream() -> Result<()> {
    logging_init("bladerf1_stream");

    let mut streamer = BladeRf1TxStreamer::new(
        BLADERF.clone(),
        65536, // buffer_size (must be multiple of 512)
        8,     // buffer_count
        SampleFormat::Sc16Q11,
    )?;
    streamer.activate()?;

    // Get a buffer, fill it with samples, and submit
    let mut buffer = streamer.get_buffer(None)?;
    // 8192 samples * 4 bytes per sample (SC16Q11: 2x i16)
    buffer.extend_from_slice(&[0u8; 32768]);
    streamer.submit(buffer, 32768)?;

    // Wait for completion
    streamer.wait_completion(Some(Duration::from_millis(20000)))?;

    log::trace!("Wrote 32768 bytes!");

    Ok(())
}
