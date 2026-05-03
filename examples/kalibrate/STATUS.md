# kalibrate Status

Last updated: 2026-04-26

## Phases

| Phase | Status | Description |
|-------|--------|-------------|
| Phase 0 | DONE | Sc8Q7Meta RX enabled, meta version 0x34 fix, loopback test |
| Phase A | DONE | CLI wiring (-s, -f, -c, -a, -b, -C, -w), calibrate() fixed |
| Phase B | DONE | scan.rs Phase 1 uses streaming_power_scan() with USB-derived parameters |
| Phase C | DONE | streaming_power_scan migrated to SC8_Q7 (2 bytes/sample, 50% USB bandwidth) |
| Phase D | DONE | scan.rs Phase 3 uses streaming_fcch_scan(), FCCH detection per bucket on CPU |

## Build and Tests

```
cargo check -p kalibrate   # clean, 4 dead_code warnings (expected)
cargo test -p kalibrate    # 9/9 pass (arfcn + fcch unit tests, no hardware)
```

## Hardware Verified

`kalibrate -s GSM900 --gain 40` scans successfully, channels detected (e.g. ARFCN 46).

## Phase A Changes (main.rs)

- CLI dispatch: `-s` → scan, `-f` → calibrate(freq), `-c` → calibrate(arfcn→freq), `-a` → auto_calibrate
- `auto_calibrate()`: all-band `streaming_power_scan()` → pick strongest → `calibrate()`
- `calibrate()`: binary search DAC trim, uses `offset_detect()` in search loop, final precise `offset_detect()`
- `-C` initial DAC trim, `-b` band disambiguation, `-w` flash write
- `parse_dac_trim()` supports decimal and 0x hex

## Phase B Changes (scan.rs, source.rs)

- `scan.rs` Phase 1: replaced sequential `tune → flush → read_samples()` loop with single `streaming_power_scan()` call
- `streaming_power_scan()`: dropped `sample_rate` and `samples_per_freq` parameters; now derives internally
- `derive_scan_params(Speed)`: computes optimal `sample_rate` / `samples_per_freq` from USB speed (see below)
- `auto_calibrate()`: simplified to `source.streaming_power_scan(&frequencies, args.gain)`
- 4 dead_code warnings: `Band::from_freq`, `DetectedChannel`, `Source::power_sweep`, `ScanParams::bytes_per_sample`

### Derived Scan Parameters

| USB Speed | sample_rate | samples_per_freq | dwell | error tol | data (725 freqs) |
|-----------|---|---|---|---|---|
| High (USB 2.0) | 4 MS/s | 4,000 | 1 ms | ~1.6% | ~5.8 MB |
| Super/Plus (USB 3.0) | 4 MS/s | 40,000 | 10 ms | ~0.5% | ~58 MB |
| USB 1.1 (fallback) | 4 MS/s | 1,000 | 0.25 ms | ~3.2% | ~1.5 MB |

## Phase C Changes (source.rs)

- `streaming_power_scan()` format: `Sc16Q11Meta` → `Sc8Q7Meta` (2 bytes/sample)
- `compute_power_raw()`: SC16_Q11 (i16/i16) → SC8_Q7 (i8/i8) decoder path
- `derive_scan_params`: `bytes_per_sample` = 2; samples_per_freq increased 4× to compensate dynamic range
- `BYTES_PER_SAMPLE` constant: 4 → 2

## Phase D Changes (scan.rs, source.rs)

- `source.rs`: Added `streaming_fcch_scan()` — batch FCCH sample collection per frequency.
- `scan.rs` Phase 3: Replaced the sequential `tune → flush → read → detect` per channel with a single `streaming_fcch_scan()` call.
- CPU-side `FcchDetector::scan()` runs per bucket after streaming completes.
- `start()`, `stop()`, `flush()`, and `read_samples()` in `source.rs` remain in use by `offset.rs` for the calibration binary search.

## Key Files

| File | Purpose |
|------|---------|
| `examples/kalibrate/src/main.rs` | CLI entrypoint, `calibrate()`, `auto_calibrate()` |
| `examples/kalibrate/src/scan.rs` | Band scan: `streaming_power_scan` → threshold → `streaming_fcch_scan` → per-bucket FCCH detection |
| `examples/kalibrate/src/source.rs` | BladeRF wrapper: streaming, DAC trim, `streaming_power_scan()`, `streaming_fcch_scan()`, `ScanParams` |
| `examples/kalibrate/src/offset.rs` | `offset_detect()` (100 samples), `offset_detect_quick()` (20 samples) |
| `examples/kalibrate/src/fcch.rs` | LMS adaptive filter + FFT frequency estimation |
| `examples/kalibrate/src/arfcn.rs` | Band/ARFCN definitions, frequency conversion |
| `src/bladerf1/board/stream.rs` | Sc8Q7Meta support, `is_valid_meta_format()` accepts 0x00 and 0x34 |
| `resources/kalibrate-bladeRF/src/kal.cc` | C reference implementation |

## Known Issues

- Device bad state: after some tests RX on endpoint 81 times out with zero data. Requires USB reconnect.
- 1 dead_code warning: `Band::from_freq` (utility for reverse frequency lookup).
