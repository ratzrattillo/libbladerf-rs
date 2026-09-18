use super::common::*;
use libbladerf_rs::Channel;
use libbladerf_rs::MaybeFuture;
use libbladerf_rs::Result;
use libbladerf_rs::bladerf1::board::SampleFormat;
use libbladerf_rs::bladerf1::hardware::lms6002d::loopback::Loopback;
use libbladerf_rs::bladerf1::{RxStream, TuningMode, TxStream};
use std::time::Duration;

#[test]
fn loopback_set_get_roundtrip() -> Result<()> {
    logging_init("bladerf1_loopback");

    let mut sdr = sdr();
    let mut rf = sdr.rf_link_session().wait()?;
    rf.set_lms_loopback(Loopback::None).wait()?;

    for desired in [
        Loopback::None,
        Loopback::BbTxlpfRxlpf,
        Loopback::BbTxlpfRxvga2,
        Loopback::BbTxvga1Rxlpf,
        Loopback::BbTxvga1Rxvga2,
        Loopback::Lna1,
        Loopback::Lna2,
        Loopback::Lna3,
    ] {
        rf.set_lms_loopback(desired).wait()?;

        let actual = rf.get_lms_loopback().wait()?;
        log::trace!("LMS Loopback (DESIRED):\t{desired:?}");
        log::trace!("LMS Loopback (ACTUAL):\t{actual:?}");
        assert_eq!(actual, desired);
    }

    rf.set_lms_loopback(Loopback::None).wait()?;

    Ok(())
}

#[test]
fn firmware_loopback_stream() -> Result<()> {
    logging_init("bladerf1_loopback");

    let mut sdr = sdr();
    let mut rf = sdr.rf_link_session().wait()?;
    let original_rx_sr = rf.get_sample_rate(Channel::Rx).wait()?;
    let original_tx_sr = rf.get_sample_rate(Channel::Tx).wait()?;

    rf.set_sample_rate(Channel::Rx, 2_000_000).wait()?;
    rf.set_sample_rate(Channel::Tx, 2_000_000).wait()?;
    rf.set_loopback(Loopback::Firmware).wait()?;

    let num_samples = 2048;
    let buffer_size = num_samples * 4;

    let mut rx_stream = {
        RxStream::builder(&mut rf)
            .buffer_size(buffer_size)
            .buffer_count(8)
            .format(SampleFormat::Sc16Q11)
            .build()
            .wait()?
    };
    rx_stream.start(&mut rf).wait()?;

    let mut tx_stream = {
        TxStream::builder(&mut rf)
            .buffer_size(buffer_size)
            .buffer_count(8)
            .format(SampleFormat::Sc16Q11)
            .build()
            .wait()?
    };
    tx_stream.start(&mut rf).wait()?;

    let tx_data: Vec<u8> = (0..num_samples)
        .flat_map(|i| {
            let phase = (i as f32 * 2.0 * std::f32::consts::PI / 64.0).sin();
            let val = (phase * 2047.0) as i16;
            let bytes = val.to_le_bytes();
            [bytes[0], bytes[1], bytes[0], bytes[1]]
        })
        .collect();

    let mut tx_buf = tx_stream.get_buffer(Some(Duration::from_secs(2))).wait()?;
    tx_buf.extend_from_slice(&tx_data);
    tx_stream.submit(tx_buf, tx_data.len())?;

    let rx_buf = rx_stream.read(Some(Duration::from_secs(5))).wait()?;
    let rx_data: &[u8] = &rx_buf;

    let non_zero = rx_data
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|chunk| **chunk != [0, 0, 0, 0])
        .count();
    log::trace!(
        "Firmware loopback: received {} bytes, {} non-zero samples out of {}",
        rx_data.len(),
        non_zero,
        rx_data.len() / 4
    );

    assert!(
        non_zero > num_samples / 4,
        "Firmware loopback should return non-zero samples, got {non_zero} out of {}",
        rx_data.len() / 4
    );

    rx_stream.recycle(rx_buf);
    rx_stream.close(&mut rf).wait()?;
    tx_stream.close(&mut rf).wait()?;

    rf.set_loopback(Loopback::None).wait()?;
    rf.set_sample_rate(Channel::Rx, original_rx_sr).wait()?;
    rf.set_sample_rate(Channel::Tx, original_tx_sr).wait()?;

    Ok(())
}

