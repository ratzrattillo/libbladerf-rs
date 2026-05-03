use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use libbladerf_rs::bladerf1::{BladeRf1, TuningMode};
use libbladerf_rs::channel::Channel;
use std::time::Instant;

fn setup_device() -> BladeRf1 {
    let mut device = BladeRf1::from_first().expect("No BladeRF1 found");
    device.initialize(true).expect("Initialize failed");
    device
}

fn bench_set_frequency(c: &mut Criterion) {
    let mut device = setup_device();
    let mut group = c.benchmark_group("hardware_tuning");
    group.sample_size(20);
    group.measurement_time(std::time::Duration::from_secs(5));

    let freqs: &[u64] = &[300_000_000, 1_000_000_000, 3_800_000_000];

    for &freq in freqs {
        group.bench_with_input(
            BenchmarkId::new("set_frequency_rx", freq),
            &freq,
            |b, &freq| {
                b.iter_custom(|iters| {
                    let start = Instant::now();
                    for _ in 0..iters {
                        device
                            .set_frequency(Channel::Rx, freq, TuningMode::Fpga)
                            .unwrap();
                    }
                    start.elapsed()
                })
            },
        );
    }
    group.finish();
}

fn bench_get_frequency(c: &mut Criterion) {
    let mut device = setup_device();
    let mut group = c.benchmark_group("hardware_tuning");
    group.sample_size(20);
    group.measurement_time(std::time::Duration::from_secs(5));

    group.bench_function("get_frequency_rx", |b| {
        b.iter_custom(|iters| {
            let start = Instant::now();
            for _ in 0..iters {
                let _ = device.get_frequency(Channel::Rx).unwrap();
            }
            start.elapsed()
        })
    });
    group.finish();
}

fn bench_set_sample_rate(c: &mut Criterion) {
    let mut device = setup_device();
    let mut group = c.benchmark_group("hardware_tuning");
    group.sample_size(10);
    group.measurement_time(std::time::Duration::from_secs(5));

    group.bench_function("set_sample_rate_rx_10msps", |b| {
        b.iter_custom(|iters| {
            let start = Instant::now();
            for _ in 0..iters {
                let _ = device.set_sample_rate(Channel::Rx, 10_000_000).unwrap();
            }
            start.elapsed()
        })
    });
    group.finish();
}

fn bench_set_bandwidth(c: &mut Criterion) {
    let mut device = setup_device();
    let mut group = c.benchmark_group("hardware_tuning");
    group.sample_size(10);
    group.measurement_time(std::time::Duration::from_secs(5));

    group.bench_function("set_bandwidth_rx_8mhz", |b| {
        b.iter_custom(|iters| {
            let start = Instant::now();
            for _ in 0..iters {
                device.set_bandwidth(Channel::Rx, 8_000_000).unwrap();
            }
            start.elapsed()
        })
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_set_frequency,
    bench_get_frequency,
    bench_set_sample_rate,
    bench_set_bandwidth,
);
criterion_main!(benches);
