use anyhow::Result;
use libbladerf_rs::MaybeFuture;

use libbladerf_rs::bladerf2::{BladeRf2, DEFAULT_VCTCXO_TRIM, fpga_bytes};

fn main() -> Result<()> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .filter_module("nusb", log::LevelFilter::Info)
        .init();

    let mut bladerf2 = BladeRf2::from_first().wait()?;
    log::info!("Serial: {}", bladerf2.serial().wait()?);
    log::info!("FX3 Firmware: {}", bladerf2.fx3_firmware_version().wait()?);

    {
        let mut flash = bladerf2.flash_session().wait()?;
        log::info!(
            "Flash size: {} Mbit ({} pages, {} sectors)",
            flash.size_bytes() >> 17,
            flash.total_pages(),
            flash.total_sectors()
        );
        let fpga_size = flash.read_flash_fpga_size().wait()?;
        log::info!(
            "FPGA size: {} ({} bytes bitstream, {})",
            fpga_size.as_str(),
            fpga_bytes(fpga_size)?,
            fpga_size.variant_label()?
        );
        match flash.read_flash_dac_trim().wait() {
            Ok(trim) => log::info!("VCTCXO trim: {trim:#06x}"),
            Err(e) => log::warn!("VCTCXO trim: {e}, default {DEFAULT_VCTCXO_TRIM:#06x}"),
        }
    }

    let mut rf = bladerf2.rf_link_session().wait()?;
    if rf.is_fpga_configured().wait()? {
        log::info!("FPGA: {}", rf.fpga_version().wait()?);
        log::info!("FPGA source: {:?}", rf.get_fpga_source().wait()?);
    } else {
        log::warn!("FPGA is not configured");
    }

    Ok(())
}
