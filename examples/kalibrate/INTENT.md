# kalibrate — Design Intent

Rust rewrite of kalibrate-bladeRF with **speed** as the primary design goal.

## Key Differences from C++ Original

The original C++ (`c0_detect.cc`) tunes channels sequentially: tune → wait → measure → tune → wait. Each retune requires the FPGA to recalculate PLL parameters, adding latency. The Rust version eliminates this bottleneck:

### Precalculated LMS Tuning Parameters

All `LmsFreq` structs are computed once before the scan begins and passed directly to `schedule_retune_with_duration()`. The FPGA can switch frequency immediately without recalculation.

### Streaming with FPGA Retune Queue

The bladeRF FPGA supports a 16-entry scheduled retune queue. The Rust implementation:

1. Activates Rx once with `SampleFormat::Sc16Q11Meta` (provides FPGA timestamps)
2. Pre-fills the queue up to 16 entries with future timestamps
3. Streams samples continuously — never stops the streamer
4. As each scheduled retune is consumed, pushes the next one into the queue
5. Uses timestamps to determine which frequency each sample chunk belongs to

The result: the FPGA switches frequency in the background while samples flow over USB without interruption.

### Minimal USB Data Transferred

The dominant cost of a power scan is USB throughput. Three independent levers control total data:

- **Format**: SC16_Q11 uses 4 bytes/sample. SC8_Q7 uses 2 bytes/sample — more than adequate for power measurement where we only need to distinguish signal from noise floor. For a 40+ dB SNR scenario, SC8_Q7's ~48 dB dynamic range is sufficient.
- **Sample rate**: Higher rate means more data per second. The minimum is constrained by Nyquist relative to the receiver bandwidth (1.5 MHz minimum → >3 MS/s). Going faster than needed wastes USB bandwidth without improving power estimates.
- **Samples per frequency**: For relative error tolerance ε, you need ~1/ε² samples. 10,000 samples gives ~1% error; 1,000 gives ~3%. A 5% tolerance (~400 samples) may be sufficient for noise-floor discrimination.

Combined effect: switching from SC16_Q11 to SC8_Q7 alone halves USB data regardless of other settings.

### Adaptive Settings by USB Speed

USB speed is available at runtime (`self.device.speed()`). Total scan time is dominated by USB transfer; optimal settings should minimize the product of (bytes/sample × sample_rate × samples_per_freq):

- **USB 2.0 High Speed** (~30 MB/s real throughput): minimize bytes aggressively — use SC8_Q7, Nyquist-rate sampling, and minimum samples per frequency.
- **USB 3.0 Super Speed+**: throughput is less constrained, but SC8_Q7 still provides a 2× advantage regardless.

### SC8_Q7Meta as Power Scan Format ✅ Implemented

SC8_Q7Meta is the **only** format for power scanning — 2 bytes/sample vs 4 bytes for SC16_Q11Meta, halving USB data. The ~48 dB dynamic range is more than enough for distinguishing signal from noise floor.

The library change (`supports_format()` allowing `Sc8Q7Meta` for RX, meta version 0x34) was done in Phase 0. `streaming_power_scan()` uses `Sc8Q7Meta` exclusively with no fallback.

### Parameter Derivation ✅ Implemented

Hardcoded sample rate and samples-per-frequency values were replaced with `derive_scan_params()` which computes values from USB speed:

| Parameter | Source | USB 2.0 Value | USB 3.0+ Value |
|-----------|--------|---------------|----------------|
| Bandwidth | LMS minimum LPF | 3.5 MHz | 3.5 MHz |
| Sample rate | Nyquist for LPF | 4 MS/s | 4 MS/s |
| Samples/freq | from USB speed | 4,000 (~1.6% error) | 40,000 (~0.5% error) |
| Format | SC8_Q7Meta | 2 bytes/sample | 2 bytes/sample |
| Total USB data | bytes × samples × freqs | ~5.8 MB (725 freqs) | ~58 MB |

`streaming_power_scan()` takes only `(frequencies, gain_db)` — all parameters are derived internally.

### Zero-Copy Power Computation

The `compute_power_raw()` function works directly on raw sample bytes from USB DMA buffers. No `Complex32` allocation, no intermediate conversion — just `(I² + Q²)` accumulated per chunk. For SC8_Q7 this reads 2 bytes per sample; for SC16_Q11 it reads 4.

