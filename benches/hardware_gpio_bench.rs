use criterion::{Criterion, criterion_group, criterion_main};
use libbladerf_rs::bladerf1::BladeRf1;
use libbladerf_rs::channel::Channel;
use std::cell::Cell;

fn bench_gpio_read(c: &mut Criterion) {
    let mut device = BladeRf1::from_first().expect("No BladeRF1 found");
    let mut rf = device.rf_link_session().expect("Session failed");
    rf.initialize(true).expect("Initialize failed");
    let mut group = c.benchmark_group("hardware_gpio");
    group.sample_size(20);
    group.measurement_time(std::time::Duration::from_secs(5));

    group.bench_function("config_gpio_read", |b| {
        b.iter(|| rf.config_gpio_read().unwrap())
    });
}

fn bench_gpio_write(c: &mut Criterion) {
    let mut device = BladeRf1::from_first().expect("No BladeRF1 found");
    let mut rf = device.rf_link_session().expect("Session failed");
    rf.initialize(true).expect("Initialize failed");
    let value = rf.config_gpio_read().unwrap();
    let mut group = c.benchmark_group("hardware_gpio");
    group.sample_size(20);
    group.measurement_time(std::time::Duration::from_secs(5));

    group.bench_function("config_gpio_write", |b| {
        b.iter(|| rf.config_gpio_write(value).unwrap())
    });
}

fn bench_gpio_modify(c: &mut Criterion) {
    let mut device = BladeRf1::from_first().expect("No BladeRF1 found");
    let mut rf = device.rf_link_session().expect("Session failed");
    rf.initialize(true).expect("Initialize failed");
    let mut group = c.benchmark_group("hardware_gpio");
    group.sample_size(20);
    group.measurement_time(std::time::Duration::from_secs(5));

    group.bench_function("config_gpio_modify", |b| {
        b.iter(|| rf.config_gpio_modify(|gpio| gpio).unwrap())
    });
}

fn bench_enable_module(c: &mut Criterion) {
    let mut device = BladeRf1::from_first().expect("No BladeRF1 found");
    let mut rf = device.rf_link_session().expect("Session failed");
    rf.initialize(true).expect("Initialize failed");
    let mut group = c.benchmark_group("hardware_gpio_module");
    group.sample_size(10);
    group.measurement_time(std::time::Duration::from_secs(5));

    let toggle = Cell::new(false);
    group.bench_function("enable_module_rx_toggle", |b| {
        b.iter(|| {
            let enable = toggle.get();
            toggle.set(!enable);
            rf.enable_module(Channel::Rx, enable).unwrap()
        })
    });
}

criterion_group!(
    benches,
    bench_gpio_read,
    bench_gpio_write,
    bench_gpio_modify,
    bench_enable_module
);
criterion_main!(benches);
