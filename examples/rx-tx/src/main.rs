use anyhow::Result;
use libbladerf_rs::Channel;
use libbladerf_rs::MaybeFuture;
use libbladerf_rs::bladerf1::ExpansionBoard;
use libbladerf_rs::bladerf1::ExpansionBoard::XbNone;
use libbladerf_rs::bladerf1::{
    BladeRf1, RfLinkSession, RxStream, SampleFormat, TuningMode, TxStream,
};
use std::time::Duration;

fn do_rx(rf: &mut RfLinkSession) -> Result<()> {
    let mut streamer = RxStream::builder(rf)
        .buffer_size(65_536)
        .buffer_count(8)
        .format(SampleFormat::Sc16Q11)
        .build()
        .wait()?;
    let result: Result<()> = (|| {
        streamer.start(rf).wait()?;
        let buffer = streamer.read(Some(Duration::from_secs(2))).wait()?;
        println!("Read {} bytes via zero-copy DMA buffer", buffer.len());
        println!("First 32 bytes: {:02x?}", &buffer[..32.min(buffer.len())]);
        streamer.recycle(buffer);
        Ok(())
    })();
    let close = streamer.close(rf).wait();
    result?;
    close?;
    Ok(())
}

fn _do_tx(rf: &mut RfLinkSession) -> Result<()> {
    let mut streamer = TxStream::builder(rf)
        .buffer_size(32_768)
        .buffer_count(8)
        .format(SampleFormat::Sc16Q11)
        .build()
        .wait()?;
    let result: Result<()> = (|| {
        streamer.start(rf).wait()?;
        let buf: Vec<u8> = (0..5_000).flat_map(|_| [0xFF, 0x07, 0xFF, 0x07]).collect();
        for _ in 0..10 {
            let mut buffer = streamer.get_buffer(Some(Duration::from_secs(2))).wait()?;
            buffer.extend_from_slice(&buf);
            streamer.submit(buffer, buf.len())?;
            streamer
                .wait_completion(Some(Duration::from_secs(2)))
                .wait()?;
            println!("Submitted buffer");
        }
        Ok(())
    })();
    let close = streamer.close(rf).wait();
    result?;
    close?;
    Ok(())
}

fn main() -> Result<()> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Trace)
        .filter_module("nusb", log::LevelFilter::Info)
        .filter_module("libbladerf_rs::nios_client", log::LevelFilter::Info)
        .filter_module("libbladerf_rs::usb", log::LevelFilter::Info)
        .init();

    let frequency: u64 = 915_000_000;
    let mut bladerf = BladeRf1::from_first().wait()?;
    let mut rf = bladerf.rf_link_session().wait()?;

    rf.initialize(false).wait()?;

    let frequency_range = rf.get_frequency_range().wait()?;
    log::debug!("Frequency Range: {frequency_range:?}");

    if frequency < frequency_range.min().unwrap() as u64 {
        let xb = rf.expansion_get_attached().wait()?;
        log::debug!("XB: {xb:?}");
        if xb == XbNone {
            rf.expansion_attach(ExpansionBoard::Xb200).wait()?;
            log::debug!("XB was attached");
            let xb = rf.expansion_get_attached().wait()?;
            log::debug!("XB: {xb:?}");
        }
    }

    rf.set_frequency(Channel::Rx, frequency, TuningMode::Fpga)
        .wait()?;
    rf.set_sample_rate(Channel::Rx, 2_000_000).wait()?;
    let gain_range_rx = RfLinkSession::get_gain_range(Channel::Rx);
    log::debug!("Gain Range RX: {gain_range_rx:?}");
    let mid_gain = (gain_range_rx.min().unwrap() + gain_range_rx.max().unwrap()) / 2.0;
    rf.set_gain(Channel::Rx, (mid_gain as i8).into()).wait()?;

    let gain_rx = rf.get_gain(Channel::Rx).wait()?;
    log::debug!("Gain RX: {}", gain_rx.db());

    let result = do_rx(&mut rf);
    let close = bladerf.close().wait();
    result?;
    close?;
    Ok(())
}
