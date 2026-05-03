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

fn bench_pss_correlate(c: &mut Criterion) {
    let sample_rate = 4_000_000.0;
    let search_center = -1_000_000.0;
    let max_shift = 300_000.0;
    let samples = deterministic_noise(4_000_000, 42);

    c.bench_function("pss_correlate_4M_noise", |b| {
        b.iter(|| {
            kalibrate_5g::pss::pss_correlate(
                black_box(&samples),
                black_box(sample_rate),
                black_box(search_center),
                black_box(max_shift),
            )
        })
    });
}

fn bench_pss_correlate_with_signal(c: &mut Criterion) {
    let sample_rate = 4_000_000.0;
    let ssb_offset = -1_000_000.0;
    let freq_error = 5_432.0;
    let n_id_2 = 0usize;

    let pss = kalibrate_5g::pss::generate_pss(n_id_2);
    let mut state: u32 = 42;
    let mut rng = || {
        state = state.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        ((state >> 16) as f32 / 32_768.0 - 1.0) * 0.3
    };

    let mut samples = Vec::with_capacity(4_000_000);
    for i in 0..4_000_000usize {
        let t = i as f64 / sample_rate;
        let mut val = Complex32::new(0.0, 0.0);
        for sc in 0..kalibrate_5g::pss::PSS_LEN {
            let sc_offset = sc as f64 - (kalibrate_5g::pss::PSS_LEN as f64 - 1.0) / 2.0;
            let freq = ssb_offset + sc_offset * 15_000.0;
            let phase = 2.0 * std::f64::consts::PI * (freq + freq_error) * t;
            val += Complex32::new(pss[sc] * phase.cos() as f32, pss[sc] * phase.sin() as f32);
        }
        val = val / kalibrate_5g::pss::PSS_LEN as f32 * 0.7;
        samples.push(val + Complex32::new(rng(), rng()));
    }

    c.bench_function("pss_correlate_4M_signal", |b| {
        b.iter(|| {
            kalibrate_5g::pss::pss_correlate(
                black_box(&samples),
                black_box(sample_rate),
                black_box(ssb_offset + freq_error),
                black_box(300_000.0),
            )
        })
    });
}

criterion_group!(
    benches,
    bench_pss_correlate,
    bench_pss_correlate_with_signal
);
criterion_main!(benches);
