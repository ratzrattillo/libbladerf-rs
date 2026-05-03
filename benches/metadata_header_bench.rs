use criterion::{Criterion, criterion_group, criterion_main};
use libbladerf_rs::bladerf1::{METADATA_HEADER_SIZE, MetadataHeader};

fn bench_from_bytes_valid(c: &mut Criterion) {
    let mut buf = [0u8; METADATA_HEADER_SIZE];
    buf[2] = 0x00;
    buf[14] = 0x01;
    buf[15] = 0x00;
    for (i, byte) in buf.iter_mut().enumerate().take(12).skip(4) {
        *byte = (i * 17) as u8;
    }

    c.bench_function("metadata_header_from_bytes_valid", |b| {
        b.iter(|| MetadataHeader::from_bytes(&buf))
    });
}

fn bench_from_bytes_zero(c: &mut Criterion) {
    let buf = [0u8; METADATA_HEADER_SIZE];

    c.bench_function("metadata_header_from_bytes_zero", |b| {
        b.iter(|| MetadataHeader::from_bytes(&buf))
    });
}

fn bench_from_bytes_short(c: &mut Criterion) {
    let buf = [0u8; 8];

    c.bench_function("metadata_header_from_bytes_short", |b| {
        b.iter(|| MetadataHeader::from_bytes(&buf))
    });
}

fn bench_from_bytes_full_parse(c: &mut Criterion) {
    let mut buf = [0u8; METADATA_HEADER_SIZE];
    buf[0] = 0x42;
    buf[1] = 0x00;
    buf[2] = 0x00;
    buf[3] = 0x34;
    for byte in buf.iter_mut().take(12).skip(4) {
        *byte = 0xFF;
    }
    buf[12] = 0x01;
    buf[13] = 0x00;
    buf[14] = 0x00;
    buf[15] = 0x00;

    c.bench_function("metadata_header_full_parse", |b| {
        b.iter(|| {
            let h = MetadataHeader::from_bytes(&buf).unwrap();
            let _ = h.timestamp();
            let _ = h.meta_flags();
            let _ = h.is_valid_meta_format();
            let _ = h.stream_flags();
            let _ = h.meta_version();
        })
    });
}

criterion_group!(
    benches,
    bench_from_bytes_valid,
    bench_from_bytes_zero,
    bench_from_bytes_short,
    bench_from_bytes_full_parse,
);
criterion_main!(benches);
