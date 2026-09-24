use super::common::*;
use libbladerf_rs::flash::{
    BLADERF_FLASH_ADDR_CAL, BLADERF_FLASH_ADDR_FPGA, BLADERF_FLASH_ERASE_BLOCK_SIZE,
    BLADERF_FLASH_PAGE_SIZE, binkv_decode_field,
};
use libbladerf_rs::{Error, MaybeFuture, Result};
use std::io::Write;

#[test]
fn geometry_calibration_and_range_validation() -> Result<()> {
    let mut sdr = sdr();
    let mut flash = sdr.flash_session().wait()?;
    let pages = flash.total_pages();
    let sectors = flash.total_sectors();
    println!(
        "flash: {} bytes, {pages} pages, {sectors} sectors",
        flash.size_bytes()
    );
    assert_eq!(pages * 256, flash.size_bytes());
    assert_eq!(sectors * 65_536, flash.size_bytes());
    let mut page = [0; BLADERF_FLASH_PAGE_SIZE];
    flash
        .read_page(BLADERF_FLASH_ADDR_CAL / 256, &mut page)
        .wait()?;
    let trim: u16 = binkv_decode_field(&page, "DAC")?.parse().unwrap();
    assert_eq!(trim, flash.read_flash_dac_trim().wait()?);
    println!(
        "FPGA: {:?}, DAC trim: {trim}",
        flash.read_flash_fpga_size().wait()?
    );
    for size in [0, 255, 257] {
        assert!(matches!(
            flash.read_page(0, &mut vec![0; size]).wait(),
            Err(Error::Argument(_))
        ));
        assert!(matches!(
            flash.write_page(0, &vec![0; size]).wait(),
            Err(Error::Argument(_))
        ));
    }
    assert!(matches!(
        flash.read_pages(pages - 1, 2, &mut [0; 512]).wait(),
        Err(Error::Argument(_))
    ));
    assert!(matches!(
        flash.write_pages(pages - 1, 2, &[0; 512]).wait(),
        Err(Error::Argument(_))
    ));
    assert!(matches!(
        flash.erase_sectors(sectors - 1, 2).wait(),
        Err(Error::Argument(_))
    ));
    assert!(matches!(
        flash.erase_write_verify(1, &[0; 256]).wait(),
        Err(Error::Argument(_))
    ));
    assert!(matches!(
        flash.verify_pages(0, &[0; 257]).wait(),
        Err(Error::Argument(_))
    ));
    assert!(matches!(
        flash.read_pages(0, usize::MAX, &mut []).wait(),
        Err(Error::Argument(_))
    ));
    flash.read_pages(pages, 0, &mut []).wait()?;
    flash.write_pages(pages, 0, &[]).wait()?;
    flash.verify_pages(pages, &[]).wait()?;
    flash.erase_sectors(sectors, 0).wait()?;
    flash.erase_write_verify(pages, &[]).wait()
}

#[test]
#[ignore = "programs and restores one full sector; requires BLADERF_FLASH_BACKUP_DIR"]
fn full_sector_programming_and_restoration() -> Result<()> {
    let directory = std::env::var_os("BLADERF_FLASH_BACKUP_DIR")
        .ok_or_else(|| Error::Argument("BLADERF_FLASH_BACKUP_DIR is required".into()))?;
    let mut sdr = sdr();
    let serial = sdr.serial().wait()?;
    let mut flash = sdr.flash_session().wait()?;
    let sector = flash.total_sectors() - 1;
    let byte_start = sector * BLADERF_FLASH_ERASE_BLOCK_SIZE as u32;
    let page_start = byte_start / BLADERF_FLASH_PAGE_SIZE as u32;
    let page_count = BLADERF_FLASH_ERASE_BLOCK_SIZE / BLADERF_FLASH_PAGE_SIZE;
    let mut metadata = [0; BLADERF_FLASH_PAGE_SIZE];
    flash
        .read_page(BLADERF_FLASH_ADDR_FPGA / 256, &mut metadata)
        .wait()?;
    let stored_bytes = if metadata.iter().all(|&byte| byte == 0xff) {
        0
    } else {
        binkv_decode_field(&metadata, "LEN")?
            .parse::<u32>()
            .map_err(|_| Error::FlashData("invalid stored FPGA length"))?
    };
    let image_end = BLADERF_FLASH_ADDR_FPGA
        .checked_add(256)
        .and_then(|offset| offset.checked_add(stored_bytes))
        .ok_or(Error::FlashData("stored FPGA range overflow"))?;
    if image_end > byte_start {
        return Err(Error::Argument(
            "no sector beyond the stored FPGA image".into(),
        ));
    }
    let mut original = vec![0; BLADERF_FLASH_ERASE_BLOCK_SIZE];
    flash
        .read_pages(page_start, page_count, &mut original)
        .wait()?;
    let directory = std::path::PathBuf::from(directory);
    std::fs::create_dir_all(&directory)?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = directory.join(format!("{serial}-sector-{sector}-{stamp}.bin"));
    let mut backup = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)?;
    backup.write_all(&original)?;
    backup.sync_all()?;
    println!("sector {sector} backup: {}", path.display());

    let mut expected = vec![0xff; BLADERF_FLASH_ERASE_BLOCK_SIZE];
    let result = (|| -> Result<()> {
        flash.erase_sector(sector).wait()?;
        flash.verify_pages(page_start, &expected).wait()?;
        for (index, byte) in expected.iter_mut().enumerate() {
            *byte = (index ^ (index >> 8)) as u8;
        }
        flash.erase_write_verify(page_start, &expected).wait()?;
        flash.verify_pages(page_start, &expected).wait()?;
        let mut wrong = expected.clone();
        wrong[511] ^= 1;
        let mismatch = flash.verify_pages(page_start, &wrong).wait();
        if !matches!(mismatch, Err(Error::FlashVerificationFailed { byte_offset: 511, expected: a, actual: b }) if a == wrong[511] && b == expected[511])
        {
            return Err(Error::Internal("flash mismatch diagnostics were incorrect"));
        }
        flash
            .erase_write_verify(page_start, &expected[..256])
            .wait()?;
        expected[256..].fill(0xff);
        flash.verify_pages(page_start, &expected).wait()
    })();
    let restoration = flash
        .erase_write_verify(page_start, &original)
        .wait()
        .and_then(|()| flash.verify_pages(page_start, &original).wait());
    match (result, restoration) {
        (Err(operation), Err(cleanup)) => Err(Error::OperationAndCleanup {
            operation: Box::new(operation),
            cleanup: Box::new(cleanup),
        }),
        (Err(error), _) | (_, Err(error)) => Err(error),
        (Ok(()), Ok(())) => {
            println!("restored and verified all {} bytes", original.len());
            Ok(())
        }
    }
}
