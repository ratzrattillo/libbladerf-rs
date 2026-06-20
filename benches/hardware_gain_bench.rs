use criterion::{Criterion, criterion_group, criterion_main};
use libbladerf_rs::bladerf1::BladeRf1;
use libbladerf_rs::bladerf1::hardware::lms6002d::gain::GainStage;
use libbladerf_rs::channel::Channel;

fn bench_get_gain(c: &mut Criterion) {
    let mut device = BladeRf1::from_first().expect("No BladeRF1 found");
    let mut rf = device.rf_link_session().expect("Session failed");
    rf.initialize(true).expect("Initialize failed");
    let mut group = c.benchmark_group("hardware_gain");
    group.sample_size(20);
    group.measurement_time(std::time::Duration::from_secs(5));

    group.bench_function("get_gain_rx", |b| {
        b.iter(|| rf.get_gain(Channel::Rx).unwrap())
    });
}

fn bench_set_gain(c: &mut Criterion) {
    let mut device = BladeRf1::from_first().expect("No BladeRF1 found");
    let mut rf = device.rf_link_session().expect("Session failed");
    rf.initialize(true).expect("Initialize failed");
    let gain_db = rf.get_gain(Channel::Rx).unwrap().db();
    let mut group = c.benchmark_group("hardware_gain");
    group.sample_size(10);
    group.measurement_time(std::time::Duration::from_secs(5));

    group.bench_function("set_gain_rx", |b| {
        b.iter(|| rf.set_gain(Channel::Rx, (gain_db).into()).unwrap())
    });
}

fn bench_set_gain_stage(c: &mut Criterion) {
    let mut device = BladeRf1::from_first().expect("No BladeRF1 found");
    let mut rf = device.rf_link_session().expect("Session failed");
    rf.initialize(true).expect("Initialize failed");
    let mut group = c.benchmark_group("hardware_gain");
    group.sample_size(10);
    group.measurement_time(std::time::Duration::from_secs(5));

    group.bench_function("set_gain_stage_rxvga1", |b| {
        b.iter(|| rf.set_gain_stage(GainStage::RxVga1, 5i8.into()).unwrap())
    });
}

criterion_group!(
    benches,
    bench_get_gain,
    bench_set_gain,
    bench_set_gain_stage,
);
criterion_main!(benches);
