/*use super::common::*;
use libbladerf_rs::bladerf1::BladeRf1;
use libbladerf_rs::flash::{
    BLADERF_FLASH_ADDR_CAL, BLADERF_FLASH_ERASE_BLOCK_SIZE, BLADERF_FLASH_PAGE_SIZE, FpgaSize,
    make_cal_region,
};
use libbladerf_rs::{Error, Result};

const CAL_PAGE: u32 = BLADERF_FLASH_ADDR_CAL / BLADERF_FLASH_PAGE_SIZE as u32;
const CAL_SECTOR: u32 = BLADERF_FLASH_ADDR_CAL / BLADERF_FLASH_ERASE_BLOCK_SIZE as u32;

trait CalBackup {
    fn backup_cal(&mut self) -> Result<[u8; BLADERF_FLASH_PAGE_SIZE]>;
    fn restore_cal(&mut self, backup: &[u8; BLADERF_FLASH_PAGE_SIZE]);
}

impl CalBackup for BladeRf1 {
    fn backup_cal(&mut self) -> Result<[u8; BLADERF_FLASH_PAGE_SIZE]> {
        let mut buf = [0u8; BLADERF_FLASH_PAGE_SIZE];
        let mut flash = self.flash_session()?;
        flash.read_pages(CAL_PAGE, 1, &mut buf)?;
        Ok(buf)
    }

    fn restore_cal(&mut self, backup: &[u8; BLADERF_FLASH_PAGE_SIZE]) {
        let mut flash = match self.flash_session() {
            Ok(f) => f,
            Err(e) => {
                log::error!("Failed to create flash session for restore: {e:#}");
                return;
            }
        };
        if let Err(e) = (|| -> Result<()> {
            flash.erase_sectors(CAL_SECTOR, 1)?;
            flash.write_pages(CAL_PAGE, 1, backup)?;
            flash.verify_pages(CAL_PAGE, backup)
        })() {
            log::error!("Failed to restore cal region: {e:#}");
        }
    }
}

#[test]
fn read_cal_region() -> Result<()> {
    logging_init("bladerf1_flash_read");

    let mut sdr = sdr();

    let mut buf = [0u8; BLADERF_FLASH_PAGE_SIZE];
    let mut flash = sdr.flash_session()?;
    flash.read_pages(CAL_PAGE, 1, &mut buf)?;

    let all_zero = buf.iter().all(|&b| b == 0x00);
    assert!(!all_zero, "calibration region should not be all zeros");

    let all_ff = buf.iter().all(|&b| b == 0xFF);
    assert!(
        !all_ff,
        "calibration region should not be all 0xFF (unwritten)"
    );

    Ok(())
}

#[test]
fn write_read_roundtrip() -> Result<()> {
    logging_init("bladerf1_flash_write_read");

    let mut sdr = sdr();
    let backup = sdr.backup_cal()?;

    let test_data = make_cal_region(FpgaSize::KLE40, 0xBEEF)?;

    {
        let mut flash = sdr.flash_session()?;
        flash.erase_sectors(CAL_SECTOR, 1)?;
        flash.write_pages(CAL_PAGE, 1, &test_data)?;
    }

    let mut readback = [0u8; BLADERF_FLASH_PAGE_SIZE];
    {
        let mut flash = sdr.flash_session()?;
        flash.read_pages(CAL_PAGE, 1, &mut readback)?;
    }
    assert_eq!(readback.as_slice(), test_data.as_slice());

    sdr.restore_cal(&backup);
    Ok(())
}

#[test]
fn verify_flash_matches() -> Result<()> {
    logging_init("bladerf1_flash_verify");

    let mut sdr = sdr();
    let backup = sdr.backup_cal()?;

    let test_data = make_cal_region(FpgaSize::KLE40, 0x1234)?;

    {
        let mut flash = sdr.flash_session()?;
        flash.erase_sectors(CAL_SECTOR, 1)?;
        flash.write_pages(CAL_PAGE, 1, &test_data)?;
    }

    {
        let mut flash = sdr.flash_session()?;
        flash.verify_pages(CAL_PAGE, &test_data)?;
    }

    let mut wrong_data = test_data;
    wrong_data[0] ^= 0xFF;
    let result = {
        let mut flash = sdr.flash_session()?;
        flash.verify_pages(CAL_PAGE, &wrong_data)
    };
    assert!(
        matches!(result, Err(Error::FlashVerificationFailed { .. })),
        "expected FlashVerificationFailed, got {result:?}"
    );

    sdr.restore_cal(&backup);
    Ok(())
}

#[test]
fn erase_flash_roundtrip() -> Result<()> {
    logging_init("bladerf1_flash_erase");

    let mut sdr = sdr();
    let backup = sdr.backup_cal()?;

    {
        let mut flash = sdr.flash_session()?;
        flash.erase_sectors(CAL_SECTOR, 1)?;
    }

    let mut erased = [0u8; BLADERF_FLASH_PAGE_SIZE];
    {
        let mut flash = sdr.flash_session()?;
        flash.read_pages(CAL_PAGE, 1, &mut erased)?;
    }
    assert!(
        erased.iter().all(|&b| b == 0xFF),
        "erased region should be all 0xFF"
    );

    sdr.restore_cal(&backup);
    Ok(())
}
*/
