use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use libbladerf_rs::protocol::nios::{
    NiosPkt8x16Target, NiosPkt8x32Target, NiosPkt32x32Target, nios_decode_read, nios_decode_write,
    nios_encode_read, nios_encode_write,
};

fn bench_encode_read(c: &mut Criterion) {
    let mut group = c.benchmark_group("nios_encode_read");

    group.bench_function("8x8", |b| {
        let mut buf = [0u8; 16];
        b.iter(|| nios_encode_read::<u8, u8>(&mut buf, 0x01, 0).unwrap())
    });
    group.bench_function("8x16", |b| {
        let mut buf = [0u8; 16];
        b.iter(|| {
            nios_encode_read::<u8, u16>(&mut buf, NiosPkt8x16Target::IqCorr as u8, 0).unwrap()
        })
    });
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
    group.finish();
}

fn bench_encode_write(c: &mut Criterion) {
    let mut group = c.benchmark_group("nios_encode_write");

    group.bench_function("8x8", |b| {
        let mut buf = [0u8; 16];
        b.iter(|| nios_encode_write::<u8, u8>(&mut buf, 0x01, 0, 42).unwrap())
    });
    group.bench_function("8x16", |b| {
        let mut buf = [0u8; 16];
        b.iter(|| {
            nios_encode_write::<u8, u16>(&mut buf, NiosPkt8x16Target::IqCorr as u8, 0, 100).unwrap()
        })
    });
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
    group.finish();
}

fn bench_decode_read(c: &mut Criterion) {
    let mut group = c.benchmark_group("nios_decode_read");
    let mut response = [0u8; 16];
    response[2] = 0x02;
    response[4] = 42;

    group.bench_function("8x8", |b| b.iter(|| nios_decode_read::<u8, u8>(&response)));
    group.bench_function("8x16", |b| {
        b.iter(|| nios_decode_read::<u8, u16>(&response))
    });
    group.bench_function("8x32", |b| {
        b.iter(|| nios_decode_read::<u8, u32>(&response))
    });
    group.bench_function("32x32", |b| {
        b.iter(|| nios_decode_read::<u32, u32>(&response))
    });
    group.finish();
}

fn bench_decode_write(c: &mut Criterion) {
    let mut group = c.benchmark_group("nios_decode_write");
    let response = [0u8; 16];

    group.bench_function("decode_write", |b| {
        b.iter(|| nios_decode_write::<u8, u32>(&response))
    });
    group.finish();
}

fn bench_roundtrip(c: &mut Criterion) {
    let mut group = c.benchmark_group("nios_roundtrip");
    let mut response = [0u8; 16];
    response[2] = 0x02;
    response[5] = 0x57;

    group.bench_function("8x32_encode_then_decode", |b| {
        let mut buf = [0u8; 16];
        b.iter(|| {
            nios_encode_read::<u8, u32>(&mut buf, NiosPkt8x32Target::Control as u8, 0).unwrap();
            let _ = nios_decode_read::<u8, u32>(&response);
        })
    });
    group.finish();
}

fn bench_packet_size_variants(c: &mut Criterion) {
    let mut group = c.benchmark_group("nios_packet_sizes");

    for (label, size) in [("16", 16usize), ("32", 32), ("64", 64)] {
        group.bench_with_input(BenchmarkId::new("8x32_encode", label), &size, |b, _| {
            let mut buf = vec![0u8; 64];
            b.iter(|| {
                nios_encode_read::<u8, u32>(&mut buf, NiosPkt8x32Target::Control as u8, 0).unwrap()
            })
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_encode_read,
    bench_encode_write,
    bench_decode_read,
    bench_decode_write,
    bench_roundtrip,
    bench_packet_size_variants,
);
criterion_main!(benches);
