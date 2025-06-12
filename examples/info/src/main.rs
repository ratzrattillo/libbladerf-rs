use anyhow::Result;
use bladerf_globals::{BLADERF_MODULE_RX, BLADERF_MODULE_TX};
use libbladerf_rs::board::bladerf1::{BladeRf1, BladeRfDirection, BladerfFormat};
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

    let frequency_rx = bladerf.get_frequency(BLADERF_MODULE_RX).await?;
    let frequency_tx = bladerf.get_frequency(BLADERF_MODULE_TX).await?;
    println!("Frequency RX: {}", frequency_rx);
    println!("Frequency TX: {}", frequency_tx);

    // bladerf.reset()?;

    // Contains mostly setup of buffers and FW version checks...
    // bladerf1_sync_config(
    //      perform_format_config
    //      int sync_init(
    //          int sync_worker_init(struct bladerf_sync *s)
    //              int async_init_stream(
    //                  dev->backend->init_stream(lstream, num_transfers); -> static int lusb_init_stream( in /home/user/sdr/bladeRF/host/libraries/libbladeRF/src/backend/usb/libusb.c
    tokio::time::sleep(Duration::from_secs(1)).await;
    bladerf
        .perform_format_config(BladeRfDirection::Rx, BladerfFormat::Sc16Q11)
        .await?;
    tokio::time::sleep(Duration::from_secs(1)).await;
    bladerf
        .bladerf_enable_module(BLADERF_MODULE_RX, true)
        .await?;
    tokio::time::sleep(Duration::from_secs(1)).await;

    bladerf.experimental_control_urb().await?;

    bladerf.async_run_stream().await?;

    bladerf.perform_format_deconfig(BladeRfDirection::Rx)?;
    tokio::time::sleep(Duration::from_secs(1)).await;
    bladerf
        .bladerf_enable_module(BLADERF_MODULE_RX, false)
        .await?;

    Ok(())
}
