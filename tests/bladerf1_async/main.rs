//! Hardware tests for the awaited (`.await`) path of the MaybeFuture API.
//!
//! Each test opens the device itself so the futures never hold a mutex
//! guard across an await point. Runs on a current-thread tokio runtime,
//! but the driver needs no runtime feature: only `tokio::time::timeout`
//! is used, to bound reads.

use libbladerf_rs::bladerf1::board::SampleFormat;
use libbladerf_rs::bladerf1::hardware::lms6002d::loopback::Loopback;
use libbladerf_rs::bladerf1::{BladeRf1, GainDb, RxStream, TuningMode, TxStream};
use libbladerf_rs::{Channel, Result};
use std::time::Duration;

mod connection;
mod metadata;
mod recovery;

fn logging_init() {
    let _ = env_logger::builder()
        .is_test(true)
        .filter_level(log::LevelFilter::Info)
        .try_init();
}

async fn open() -> Result<BladeRf1> {
    let mut dev = BladeRf1::from_first().await?;
    dev.rf_link_session().await?.initialize(false).await?;
    Ok(dev)
}

#[tokio::test(flavor = "current_thread")]
async fn open_initialize_close() -> Result<()> {
    logging_init();
    let mut dev = open().await?;
    let version = dev.rf_link_session().await?.fpga_version().await?;
    log::info!("FPGA version {version}");
    dev.close().await
}

#[tokio::test(flavor = "current_thread")]
async fn tuning_sample_rate_gain() -> Result<()> {
    logging_init();
    let mut dev = open().await?;
    let mut rf = dev.rf_link_session().await?;

    let original_freq = rf.get_frequency(Channel::Rx).await?;
    let original_sr = rf.get_sample_rate(Channel::Rx).await?;
    let original_gain = rf.get_gain(Channel::Rx).await?;

    rf.set_frequency(Channel::Rx, 915_000_000, TuningMode::Fpga)
        .await?;
    let freq = rf.get_frequency(Channel::Rx).await?;
    assert!((freq as i64 - 915_000_000).abs() < 1_000, "got {freq}");

    let actual = rf.set_sample_rate(Channel::Rx, 4_000_000).await?;
    assert!((actual as i64 - 4_000_000).abs() < 1_000, "got {actual}");
    assert_eq!(rf.get_sample_rate(Channel::Rx).await?, actual);

    rf.set_gain(Channel::Rx, GainDb::from(30)).await?;
    let gain = rf.get_gain(Channel::Rx).await?;
    assert!((gain.db() - 30).abs() <= 3, "got {gain:?}");

    rf.set_frequency(Channel::Rx, original_freq, TuningMode::Fpga)
        .await?;
    rf.set_sample_rate(Channel::Rx, original_sr).await?;
    rf.set_gain(Channel::Rx, original_gain).await?;
    dev.close().await
}

