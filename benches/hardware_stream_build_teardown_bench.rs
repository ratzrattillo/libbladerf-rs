use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use libbladerf_rs::MaybeFuture;
use libbladerf_rs::bladerf1::{BladeRf1, RxStream, SampleFormat};
use std::time::Duration;

fn bench_stream_build_teardown(c: &mut Criterion) {
    let mut group = c.benchmark_group("hardware_stream_build_teardown");
    group.sample_size(10);
    group.measurement_time(std::time::Duration::from_secs(10));
    group.throughput(Throughput::Elements(1));

    group.bench_function("rx_stream_build_teardown", |b| {
        b.iter_batched(
            || BladeRf1::from_first().wait().unwrap(),
            |mut device| {
                let mut rf = device.rf_link_session().wait().unwrap();
                rf.initialize(true).wait().unwrap();
                let mut streamer = RxStream::builder(&mut rf)
                    .buffer_size(65_536)
                    .buffer_count(8)
                    .format(SampleFormat::Sc16Q11)
                    .build()
                    .wait()
                    .unwrap();
                streamer.start(&mut rf).wait().unwrap();
                let buf = streamer.read(Some(Duration::from_secs(2))).wait().unwrap();
                streamer.recycle(buf);
                streamer.close(&mut rf).wait().unwrap();
            },
            criterion::BatchSize::PerIteration,
        )
    });
}

criterion_group!(benches, bench_stream_build_teardown,);
criterion_main!(benches);
