use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use libbladerf_rs::bladerf1::SampleFormat;

fn bench_pack(c: &mut Criterion) {
    let mut group = c.benchmark_group("pack_sc16q11_packed");
    let sizes: &[usize] = &[256, 4_096, 65_536];

    for &num_samples in sizes {
        group.bench_with_input(
            BenchmarkId::new("pack", num_samples),
            &num_samples,
            |b, &n| {
                let src = vec![0u8; 4 * n];
                let mut dst = vec![0u8; 3 * n];
                b.iter(|| SampleFormat::pack_sc16q11_packed(&src, &mut dst, n).unwrap())
            },
        );
    }
    group.finish();
}

fn bench_unpack(c: &mut Criterion) {
    let mut group = c.benchmark_group("unpack_sc16q11_packed");
    let sizes: &[usize] = &[256, 4_096, 65_536];

    for &num_samples in sizes {
        group.bench_with_input(
            BenchmarkId::new("unpack", num_samples),
            &num_samples,
            |b, &n| {
                let src = vec![0u8; 3 * n];
                let mut dst = vec![0u8; 4 * n];
                b.iter(|| SampleFormat::unpack_sc16q11_packed(&src, &mut dst, n).unwrap())
            },
        );
    }
    group.finish();
}

fn bench_pack_nonzero(c: &mut Criterion) {
    let mut group = c.benchmark_group("pack_sc16q11_packed_nonzero");
    let n: usize = 65_536;
    let mut src = vec![0u8; 4 * n];
    for i in 0..n {
        let off = i * 4;
        let val = (i as i16).to_le_bytes();
        src[off] = val[0];
        src[off + 1] = val[1];
        src[off + 2] = val[0];
        src[off + 3] = val[1];
    }
    let mut dst = vec![0u8; 3 * n];

    group.bench_function("65536_nonzero", |b| {
        b.iter(|| SampleFormat::pack_sc16q11_packed(&src, &mut dst, n).unwrap())
    });
    group.finish();
}

fn bench_unpack_nonzero(c: &mut Criterion) {
    let mut group = c.benchmark_group("unpack_sc16q11_packed_nonzero");
    let n: usize = 65_536;
    let mut src = vec![0u8; 3 * n];
    for i in 0..(n / 2) {
        let off = i * 6;
        let w0 = (0x0ABCu16 | ((i as u16) & 0x0FFF)).to_le_bytes();
        let w1 = 0x1234u16.to_le_bytes();
        let w2 = 0x5678u16.to_le_bytes();
        src[off] = w0[0];
        src[off + 1] = w0[1];
        src[off + 2] = w1[0];
        src[off + 3] = w1[1];
        src[off + 4] = w2[0];
        src[off + 5] = w2[1];
    }
    let mut dst = vec![0u8; 4 * n];

    group.bench_function("65536_nonzero", |b| {
        b.iter(|| SampleFormat::unpack_sc16q11_packed(&src, &mut dst, n).unwrap())
    });
    group.finish();
}

fn bench_sample_size(c: &mut Criterion) {
    let mut group = c.benchmark_group("sample_size");
    group.bench_function("Sc16Q11", |b| {
        b.iter(|| SampleFormat::Sc16Q11.sample_size())
    });
    group.bench_function("Sc16Q11Packed", |b| {
        b.iter(|| SampleFormat::Sc16Q11Packed.sample_size())
    });
    group.bench_function("Sc8Q7", |b| b.iter(|| SampleFormat::Sc8Q7.sample_size()));
    group.finish();
}

criterion_group!(
    benches,
    bench_pack,
    bench_unpack,
    bench_pack_nonzero,
    bench_unpack_nonzero,
    bench_sample_size,
);
criterion_main!(benches);
