use libbladerf_rs::bladerf1::BladeRf1;
use std::sync::{LazyLock, Mutex};

pub static BLADERF: LazyLock<Mutex<BladeRf1>> = LazyLock::new(|| {
    let mut sdr = BladeRf1::from_first().unwrap();
    sdr.initialize(false).unwrap();
    Mutex::new(sdr)
});

pub fn sdr() -> std::sync::MutexGuard<'static, BladeRf1> {
    match BLADERF.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            log::warn!("BLADERF mutex poisoned — recovering");
            poisoned.into_inner()
        }
    }
}

pub fn logging_init(module: &str) {
    let _ = env_logger::builder()
        .is_test(true)
        .filter_level(log::LevelFilter::Trace)
        .filter_module(module, log::LevelFilter::Trace)
        .try_init();
}
