# dc_cal_table

Generates DC calibration lookup tables for the BladeRF1.

## Commands

All commands run from the **repository root**:

| Action | Command |
|--------|---------|
| Build | `cargo build -p dc_cal_table` |
| Run | `cargo run -p dc_cal_table -- <rx\|tx> <f_min> <f_max> <f_inc>` |

## Example

```bash
cargo run -p dc_cal_table -- rx 300000000 3800000000 10000000
```

## API used

- `BladeRf1::from_first`, `calibrate_and_save_table`
- `Channel::Rx`, `Channel::Tx`

## Hardware

Requires a physical BladeRF1 connected via USB (High or SuperSpeed).

The calibration process takes several minutes and requires stable thermal conditions. The resulting table file (`{serial}_dc_rx.tbl` or `{serial}_dc_tx.tbl`) is automatically loaded on the next device open.
