use criterion::{Criterion, criterion_group, criterion_main};
use libbladerf_rs::bladerf1::{BladeRf1, RxStream, SampleFormat};
use std::time::{Duration, Instant};

fn setup_device() -> BladeRf1 {
    let mut device = BladeRf1::from_first().expect("No BladeRF1 found");
    device.initialize(true).expect("Initialize failed");
    device
}

fn bench_rx_read_latency(c: &mut Criterion) {
    let mut device = setup_device();
    let mut group = c.benchmark_group("hardware_stream_latency");
    group.sample_size(50);
    group.measurement_time(Duration::from_secs(10));

    let mut streamer = RxStream::builder(&mut device)
        .buffer_size(65_536)
        .buffer_count(8)
        .format(SampleFormat::Sc16Q11)
        .build()
        .unwrap();

    group.bench_function("rx_read_latency", |b| {
        b.iter_custom(|iters| {
            let start = Instant::now();
            for _ in 0..iters {
                let buf = streamer.read(Some(Duration::from_secs(2))).unwrap();
                streamer.recycle(buf);
            }
            start.elapsed()
        })
    });
    group.finish();
}

fn bench_stream_build_teardown(c: &mut Criterion) {
    let mut group = c.benchmark_group("hardware_stream_latency");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(10));

    group.bench_function("rx_stream_build_teardown", |b| {
        b.iter_custom(|iters| {
            let start = Instant::now();
            for _ in 0..iters {
                let mut device = setup_device();
                let mut streamer = RxStream::builder(&mut device)
                    .buffer_size(65_536)
                    .buffer_count(8)
                    .format(SampleFormat::Sc16Q11)
                    .build()
                    .unwrap();
                let buf = streamer.read(Some(Duration::from_secs(2))).unwrap();
                streamer.recycle(buf);
                drop(streamer);
            }
            start.elapsed()
        })
    });
    group.finish();
}

criterion_group!(benches, bench_rx_read_latency, bench_stream_build_teardown,);
criterion_main!(benches);
