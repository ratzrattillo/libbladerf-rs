use anyhow::Result;
use libbladerf_rs::bladerf1::xb::ExpansionBoard;
use libbladerf_rs::bladerf1::xb::ExpansionBoard::XbNone;
use libbladerf_rs::bladerf1::{BladeRf1, BladeRf1RxStreamer, BladeRf1TxStreamer, SampleFormat};
use libbladerf_rs::{BLADERF_MODULE_RX, BLADERF_MODULE_TX, Direction};
use num_complex::Complex32;
use std::thread::sleep;
use std::time::Duration;

// RX
fn do_rx(bladerf: &BladeRf1) -> Result<()> {
    bladerf.perform_format_config(Direction::Rx, SampleFormat::Sc16Q11)?;
    bladerf.enable_module(BLADERF_MODULE_RX, true)?;
    bladerf.experimental_control_urb()?;
    let mut rx_streamer = BladeRf1RxStreamer::new(bladerf.clone(), 65536, Some(8), None)?;

    let mut buffer = [Complex32::new(0.0, 0.0); 1024];

    rx_streamer.read_sync(&mut [buffer.as_mut_slice()], 300000)?;
    println!("Read into buffers");

    bladerf.perform_format_deconfig(Direction::Rx)?;
    bladerf.enable_module(BLADERF_MODULE_RX, false)?;

    println!("{:x?}", buffer);
    Ok(())
}

// TX
fn do_tx(bladerf: &BladeRf1) -> Result<()> {
    println!("called do_tx()");
    sleep(Duration::from_millis(5000));
    bladerf.perform_format_config(Direction::Tx, SampleFormat::Sc16Q11)?;
    println!("called perform_format_config(Direction::Tx, SampleFormat::Sc16Q11)");
    sleep(Duration::from_millis(5000));
    bladerf.enable_module(BLADERF_MODULE_TX, true)?;
    println!("called enable_module(BLADERF_MODULE_TX, true)");
    sleep(Duration::from_millis(5000));
    // bladerf.experimental_control_urb()?;
    // println!("experimental_control_urb");
    // sleep(Duration::from_millis(5000));

    let mut tx_streamer = BladeRf1TxStreamer::new(bladerf.clone(), 32768, Some(8), None)?;

    let buf = vec![Complex32::new(2047.0, 2047.0); 5000];
    let buffers = &[buf.as_slice()];
    for _ in 0..10 {
        // sync.c, Line: 1056, int sync_tx(struct bladerf_sync *s,
        tx_streamer.write(buffers, None, false, 5000000)?;
        println!("tx_streamer.write(buffers, None, false, 5000000)");
    }
    sleep(Duration::from_millis(5000));

    bladerf.perform_format_deconfig(Direction::Tx)?;
    println!("called perform_format_deconfig(Direction::Tx)");
    sleep(Duration::from_millis(5000));
    bladerf.enable_module(BLADERF_MODULE_TX, false)?;
    println!("called enable_module(BLADERF_MODULE_TX, false)");
    // bladerf.experimental_control_urb2()?;
    // println!("experimental_control_urb2()");

    Ok(())
}
fn main() -> Result<()> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .filter_module("nusb", log::LevelFilter::Info)
        .init();

    let bladerf = BladeRf1::from_first()?;

    bladerf.initialize()?;

    let xb = bladerf.expansion_get_attached()?;
    log::debug!("XB: {xb:?}");
    if xb == XbNone {
        bladerf.expansion_attach(ExpansionBoard::Xb200)?;

        let xb = bladerf.expansion_get_attached()?;
        log::debug!("XB: {xb:?}");
    }

    bladerf.set_frequency(BLADERF_MODULE_TX, 1000000000)?;

    do_rx(&bladerf)?;

    do_tx(&bladerf)?;

    Ok(())
}
