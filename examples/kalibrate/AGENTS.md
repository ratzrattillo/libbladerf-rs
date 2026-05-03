# kalibrate

Rust rewrite of kalibrate-bladeRF — VCTCXO oscillator calibration via GSM FCCH signals.

## Commands

All commands run from the **repository root**:

| Action | Command |
|--------|---------|
| Build | `cargo build -p kalibrate` |
| Run | `cargo run -p kalibrate -- [OPTIONS]` |
| Test | `cargo test -p kalibrate` |
| Type check | `cargo check -p kalibrate` |

## Architecture

- `main.rs` — CLI entrypoint, `factory_dac_trim()` HTTP lookup, `calibrate()` binary search, `auto_calibrate()`
- `source.rs` — `Source` wrapper around `BladeRf1`; streaming, DAC trim, `streaming_power_scan()`, `streaming_fcch_scan()`, `ScanParams`/`derive_scan_params()`
- `scan.rs` — Band scan: `streaming_power_scan()` for Phase 1 power sweep → threshold → `streaming_fcch_scan()` Phase 3 → per-bucket FCCH detection
- `offset.rs` — Offset measurement via FCCH detection (quick and precise modes)
- `fcch.rs` — LMS adaptive filter + FFT frequency estimation (Varma et al.)
- `arfcn.rs` — GSM band/ARFCN definitions and frequency conversion

## Implementation Status

All CLI modes wired and functional. All phases complete.

| Phase | Status |
|-------|--------|
| Phase 0 | DONE — Sc8Q7Meta RX enabled |
| Phase A | DONE — CLI wiring, `calibrate()`, `auto_calibrate()` |
| Phase B | DONE — `streaming_power_scan()` for power sweep, USB-derived parameters |
| Phase C | DONE — SC8_Q7 format (2 bytes/sample) |
| Phase D | DONE — `streaming_fcch_scan()` batch FCCH per-bucket detection |

## Reference Implementation

The original C++ kalibrate-bladeRF source lives at `resources/kalibrate-bladeRF/src/`. Use it to verify algorithm behavior or implement missing CLI wiring:

- `kal.cc` — Complete CLI flow: scan → `c0_detect()`, calibrate → binary DAC search. Rust rewrite matches this flow.
- `c0_detect.cc` — Band-by-band GSM basestation scan → maps to Rust `scan.rs`
- `arfcn_freq.cc` — ARFCN↔frequency formulas → Rust `arfcn.rs` is a direct port
- `fcch_detector.cc` — LMS adaptive filter + FFT → Rust `fcch.rs`
- `offset.cc` — Single precise offset mode → Rust `offset.rs` (split into quick + precise)
- `bladeRF_source.cc` — Source abstraction over libbladeRF → Rust `source.rs`

## Hardware Resources

Local copies of hardware datasheets are in `resources/markdown/`:
- `si5338/` — SI5338 PLL reference manual, datasheet, FAQ
- `lms6002d/` — LMS6002D transceiver datasheet, programming guide, FAQ
- `dac161s055/` — DAC161S055 VCTCXO trim DAC datasheet and app notes

## Tests

- `arfcn.rs` and `fcch.rs` — Unit tests are pure computation, no hardware needed. Run with `cargo test -p kalibrate`.
- `offset.rs` and `scan.rs` — Require physical BladeRF1 device (need `Source`).

## Hardware / Network

- Requires a physical BladeRF1 for integrated behavior (`Source::open()`).
- `factory_dac_trim()` in `main.rs` queries `https://www.nuand.com/calibration/?serial=...` via `minreq`. Fails gracefully without internet.

## Data Format

- FCCH detection samples: SC16_Q11 (i16 I, i16 Q), 4 bytes/sample
- Power scan: SC8_Q7 (i8 I, i8 Q), 2 bytes/sample via `SampleFormat::Sc8Q7Meta`
- `derive_scan_params(usb_speed)` computes `sample_rate` and `samples_per_freq` from `nusb::Speed`
- FPGA retune queue depth: 16 entries (`source.rs`, `REUNE_QUEUE_DEPTH`)

## INTENT.md

Describes the design intent: why the Rust version differs from the C++ original, the streaming architecture, and the rationale for SC8_Q7Meta + USB-derived parameters. All intent goals in INTENT.md have been achieved.
