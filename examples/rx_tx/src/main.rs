use anyhow::Result;
use libbladerf_rs::Channel;
use libbladerf_rs::bladerf1::xb::ExpansionBoard;
use libbladerf_rs::bladerf1::xb::ExpansionBoard::XbNone;
use libbladerf_rs::bladerf1::{BladeRf1, GainDb, RxStream, SampleFormat, TuningMode, TxStream};
use std::thread::sleep;
use std::time::Duration;

fn do_rx(bladerf: &mut BladeRf1) -> Result<()> {
    let mut streamer = RxStream::builder(bladerf)
        .buffer_size(65_536)
        .buffer_count(8)
        .format(SampleFormat::Sc16Q11)
        .build()?;

    let buffer = streamer.read(None)?;
    let n = buffer.len();

    println!("Read {} bytes via zero-copy DMA buffer", n);
    println!("First 32 bytes: {:02x?}", &buffer[..32.min(buffer.len())]);

    streamer.recycle(buffer);
    let _ = streamer.close(bladerf);
    Ok(())
}

fn _do_tx(bladerf: &mut BladeRf1) -> Result<()> {
    println!("called do_tx()");
    sleep(Duration::from_millis(5_000));
    bladerf.perform_format_config(Channel::Tx, SampleFormat::Sc16Q11)?;
    println!("called perform_format_config(Channel::Tx, SampleFormat::Sc16Q11)");
    sleep(Duration::from_millis(5_000));
    bladerf.enable_module(Channel::Tx, true)?;
    println!("called enable_module(Channel::Tx, true)");
    sleep(Duration::from_millis(5_000));

    let mut streamer = TxStream::builder(bladerf)
        .buffer_size(32_768)
        .buffer_count(8)
        .format(SampleFormat::Sc16Q11)
        .build()?;

    let buf: Vec<u8> = (0..5_000).flat_map(|_| [0xFF, 0x07, 0xFF, 0x07]).collect();

    for _ in 0..10 {
        let mut buffer = streamer.get_buffer(None)?;
        buffer.extend_from_slice(&buf);
        streamer.submit(buffer, buf.len())?;
        streamer.wait_completion(Some(Duration::from_millis(5_000)))?;
        println!("Submitted buffer");
    }

    sleep(Duration::from_millis(5_000));

    println!("called enable_module(Channel::Tx, false)");

    Ok(())
}

fn main() -> Result<()> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .filter_module("nusb", log::LevelFilter::Info)
        .init();

    let frequency: u64 = 100_000_000;
    let mut bladerf = BladeRf1::from_first()?;

    bladerf.initialize(false)?;

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

    bladerf.set_frequency(Channel::Rx, frequency, TuningMode::Fpga)?;
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

    do_rx(&mut bladerf)?;

    Ok(())
}
