//! Runtime-free `.wait()` smoke test.
//!
//! Built with `--no-default-features --features bladerf1`, so neither `smol`
//! nor `tokio` is compiled in and nusb has no async runtime. Device
//! enumeration is the one `MaybeFuture` entry point that runs without
//! hardware; on Windows nusb backs it with a `Blocking` operation, so this
//! exercises the exact path that must route through `MaybeFuture::wait`
//! instead of spawning a task. A regression here panics with nusb's "no async
//! runtime" message rather than failing an assertion.

#![cfg(all(feature = "bladerf1", not(target_os = "android")))]

use libbladerf_rs::MaybeFuture;
use libbladerf_rs::bladerf1::BladeRf1;

#[test]
fn list_devices_wait_without_runtime() {
    let devices = BladeRf1::list_bladerf1()
        .wait()
        .expect("USB enumeration must succeed without an async runtime");
    // Read-only: enumeration is valid whether or not a BladeRF is attached.
    let _ = devices.count();
}
