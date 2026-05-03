use crate::bladerf1::BladeRf1;
use crate::bladerf1::hardware::spi_flash;
use crate::error::Result;
use crate::flash::{
    BLADERF_FLASH_ADDR_CAL, BLADERF_FLASH_ERASE_BLOCK_SIZE, BLADERF_FLASH_PAGE_SIZE, FpgaSize,
    binkv_decode_field, make_cal_region,
};

impl BladeRf1 {
    pub fn read_flash_dac_trim(&mut self) -> Result<u16> {
        let mut buf = [0u8; BLADERF_FLASH_PAGE_SIZE];
        spi_flash::read_cal_cache(&mut self.nios, self.chunk_size, &mut buf)?;
        let dac_str = binkv_decode_field(&buf, "DAC")?;
        dac_str
            .parse::<u16>()
            .map_err(|_| crate::error::Error::HardwareState("failed to parse DAC trim from flash"))
    }

    pub fn read_flash_fpga_size(&mut self) -> Result<FpgaSize> {
        let mut buf = [0u8; BLADERF_FLASH_PAGE_SIZE];
        spi_flash::read_cal_cache(&mut self.nios, self.chunk_size, &mut buf)?;
        let fpga_str = binkv_decode_field(&buf, "B")?;
        FpgaSize::parse(&fpga_str)
    }

    pub fn save_dac_trim(&mut self, dac_trim: u16) -> Result<()> {
        let fpga_size = self.read_flash_fpga_size()?;
        let cal_image = make_cal_region(fpga_size, dac_trim)?;
        let cal_sector = (BLADERF_FLASH_ADDR_CAL / BLADERF_FLASH_ERASE_BLOCK_SIZE as u32) as u16;
        let cal_page = (BLADERF_FLASH_ADDR_CAL / BLADERF_FLASH_PAGE_SIZE as u32) as u16;
        spi_flash::erase_and_write_page(
            &mut self.nios,
            self.chunk_size,
            cal_sector,
            cal_page,
            &cal_image,
        )
    }
}
