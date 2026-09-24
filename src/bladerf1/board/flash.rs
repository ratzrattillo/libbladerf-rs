//! Low-level SPI flash erase/write/verify operations.
//!
//! Provides the core flash programming primitive used by higher-level
//! firmware and FPGA flashing. The process operates in sector-sized
//! chunks: erases a sector, writes constituent pages, then verifies
//! the data. Each sector undergoes up to three retry attempts if
//! verification fails.

use crate::bladerf1::board::FlashSession;
use crate::bladerf1::hardware::spi_flash::BLADERF_FLASH_ERASE_BLOCK_SIZE;
use crate::bladerf1::hardware::spi_flash::layout::{PAGES_PER_SECTOR, Page, SectorIndex, pages};
use crate::error::{Error, Result};
use crate::maybe_future::{NonWasmSend, Op};
use nusb::MaybeFuture;
use std::future::Future;

const MAX_RETRIES: u8 = 3;

impl FlashSession<'_> {
    /// Erases, writes, and verifies whole pages starting at a sector boundary.
    ///
    /// Each affected 64-KiB sector is erased in full, including any unwritten
    /// tail in the final sector. Preserve neighboring data by supplying a
    /// complete replacement sector. Empty data is a validated no-op.
    ///
    /// A sector gets one initial attempt and at most three verification
    /// retries. Only a byte mismatch triggers erase/rewrite; communication
    /// failures are returned immediately. Cancellation keeps completed side
    /// effects and retains any pending page transaction. Calling this method
    /// again deliberately restarts the whole requested sector range.
    ///
    /// # Errors
    /// Rejects partial pages, misaligned starts, and invalid complete ranges
    /// before I/O. Returns the actual final verification or USB/firmware error.
    /// Verification offsets are relative to the supplied `page_start`.
    pub fn erase_write_verify(
        &mut self,
        page_start: u32,
        data: &[u8],
    ) -> impl MaybeFuture<Output = Result<()>> {
        Op::new(async move {
            let pages = pages(data)?;
            let sectors = self.flash_meta.program_sectors(page_start, pages)?;
            for (index, (sector, data)) in sectors
                .zip(pages.chunks(PAGES_PER_SECTOR as usize))
                .enumerate()
            {
                self.program_sector(sector, data).await.map_err(|error| {
                    verification_offset(error, index * BLADERF_FLASH_ERASE_BLOCK_SIZE)
                })?;
            }
            Ok(())
        })
    }
}

trait SectorIo: NonWasmSend {
    fn rewrite(
        &mut self,
        sector: SectorIndex,
        pages: &[Page],
    ) -> impl Future<Output = Result<()>> + NonWasmSend;
    fn verify(
        &mut self,
        sector: SectorIndex,
        pages: &[Page],
    ) -> impl Future<Output = Result<()>> + NonWasmSend;

    async fn program_sector(&mut self, sector: SectorIndex, pages: &[Page]) -> Result<()> {
        for attempt in 0..=MAX_RETRIES {
            self.rewrite(sector, pages).await?;
            match self.verify(sector, pages).await {
                Err(Error::FlashVerificationFailed { .. }) if attempt < MAX_RETRIES => {}
                result => return result,
            }
        }
        unreachable!()
    }
}

impl SectorIo for FlashSession<'_> {
    async fn rewrite(&mut self, sector: SectorIndex, pages: &[Page]) -> Result<()> {
        self.erase_sector_index(sector).await?;
        self.write_pages(
            u32::from(sector.first_page().get()),
            pages.len(),
            pages.as_flattened(),
        )
        .await
    }

    async fn verify(&mut self, sector: SectorIndex, pages: &[Page]) -> Result<()> {
        self.verify_pages(u32::from(sector.first_page().get()), pages.as_flattened())
            .await
    }
}

fn verification_offset(error: Error, base: usize) -> Error {
    match error {
        Error::FlashVerificationFailed {
            byte_offset,
            expected,
            actual,
        } => Error::FlashVerificationFailed {
            byte_offset: base + byte_offset,
            expected,
            actual,
        },
        error => error,
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use crate::bladerf1::hardware::spi_flash::BLADERF_FLASH_PAGE_SIZE;
    use crate::bladerf1::hardware::spi_flash::layout::FlashMeta;
    use crate::maybe_future::block_on;
    use std::collections::VecDeque;

    struct Io {
        outcomes: VecDeque<Result<()>>,
        writes: usize,
    }

    impl SectorIo for Io {
        async fn rewrite(&mut self, _: SectorIndex, _: &[Page]) -> Result<()> {
            self.writes += 1;
            Ok(())
        }
        async fn verify(&mut self, _: SectorIndex, _: &[Page]) -> Result<()> {
            self.outcomes.pop_front().unwrap()
        }
    }

    fn mismatch(offset: usize) -> Error {
        Error::FlashVerificationFailed {
            byte_offset: offset,
            expected: 0xab,
            actual: 0xcd,
        }
    }

    #[test]
    fn verification_retries_only_mismatches_and_preserves_the_final_error() {
        let sector = FlashMeta::new(4 << 20).unwrap().sector(7).unwrap();
        let mut io = Io {
            outcomes: (0..4).map(|offset| Err(mismatch(512 + offset))).collect(),
            writes: 0,
        };
        let error =
            block_on(io.program_sector(sector, &[[0; BLADERF_FLASH_PAGE_SIZE]])).unwrap_err();
        assert_eq!(io.writes, 4);
        assert!(matches!(
            verification_offset(error, 65_536),
            Error::FlashVerificationFailed {
                byte_offset: 66_051,
                expected: 0xab,
                actual: 0xcd
            }
        ));
        let mut io = Io {
            outcomes: VecDeque::from([Err(mismatch(3)), Ok(())]),
            writes: 0,
        };
        block_on(io.program_sector(sector, &[[0; BLADERF_FLASH_PAGE_SIZE]])).unwrap();
        assert_eq!(io.writes, 2);
        for error in [
            Error::Timeout,
            Error::FirmwareStatus {
                request: 100,
                status: 5,
            },
        ] {
            let mut io = Io {
                outcomes: VecDeque::from([Err(error)]),
                writes: 0,
            };
            assert!(block_on(io.program_sector(sector, &[[0; BLADERF_FLASH_PAGE_SIZE]])).is_err());
            assert_eq!(io.writes, 1);
        }
    }
}
