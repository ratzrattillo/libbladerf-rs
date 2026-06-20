# flash_firmware

Downloads the latest FX3 firmware from Nuand, flashes it to the bladeRF1 SPI flash, resets the device, and verifies the new firmware version.

## Commands

All commands run from the **repository root**:

| Action | Command |
|--------|---------|
| Build | `cargo build -p flash_firmware` |
| Run | `cargo run -p flash_firmware` |

## API used

- `BladeRf1::from_first`
- `fx3_firmware_version`
- `flash_firmware`
- `device_reset`

## Hardware

Requires a physical BladeRF1 connected via USB (High or SuperSpeed).

## Network

Requires internet access to `https://nuand.com/versions.json` and `https://www.nuand.com/fx3/`.
