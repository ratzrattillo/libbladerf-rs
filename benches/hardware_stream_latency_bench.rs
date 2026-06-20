use criterion::{Criterion, criterion_group, criterion_main};
use libbladerf_rs::bladerf1::{BladeRf1, RxStream, SampleFormat, TxStream};
use std::time::Duration;

fn bench_rx_read_latency(c: &mut Criterion) {
    let mut device = BladeRf1::from_first().expect("No BladeRF1 found");
    let mut rf = device.rf_link_session().expect("Session failed");
    rf.initialize(true).expect("Initialize failed");
    let mut group = c.benchmark_group("hardware_stream_latency");
    group.sample_size(50);
    group.measurement_time(Duration::from_secs(10));

    let mut streamer = RxStream::builder(&mut rf)
        .buffer_size(65_536)
        .buffer_count(8)
        .format(SampleFormat::Sc16Q11)
        .build()
        .unwrap();
    streamer.start(&mut rf).unwrap();

    group.bench_function("rx_read_latency", |b| {
        b.iter(|| {
            let buf = streamer.read(Some(Duration::from_secs(2))).unwrap();
            streamer.recycle(buf);
        })
    });
}

fn bench_tx_write_latency(c: &mut Criterion) {
    let mut device = BladeRf1::from_first().expect("No BladeRF1 found");
    let mut rf = device.rf_link_session().expect("Session failed");
    rf.initialize(true).expect("Initialize failed");
    let mut group = c.benchmark_group("hardware_stream_latency");
    group.sample_size(50);
    group.measurement_time(Duration::from_secs(10));

    let mut streamer = TxStream::builder(&mut rf)
        .buffer_size(65_536)
        .buffer_count(8)
        .format(SampleFormat::Sc16Q11)
        .build()
        .unwrap();
    streamer.start(&mut rf).unwrap();

    group.bench_function("tx_write_latency", |b| {
        b.iter(|| {
            let buf = streamer.get_buffer(Some(Duration::from_secs(2))).unwrap();
            streamer.submit(buf, 0).unwrap();
        })
    });
}

criterion_group!(benches, bench_rx_read_latency, bench_tx_write_latency,);
criterion_main!(benches);
