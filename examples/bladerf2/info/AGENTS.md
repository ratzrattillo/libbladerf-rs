# info

Demonstrates BladeRF2 device identification: open, firmware and FPGA versions, FPGA size and source, flash size, and calibration data.

## Commands

All commands run from the **repository root**:

| Action | Command |
|--------|---------|
| Build | `cargo build -p info-bladerf2` |
| Run | `cargo run -p info-bladerf2` |

## API used

- `BladeRf2::from_first`, `serial`
- `fx3_firmware_version`, `fpga_version`
- `flash_session`: `size_bytes`, `total_pages` `total_sectors` `read_flash_fpga_size`, `read_flash_dac_trim`
- `fpga_bytes`, `FpgaSize::variant_label`
- `rf_link_session`: `is_fpga_configured`, `get_fpga_source`

## Hardware

Requires a physical BladeRF2 connected via USB (High or SuperSpeed).