## Streaming Architecture (Implemented)

See `Source::streaming_power_scan()` in `source.rs`. Returns `Vec<(f64, f64)>` — frequency and mean-squared-magnitude tuples for every scanned channel.

## Why NOT Wideband Power Scan

A natural idea is to increase the LMS6002D receiver bandwidth so one retune covers multiple ARFCNs (200 kHz spacing), reducing the number of retunes needed. This doesn't work for the following reasons:

**Inseparable power:** A scalar power measurement (`sum(I² + Q²)`) on a wideband capture gives the combined power of *all* signals plus noise within the passband. You can't decompose it into per-ARFCN power without per-chunk spectral analysis (FFT).

**FFT overhead kills the speed advantage:** FFT resolution = `sample_rate / N`. To distinguish 200 kHz-spaced ARFCNs at 20 MS/s requires N ≥ 100,000. At 40 MS/s (max), N ≥ 200,000. Those FFTs — done per sample chunk — are orders of magnitude slower than the current zero-copy byte iteration (`compute_power_raw()` on raw SC16_Q11 bytes).

**USB bottleneck:** Higher sample rate means more data. 20 MS/s at SC16_Q11 = 80 MB/s (saturates USB 2.0 High Speed); at SC8_Q7 that's only 40 MB/s. 40 MS/s at SC16_Q11 = 160 MB/s (needs SuperSpeed); at SC8_Q7 it's 80 MB/s (fits USB 2.0). The FPGA timestamp infrastructure must also keep up, adding more pressure.

**Noise floor regression:** Wider bandwidth inherently captures more noise power, raising the noise floor and making weak signals harder to distinguish.

**Retune overhead is already negligible:** With precalculated `LmsFreq` and the 16-entry FPGA retune queue scheduling retunes ahead, per-retune latency is near zero. There's no retune bottleneck to shortcut.

The current approach wins: narrow bandwidth per retune, FPGA timestamps identify which frequency, scalar sum-of-squares gives power. No FFT, no allocation, no ambiguity.

## Implementation Status

### All Intent Goals Achieved ✅

| Intent Goal | Status | Evidence / Notes |
|---|---|---|
| Precalculated `LmsFreq` | ✅ | Computed once before scan (`source.rs:222-226`) |
| Streaming with FPGA retune queue | ✅ | 16-entry pre-fill + consume/schedule cycle (`source.rs`) |
| Zero-copy power computation (SC8) | ✅ | `compute_power_raw()` reads 2-byte SC8_Q7 samples (`source.rs:803`) |
| Returns `Vec<(freq, power)>` | ✅ | Return type of `streaming_power_scan()` |
| Adaptive scan parameters | ✅ | `derive_scan_params(usb_speed)` in `source.rs:783` |
| SC8_Q7Meta as only power scan format | ✅ | `streaming_power_scan()` uses `Sc8Q7Meta` exclusively |
| `streaming_fcch_scan()` batch detection | ✅ | Phase D: Sc16Q11Meta bucket collection, per-bucket FCCH detection on CPU |

### `scan::scan()` — Fully Streaming

Phase 1 uses `streaming_power_scan()` (SC8_Q7, USB-derived parameters).
Phase 3 uses `streaming_fcch_scan()` (SC16_Q11Meta, per-frequency sample buckets).
FCCH detection runs per bucket on CPU — zero USB overhead during detection.

### CLI Wiring — Complete

All CLI flags (`-s`, `-f`, `-c`, `-a`, `-b`, `-C`, `-m`, `-w`) are wired and functional.

## Architecture

- `main.rs` — CLI entrypoint, `factory_dac_trim()` HTTP lookup, `calibrate()` binary search, `auto_calibrate()`
- `source.rs` — `Source` wrapper around `BladeRf1`; streaming, DAC trim, `streaming_power_scan()`, `streaming_fcch_scan()`, `ScanParams`/`derive_scan_params()`
- `scan.rs` — Band scan: `streaming_power_scan()` Phase 1 → threshold → `streaming_fcch_scan()` Phase 3 → per-bucket FCCH detection
- `offset.rs` — Offset measurement via FCCH detection (quick and precise); uses `start()`/`flush()`/`read_samples()`/`stop()`
- `fcch.rs` — LMS adaptive filter + FFT frequency estimation (Varma et al.)
- `arfcn.rs` — GSM band/ARFCN definitions and frequency conversion
