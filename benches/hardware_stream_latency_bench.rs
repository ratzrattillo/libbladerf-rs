use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use libbladerf_rs::Channel;
use libbladerf_rs::MaybeFuture;
use libbladerf_rs::bladerf1::{BladeRf1, RxStream, SampleFormat, TuningMode, TxStream};
use std::time::Duration;

fn bench_rx_read_latency(c: &mut Criterion) {
    let mut device = BladeRf1::from_first().wait().expect("No BladeRF1 found");
    let mut rf = device.rf_link_session().wait().expect("Session failed");
    rf.initialize(true).wait().expect("Initialize failed");
    rf.set_frequency(Channel::Rx, 915_000_000, TuningMode::Fpga)
        .wait()
        .unwrap();
    rf.set_sample_rate(Channel::Rx, 10_000_000).wait().unwrap();
    let mut group = c.benchmark_group("hardware_stream_latency");
    group.sample_size(50);
    group.measurement_time(Duration::from_secs(10));
    group.throughput(Throughput::Bytes(65_536));

    let mut streamer = RxStream::builder(&mut rf)
        .buffer_size(65_536)
        .buffer_count(8)
        .format(SampleFormat::Sc16Q11)
        .build()
        .wait()
        .unwrap();
    streamer.start(&mut rf).wait().unwrap();

    group.bench_function("rx_read_latency", |b| {
        b.iter(|| {
            let buf = streamer.read(Some(Duration::from_secs(2))).wait().unwrap();
            streamer.recycle(buf);
        })
    });
    streamer.close(&mut rf).wait().unwrap();
    device.close().wait().unwrap();
}

fn bench_tx_write_latency(c: &mut Criterion) {
    let mut device = BladeRf1::from_first().wait().expect("No BladeRF1 found");
    let mut rf = device.rf_link_session().wait().expect("Session failed");
    rf.initialize(true).wait().expect("Initialize failed");
    rf.set_frequency(Channel::Tx, 915_000_000, TuningMode::Fpga)
        .wait()
        .unwrap();
    rf.set_sample_rate(Channel::Tx, 10_000_000).wait().unwrap();
    let mut group = c.benchmark_group("hardware_stream_latency");
    group.sample_size(50);
    group.measurement_time(Duration::from_secs(10));
    group.throughput(Throughput::Bytes(65_536));

    let mut streamer = TxStream::builder(&mut rf)
        .buffer_size(65_536)
        .buffer_count(8)
        .format(SampleFormat::Sc16Q11)
        .build()
        .wait()
        .unwrap();
    streamer.start(&mut rf).wait().unwrap();

    group.bench_function("tx_write_latency", |b| {
        b.iter(|| {
            let mut buf = streamer
                .get_buffer(Some(Duration::from_secs(2)))
                .wait()
                .unwrap();
            buf.extend_fill(65_536, 0);
            streamer.submit(buf, 65_536).unwrap();
            streamer
                .wait_completion(Some(Duration::from_secs(2)))
                .wait()
                .unwrap();
        })
    });
    streamer.close(&mut rf).wait().unwrap();
    device.close().wait().unwrap();
}

criterion_group!(benches, bench_rx_read_latency, bench_tx_write_latency,);
criterion_main!(benches);
