use libbladerf_rs::MaybeFuture;
use libbladerf_rs::bladerf2::BladeRf2;

pub fn open() -> BladeRf2 {
    BladeRf2::from_first().wait().unwrap()
}

pub fn logging_init(module: &str) {
    let _ = env_logger::builder()
        .is_test(true)
        .filter_level(log::LevelFilter::Debug)
        .filter_module(module, log::LevelFilter::Trace)
        .try_init();
}
