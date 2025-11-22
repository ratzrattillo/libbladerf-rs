use anyhow::Result;
use libbladerf_rs::bladerf1::xb::ExpansionBoard;
use libbladerf_rs::bladerf1::{
    BladeRf1, BladeRf1RxStreamer, BladeRf1TxStreamer, GainDb, SampleFormat,
};
use libbladerf_rs::{BLADERF_MODULE_RX, BLADERF_MODULE_TX, Direction};
use num_complex::Complex32;
use std::thread::sleep;
use std::time::Duration;

fn main() -> Result<()> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .filter_module("nusb", log::LevelFilter::Info)
        .init();

    let bladerf = BladeRf1::from_first()?;

    // log::debug!("Speed: {:?}", bladerf.speed());
    // log::debug!("Serial: {}", bladerf.serial()?);
    // log::debug!("Manufacturer: {}", bladerf.manufacturer()?);
    // log::debug!("FX3 Firmware: {}", bladerf.fx3_firmware()?);
    // log::debug!("Product: {}", bladerf.product()?);
    //
    // let languages = bladerf.get_supported_languages()?;
    // log::debug!("Languages: {:x?}", languages);
    //
    bladerf.initialize()?;
    //
    // log::debug!("FPGA: {}", bladerf.fpga_version()?);
    //
    // let xb = bladerf.expansion_get_attached();
    // log::debug!("XB: {xb:?}");

    bladerf.expansion_attach(ExpansionBoard::Xb200)?;

    let xb = bladerf.expansion_get_attached();
    log::debug!("XB: {xb:?}");

    bladerf.set_frequency(BLADERF_MODULE_TX, 1000000000)?;

    // let frequency_range = bladerf.get_frequency_range()?;
    // log::debug!("Frequency Range: {frequency_range:?}");
    //
    // // Set Frequency to minimum frequency
    // bladerf.set_frequency(BLADERF_MODULE_RX, frequency_range.min().unwrap() as u64)?;
    // bladerf.set_frequency(BLADERF_MODULE_TX, frequency_range.min().unwrap() as u64)?;
    //
    // let frequency_rx = bladerf.get_frequency(BLADERF_MODULE_RX)?;
    // let frequency_tx = bladerf.get_frequency(BLADERF_MODULE_TX)?;
    // log::debug!("Frequency RX: {}", frequency_rx);
    // log::debug!("Frequency TX: {}", frequency_tx);
    //
    // let sample_rate_range = BladeRf1::get_sample_rate_range();
    // log::debug!("Sample Rate: {sample_rate_range:?}");
    //
    // // Set Sample Rate to minimum Sample Rate
    // bladerf.set_sample_rate(BLADERF_MODULE_RX, sample_rate_range.min().unwrap() as u32)?;
    // bladerf.set_sample_rate(BLADERF_MODULE_TX, sample_rate_range.min().unwrap() as u32)?;
    //
    // let sample_rate_rx = bladerf.get_sample_rate(BLADERF_MODULE_RX)?;
    // let sample_rate_tx = bladerf.get_sample_rate(BLADERF_MODULE_TX)?;
    // log::debug!("Sample Rate RX: {}", sample_rate_rx);
    // log::debug!("Sample Rate TX: {}", sample_rate_tx);
    //
    // let bandwidth_range = BladeRf1::get_bandwidth_range();
    // log::debug!("Bandwidth: {bandwidth_range:?}");
    //
    // // Set Sample Rate to minimum Sample Rate
    // bladerf.set_bandwidth(BLADERF_MODULE_RX, bandwidth_range.min().unwrap() as u32)?;
    // bladerf.set_bandwidth(BLADERF_MODULE_TX, bandwidth_range.min().unwrap() as u32)?;
    //
    // let bandwidth_rx = bladerf.get_bandwidth(BLADERF_MODULE_RX)?;
    // let bandwidth_tx = bladerf.get_bandwidth(BLADERF_MODULE_TX)?;
    // log::debug!("Bandwidth RX: {}", bandwidth_rx);
    // log::debug!("Bandwidth TX: {}", bandwidth_tx);
    //
    // let gain_stages_rx = BladeRf1::get_gain_stages(BLADERF_MODULE_RX);
    // let gain_stages_tx = BladeRf1::get_gain_stages(BLADERF_MODULE_TX);
    // log::debug!("Gain Stages RX: {gain_stages_rx:?}");
    // log::debug!("Gain Stages TX: {gain_stages_tx:?}");
    //
    // let gain_range_rx = BladeRf1::get_gain_range(BLADERF_MODULE_RX);
    // let gain_range_tx = BladeRf1::get_gain_range(BLADERF_MODULE_TX);
    // log::debug!("Gain Range RX: {gain_range_rx:?}");
    // log::debug!("Gain Range TX: {gain_range_tx:?}");
    //
    // // Set Sample Rate to minimum Sample Rate
    // bladerf.set_gain(
    //     BLADERF_MODULE_RX,
    //     GainDb {
    //         db: gain_range_rx.min().unwrap() as i8,
    //     },
    // )?;
    // bladerf.set_gain(
    //     BLADERF_MODULE_TX,
    //     GainDb {
    //         db: gain_range_tx.min().unwrap() as i8,
    //     },
    // )?;
    //
    // let gain_rx = bladerf.get_gain(BLADERF_MODULE_RX)?;
    // let gain_tx = bladerf.get_gain(BLADERF_MODULE_TX)?;
    // log::debug!("Gain RX: {}", gain_rx.db);
    // log::debug!("Gain TX: {}", gain_tx.db);

    // RX
    fn do_rx(bladerf: BladeRf1) -> Result<()> {
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
    fn do_tx(bladerf: BladeRf1) -> Result<()> {
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

    do_tx(bladerf)?;

    Ok(())
}
