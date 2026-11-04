//! Receives a few buffers using the awaited (`.await`) API.
//!
//! The driver is executor-agnostic; tokio is used here only for the
//! runtime and `tokio::time::timeout`. On wasm the same code runs under
//! `wasm_bindgen_futures::spawn_local` after opening the device with
//! `BladeRf1::from_device(nusb::Device::from_js(..).await?)`.

use anyhow::Result;
use libbladerf_rs::Channel;
use libbladerf_rs::bladerf1::{BladeRf1, RxStream, SampleFormat, TuningMode};
use std::time::Duration;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .init();

    let mut dev = BladeRf1::from_first().await?;
    let mut rf = dev.rf_link_session().await?;
    rf.initialize(false).await?;
    rf.set_frequency(Channel::Rx, 915_000_000, TuningMode::Fpga)
        .await?;
    rf.set_sample_rate(Channel::Rx, 2_000_000).await?;

    let mut rx = RxStream::builder(&mut rf)
        .buffer_size(65_536)
        .buffer_count(8)
        .format(SampleFormat::Sc16Q11)
        .build()
        .await?;
    let result: Result<()> = async {
        rx.start(&mut rf).await?;
        for i in 0..10 {
            let buffer = tokio::time::timeout(Duration::from_secs(2), rx.read(None)).await??;
            println!(
                "buffer {i}: {} bytes, first samples {:02x?}",
                buffer.len(),
                &buffer[..8.min(buffer.len())]
            );
            rx.recycle(buffer);
        }
        Ok(())
    }
    .await;
    let close_rx = rx.close(&mut rf).await;
    let close_dev = dev.close().await;
    result?;
    close_rx?;
    close_dev?;
    Ok(())
}