fn run_loopback_stream_test(loopback_mode: Loopback, test_name: &str) -> Result<()> {
    logging_init("bladerf1_loopback");

    let mut sdr = sdr();
    let mut rf = sdr.rf_link_session().wait()?;
    let original_rx_freq = rf.get_frequency(Channel::Rx).wait()?;
    let original_tx_freq = rf.get_frequency(Channel::Tx).wait()?;
    let original_rx_sr = rf.get_sample_rate(Channel::Rx).wait()?;
    let original_tx_sr = rf.get_sample_rate(Channel::Tx).wait()?;

    rf.set_sample_rate(Channel::Rx, 2_000_000).wait()?;
    rf.set_sample_rate(Channel::Tx, 2_000_000).wait()?;
    rf.set_frequency(Channel::Rx, 1_000_000_000, TuningMode::Fpga)
        .wait()?;
    rf.set_frequency(Channel::Tx, 1_000_000_000, TuningMode::Fpga)
        .wait()?;
    rf.set_lms_loopback(loopback_mode).wait()?;

    let num_samples = 2048;
    let buffer_size = num_samples * 4;

    let mut rx_stream = {
        RxStream::builder(&mut rf)
            .buffer_size(buffer_size)
            .buffer_count(8)
            .format(SampleFormat::Sc16Q11)
            .build()
            .wait()?
    };
    rx_stream.start(&mut rf).wait()?;

    let mut tx_stream = {
        TxStream::builder(&mut rf)
            .buffer_size(buffer_size)
            .buffer_count(8)
            .format(SampleFormat::Sc16Q11)
            .build()
            .wait()?
    };
    tx_stream.start(&mut rf).wait()?;

    let tx_data: Vec<u8> = (0..num_samples)
        .flat_map(|i| {
            let phase = (i as f32 * 2.0 * std::f32::consts::PI / 64.0).sin();
            let val = (phase * 2047.0) as i16;
            let bytes = val.to_le_bytes();
            [bytes[0], bytes[1], bytes[0], bytes[1]]
        })
        .collect();

    let mut tx_buf = tx_stream.get_buffer(Some(Duration::from_secs(2))).wait()?;
    tx_buf.extend_from_slice(&tx_data);
    tx_stream.submit(tx_buf, tx_data.len())?;

    let rx_buf = rx_stream.read(Some(Duration::from_secs(5))).wait()?;
    let rx_data: &[u8] = &rx_buf;

    let non_zero = rx_data
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|chunk| **chunk != [0, 0, 0, 0])
        .count();
    log::trace!(
        "{test_name}: received {} bytes, {} non-zero samples out of {}",
        rx_data.len(),
        non_zero,
        rx_data.len() / 4
    );

    assert!(
        non_zero > num_samples / 4,
        "{test_name} should return non-zero samples, got {non_zero} out of {}",
        rx_data.len() / 4
    );

    rx_stream.recycle(rx_buf);
    rx_stream.close(&mut rf).wait()?;
    tx_stream.close(&mut rf).wait()?;

    rf.set_lms_loopback(Loopback::None).wait()?;
    rf.set_frequency(Channel::Rx, original_rx_freq, TuningMode::Fpga)
        .wait()?;
    rf.set_frequency(Channel::Tx, original_tx_freq, TuningMode::Fpga)
        .wait()?;
    rf.set_sample_rate(Channel::Rx, original_rx_sr).wait()?;
    rf.set_sample_rate(Channel::Tx, original_tx_sr).wait()?;

    Ok(())
}

#[test]
fn bb_txlpf_rxlpf_loopback_stream() -> Result<()> {
    run_loopback_stream_test(Loopback::BbTxlpfRxlpf, "BB TXLPF RXLPF loopback")
}

#[test]
fn bb_txvga1_rxvga2_loopback_stream() -> Result<()> {
    run_loopback_stream_test(Loopback::BbTxvga1Rxvga2, "BB TXVGA1 RXVGA2 loopback")
}

#[test]
fn lna3_loopback_stream() -> Result<()> {
    run_loopback_stream_test(Loopback::Lna3, "LNA3 RF loopback")
}
