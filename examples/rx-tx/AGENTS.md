# rx-tx

Demonstrates the BladeRF1 streaming API (RX and TX).

## Commands

All commands run from the **repository root**:

| Action | Command |
|--------|---------|
| Build | `cargo build -p rx-tx` |
| Run | `cargo run -p rx-tx` |

## API used

- `BladeRf1::from_first`, `initialize`
- `RxStream::builder`, `start`, `read`, `recycle`, `close`
- `TxStream::builder`, `start`, `get_buffer`, `submit`, `wait_completion`, `close`
- `expansion_get_attached`, `expansion_attach` (when frequency below LMS6002D minimum)

## Feature flags

Requires the `xb200` feature in `Cargo.toml` for `ExpansionBoard::Xb200`:

```toml
libbladerf-rs = { path = "../..", features = ["xb200"] }
```

## Notes

- `_do_tx` is defined but not called from `main()`. It uses the stream lifecycle to configure format and module state.
- RX tunes to 915 MHz at 2 MS/s, reads one buffer, prints its first 32 bytes, and closes the stream and device.
- Both helpers attempt stream cleanup before propagating data-path errors.

## Hardware

Requires a physical BladeRF1 connected via USB (High or SuperSpeed). An XB-200 expansion board is optional (auto-detected if frequency range requires it).
