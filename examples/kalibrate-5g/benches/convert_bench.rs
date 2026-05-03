use criterion::{Criterion, black_box, criterion_group, criterion_main};
use rustfft::num_complex::Complex32;

fn convert_sc8q7_scalar(payload: &[u8], out: &mut [Complex32]) {
    for i in 0..out.len() {
        let off = i * 2;
        out[i] = Complex32::new(
            payload[off] as i8 as f32 / 128.0,
            payload[off + 1] as i8 as f32 / 128.0,
        );
    }
}

fn bench_convert_4k(c: &mut Criterion) {
    let payload = vec![0u8; 8_192];
    let mut out = vec![Complex32::new(0.0, 0.0); 4_096];

    c.bench_function("convert_sc8q7_4k_scalar", |b| {
        b.iter(|| convert_sc8q7_scalar(black_box(&payload), black_box(&mut out)))
    });
}

fn bench_convert_16k(c: &mut Criterion) {
    let payload = vec![0u8; 32_768];
    let mut out = vec![Complex32::new(0.0, 0.0); 16_384];

    c.bench_function("convert_sc8q7_16k_scalar", |b| {
        b.iter(|| convert_sc8q7_scalar(black_box(&payload), black_box(&mut out)))
    });
}

criterion_group!(benches, bench_convert_4k, bench_convert_16k);
criterion_main!(benches);
