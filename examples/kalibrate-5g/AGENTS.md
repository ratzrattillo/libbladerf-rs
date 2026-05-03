# kalibrate-5g

5G NR-SSB-based VCTCXO oscillator calibration for BladeRF1. Sibling to `kalibrate` (GSM FCCH-based).

## Status

**Phase 2b complete (PSS correlator + wideband scanning).** GSCN-based frequency sweep detects 5G NR signals reliably. PSS-based frequency offset measurement implemented with brute-force frequency-domain correlation. Wideband scanning (40 Ms/s, 28 MHz BW on USB3; 20 Ms/s, 14 MHz BW on USB2) with DC-offset tuning reduces scan time from ~1070 to ~36 tuning steps. See "Architecture" and "Key Design Decisions" sections below.

## Commands

All commands run from the **repository root**:

| Action | Command |
|--------|---------|
| Build | `cargo build -p kalibrate-5g` |
| Run | `cargo run -p kalibrate-5g -- [OPTIONS]` |
| Check | `cargo check -p kalibrate-5g` |
| Test | `cargo test -p kalibrate-5g` |

## Dependencies

- `libbladerf-rs` (path `../..`) — BladeRF1 driver
- `minreq` (https-rustls) — HTTP calls (factory DAC trim lookup on nuand.com)
- `nusb` — USB transport (USB speed affects sample rate derivation)
- `rustfft` — FFT for spectral detection and PSS correlation
- `anyhow` — error handling
- `env_logger` — logging

## Architecture

| Module | Purpose |
|--------|---------|
| `src/main.rs` | Entry point: group GSCN frequencies into tuning steps, scan, map detections to GSCN, dedup, filter by fill rate > 0.5, measure offset for top candidates |
| `src/gscn.rs` | NR GSCN tables for 7 German sub-6 GHz bands (n8, n20, n28, n3, n1, n7, n78), `gscn_to_freq`/`freq_to_gscn` conversion, `group_frequencies()` for wideband scan grouping, `nearest_gscn()` for mapping, 1070 total GSCN positions |
| `src/detect.rs` | Multi-detection SSB detector: Welch PSD (8192-point FFT, up to 8 segments), cumsum sliding window, fill-rate validation, PAR threshold 10 dB, DC exclusion ±250 kHz, successive-cancellation noise floor for multiple detections |
| `src/source.rs` | BladeRF1 Source wrapper: FPGA retune queue, SC8_Q7_Meta format, background detection thread via mpsc, wideband scan params (40/20 Ms/s, 28/14 MHz BW, 1.5 MHz tuning offset), offset measurement |
| `src/offset.rs` | PSS-based frequency offset measurement: 65536-point FFT, brute-force correlation over ±200 ppm for all 3 n_id_2 hypotheses, parabolic interpolation, SNR threshold 13 dB |
| `src/pss.rs` | PSS m-sequence generation (3 sequences, N_id_2 = 0/1/2) |
| `src/factory.rs` | Factory DAC trim HTTP lookup on nuand.com |

## Key Design Decisions

- **Sample rate (scan)**: 40 Ms/s on USB3, 20 Ms/s on usb2 (fallback). LMS6002D LPF supports up to 28 MHz BW.
- **Bandwidth (scan)**: 28 MHz on USB3, 14 MHz on USB2. Usable passband after DC exclusion is ~24 MHz / ~12 MHz.
- **Tuning offset**: +1.5 MHz. SSB is placed at -1.5 MHz in baseband, well clear of the DC spike (which can be 50-200 kHz wide, 20+ dB above noise on BladeRF1).
- **GSCN grouping**: `group_frequencies()` greedily groups GSCN positions into tuning steps, each covering up to `usable_bw` Hz. Reduces 1070 individual positions to ~36 tuning steps (30× reduction).
- **Detection**: Multi-detection (`ssb_detect_all`) returns all valid SSB candidates per tuning step, with successive-cancellation noise floor to handle multiple signals.
- **Mapping**: Detected offsets are mapped back to absolute frequencies, then to nearest GSCN via `nearest_gscn()`. Detections within 2 MHz of a GSCN position are accepted.
- **Dedup**: Multiple tuning steps may detect the same GSCN. The detection with highest fill rate wins.
- **Offset measurement filter**: Only detections with `fill_rate > 0.5` are considered for offset measurement (rejects weak/spurious detections).
- **Offset measurement**: PSS brute-force correlation. For each of 3 n_id_2 hypotheses, try every integer bin shift within ±200 ppm. Peak correlation magnitude identifies n_id_2 and coarse offset. Parabolic interpolation refines to sub-bin precision. SNR = peak / median of all correlation magnitudes, threshold 13 dB.
- **FFT size (detect)**: 8192 → ~4.88 kHz bin resolution at 40 Ms/s; SSB window ≈ 738 bins
- **FFT size (offset)**: 65536 → ~610 Hz bin resolution at 40 Ms/s; parabolic interpolation gives ~23 Hz precision
- **PAR threshold**: 10 dB (real signals typically 15-30 dB above noise)
- **Fill-rate threshold**: ≥0.25 for detection, >0.5 for offset measurement
- **Fill-rate margin**: 1 dB above noise floor for bin counting
- **DC exclusion (detect)**: ±250 kHz around DC
- **DC exclusion (offset)**: ±250 kHz around DC (zeroed in FFT before correlation)
- **Edge guard band**: 10% of PSD bins excluded at each edge
- **Welch segments**: Capped at 8
- **Background detection**: `ssb_detect_all` runs in a separate thread to avoid starving USB transfers
- **Empty bucket dispatch**: Retune gaps that produce 0 samples are force-dispatched to prevent hangs
- **Gain**: Hardcoded 40 dB (no CLI parameter)

