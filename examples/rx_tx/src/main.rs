use anyhow::Result;
use libbladerf_rs::Channel;
use libbladerf_rs::bladerf1::board::{BladeRf1RxStreamer, BladeRf1TxStreamer};
use libbladerf_rs::bladerf1::hardware::lms6002d::gain::GainDb;
use libbladerf_rs::bladerf1::xb::ExpansionBoard;
use libbladerf_rs::bladerf1::xb::ExpansionBoard::XbNone;
use libbladerf_rs::bladerf1::{BladeRf1, SampleFormat};
use std::thread::sleep;
use std::time::Duration;

// RX
fn do_rx(bladerf: &BladeRf1) -> Result<()> {
    let mut rx_streamer =
        BladeRf1RxStreamer::new(bladerf.clone(), 65536, 8, SampleFormat::Sc16Q11)?;

    rx_streamer.activate()?;

    // Read a buffer using the new ownership-based API
    let buffer = rx_streamer.read(None)?;
    let n = buffer.len();

    println!("Read {} bytes via zero-copy DMA buffer", n);
    println!("First 32 bytes: {:02x?}", &buffer[..32.min(buffer.len())]);

    // Recycle the buffer back to the pool
    rx_streamer.recycle(buffer)?;

    rx_streamer.deactivate()?;

    Ok(())
}

// TX
fn _do_tx(bladerf: &BladeRf1) -> Result<()> {
    println!("called do_tx()");
    sleep(Duration::from_millis(5000));
    bladerf.perform_format_config(Channel::Tx, SampleFormat::Sc16Q11)?;
    println!("called perform_format_config(Channel::Tx, SampleFormat::Sc16Q11)");
    sleep(Duration::from_millis(5000));
    bladerf.enable_module(Channel::Tx, true)?;
    println!("called enable_module(Channel::Tx, true)");
    sleep(Duration::from_millis(5000));

    let mut tx_streamer =
        BladeRf1TxStreamer::new(bladerf.clone(), 32768, 8, SampleFormat::Sc16Q11)?;

    tx_streamer.activate()?;

    // 5000 samples * 4 bytes per sample (SC16Q11: 2x i16)
    // Each sample: I=2047 (0x07FF), Q=2047 (0x07FF) in little-endian
    let buf: Vec<u8> = (0..5000).flat_map(|_| [0xFF, 0x07, 0xFF, 0x07]).collect();

    for _ in 0..10 {
        let mut buffer = tx_streamer.get_buffer(None)?;
        buffer.extend_from_slice(&buf);
        tx_streamer.submit(buffer, buf.len())?;
        tx_streamer.wait_completion(Some(Duration::from_millis(5000)))?;
        println!("Submitted buffer");
    }

    sleep(Duration::from_millis(5000));

    tx_streamer.deactivate()?;
    println!("called enable_module(Channel::Tx, false)");

    Ok(())
}

fn main() -> Result<()> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .filter_module("nusb", log::LevelFilter::Info)
        .init();

    let frequency: u64 = 100_000_000;
    let bladerf = BladeRf1::from_first()?;

    bladerf.initialize()?;

    let frequency_range = bladerf.get_frequency_range()?;
    log::debug!("Frequency Range: {frequency_range:?}");

    if frequency < frequency_range.min().unwrap() as u64 {
        let xb = bladerf.expansion_get_attached()?;
        log::debug!("XB: {xb:?}");
        if xb == XbNone {
            bladerf.expansion_attach(ExpansionBoard::Xb200)?;

            let xb = bladerf.expansion_get_attached()?;
            log::debug!("XB: {xb:?}");
        }
    }

    bladerf.set_frequency(Channel::Rx, frequency)?;
    let gain_range_rx = BladeRf1::get_gain_range(Channel::Rx);
    log::debug!("Gain Range RX: {gain_range_rx:?}");
    bladerf.set_gain(
        Channel::Rx,
        GainDb {
            db: gain_range_rx.min().unwrap() as i8,
        },
    )?;

    let gain_rx = bladerf.get_gain(Channel::Rx)?;
    log::debug!("Gain RX: {}", gain_rx.db);

    do_rx(&bladerf)?;

    // bladerf.set_frequency(Channel::Tx, frequency)?;
    //
    // let gain_range_tx = BladeRf1::get_gain_range(Channel::Tx);
    // log::debug!("Gain Range TX: {gain_range_tx:?}");
    //
    // bladerf.set_gain(
    //     Channel::Tx,
    //     GainDb {
    //         db: gain_range_tx.min().unwrap() as i8,
    //     },
    // )?;
    //
    // let gain_tx = bladerf.get_gain(Channel::Tx)?;
    // log::debug!("Gain TX: {}", gain_tx.db);

    // do_tx(&bladerf)?;

    Ok(())
}
