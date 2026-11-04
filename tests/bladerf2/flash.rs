use super::common::*;
use libbladerf_rs::bladerf2::{BLADERF_FLASH_FPGA_SIZE_A9, fpga_bytes};
use libbladerf_rs::flash::FpgaSize;
use libbladerf_rs::{MaybeFuture, Result};

#[test]
fn geometry_is_consistent() -> Result<()> {
    logging_init("bladerf2_flash");
    let mut dev = open();
    let flash = dev.flash_session().wait()?;
    println!("flash: {} Mbit", flash.size_bytes() >> 17);
    assert_eq!(flash.total_pages() * 256, flash.size_bytes());
    assert_eq!(flash.total_sectors() * 65_536, flash.size_bytes());
    assert_eq!(
        flash.fpga_flash_bytes(),
        flash.size_bytes() as usize - 0x40000
    );
    Ok(())
}

#[test]
fn calibration_region_holds_bladerf2_fpga_size_and_trim() -> Result<()> {
    logging_init("bladerf2_flash");
    let mut dev = open();
    let mut flash = dev.flash_session().wait()?;
    let fpga_size = flash.read_flash_fpga_size().wait()?;
    let trim = flash.read_flash_dac_trim().wait()?;
    println!("FPGA: {fpga_size:?}, DAC trim: {trim:#06x}");
    assert!(matches!(
        fpga_size,
        FpgaSize::A4 | FpgaSize::A5 | FpgaSize::A9
    ));
    assert!(fpga_bytes(fpga_size)? <= flash.fpga_flash_bytes());
    if fpga_size == FpgaSize::A9 {
        assert!(BLADERF_FLASH_FPGA_SIZE_A9 <= flash.fpga_flash_bytes());
    }
    Ok(())
}

#[test]
fn read_page_rejects_out_of_range_and_short_buffers() -> Result<()> {
    let mut dev = open();
    let mut flash = dev.flash_session().wait()?;
    let mut page = [0; 256];
    assert!(
        flash
            .read_page(flash.total_pages(), &mut page)
            .wait()
            .is_err()
    );
    assert!(flash.read_page(0, &mut [0; 255]).wait().is_err());
    flash.read_page(0, &mut page).wait()?;
    Ok(())
}
