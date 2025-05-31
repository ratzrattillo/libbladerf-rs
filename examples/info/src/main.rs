use anyhow::Result;
use bladerf_globals::BLADERF_MODULE_RX;
use std::time::Duration;
// use seify_bladerf::backend::nusb::NusbBackend;
// use seify_bladerf::backend::rusb::RusbBackend;
// use seify_bladerf::backend::{UsbBackend, UsbBackendMarker};
use libbladerf_rs::board::bladerf1::{BladeRf1, BladeRfDirection, BladerfFormat};
//use seify_bladerf::backend::nusb::NusbBackend;
//use seify_bladerf::backend::BladeRfBackend;
//use seify_bladerf::board::bladerf1::BladeRf1;
// use rusb;
// use std::time::Duration;
// use seify_bladerf::board::bladerf1::{BLADERF1_USB_PID, BLADERF1_USB_VID};
// use seify_bladerf::nios::constants::{NIOS_PKT_8X32_TARGET_CONTROL, NIOS_PKT_FLAG_READ};
// use seify_bladerf::nios::packet8x32::NiosPacket8x32;
// use seify_bladerf::nios::packet8x8::NiosPacket8x8;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    // TODO: To start RX stream, try:
    // /home/jl/sdr/bladeRF/host/libraries/libbladeRF/src/board/bladerf1/bladerf1.c:2848
    //static int bladerf1_sync_rx
    //https://www.nuand.com/libbladeRF-doc/v2.5.0/group___f_n___s_t_r_e_a_m_i_n_g___a_s_y_n_c.html#gab3d5bbea596957d7fd4e4eceb1e27faf
    //bladerf_stream

    // TODO: Buffer sizes do not fit. Somehow only 64Byte are received instead of 80 Byte when using BulkIN with NUSB

    // let backend = NusbBackend {};

    // for device in backend.list_devices()? {
    //     println!("{:?}", device.serial_number());
    // }
    // let device = backend.find_by_serial("0617f60964e8f3efcbf78adc8ed94c26")?.open()?;
    // println!("{}", device.hello());
    // let _ = device.reset()?;

    //let devices = rusb::devices()?;

    // if let Some(device) = rusb::open_device_with_vid_pid(BLADERF1_USB_VID, BLADERF1_USB_PID) {
    //     const ENDPOINT_OUT: u8 = 0x02;
    //     const ENDPOINT_IN: u8 = 0x82;
    //
    //     device.claim_interface(0x0)?;
    //     device.set_alternate_setting(0x0, 0x1)?;
    //
    //     let mut request = NiosPacket8x32::new();
    //
    //     request.set(NIOS_PKT_8X32_TARGET_CONTROL, NIOS_PKT_FLAG_READ, 0x0, 0x0);
    //     let mut vec = request.into_vec();
    //     println!("{:x?}", vec);
    //     let result = device.write_bulk(ENDPOINT_OUT, vec.as_slice(), Duration::from_secs(1))?;
    //     println!("Result: {}", result);
    //     let result = device.read_bulk(ENDPOINT_IN, vec.as_mut_slice(), Duration::from_secs(1))?;
    //     println!("Result: {}", result);
    //     println!("{:x?}", vec);
    // }

    let bladerf = BladeRf1::builder()
        .with_serial("0617f60964e8f3efcbf78adc8ed94c26")
        .await?
        .build()
        .await?;

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

    //bladerf.hello();
    // for descriptor in bladerf.interface().descriptors(){
    //     println!("{:#?}", descriptor);
    // }

    // bladerf.list_descriptors();

    // let usbhub_devinfo = UsbHub::find_by_bus_addr(2, 1)?;
    // println!("USB Hub info: {:?}", usbhub_devinfo);
    // let usbhub = UsbHub::open_by_device_info(usbhub_devinfo)?;
    // {
    //     // let device = nusb::list_devices()?
    //     //     .find(|dev| dev.vendor_id() == 0x1D6B
    //     //         && dev.product_id() == 0x0003)
    //     //     .ok_or(Error::NotFound)?.open()?;
    //     // let interface = device.detach_and_claim_interface(0)?;
    //     // for setting in interface.descriptors() {
    //     //     println!("{:#?}", setting);
    //     // }
    //     //device.detach_kernel_driver(0)?;
    //     //device.reset()?;
    // }

    // The interesting commands are probably board related:
    /* Open board in bladerf.c, Line: 154 */
    // status = dev->board->open(dev, devinfo);
    //TODO: Check board/bladerf1/bladerf1.c Line 623. SetFPGA Protocol

    // let devices = nusb::list_devices()?.collect::<Vec<_>>();
    // for device in devices {
    //     println!(
    //         "{}.{} - {}",
    //         device.bus_number(),
    //         device.device_address(),
    //         device.product_string().unwrap_or("None")
    //     );
    // }
    //
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

    //bladerf.set_configuration(0x01)?;
    //bladerf.initialize()?;

    // bladerf.set_isoch_delay(0x28)?;
    // bladerf.set_configuration(0x01)?;
    // bladerf.prepare_device()?;
    //
    // let freq = bladerf.get_freq(0x0)?;
    // println!("freq: {:?}", freq);
    // let freq_hz = BladeRf::lms_frequency_to_hz(&freq);
    // println!("freq_hz: {:?}", freq_hz);

    // println!("Board ID: {}", device.board_id().context("Read board id")?);
    // println!(
    //     "Firmware version: {}",
    //     radio.version().context("Read board version")?
    // );
    // println!("Device version: {}", radio.device_version());

    Ok(())
}
