use super::{ControlResult, TIMEOUT, UsbTransport, VendorRequest, firmware_status, require_length};
use crate::maybe_future::NonWasmSend;
use crate::{Error, Result};
use nusb::transfer::{ControlIn, ControlOut, ControlType, Recipient};
use nusb::{Interface, Speed};
use std::future::Future;

type Page = [u8; crate::flash::BLADERF_FLASH_PAGE_SIZE];

enum PageSource {
    Flash(u16),
    Calibration,
}

trait PageIo {
    fn read(
        &mut self,
        request: VendorRequest,
        index: u16,
        length: u16,
    ) -> impl Future<Output = Result<Vec<u8>>> + NonWasmSend;
    fn write(&mut self, index: u16, data: &[u8]) -> impl Future<Output = Result<()>> + NonWasmSend;
}

impl PageIo for Interface {
    async fn read(&mut self, request: VendorRequest, index: u16, length: u16) -> Result<Vec<u8>> {
        let bytes = self
            .control_in(
                ControlIn {
                    control_type: ControlType::Vendor,
                    recipient: Recipient::Device,
                    request: request as u8,
                    value: 0,
                    index,
                    length,
                },
                TIMEOUT,
            )
            .await?;
        require_length(usize::from(length), bytes.len())?;
        Ok(bytes)
    }

    async fn write(&mut self, index: u16, data: &[u8]) -> Result<()> {
        self.control_out(
            ControlOut {
                control_type: ControlType::Vendor,
                recipient: Recipient::Device,
                request: VendorRequest::WritePageBuffer as u8,
                value: 0,
                index,
                data,
            },
            TIMEOUT,
        )
        .await
        .map_err(Error::from)
    }
}

#[derive(Clone, Copy)]
enum PageChunks {
    High,
    Super,
}

impl TryFrom<Speed> for PageChunks {
    type Error = Error;
    fn try_from(speed: Speed) -> Result<Self> {
        match speed {
            Speed::High => Ok(Self::High),
            Speed::Super | Speed::SuperPlus => Ok(Self::Super),
            _ => Err(Error::UnsupportedSpeed),
        }
    }
}

impl PageChunks {
    fn size(self) -> usize {
        match self {
            Self::High => 64,
            Self::Super => 256,
        }
    }

    async fn read(self, io: &mut impl PageIo, request: VendorRequest) -> Result<Page> {
        let mut page = [0; 256];
        for (index, chunk) in page.chunks_exact_mut(self.size()).enumerate() {
            let bytes = io
                .read(request, (index * self.size()) as u16, self.size() as u16)
                .await?;
            require_length(self.size(), bytes.len())?;
            chunk.copy_from_slice(&bytes);
        }
        Ok(page)
    }

    async fn write(self, io: &mut impl PageIo, index: u16, page: &Page) -> Result<()> {
        for (chunk_index, chunk) in page.chunks_exact(self.size()).enumerate() {
            io.write((chunk_index * self.size()) as u16, chunk).await?;
        }
        let status = io.read(VendorRequest::FlashWrite, index, 4).await?;
        status_result(VendorRequest::FlashWrite, &status)
    }
}

fn status_result(request: VendorRequest, bytes: &[u8]) -> Result<()> {
    require_length(4, bytes.len())?;
    firmware_status(request, u32::from_le_bytes(bytes.try_into().unwrap()))
}

impl UsbTransport {
    pub(crate) async fn read_flash_page(&mut self, index: u16) -> Result<Page> {
        self.read_page(PageSource::Flash(index)).await
    }

    pub(crate) async fn read_calibration_page(&mut self) -> Result<Page> {
        self.read_page(PageSource::Calibration).await
    }

    async fn read_page(&mut self, source: PageSource) -> Result<Page> {
        let chunks = PageChunks::try_from(self.speed())?;
        self.prepare_io().await?;
        let mut interface = self.interface.clone();
        self.pending.begin(async move {
            let request = match source {
                PageSource::Flash(index) => {
                    let status = interface.read(VendorRequest::FlashRead, index, 4).await?;
                    status_result(VendorRequest::FlashRead, &status)?;
                    VendorRequest::ReadPageBuffer
                }
                PageSource::Calibration => VendorRequest::ReadCalCache,
            };
            chunks
                .read(&mut interface, request)
                .await
                .map(ControlResult::FlashPage)
        });
        match self.finish_pending(TIMEOUT).await? {
            Some(ControlResult::FlashPage(page)) => Ok(page),
            _ => Err(Error::Internal("missing flash page response")),
        }
    }

