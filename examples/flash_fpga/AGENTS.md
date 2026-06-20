# flash_fpga

Downloads the latest FPGA bitstream from Nuand, flashes it to the bladeRF1 SPI flash, and optionally loads it into the FPGA. Auto-detects the FPGA variant (x40/x115) from calibration data.

## Commands

All commands run from the **repository root**:

| Action | Command |
|--------|---------|
| Build | `cargo build -p flash_fpga` |
| Run | `cargo run -p flash_fpga` |

## API used

- `BladeRf1::from_first`
- `read_flash_fpga_size`
- `flash_fpga`
- `load_fpga`
- `initialize`

## Hardware

Requires a physical BladeRF1 connected via USB (High or SuperSpeed).

## Network

Requires internet access to `https://nuand.com/versions.json` and `https://www.nuand.com/fpga/`.
