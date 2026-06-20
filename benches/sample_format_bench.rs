use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use libbladerf_rs::bladerf1::SampleFormat;

fn bench_pack(c: &mut Criterion) {
    let mut group = c.benchmark_group("pack_sc16q11_packed");
    let sizes: &[usize] = &[256, 4_096, 65_536];

    for &num_samples in sizes {
        group.throughput(Throughput::Elements(num_samples as u64));
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
}

fn bench_unpack(c: &mut Criterion) {
    let mut group = c.benchmark_group("unpack_sc16q11_packed");
    let sizes: &[usize] = &[256, 4_096, 65_536];

    for &num_samples in sizes {
        group.throughput(Throughput::Elements(num_samples as u64));
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
}

criterion_group!(benches, bench_pack, bench_unpack,);
criterion_main!(benches);
