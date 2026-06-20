use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use libbladerf_rs::bladerf1::{BladeRf1, TuningMode, protocol::RetuneTimestamp};
use libbladerf_rs::channel::Channel;

fn bench_set_frequency(c: &mut Criterion) {
    let mut device = BladeRf1::from_first().expect("No BladeRF1 found");
    let mut rf = device.rf_link_session().expect("Session failed");
    rf.initialize(true).expect("Initialize failed");
    let mut group = c.benchmark_group("hardware_tuning");
    group.sample_size(20);
    group.measurement_time(std::time::Duration::from_secs(5));

    let freqs: &[u64] = &[300_000_000, 1_000_000_000, 3_800_000_000];

    for &freq in freqs {
        group.bench_with_input(
            BenchmarkId::new("set_frequency_rx", freq),
            &freq,
            |b, &freq| {
                b.iter(|| {
                    rf.set_frequency(Channel::Rx, freq, TuningMode::Fpga)
                        .unwrap()
                })
            },
        );
    }
}

fn bench_get_frequency(c: &mut Criterion) {
    let mut device = BladeRf1::from_first().expect("No BladeRF1 found");
    let mut rf = device.rf_link_session().expect("Session failed");
    rf.initialize(true).expect("Initialize failed");
    let mut group = c.benchmark_group("hardware_tuning");
    group.sample_size(20);
    group.measurement_time(std::time::Duration::from_secs(5));

    group.bench_function("get_frequency_rx", |b| {
        b.iter(|| rf.get_frequency(Channel::Rx).unwrap())
    });
}

fn bench_schedule_retune(c: &mut Criterion) {
    let mut device = BladeRf1::from_first().expect("No BladeRF1 found");
    let mut rf = device.rf_link_session().expect("Session failed");
    rf.initialize(true).expect("Initialize failed");
    let mut group = c.benchmark_group("hardware_tuning");
    group.sample_size(20);
    group.measurement_time(std::time::Duration::from_secs(5));

    let freq = rf.get_frequency(Channel::Rx).unwrap();

    group.bench_function("schedule_retune_rx", |b| {
        b.iter(|| {
            rf.schedule_retune(Channel::Rx, RetuneTimestamp::ClearQueue, freq, None)
                .unwrap();
            rf.cancel_scheduled_retunes(Channel::Rx).unwrap();
        })
    });
}

fn bench_set_sample_rate(c: &mut Criterion) {
    let mut device = BladeRf1::from_first().expect("No BladeRF1 found");
    let mut rf = device.rf_link_session().expect("Session failed");
    rf.initialize(true).expect("Initialize failed");
    let mut group = c.benchmark_group("hardware_tuning_sample_rate");
    group.sample_size(10);
    group.measurement_time(std::time::Duration::from_secs(5));

    group.bench_function("set_sample_rate_rx_10msps", |b| {
        b.iter(|| rf.set_sample_rate(Channel::Rx, 10_000_000).unwrap())
    });
}

fn bench_set_bandwidth(c: &mut Criterion) {
    let mut device = BladeRf1::from_first().expect("No BladeRF1 found");
    let mut rf = device.rf_link_session().expect("Session failed");
    rf.initialize(true).expect("Initialize failed");
    let mut group = c.benchmark_group("hardware_tuning_bandwidth");
    group.sample_size(10);
    group.measurement_time(std::time::Duration::from_secs(5));

    group.bench_function("set_bandwidth_rx_8mhz", |b| {
        b.iter(|| rf.set_bandwidth(Channel::Rx, 8_000_000).unwrap())
    });
}

criterion_group!(
    benches,
    bench_set_frequency,
    bench_get_frequency,
    bench_schedule_retune,
    bench_set_sample_rate,
    bench_set_bandwidth,
);
criterion_main!(benches);
