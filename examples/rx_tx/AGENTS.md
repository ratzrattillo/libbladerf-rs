# rx_tx

Demonstrates the BladeRF1 streaming API (RX and TX).

## Commands

All commands run from the **repository root**:

| Action | Command |
|--------|---------|
| Build | `cargo build -p rx_tx` |
| Run | `cargo run -p rx_tx` |

## API used

- `BladeRf1::from_first`, `initialize`
- `RxStream::builder`, `read`, `recycle`, `close`
- `TxStream::builder`, `get_buffer`, `submit`, `wait_completion`
- `expansion_get_attached`, `expansion_attach` (when frequency below LMS6002D minimum)
- `perform_format_config`, `enable_module` (in `_do_tx`)

## Feature flags

Requires the `xb200` feature in `Cargo.toml` for `ExpansionBoard::Xb200`:

```toml
libbladerf-rs = { path = "../..", features = ["xb200"] }
```

## Notes

- `_do_tx` is defined but not called from `main()`. It demonstrates manual format config + module enable + streaming.
- RX reads one buffer, prints first 32 bytes, then closes the stream.

## Hardware

Requires a physical BladeRF1 connected via USB (High or SuperSpeed). An XB-200 expansion board is optional (auto-detected if frequency range requires it).