#[tokio::test(flavor = "current_thread")]
async fn firmware_loopback_stream() -> Result<()> {
    logging_init();
    let mut dev = open().await?;
    let mut rf = dev.rf_link_session().await?;
    let original_loopback = rf.get_loopback().await?;
    rf.set_loopback(Loopback::Firmware).await?;
    let buffer_size = 8192;
    let mut rx = RxStream::builder(&mut rf)
        .buffer_size(buffer_size)
        .buffer_count(4)
        .format(SampleFormat::Sc16Q11)
        .build()
        .await?;
    let mut tx = TxStream::builder(&mut rf)
        .buffer_size(buffer_size)
        .buffer_count(4)
        .format(SampleFormat::Sc16Q11)
        .build()
        .await?;
    let mut expected = vec![0; buffer_size];
    let mut mismatches = 0;
    let result: Result<()> = async {
        for restart in 0..3 {
            rx.start(&mut rf).await?;
            tx.start(&mut rf).await?;
            for frame in 0..64 {
                for (word, bytes) in expected.as_chunks_mut::<4>().0.iter_mut().enumerate() {
                    let index = ((restart * 64 + frame) * (buffer_size / 4) + word) as u32;
                    *bytes = index.wrapping_mul(0x9e37_79b9).to_le_bytes();
                }
                let mut buffer = tx.get_buffer(None).await?;
                buffer.extend_from_slice(&expected);
                tx.submit(buffer, buffer_size)?;
                tokio::time::timeout(Duration::from_secs(2), tx.wait_completion(None))
                    .await
                    .map_err(|_| libbladerf_rs::Error::Timeout)??;
                let buffer = tokio::time::timeout(Duration::from_secs(2), rx.read(None))
                    .await
                    .map_err(|_| libbladerf_rs::Error::Timeout)??;
                if buffer[..] != expected {
                    mismatches += 1;
                }
                rx.recycle(buffer);
            }
            for _ in 0..rx.pending_transfers()? {
                let mut buffer = tx.get_buffer(None).await?;
                buffer.extend_from_slice(&expected);
                tx.submit(buffer, buffer_size)?;
                tokio::time::timeout(Duration::from_secs(2), tx.wait_completion(None))
                    .await
                    .map_err(|_| libbladerf_rs::Error::Timeout)??;
            }
            rx.stop(&mut rf).await?;
            tx.stop(&mut rf).await?;
        }
        Ok(())
    }
    .await;
    let close_rx = rx.close(&mut rf).await;
    let close_tx = tx.close(&mut rf).await;
    let restore = rf.set_loopback(original_loopback).await;
    result?;
    close_rx?;
    close_tx?;
    restore?;
    assert_eq!(
        mismatches, 0,
        "numbered loopback payloads must match byte for byte"
    );
    println!("192 numbered loopback buffers verified across three starts");
    dev.close().await
}

#[tokio::test(flavor = "current_thread")]
async fn read_is_cancel_safe() -> Result<()> {
    logging_init();
    let mut dev = open().await?;
    let mut rf = dev.rf_link_session().await?;
    let original_sr = rf.get_sample_rate(Channel::Rx).await?;
    rf.set_sample_rate(Channel::Rx, 1_000_000).await?;

    let mut rx = RxStream::builder(&mut rf)
        .buffer_size(16 * 1024)
        .buffer_count(4)
        .build()
        .await?;
    rx.start(&mut rf).await?;

    let mut cancelled = 0;
    for _ in 0..20 {
        match tokio::time::timeout(Duration::from_micros(10), rx.read(None)).await {
            Ok(buf) => rx.recycle(buf?),
            Err(_) => cancelled += 1,
        }
    }
    log::info!("{cancelled} of 20 reads were cancelled by the timeout");

    for _ in 0..8 {
        let buf = tokio::time::timeout(Duration::from_secs(5), rx.read(None))
            .await
            .expect("RX read timed out after cancellations")?;
        assert_eq!(buf.len(), 16 * 1024);
        rx.recycle(buf);
    }

    rx.close(&mut rf).await?;
    rf.set_sample_rate(Channel::Rx, original_sr).await?;
    dev.close().await
}

#[tokio::test(flavor = "current_thread")]
async fn stream_stop_restart() -> Result<()> {
    logging_init();
    let mut dev = open().await?;
    let mut rf = dev.rf_link_session().await?;

    let mut rx = RxStream::builder(&mut rf).buffer_count(4).build().await?;
    for _ in 0..2 {
        rx.start(&mut rf).await?;
        let buf = tokio::time::timeout(Duration::from_secs(5), rx.read(None))
            .await
            .expect("RX read timed out")?;
        rx.recycle(buf);
        rx.stop(&mut rf).await?;
    }
    rx.close(&mut rf).await?;
    assert!(matches!(
        rx.close(&mut rf).await,
        Err(libbladerf_rs::Error::StreamClosed)
    ));
    dev.close().await
}
