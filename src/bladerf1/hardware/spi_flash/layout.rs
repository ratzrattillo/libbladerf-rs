use super::{BLADERF_FLASH_ADDR_FPGA, BLADERF_FLASH_ERASE_BLOCK_SIZE, BLADERF_FLASH_PAGE_SIZE};
use crate::{Error, Result};

pub(crate) type Page = [u8; BLADERF_FLASH_PAGE_SIZE];
pub(crate) const PAGES_PER_SECTOR: u32 =
    (BLADERF_FLASH_ERASE_BLOCK_SIZE / BLADERF_FLASH_PAGE_SIZE) as u32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PageIndex(u16);

impl PageIndex {
    pub(crate) fn get(self) -> u16 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SectorIndex(u16);

impl SectorIndex {
    pub(crate) fn get(self) -> u16 {
        self.0
    }
    pub(crate) fn first_page(self) -> PageIndex {
        PageIndex((u32::from(self.0) * PAGES_PER_SECTOR) as u16)
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

    pub(crate) fn pages(
        &self,
        start: u32,
        count: usize,
    ) -> Result<impl Iterator<Item = PageIndex> + use<>> {
        let range = checked_range(start, count, self.total_pages())?;
        Ok(range.map(|index| PageIndex(index as u16)))
    }

    pub(crate) fn page(&self, index: u32) -> Result<PageIndex> {
        self.pages(index, 1)?
            .next()
            .ok_or(Error::Internal("missing validated flash page"))
    }

    pub(crate) fn sectors(
        &self,
        start: u32,
        count: usize,
    ) -> Result<impl Iterator<Item = SectorIndex> + use<>> {
        let range = checked_range(start, count, self.total_sectors())?;
        Ok(range.map(|index| SectorIndex(index as u16)))
    }

    pub(crate) fn sector(&self, index: u32) -> Result<SectorIndex> {
        self.sectors(index, 1)?
            .next()
            .ok_or(Error::Internal("missing validated flash sector"))
    }

    pub(crate) fn program_sectors(
        &self,
        page_start: u32,
        pages: &[Page],
    ) -> Result<impl Iterator<Item = SectorIndex> + use<>> {
        if !page_start.is_multiple_of(PAGES_PER_SECTOR) {
            return Err(Error::Argument(
                "flash programming must start at a sector boundary".into(),
            ));
        }
        checked_range(page_start, pages.len(), self.total_pages())?;
        self.sectors(
            page_start / PAGES_PER_SECTOR,
            pages.len().div_ceil(PAGES_PER_SECTOR as usize),
        )
    }
}

fn checked_range(start: u32, count: usize, capacity: u32) -> Result<std::ops::Range<u32>> {
    let count = u32::try_from(count)
        .map_err(|_| Error::Argument("flash range count exceeds u32".into()))?;
    let end = start
        .checked_add(count)
        .filter(|&end| end <= capacity)
        .ok_or_else(|| Error::Argument("flash range exceeds capacity".into()))?;
    Ok(start..end)
}

pub(crate) fn pages(bytes: &[u8]) -> Result<&[Page]> {
    let (pages, remainder) = bytes.as_chunks();
    if !remainder.is_empty() {
        return Err(Error::Argument(
            "flash data must contain complete 256-byte pages".into(),
        ));
    }
    Ok(pages)
}

pub(crate) fn pages_mut(bytes: &mut [u8]) -> Result<&mut [Page]> {
    let (pages, remainder) = bytes.as_chunks_mut();
    if !remainder.is_empty() {
        return Err(Error::Argument(
            "flash buffer must contain complete 256-byte pages".into(),
        ));
    }
    Ok(pages)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flash_geometry_checks_units_full_ranges_and_usb_index_width() {
        let geometry = FlashMeta::new(16 << 20).unwrap();
        assert_eq!(geometry.page(65_535).unwrap().get(), 65_535);
        assert!(geometry.page(65_536).is_err());
        assert_eq!(geometry.pages(65_536, 0).unwrap().count(), 0);
        for (start, count) in [(65_537, 0), (65_535, 2), (u32::MAX, 2), (0, usize::MAX)] {
            assert!(geometry.pages(start, count).is_err());
        }
        assert!(geometry.sectors(255, 2).is_err());
        assert_eq!(geometry.sectors(256, 0).unwrap().count(), 0);
        assert_eq!(geometry.sector(255).unwrap().first_page().get(), 65_280);
        for size in [0, 1, 65_536, 4_194_305, 32 << 20] {
            assert!(FlashMeta::new(size).is_err());
        }
    }

    #[test]
    fn page_views_and_programming_reject_partial_pages_and_unaligned_starts() {
        for size in [0, 255, 256, 257] {
            let mut data = vec![0; size];
            assert_eq!(pages(&data).is_ok(), size.is_multiple_of(256));
            assert_eq!(pages_mut(&mut data).is_ok(), size.is_multiple_of(256));
        }
        let geometry = FlashMeta::new(4 << 20).unwrap();
        assert!(geometry.program_sectors(1, &[[0; 256]]).is_err());
        assert_eq!(
            geometry.program_sectors(256, &[[0; 256]]).unwrap().count(),
            1
        );
        assert_eq!(geometry.program_sectors(16_384, &[]).unwrap().count(), 0);
        assert!(geometry.program_sectors(16_384, &[[0; 256]]).is_err());
    }
}
