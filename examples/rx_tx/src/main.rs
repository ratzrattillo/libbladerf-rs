use anyhow::Result;
use libbladerf_rs::Channel;
use libbladerf_rs::bladerf1::ExpansionBoard;
use libbladerf_rs::bladerf1::ExpansionBoard::XbNone;
use libbladerf_rs::bladerf1::{
    BladeRf1, RfLinkSession, RxStream, SampleFormat, TuningMode, TxStream,
};
use std::thread::sleep;
use std::time::Duration;

fn do_rx(rf: &mut RfLinkSession) -> Result<()> {
    let mut streamer = RxStream::builder(rf)
        .buffer_size(65_536)
        .buffer_count(8)
        .format(SampleFormat::Sc16Q11)
        .build()?;
    streamer.start(rf)?;

    let buffer = streamer.read(None)?;
    let n = buffer.len();

    println!("Read {} bytes via zero-copy DMA buffer", n);
    println!("First 32 bytes: {:02x?}", &buffer[..32.min(buffer.len())]);

    streamer.recycle(buffer);
    let _ = streamer.close(rf);
    Ok(())
}

fn _do_tx(rf: &mut RfLinkSession) -> Result<()> {
    println!("called do_tx()");
    sleep(Duration::from_millis(5_000));
    rf.perform_format_config(SampleFormat::Sc16Q11)?;
    println!("called perform_format_config(SampleFormat::Sc16Q11)");
    sleep(Duration::from_millis(5_000));
    rf.enable_module(Channel::Tx, true)?;
    println!("called enable_module(Channel::Tx, true)");
    sleep(Duration::from_millis(5_000));

    let mut streamer = TxStream::builder(rf)
        .buffer_size(32_768)
        .buffer_count(8)
        .format(SampleFormat::Sc16Q11)
        .build()?;
    streamer.start(rf)?;

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
    let _ = streamer.close(rf);

    Ok(())
}

fn main() -> Result<()> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Trace)
        .filter_module("nusb", log::LevelFilter::Info)
        .filter_module("libbladerf_rs::nios_client", log::LevelFilter::Info)
        .filter_module("libbladerf_rs::usb", log::LevelFilter::Info)
        .init();

    let frequency: u64 = 100_000_000;
    let mut bladerf = BladeRf1::from_first()?;
    let mut rf = bladerf.rf_link_session()?;

    rf.initialize(false)?;

    let frequency_range = rf.get_frequency_range()?;
    log::debug!("Frequency Range: {frequency_range:?}");

    if frequency < frequency_range.min().unwrap() as u64 {
        let xb = rf.expansion_get_attached()?;
        log::debug!("XB: {xb:?}");
        if xb == XbNone {
            rf.expansion_attach(ExpansionBoard::Xb200)?;
            log::debug!("XB was attached");
            let xb = rf.expansion_get_attached()?;
            log::debug!("XB: {xb:?}");
        }
    }

    rf.set_frequency(Channel::Rx, frequency, TuningMode::Fpga)?;
    let gain_range_rx = RfLinkSession::get_gain_range(Channel::Rx);
    log::debug!("Gain Range RX: {gain_range_rx:?}");
    let mid_gain = (gain_range_rx.min().unwrap() + gain_range_rx.max().unwrap()) / 2.0;
    rf.set_gain(Channel::Rx, (mid_gain as i8).into())?;

    let gain_rx = rf.get_gain(Channel::Rx)?;
    log::debug!("Gain RX: {}", gain_rx.db());

    do_rx(&mut rf)?;

    Ok(())
}