## GSCN Reference

FR1 GSCN formulas (3GPP TS 38.104 Table 5.4.3.1-1):

- **Range 1** (0–3000 MHz): `SS_ref = N × 1,200,000 + M × 50,000` Hz, M∈{1,3,5}, GSCN = `3N + (M-3)/2`
- **Range 2** (3000–6000 MHz): `SS_ref = (3,000,000 + N × 1,440) × 1,000` Hz, GSCN = `7,499 + N`
- **Boundary**: At exactly 3 GHz, use `<=` (range 1), not `<`
- **N upper bound**: Range 1 N can exceed 619; ocudu uses `N_UB_SYNC_RASTER_1 = 2500`

Per-band GSCN ranges from Table 5.4.3.3-1 (not derivable from frequency ranges alone): n8: 2318–2395, n20: 1982–2047, n28: 1901–2002, n3: 4517–4693, n1: 5279–5419, n7: 6554–6718, n78: 7711–8051.

## Reference Materials

- `resources/TR 21.918/` — 3GPP TR 21.918 Release 18 (5G-Advanced) OCR'd pages
- `resources/markdown/` — Hardware datasheets: SI5338, LMS6002D, DAC161S055
- `resources/ocudu/lib/ran/ssb/ssb_gscn.cpp` — Authoritative C++ GSCN reference
- `resources/ocudu/include/ocudu/ran/band_helper_constants.h` — GSCN constants
- Sibling `examples/kalibrate/` — GSM-based calibration (may have bugs, use with caution)

## Hardware

Requires a physical BladeRF1 (x40/x115). Needs a 5G NR base station signal in range for calibration to succeed.

## Real Hardware Scan Results (BladeRF1 x40, Germany, 40 dB gain, factory DAC trim 0xabd1)

Three consecutive scans show consistent, reproducible results:

- **n1 band (2.1 GHz)**: Strongest and most reliable detections. PAR 20-25 dB, fill rate 60-77%.
  Best GSCN positions: 5337–5344 (2133–2138 MHz). Fill rates up to 77% indicate genuine SSB.
- **n3 band (1.84–1.86 GHz)**: Consistently detected. PAR 20-25 dB, fill rate 25-81%.
  GSCN positions 4593–4648 (1837–1859 MHz). Best fill rates around 80%.
- **n78 band (~3.4 GHz)**: Marginal. Occasionally detected at PAR 12-19 dB with fill rates 25-35%.
  Not reliable enough for calibration at this gain/dwell setting.
- **Other bands (n8, n20, n28, n7)**: No detections in any run.

**Best calibration candidate**: n1/GSCN-5344 (2137.45 MHz) — consistently highest fill rate (67-77%) with PAR ~24 dB.

With DAC trim 0x0000 (uncalibrated), a spurious detection appeared at n3/1848 MHz (PAR 22 dB, fill 60%), which disappeared with factory trim 0xabd1 — confirming it was a clock artifact.

With 60 dB gain (max), the LNA saturates, raising the noise floor and lowering fill rates (25-37% for n78 signals that were visible). 40 dB gain is better for detection.

## Known Issues

1. **Fill rates lower than expected for real SSB signals**: Real 5G signals show fill rates of 25-77% rather than the theoretical ~100%. This is acceptable for detection (threshold 0.25 rejects noise at ~2%) but means fill rate alone cannot distinguish SSB from wideband interference. The PAR + fill-rate combination works well in practice.

2. **n78 detections are marginal**: At 40 dB gain, n78 signals (~3.4 GHz) are near the detection threshold with fill rates of 25-35%. Higher gain (60 dB) saturates the LNA and doesn't help. A longer dwell time or directional antenna may be needed for reliable n78 detection.

3. **Streamer deactivation fails, wedging the device**: After the streaming scan completes, `streamer.deactivate()` sometimes fails with "USB transfer error: transfer was cancelled". This leaves the USB alt setting active and the device in a bad state. Subsequent `cancel_scheduled_retunes` and `measure_ssb_offset` calls then fail with timeouts. **Physical replug is required to recover.** Workaround in place: deactivation errors are logged as warnings instead of propagated. Root cause likely in libbladerf-rs streamer implementation.

4. **Factory DAC trim may be stale**: The factory calibration was done ~15 years ago. Crystal aging can cause several ppm of drift, so the factory trim value does not guarantee 0 ppm offset today. Still, it should be within ~5-10 ppm, not hundreds of ppm.

## Not Yet Implemented (Future Phases)

- SSS correlator — for complete cell ID identification (N_id_1 via SSS)
- Binary-search DAC calibration loop (equivalent to `calibrate()` in kalibrate main.rs)
- Flash write support (equivalent to `--write` flag)
- Band-specific CLI options
- Multi-dwell offset averaging (like kalibrate's quick/precise modes)
