use criterion::{Criterion, criterion_group, criterion_main};
use libbladerf_rs::protocol::nios::{
    NiosPkt8x32Target, NiosPkt32x32Target, nios_decode_read, nios_encode_read, nios_encode_write,
};

fn bench_encode_read(c: &mut Criterion) {
    let mut group = c.benchmark_group("nios_encode_read");

    group.bench_function("8x32", |b| {
        let mut buf = [0u8; 16];
        b.iter(|| {
            nios_encode_read::<u8, u32>(&mut buf, NiosPkt8x32Target::Control as u8, 0).unwrap()
        })
    });
    group.bench_function("32x32", |b| {
        let mut buf = [0u8; 16];
        b.iter(|| {
            nios_encode_read::<u32, u32>(&mut buf, NiosPkt32x32Target::Exp as u8, u32::MAX).unwrap()
        })
    });
}

fn bench_encode_write(c: &mut Criterion) {
    let mut group = c.benchmark_group("nios_encode_write");

    group.bench_function("8x32", |b| {
        let mut buf = [0u8; 16];
        b.iter(|| {
            nios_encode_write::<u8, u32>(&mut buf, NiosPkt8x32Target::Control as u8, 0, 0x57)
                .unwrap()
        })
    });
    group.bench_function("32x32", |b| {
        let mut buf = [0u8; 16];
        b.iter(|| {
            nios_encode_write::<u32, u32>(&mut buf, NiosPkt32x32Target::Exp as u8, u32::MAX, 0xFF)
                .unwrap()
        })
    });
}

fn bench_decode_read(c: &mut Criterion) {
    let mut group = c.benchmark_group("nios_decode_read");
    let mut response = [0u8; 16];
    response[2] = 0x02;
    response[4] = 42;

    group.bench_function("8x32", |b| {
        b.iter(|| nios_decode_read::<u8, u32>(&response))
    });
    group.bench_function("32x32", |b| {
        b.iter(|| nios_decode_read::<u32, u32>(&response))
    });
}

criterion_group!(
    benches,
    bench_encode_read,
    bench_encode_write,
    bench_decode_read,
);
criterion_main!(benches);
