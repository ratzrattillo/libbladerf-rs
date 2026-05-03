use criterion::{Criterion, black_box, criterion_group, criterion_main};
use rustfft::num_complex::Complex32;

fn deterministic_noise(n: usize, seed: u32) -> Vec<Complex32> {
    let mut state = seed;
    let mut rng = || {
        state = state.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        ((state >> 16) as f32 / 32_768.0 - 1.0)
    };
    (0..n).map(|_| Complex32::new(rng(), rng())).collect()
}

fn bench_ssb_detect_all_16m(c: &mut Criterion) {
    let sample_rate = 40_000_000.0;
    let samples = deterministic_noise(16_000_000, 42);

    c.bench_function("ssb_detect_all_16M_noise", |b| {
        b.iter(|| kalibrate_5g::detect::ssb_detect_all(black_box(&samples), black_box(sample_rate)))
    });
}

fn bench_ssb_detect_all_8m(c: &mut Criterion) {
    let sample_rate = 20_000_000.0;
    let samples = deterministic_noise(8_000_000, 77);

    c.bench_function("ssb_detect_all_8M_noise", |b| {
        b.iter(|| kalibrate_5g::detect::ssb_detect_all(black_box(&samples), black_box(sample_rate)))
    });
}

criterion_group!(benches, bench_ssb_detect_all_16m, bench_ssb_detect_all_8m);
criterion_main!(benches);