    pub(crate) async fn write_flash_page(&mut self, index: u16, page: &Page) -> Result<()> {
        let chunks = PageChunks::try_from(self.speed())?;
        self.prepare_io().await?;
        let mut interface = self.interface.clone();
        let page = *page;
        self.pending.begin(async move {
            chunks.write(&mut interface, index, &page).await?;
            Ok(ControlResult::Done)
        });
        self.finish_pending(TIMEOUT).await.map(|_| ())
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use crate::maybe_future::block_on;
    use crate::usb::pending::Pending;
    use std::sync::{Arc, Mutex};
    use std::task::{Context, Poll, Waker};
    use std::time::Duration;

    #[derive(Default)]
    struct State {
        chunks: Vec<(u16, Vec<u8>)>,
        writes: usize,
        status: u32,
        short: bool,
        pause: Option<u16>,
    }
    struct Io(Arc<Mutex<State>>);

    impl PageIo for Io {
        async fn read(
            &mut self,
            request: VendorRequest,
            _index: u16,
            length: u16,
        ) -> Result<Vec<u8>> {
            let mut state = self.0.lock().unwrap();
            if request == VendorRequest::FlashWrite {
                state.writes += 1;
                Ok(state.status.to_le_bytes().to_vec())
            } else {
                Ok(vec![0x7e; usize::from(length) - usize::from(state.short)])
            }
        }
        async fn write(&mut self, index: u16, data: &[u8]) -> Result<()> {
            std::future::poll_fn(|_| {
                let mut state = self.0.lock().unwrap();
                if state.pause == Some(index) {
                    return Poll::Pending;
                }
                state.chunks.push((index, data.to_vec()));
                Poll::Ready(Ok(()))
            })
            .await
        }
    }

    #[test]
    fn page_transfers_use_exact_chunks_and_surface_status_and_length_failures() {
        for speed in [Speed::High, Speed::Super, Speed::SuperPlus] {
            let chunks = PageChunks::try_from(speed).unwrap();
            let state = Arc::new(Mutex::new(State::default()));
            let mut io = Io(state.clone());
            block_on(chunks.write(&mut io, 7, &[0x3b; 256])).unwrap();
            assert_eq!(state.lock().unwrap().chunks.len(), 256 / chunks.size());
            assert_eq!(state.lock().unwrap().writes, 1);
            assert_eq!(
                block_on(chunks.read(&mut io, VendorRequest::ReadPageBuffer)).unwrap(),
                [0x7e; 256]
            );
            state.lock().unwrap().status = 5;
            assert!(matches!(
                block_on(chunks.write(&mut io, 7, &[0; 256])),
                Err(Error::FirmwareStatus { status: 5, .. })
            ));
            state.lock().unwrap().short = true;
            assert!(matches!(
                block_on(chunks.read(&mut io, VendorRequest::ReadPageBuffer)),
                Err(Error::UsbTransferLength { .. })
            ));
        }
    }

    #[test]
    fn cancelled_page_wait_retains_remaining_chunks_before_programming() {
        for pause in [0, 64, 128, 192] {
            let state = Arc::new(Mutex::new(State {
                pause: Some(pause),
                ..Default::default()
            }));
            let mut io = Io(state.clone());
            let mut pending = Pending::default();
            pending.begin(async move { PageChunks::High.write(&mut io, 1, &[0xa5; 256]).await });
            {
                let mut future = std::pin::pin!(pending.finish(Duration::MAX));
                assert!(
                    future
                        .as_mut()
                        .poll(&mut Context::from_waker(Waker::noop()))
                        .is_pending()
                );
            }
            assert_eq!(state.lock().unwrap().writes, 0);
            state.lock().unwrap().pause = None;
            block_on(pending.finish(Duration::MAX)).unwrap();
            let state = state.lock().unwrap();
            assert_eq!(
                state
                    .chunks
                    .iter()
                    .map(|(index, _)| *index)
                    .collect::<Vec<_>>(),
                [0, 64, 128, 192]
            );
            assert_eq!(state.writes, 1);
        }
    }
}
