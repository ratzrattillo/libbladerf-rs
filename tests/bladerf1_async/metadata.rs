use super::*;
use libbladerf_rs::Error;
use libbladerf_rs::bladerf1::{MetadataLayout, RxMux};

#[derive(Default)]
struct Continuity {
    sample: Option<u32>,
    timestamp: Option<u64>,
    gaps: usize,
    timestamp_gaps: usize,
    bad_headers: usize,
    messages: usize,
}

impl Continuity {
    fn samples(&mut self, bytes: &[u8]) {
        for sample in bytes.as_chunks::<4>().0 {
            let value = u32::from_le_bytes(*sample);
            if self
                .sample
                .is_some_and(|previous| value != previous.wrapping_add(1))
            {
                self.gaps += 1;
            }
            self.sample = Some(value);
        }
    }

    fn metadata(&mut self, layout: MetadataLayout, bytes: &[u8]) -> Result<()> {
        for message in layout.messages(bytes)? {
            let header = message.header();
            if header.to_bytes()[..4] != [0x21, 0x43, 0x34, 0x12] {
                self.bad_headers += 1;
            }
            if self.timestamp.is_some_and(|previous| {
                header.timestamp().wrapping_sub(previous) != layout.samples_per_message() as u64
            }) {
                self.timestamp_gaps += 1;
            }
            self.timestamp = Some(header.timestamp());
            self.messages += 1;
            self.samples(message.payload());
        }
        Ok(())
    }
}

#[tokio::test(flavor = "current_thread")]
async fn counter_metadata_restart_and_duplex_format_preservation() -> Result<()> {
    let mut device = open().await?;
    let mut rf = device.rf_link_session().await?;
    let original_rate = rf.get_sample_rate(Channel::Rx).await?;
    let original_mux = rf.get_rx_mux().await?;
    let original_loopback = rf.get_loopback().await?;
    rf.set_loopback(Loopback::None).await?;
    rf.set_sample_rate(Channel::Rx, 1_000_000).await?;
    rf.set_rx_mux(RxMux::Mux32BitCounter).await?;
    let layout = rf.metadata_layout().await?;
    let mut rx = RxStream::builder(&mut rf)
        .buffer_size(layout.message_size() * 2 + 1)
        .buffer_count(8)
        .format(SampleFormat::Sc16Q11Meta)
        .build()
        .await?;
    let mut tx = TxStream::builder(&mut rf)
        .format(SampleFormat::Sc16Q11Meta)
        .buffer_count(2)
        .build()
        .await?;
    let mut continuity = Vec::new();
    let result: Result<()> = async {
        assert_eq!(rx.buffer_size()?, layout.message_size() * 3);
        assert_eq!(rx.metadata_layout()?, layout);
        for _ in 0..3 {
            rx.start(&mut rf).await?;
            tx.start(&mut rf).await?;
            let mut state = Continuity::default();
            for index in 0..24 {
                let buffer = tokio::time::timeout(Duration::from_secs(2), rx.read(None))
                    .await
                    .map_err(|_| Error::Timeout)??;
                let parsed = state.metadata(layout, &buffer);
                rx.recycle(buffer);
                parsed?;
                if index == 7 {
                    tx.stop(&mut rf).await?;
                    let gpio = rf.config_gpio_read().await?;
                    if (gpio & ((1 << 16) | (1 << 17))) != ((1 << 16) | (1 << 17)) {
                        return Err(Error::Internal("peer stop cleared timestamp bits"));
                    }
                }
            }
            rx.stop(&mut rf).await?;
            continuity.push(state);
        }
        Ok(())
    }
    .await;
    let close_rx = rx.close(&mut rf).await;
    let close_tx = tx.close(&mut rf).await;
    let restore_mux = rf.set_rx_mux(original_mux).await;
    let restore_rate = rf.set_sample_rate(Channel::Rx, original_rate).await;
    let restore_loopback = rf.set_loopback(original_loopback).await;
    result?;
    close_rx?;
    close_tx?;
    restore_mux?;
    restore_rate?;
    restore_loopback?;
    for state in continuity {
        println!(
            "{}-byte messages: {}, sample gaps: {}, timestamp gaps: {}, bad headers: {}",
            layout.message_size(),
            state.messages,
            state.gaps,
            state.timestamp_gaps,
            state.bad_headers
        );
        assert_eq!(
            (state.gaps, state.timestamp_gaps, state.bad_headers),
            (0, 0, 0)
        );
        assert_eq!(state.messages, 72);
    }
    device.close().await
}
