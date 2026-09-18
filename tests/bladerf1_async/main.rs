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
    let original_rx_sr = rf.get_sample_rate(Channel::Rx).await?;
    let original_tx_sr = rf.get_sample_rate(Channel::Tx).await?;
    rf.set_sample_rate(Channel::Rx, 2_000_000).await?;
    rf.set_sample_rate(Channel::Tx, 2_000_000).await?;
    rf.set_loopback(Loopback::Firmware).await?;

    let num_samples = 2048;
    let buffer_size = num_samples * 4;
    let mut rx = RxStream::builder(&mut rf)
        .buffer_size(buffer_size)
        .buffer_count(8)
        .format(SampleFormat::Sc16Q11)
        .build()
        .await?;
    rx.start(&mut rf).await?;
    let mut tx = TxStream::builder(&mut rf)
        .buffer_size(buffer_size)
        .buffer_count(8)
        .format(SampleFormat::Sc16Q11)
        .build()
        .await?;
    tx.start(&mut rf).await?;

    let tx_data: Vec<u8> = (0..num_samples)
        .flat_map(|i| {
            let phase = (i as f32 * 2.0 * std::f32::consts::PI / 64.0).sin();
            let val = (phase * 2047.0) as i16;
            let bytes = val.to_le_bytes();
            [bytes[0], bytes[1], bytes[0], bytes[1]]
        })
        .collect();
    let mut tx_buf = tx.get_buffer(None).await?;
    tx_buf.extend_from_slice(&tx_data);
    tx.submit(tx_buf, tx_data.len())?;
    tokio::time::timeout(Duration::from_secs(5), tx.wait_completion(None))
        .await
        .expect("TX completion timed out")?;

    let rx_buf = tokio::time::timeout(Duration::from_secs(5), rx.read(None))
        .await
        .expect("RX read timed out")?;
    let non_zero = rx_buf
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|chunk| **chunk != [0, 0, 0, 0])
        .count();
    log::info!(
        "async firmware loopback: {} bytes, {non_zero} non-zero samples",
        rx_buf.len()
    );
    assert!(
        non_zero > num_samples / 4,
        "got {non_zero} non-zero samples"
    );
    rx.recycle(rx_buf);

    rx.close(&mut rf).await?;
    tx.close(&mut rf).await?;
    rf.set_loopback(Loopback::None).await?;
    rf.set_sample_rate(Channel::Rx, original_rx_sr).await?;
    rf.set_sample_rate(Channel::Tx, original_tx_sr).await?;
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
