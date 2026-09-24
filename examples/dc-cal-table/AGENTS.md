# dc-cal-table

Generates DC calibration lookup tables for the BladeRF1.

## Commands

All commands run from the **repository root**:

| Action | Command |
|--------|---------|
| Build | `cargo build -p dc-cal-table` |
| Run | `cargo run -p dc-cal-table -- <rx\|tx> <f_min> <f_max> <f_inc>` |

## Example

```bash
cargo run -p dc-cal-table -- rx 300000000 3800000000 10000000
```

## API used

- `BladeRf1::from_first`, `calibrate_and_save_table`
- `Channel::Rx`, `Channel::Tx`

## Hardware

Requires a physical BladeRF1 connected via USB (High or SuperSpeed).

The calibration process takes several minutes and requires stable thermal conditions. The resulting validated JSON table (`{serial}_dc_rx.json` or `{serial}_dc_tx.json`) is automatically loaded on the next device open when present in its calibration directory.

Streams are closed before restoring settings that change USB mode. TX band changes use `set_lms_loopback` for the analog path while streaming. Cleanup runs before propagating measurement errors; zero frequency steps and unrepresentable table frequencies are rejected before opening hardware.
