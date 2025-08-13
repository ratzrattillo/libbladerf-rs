use libbladerf_rs::bladerf1::BladeRf1;
/// This module has been created using mod.rs in a subfolder, instead of just creating a common.rs under tests
/// This is due to the test runner then not searching for runnable tests in mod.rs
/// https://doc.rust-lang.org/rust-by-example/testing/integration_testing.html
use std::sync::LazyLock;

#[allow(dead_code)]
pub static BLADERF: LazyLock<BladeRf1> = LazyLock::new(|| {
    let sdr = BladeRf1::from_first().unwrap();
    // sdr.device_reset().unwrap();
    sdr.initialize().unwrap();
    sdr
});

pub fn logging_init(module: &str) {
    let _ = env_logger::builder()
        .is_test(true)
        .filter_level(log::LevelFilter::Error)
        .filter_module(module, log::LevelFilter::Trace)
        .try_init();
}
