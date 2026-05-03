use criterion::{Criterion, criterion_group, criterion_main};
use libbladerf_rs::bladerf1::BladeRf1;
use std::time::Instant;

fn setup_device() -> BladeRf1 {
    let mut device = BladeRf1::from_first().expect("No BladeRF1 found");
    device.initialize(true).expect("Initialize failed");
    device
}

fn bench_nios_config_read(c: &mut Criterion) {
    let mut device = setup_device();
    let mut group = c.benchmark_group("hardware_nios_read");
    group.sample_size(20);
    group.measurement_time(std::time::Duration::from_secs(5));

    group.bench_function("config_gpio_read", |b| {
        b.iter_custom(|iters| {
            let start = Instant::now();
            for _ in 0..iters {
                let _ = device.config_gpio_read().unwrap();
            }
            start.elapsed()
        })
    });
    group.finish();
}

fn bench_nios_config_write(c: &mut Criterion) {
    let mut device = setup_device();
    let value = device.config_gpio_read().unwrap();
    let mut group = c.benchmark_group("hardware_nios_write");
    group.sample_size(20);
    group.measurement_time(std::time::Duration::from_secs(5));

    group.bench_function("config_gpio_write", |b| {
        b.iter_custom(|iters| {
            let start = Instant::now();
            for _ in 0..iters {
                device.config_gpio_write(value).unwrap();
            }
            start.elapsed()
        })
    });
    group.finish();
}

fn bench_nios_fpga_version(c: &mut Criterion) {
    let mut device = setup_device();
    let mut group = c.benchmark_group("hardware_nios_read");
    group.sample_size(20);
    group.measurement_time(std::time::Duration::from_secs(5));

    group.bench_function("fpga_version", |b| {
        b.iter_custom(|iters| {
            let start = Instant::now();
            for _ in 0..iters {
                let _ = device.fpga_version().unwrap();
            }
            start.elapsed()
        })
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_nios_config_read,
    bench_nios_config_write,
    bench_nios_fpga_version,
);
criterion_main!(benches);
