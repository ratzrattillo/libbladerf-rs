use anyhow::Result;
use bladerf_globals::BladeRfDirection::Rx;
use bladerf_globals::BladerfFormat::Sc16Q11;
use bladerf_globals::bladerf1::BladerfXb::BladerfXb200;
use bladerf_globals::{BLADERF_MODULE_RX, BLADERF_MODULE_TX};
use libbladerf_rs::board::bladerf1::BladeRf1;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .filter_module("nusb", log::LevelFilter::Info)
        .init();

    let mut bladerf = BladeRf1::from_first().await?;

    log::debug!("Speed: {:?}", bladerf.speed());
    log::debug!("Serial: {}", bladerf.serial().await?);
    log::debug!("Manufacturer: {}", bladerf.manufacturer().await?);
    log::debug!("FX3 Firmware: {}", bladerf.fx3_firmware().await?);
    log::debug!("Product: {}", bladerf.product().await?);

    let languages = bladerf.get_supported_languages().await?;
    log::debug!("Languages: {:x?}", languages);

    bladerf.initialize().await?;

    log::debug!("FPGA: {}", bladerf.fpga_version().await?);

    let xb = bladerf.expansion_get_attached();
    log::debug!("XB: {xb:?}");

    // tokio::time::sleep(Duration::from_secs(10)).await;

    bladerf.expansion_attach(BladerfXb200).await?;

    let xb = bladerf.expansion_get_attached();
    log::debug!("XB: {xb:?}");

    let frequency_range = bladerf.get_frequency_range();
    log::debug!("Frequency Range: {frequency_range:?}");

    // Set Frequency to minimum frequency
    bladerf
        .set_frequency(BLADERF_MODULE_RX, frequency_range.min as u64)
        .await?;
    bladerf
        .set_frequency(BLADERF_MODULE_TX, frequency_range.min as u64)
        .await?;

    let frequency_rx = bladerf.get_frequency(BLADERF_MODULE_RX).await?;
    let frequency_tx = bladerf.get_frequency(BLADERF_MODULE_TX).await?;
    log::debug!("Frequency RX: {}", frequency_rx);
    log::debug!("Frequency TX: {}", frequency_tx);

    let sample_rate_range = BladeRf1::get_sample_rate_range();
    log::debug!("Sample Rate: {sample_rate_range:?}");
    //
    // // Set Sample Rate to minimum Sample Rate
    // bladerf
    //     .set_sample_rate(BLADERF_MODULE_RX, sample_rate_range.min)
    //     .await?;
    // bladerf
    //     .set_sample_rate(BLADERF_MODULE_TX, sample_rate_range.min)
    //     .await?;
    //
    let sample_rate_rx = bladerf.get_sample_rate(BLADERF_MODULE_RX).await?;
    let sample_rate_tx = bladerf.get_sample_rate(BLADERF_MODULE_TX).await?;
    log::debug!("Sample Rate RX: {}", sample_rate_rx);
    log::debug!("Sample Rate TX: {}", sample_rate_tx);

    let bandwidth_range = BladeRf1::get_bandwidth_range();
    log::debug!("Bandwidth: {bandwidth_range:?}");
    //
    // // Set Sample Rate to minimum Sample Rate
    // bladerf
    //     .set_bandwidth(BLADERF_MODULE_RX, bandwidth_range.min)
    //     .await?;
    // bladerf
    //     .set_bandwidth(BLADERF_MODULE_TX, bandwidth_range.min)
    //     .await?;
    //
    let bandwidth_rx = bladerf.get_bandwidth(BLADERF_MODULE_RX).await?;
    let bandwidth_tx = bladerf.get_bandwidth(BLADERF_MODULE_TX).await?;
    log::debug!("Bandwidth RX: {}", bandwidth_rx);
    log::debug!("Bandwidth TX: {}", bandwidth_tx);

    let gain_stages_rx = BladeRf1::get_gain_stages(BLADERF_MODULE_RX);
    let gain_stages_tx = BladeRf1::get_gain_stages(BLADERF_MODULE_TX);
    log::debug!("Gain Stages RX: {gain_stages_rx:?}");
    log::debug!("Gain Stages TX: {gain_stages_tx:?}");

    let gain_range_rx = BladeRf1::get_gain_range(BLADERF_MODULE_RX);
    let gain_range_tx = BladeRf1::get_gain_range(BLADERF_MODULE_TX);
    log::debug!("Gain Range RX: {gain_range_rx:?}");
    log::debug!("Gain Range TX: {gain_range_tx:?}");
    //
    // // Set Sample Rate to minimum Sample Rate
    // bladerf
    //     .set_gain(BLADERF_MODULE_RX, gain_range_rx.min)
    //     .await?;
    // bladerf
    //     .set_gain(BLADERF_MODULE_TX, gain_range_tx.min)
    //     .await?;

    let gain_rx = bladerf.get_gain(BLADERF_MODULE_RX).await?;
    let gain_tx = bladerf.get_gain(BLADERF_MODULE_TX).await?;
    log::debug!("Gain RX: {}", gain_rx);
    log::debug!("Gain TX: {}", gain_tx);

    // bladerf.reset().await?;

    // Contains mostly setup of buffers and FW version checks...
    // bladerf1_sync_config(
    //      perform_format_config
    //      int sync_init(
    //          int sync_worker_init(struct bladerf_sync *s)
    //              int async_init_stream(
    //                  dev->backend->init_stream(lstream, num_transfers); -> static int lusb_init_stream( in /home/user/sdr/bladeRF/host/libraries/libbladeRF/src/backend/usb/libusb.c
    // tokio::time::sleep(Duration::from_secs(1)).await;
    bladerf.perform_format_config(Rx, Sc16Q11).await?;
    // tokio::time::sleep(Duration::from_secs(1)).await;
    bladerf.enable_module(BLADERF_MODULE_RX, true).await?;
    // tokio::time::sleep(Duration::from_secs(1)).await;

    bladerf.experimental_control_urb().await?;

    // bladerf.async_run_stream().await?;

    bladerf.perform_format_deconfig(Rx)?;
    // tokio::time::sleep(Duration::from_secs(1)).await;
    bladerf.enable_module(BLADERF_MODULE_RX, false).await?;

    Ok(())
}
