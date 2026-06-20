/*use super::common::*;
use libbladerf_rs::Result;
use libbladerf_rs::bladerf1::BladeRf1;
use libbladerf_rs::flash::{
    BLADERF_FLASH_ADDR_FPGA, BLADERF_FLASH_ERASE_BLOCK_SIZE, BLADERF_FLASH_FPGA_SIZE_40KLE,
    BLADERF_FLASH_FPGA_SIZE_115KLE, BLADERF_FLASH_PAGE_SIZE, FpgaSize, binkv_decode_field,
};

const FPGA_PAGE: u32 = BLADERF_FLASH_ADDR_FPGA / BLADERF_FLASH_PAGE_SIZE as u32;
const FPGA_SECTOR: u32 = BLADERF_FLASH_ADDR_FPGA / BLADERF_FLASH_ERASE_BLOCK_SIZE as u32;

trait FpgaBackup {
    fn backup_fpga(&mut self) -> Result<Vec<u8>>;
    fn restore_fpga(&mut self, backup: &[u8]);
    fn detect_fpga_size(&mut self) -> FpgaSize;
}

impl FpgaBackup for BladeRf1 {
    fn backup_fpga(&mut self) -> Result<Vec<u8>> {
        let mut flash = self.flash_session()?;
        let buf_size = flash.size_bytes() as usize - BLADERF_FLASH_ADDR_FPGA as usize;
        let mut buf = vec![0u8; buf_size];
        let page_count = buf.len() / BLADERF_FLASH_PAGE_SIZE;
        flash.read_pages(FPGA_PAGE, page_count, &mut buf)?;
        Ok(buf)
    }

    fn restore_fpga(&mut self, backup: &[u8]) {
        let mut flash = match self.flash_session() {
            Ok(f) => f,
            Err(e) => {
                log::error!("Failed to create flash session for restore: {e:#}");
                return;
            }
        };
        let num_blocks = flash.total_sectors() - FPGA_SECTOR;
        let page_count = backup.len() / BLADERF_FLASH_PAGE_SIZE;
        if let Err(e) = (|| -> Result<()> {
            flash.erase_sectors(FPGA_SECTOR, num_blocks)?;
            flash.write_pages(FPGA_PAGE, page_count, backup)?;
            flash.verify_pages(FPGA_PAGE, backup)
        })() {
            log::error!("Failed to restore FPGA region: {e:#}");
        }
    }

    fn detect_fpga_size(&mut self) -> FpgaSize {
        let mut flash = self
            .flash_session()
            .expect("failed to create flash session");
        flash
            .read_flash_fpga_size()
            .expect("failed to read FPGA size from flash")
    }
}

fn valid_fpga_size(size: &FpgaSize) -> usize {
    match size {
        FpgaSize::KLE40 => BLADERF_FLASH_FPGA_SIZE_40KLE,
        FpgaSize::KLE115 => BLADERF_FLASH_FPGA_SIZE_115KLE,
        _ => panic!("unsupported FPGA variant for flash test"),
    }
}

#[test]
fn read_fpga_metadata() -> Result<()> {
    logging_init("bladerf1_fpga_metadata");

    let mut sdr = sdr();

    let mut meta = vec![0u8; BLADERF_FLASH_PAGE_SIZE];
    let mut flash = sdr.flash_session()?;
    flash.read_pages(FPGA_PAGE, 1, &mut meta)?;

    let all_ff = meta.iter().all(|&b| b == 0xFF);
    if all_ff {
        log::info!("No FPGA bitstream stored in flash (metadata page is all 0xFF)");
    } else {
        let len_str = binkv_decode_field(&meta, "LEN").unwrap_or_default();
        log::info!("Stored FPGA bitstream length from metadata: {len_str} bytes");
        let len_val: usize = len_str.parse().unwrap_or(0);
        assert!(
            len_val > 0,
            "LEN field should be positive when metadata is present"
        );
        assert!(
            len_val == BLADERF_FLASH_FPGA_SIZE_40KLE || len_val == BLADERF_FLASH_FPGA_SIZE_115KLE,
            "LEN field should match a valid FPGA size, got {len_val}"
        );
    }

    Ok(())
}

#[test]
fn flash_fpga_roundtrip() -> Result<()> {
    logging_init("bladerf1_fpga_flash_roundtrip");

    let mut sdr = sdr();

    let fpga_size = sdr.detect_fpga_size();
    let bitstream_len = valid_fpga_size(&fpga_size);
    let dummy = vec![0xA5u8; bitstream_len];

    let backup = sdr.backup_fpga()?;

    {
        let mut flash = sdr.flash_session()?;
        flash.flash_fpga(&dummy)?;
    }

    let total_pages = 1 + bitstream_len.div_ceil(BLADERF_FLASH_PAGE_SIZE);
    let mut readback = vec![0u8; total_pages * BLADERF_FLASH_PAGE_SIZE];
    {
        let mut flash = sdr.flash_session()?;
        flash.read_pages(FPGA_PAGE, total_pages, &mut readback)?;
    }

    let meta_len_str = binkv_decode_field(&readback[..BLADERF_FLASH_PAGE_SIZE], "LEN")
        .expect("metadata should contain LEN field");
    assert_eq!(
        meta_len_str.parse::<usize>().unwrap(),
        bitstream_len,
        "LEN in metadata should match bitstream length"
    );

    let stored_bitstream = &readback[BLADERF_FLASH_PAGE_SIZE..];
    let padded_len = bitstream_len.div_ceil(BLADERF_FLASH_PAGE_SIZE) * BLADERF_FLASH_PAGE_SIZE;
    assert_eq!(
        &stored_bitstream[..bitstream_len],
        &dummy[..],
        "stored bitstream should match written data"
    );
    assert!(
        stored_bitstream[bitstream_len..padded_len]
            .iter()
            .all(|&b| b == 0xFF),
        "padding bytes should be 0xFF"
    );

    sdr.restore_fpga(&backup);
    Ok(())
}

#[test]
fn erase_stored_fpga_roundtrip() -> Result<()> {
    logging_init("bladerf1_fpga_erase");

    let mut sdr = sdr();

    let backup = sdr.backup_fpga()?;

    {
        let mut flash = sdr.flash_session()?;
        flash.erase_stored_fpga()?;
    }

    let mut erased = vec![0u8; sdr.flash_session()?.fpga_flash_bytes()];
    let page_count = erased.len() / BLADERF_FLASH_PAGE_SIZE;
    {
        let mut flash = sdr.flash_session()?;
        flash.read_pages(FPGA_PAGE, page_count, &mut erased)?;
    }
    assert!(
        erased.iter().all(|&b| b == 0xFF),
        "erased FPGA region should be all 0xFF"
    );

    sdr.restore_fpga(&backup);
    Ok(())
}
*/
