use anyhow::Result;
use bladerf_globals::BLADERF_MODULE_RX;
use libbladerf_rs::board::bladerf1::{BladeRf1, BladeRfDirection, BladerfFormat};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let bladerf = BladeRf1::from_serial("0617f60964e8f3efcbf78adc8ed94c26").await?;

    let languages = bladerf.get_supported_languages().await?;
    println!("{languages:x?}");
    bladerf.initialize().await?;

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
        .perform_format_config(
            BladeRfDirection::BladerfRx,
            BladerfFormat::BladerfFormatSc16Q11,
        )
        .await?;
    tokio::time::sleep(Duration::from_secs(1)).await;
    bladerf
        .bladerf_enable_module(BLADERF_MODULE_RX, true)
        .await?;
    tokio::time::sleep(Duration::from_secs(1)).await;

    bladerf.experimental_control_urb().await?;

    bladerf.async_run_stream().await?;

    bladerf.perform_format_deconfig(BladeRfDirection::BladerfRx)?;
    tokio::time::sleep(Duration::from_secs(1)).await;
    bladerf
        .bladerf_enable_module(BLADERF_MODULE_RX, false)
        .await?;

    // println!(
    //     "Supported Languages: {:x?}",
    //     bladerf.get_supported_languages()?
    // );
    // println!("Configurations: {:?}", bladerf.get_configurations());
    // println!(
    //     "Serial: {}",
    //     bladerf.get_string_descriptor(StringDescriptors::Serial.into())?
    // );
    // println!(
    //     "Manufacturer: {}",
    //     bladerf.get_string_descriptor(StringDescriptors::Manufacturer.into())?
    // );
    // println!(
    //     "Product: {}",
    //     bladerf.get_string_descriptor(StringDescriptors::Product.into())?
    // );
    // println!(
    //     "FX3 Firmware: {}",
    //     bladerf.get_string_descriptor(StringDescriptors::Fx3Firmware.into())?
    // );
    //
    // println!(
    //     "Configuration Descriptor: {:?}",
    //     bladerf.get_configuration_descriptor(0x00)?
    // );

    Ok(())
}
