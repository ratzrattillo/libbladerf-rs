//! Compile-time contract for the public API.
//!
//! No hardware is touched: the functions below are never called. They only
//! have to type-check, which pins the documented entry points, their
//! `MaybeFuture` shape, feature gating and `Send`-ness. Change this file
//! deliberately when the public API changes.

#![cfg(feature = "bladerf1")]

use libbladerf_rs::bladerf1::{
    BladeRf1, ConfigSession, ExpansionBoard, FlashSession, GainDb, GainMode, RfLinkSession,
    RxStream, SampleFormat, TuningMode, TxStream,
};
use libbladerf_rs::{Buffer, Channel, Error, ErrorKind, MaybeFuture, Result};
use std::future::IntoFuture;
use std::time::Duration;

#[cfg(not(target_arch = "wasm32"))]
fn assert_send<T: Send>(_: &T) {}

fn maybe_future<T>(_: &impl MaybeFuture<Output = T>) {}

#[allow(dead_code)]
async fn open_paths(device: nusb::Device) -> Result<()> {
    #[cfg(not(target_os = "android"))]
    {
        let by_first: BladeRf1 = BladeRf1::from_first().await?;
        let by_serial: BladeRf1 = BladeRf1::from_serial("serial").await?;
        by_first.close().await?;
        by_serial.close().await?;
    }
    let by_device = BladeRf1::from_device(device);
    maybe_future(&by_device);
    let by_device: BladeRf1 = by_device.await?;
    #[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
    {
        let by_bus: BladeRf1 = BladeRf1::from_bus_addr("1", 2).await?;
        by_bus.close().await?;
    }
    let _serial: String = by_device.serial().await?;
    let _speed: nusb::Speed = by_device.speed();
    by_device.close().await
}

#[cfg(any(target_os = "linux", target_os = "android"))]
#[allow(dead_code)]
async fn open_fd(fd: std::os::fd::OwnedFd) -> Result<()> {
    let open = BladeRf1::from_fd(fd);
    maybe_future(&open);
    assert_send(&open);
    open.await?.close().await
}

#[allow(dead_code)]
async fn sessions(dev: &mut BladeRf1) -> Result<()> {
    {
        let mut rf: RfLinkSession<'_> = dev.rf_link_session().await?;
        rf.initialize(false).await?;
        rf.set_frequency(Channel::Rx, 915_000_000, TuningMode::Fpga)
            .await?;
        let _hz: u64 = rf.get_frequency(Channel::Rx).await?;
        let _actual: u32 = rf.set_sample_rate(Channel::Rx, 4_000_000).await?;
        let _bw: u32 = rf.set_bandwidth(Channel::Rx, 4_000_000).await?;
        rf.set_gain(Channel::Rx, GainDb::from(30)).await?;
        let _gain: GainDb = rf.get_gain(Channel::Rx).await?;
        rf.set_gain_mode(Channel::Rx, GainMode::Mgc).await?;
        rf.expansion_attach(ExpansionBoard::XbNone).await?;
        let _range = RfLinkSession::get_sample_rate_range();
        let _stages = RfLinkSession::get_gain_stages(Channel::Rx);
    }
    {
        let flash: FlashSession<'_> = dev.flash_session().await?;
        let _bytes: u32 = flash.size_bytes();
    }
    {
        let _cfg: ConfigSession<'_> = dev.config_session().await?;
    }
    let _configured: bool = dev.is_fpga_configured().await?;
    Ok(())
}

#[allow(dead_code)]
async fn streams(rf: &mut RfLinkSession<'_>) -> Result<()> {
    let mut rx: RxStream = RxStream::builder(rf)
        .buffer_size(65_536)
        .buffer_count(8)
        .format(SampleFormat::Sc16Q11)
        .build()
        .await?;
    rx.start(rf).await?;
    let buffer: Buffer = rx.read(None).await?;
    rx.recycle(buffer);
    let _ = rx.try_read();
    rx.stop(rf).await?;
    rx.close(rf).await?;

    let mut tx: TxStream = TxStream::builder(rf).build().await?;
    tx.start(rf).await?;
    let mut buffer: Buffer = tx.get_buffer(Some(Duration::from_secs(1))).await?;
    buffer.extend_from_slice(&[0; 4]);
    tx.submit(buffer, 4)?;
    tx.wait_completion(None).await?;
    tx.close(rf).await
}

/// Every I/O method is a `MaybeFuture`: the same call can be `.wait()`ed
/// (native) or `.await`ed. On native the futures are `Send`.
#[allow(dead_code)]
fn dual_mode(dev: &mut BladeRf1, rx: &mut RxStream) {
    let session = dev.rf_link_session();
    maybe_future(&session);
    let fut = session.into_future();
    #[cfg(not(target_arch = "wasm32"))]
    assert_send(&fut);
    drop(fut);
    let read = rx.read(None);
    maybe_future(&read);
    #[cfg(not(target_arch = "wasm32"))]
    assert_send(&read);
    drop(read);
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _: Result<RxStream> = RxStream::builder(&mut dev.rf_link_session().wait().unwrap())
            .build()
            .wait();
    }
    #[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
    {
        let open = BladeRf1::from_first();
        assert_send(&open.into_future());
    }
}

#[test]
fn error_kinds_are_public_and_matchable() {
    let e = Error::StreamNotStarted;
    assert_eq!(e.kind(), ErrorKind::State);
    match e.kind() {
        ErrorKind::Usb
        | ErrorKind::Protocol
        | ErrorKind::Timeout
        | ErrorKind::WouldBlock
        | ErrorKind::NotFound
        | ErrorKind::InvalidArgument
        | ErrorKind::Unsupported
        | ErrorKind::State
        | ErrorKind::Hardware
        | ErrorKind::Calibration
        | ErrorKind::Flash
        | ErrorKind::Io
        | ErrorKind::Internal => {}
        _ => unreachable!("ErrorKind is non_exhaustive"),
    }
}

#[test]
fn sample_format_helpers_are_pure() {
    assert_eq!(SampleFormat::Sc16Q11.sample_size(), 4);
    assert!(SampleFormat::Sc16Q11Meta.requires_timestamps());
    let src = [0u8; 8];
    let mut dst = [0u8; 6];
    SampleFormat::pack_sc16q11_packed(&src, &mut dst, 2).unwrap();
}
