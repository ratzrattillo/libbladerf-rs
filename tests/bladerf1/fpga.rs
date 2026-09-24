use super::common::*;
use libbladerf_rs::flash::{
    BLADERF_FLASH_ADDR_FPGA, BLADERF_FLASH_PAGE_SIZE, binkv_decode_field, is_valid_fpga_size,
};
use libbladerf_rs::{Error, MaybeFuture, Result};

#[test]
fn read_fpga_metadata_and_validate_images() -> Result<()> {
    let mut sdr = sdr();
    let mut flash = sdr.flash_session().wait()?;
    let mut metadata = [0; BLADERF_FLASH_PAGE_SIZE];
    flash
        .read_page(BLADERF_FLASH_ADDR_FPGA / 256, &mut metadata)
        .wait()?;
    if metadata.iter().all(|&byte| byte == 0xff) {
        println!("no stored FPGA image");
    } else {
        let length: usize = binkv_decode_field(&metadata, "LEN")?.parse().unwrap();
        assert!(is_valid_fpga_size(length));
        assert!(length + BLADERF_FLASH_PAGE_SIZE <= flash.fpga_flash_bytes());
        println!("stored FPGA image: {length} bytes");
    }
    assert!(matches!(
        flash.flash_fpga(&[0; 257]).wait(),
        Err(Error::Argument(_))
    ));
    assert!(matches!(
        flash.flash_firmware(&[0; 257]).wait(),
        Err(Error::Argument(_))
    ));
    Ok(())
}
