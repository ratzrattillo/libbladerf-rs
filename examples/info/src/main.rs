use anyhow::Result;
use bladerf_globals::{BLADERF_MODULE_RX, BLADERF_MODULE_TX, BladeRfDirection, BladerfFormat};
use libbladerf_rs::board::bladerf1::BladeRf1;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .init();

    let bladerf = BladeRf1::from_first().await?;

    println!("Speed: {:?}", bladerf.speed());
    println!("Serial: {}", bladerf.serial().await?);
    println!("Manufacturer: {}", bladerf.manufacturer().await?);
    println!("FX3 Firmware: {}", bladerf.fx3_firmware().await?);
    println!("Product: {}", bladerf.product().await?);

    let languages = bladerf.get_supported_languages().await?;
    println!("Languages: {:x?}", languages);

    bladerf.initialize().await?;

    let frequency_range = BladeRf1::get_frequency_range();
    println!("Frequency Range: {frequency_range:?}");

    // Set Frequency to minimum frequency
    bladerf
        .set_frequency(BLADERF_MODULE_RX, frequency_range.min as u64)
        .await?;
    bladerf
        .set_frequency(BLADERF_MODULE_TX, frequency_range.min as u64)
        .await?;

    let frequency_rx = bladerf.get_frequency(BLADERF_MODULE_RX).await?;
    let frequency_tx = bladerf.get_frequency(BLADERF_MODULE_TX).await?;
    println!("Frequency RX: {}", frequency_rx);
    println!("Frequency TX: {}", frequency_tx);

    let sample_rate_range = BladeRf1::get_sample_rate_range();
    println!("Sample Rate: {sample_rate_range:?}");

    // Set Sample Rate to minimum Sample Rate
    bladerf
        .set_sample_rate(BLADERF_MODULE_RX, sample_rate_range.min)
        .await?;
    bladerf
        .set_sample_rate(BLADERF_MODULE_TX, sample_rate_range.min)
        .await?;

    let sample_rate_rx = bladerf.get_sample_rate(BLADERF_MODULE_RX).await?;
    let sample_rate_tx = bladerf.get_sample_rate(BLADERF_MODULE_TX).await?;
    println!("Sample Rate RX: {}", sample_rate_rx);
    println!("Sample Rate TX: {}", sample_rate_tx);

    let bandwidth_range = BladeRf1::get_bandwidth_range();
    println!("Bandwidth: {bandwidth_range:?}");

    // Set Sample Rate to minimum Sample Rate
    bladerf
        .set_bandwidth(BLADERF_MODULE_RX, bandwidth_range.min)
        .await?;
    bladerf
        .set_bandwidth(BLADERF_MODULE_TX, bandwidth_range.min)
        .await?;

    let bandwidth_rx = bladerf.get_bandwidth(BLADERF_MODULE_RX).await?;
    let bandwidth_tx = bladerf.get_bandwidth(BLADERF_MODULE_TX).await?;
    println!("Bandwidth RX: {}", bandwidth_rx);
    println!("Bandwidth TX: {}", bandwidth_tx);

    let gain_range_rx = BladeRf1::get_gain_range(BLADERF_MODULE_RX);
    let gain_range_tx = BladeRf1::get_gain_range(BLADERF_MODULE_TX);
    println!("Gain Range RX: {gain_range_rx:?}");
    println!("Gain Range TX: {gain_range_tx:?}");

    // Set Sample Rate to minimum Sample Rate
    bladerf
        .set_gain(BLADERF_MODULE_RX, gain_range_rx.min)
        .await?;
    bladerf
        .set_gain(BLADERF_MODULE_TX, gain_range_tx.min)
        .await?;

    let gain_rx = bladerf.get_gain(BLADERF_MODULE_RX).await?;
    let gain_tx = bladerf.get_gain(BLADERF_MODULE_TX).await?;
    println!("Gain RX: {}", gain_rx);
    println!("Gain TX: {}", gain_tx);

    // bladerf.reset()?;

    // Contains mostly setup of buffers and FW version checks...
    // bladerf1_sync_config(
    //      perform_format_config
    //      int sync_init(
    //          int sync_worker_init(struct bladerf_sync *s)
    //              int async_init_stream(
    //                  dev->backend->init_stream(lstream, num_transfers); -> static int lusb_init_stream( in /home/user/sdr/bladeRF/host/libraries/libbladeRF/src/backend/usb/libusb.c
    // tokio::time::sleep(Duration::from_secs(1)).await;
    // bladerf
    //     .perform_format_config(BladeRfDirection::Rx, BladerfFormat::Sc16Q11)
    //     .await?;
    // tokio::time::sleep(Duration::from_secs(1)).await;
    // bladerf
    //     .bladerf_enable_module(BLADERF_MODULE_RX, true)
    //     .await?;
    // tokio::time::sleep(Duration::from_secs(1)).await;
    //
    // bladerf.experimental_control_urb().await?;
    //
    // bladerf.async_run_stream().await?;
    //
    // bladerf.perform_format_deconfig(BladeRfDirection::Rx)?;
    // tokio::time::sleep(Duration::from_secs(1)).await;
    // bladerf
    //     .bladerf_enable_module(BLADERF_MODULE_RX, false)
    //     .await?;

    Ok(())
}
