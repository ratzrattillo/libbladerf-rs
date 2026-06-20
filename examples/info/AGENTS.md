# info

Demonstrates basic BladeRF1 device access: open, initialize, read versions, set and read frequency/sample rate/bandwidth/gain.

## Commands

All commands run from the **repository root**:

| Action | Command |
|--------|---------|
| Build | `cargo build -p info` |
| Run | `cargo run -p info` |

## API used

- `BladeRf1::from_first`, `initialize`
- `fx3_firmware_version`, `fpga_version`
- `expansion_get_attached`
- `get_frequency_range`, `set_frequency`, `get_frequency`
- `get_sample_rate_range`, `set_sample_rate`, `get_sample_rate`
- `get_bandwidth_range`, `set_bandwidth`, `get_bandwidth`
- `get_gain_stages`, `get_gain_range`, `set_gain`, `get_gain`

## Hardware

Requires a physical BladeRF1 connected via USB (High or SuperSpeed).
