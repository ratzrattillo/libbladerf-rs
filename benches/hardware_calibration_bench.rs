use criterion::{Criterion, criterion_group, criterion_main};
use libbladerf_rs::bladerf1::BladeRf1;
use libbladerf_rs::bladerf1::hardware::lms6002d::dc_calibration::DcCalModule;

fn bench_calibrate_dc(c: &mut Criterion) {
    let mut device = BladeRf1::from_first().expect("No BladeRF1 found");
    let mut rf = device.rf_link_session().expect("Session failed");
    rf.initialize(true).expect("Initialize failed");
    let mut group = c.benchmark_group("hardware_calibration");
    group.sample_size(10);
    group.measurement_time(std::time::Duration::from_secs(10));

    group.bench_function("calibrate_dc_rx_lpf", |b| {
        b.iter(|| rf.calibrate_dc(DcCalModule::RxLpf).unwrap())
    });
}

criterion_group!(benches, bench_calibrate_dc,);
criterion_main!(benches);
