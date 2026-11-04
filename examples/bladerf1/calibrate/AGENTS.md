# calibrate

Demonstrates DC calibration on the BladeRF1.

## Commands

All commands run from the **repository root**:

| Action | Command |
|--------|---------|
| Build | `cargo build -p calibrate` |
| Run | `cargo run -p calibrate` |

## API used

- `BladeRf1::from_first`, `initialize`
- `get_dc_cals`, `calibrate_dc`, `cal_tx_lpf`
- `DcCalModule::RxVga2`, `DcCalModule::RxLpf`, `DcCalModule::TxLpf`, `DcCalModule::LpfTuning`

## Hardware

Requires a physical BladeRF1 connected via USB (High or SuperSpeed).
