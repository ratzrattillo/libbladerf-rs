use super::{BLADERF_FLASH_ADDR_FPGA, BLADERF_FLASH_ERASE_BLOCK_SIZE, BLADERF_FLASH_PAGE_SIZE};
use crate::{Error, Result};

pub(crate) type Page = [u8; BLADERF_FLASH_PAGE_SIZE];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PageIndex(u16);

impl PageIndex {
    pub(crate) fn get(self) -> u16 {
        self.0
    }
}

#[derive(Debug)]
pub(crate) struct FlashMeta {
    size_bytes: u32,
}

impl FlashMeta {
    pub(crate) fn new(size_bytes: u32) -> Result<Self> {
        if size_bytes < BLADERF_FLASH_ADDR_FPGA
            || !size_bytes.is_multiple_of(BLADERF_FLASH_ERASE_BLOCK_SIZE as u32)
            || size_bytes / BLADERF_FLASH_PAGE_SIZE as u32 > u32::from(u16::MAX) + 1
        {
            return Err(Error::Unsupported(
                "flash geometry cannot be addressed by the firmware",
            ));
        }
        Ok(Self { size_bytes })
    }

    pub(crate) fn size_bytes(&self) -> u32 {
        self.size_bytes
    }

    pub(crate) fn total_pages(&self) -> u32 {
        self.size_bytes / BLADERF_FLASH_PAGE_SIZE as u32
    }

    pub(crate) fn total_sectors(&self) -> u32 {
        self.size_bytes / BLADERF_FLASH_ERASE_BLOCK_SIZE as u32
    }

    pub(crate) fn page(&self, index: u32) -> Result<PageIndex> {
        if index >= self.total_pages() {
            return Err(Error::Argument("flash page exceeds capacity".into()));
        }
        Ok(PageIndex(index as u16))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geometry_covers_bladerf2_flash_sizes() {
        for (size, pages, sectors) in [(4 << 20, 16_384, 64), (16 << 20, 65_536, 256)] {
            let meta = FlashMeta::new(size).unwrap();
            assert_eq!(meta.total_pages(), pages);
            assert_eq!(meta.total_sectors(), sectors);
            assert_eq!(meta.page(pages - 1).unwrap().get() as u32, pages - 1);
            assert!(meta.page(pages).is_err());
        }
        for size in [0, 1, 65_536, 4_194_305, 32 << 20] {
            assert!(FlashMeta::new(size).is_err());
        }
    }
}
