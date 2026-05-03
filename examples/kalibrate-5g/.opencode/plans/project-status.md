# Project Status — kalibrate-5g / libbladerf-rs

## Goal

Implement PSS (Primary Synchronization Signal) correlation as a verification step inside the 5G NR SSB detector for `kalibrate-5g`, so that only real 5G signals pass detection and LTE/wideband/CW interferers are rejected. The ultimate goal is end-to-end VCTCXO calibration — finding the correct DAC trim value that zeroes the frequency offset. A secondary goal emerged: analyze libbladerf-rs API patterns for safety/lifecycle issues and performance, producing actionable plans.

## Instructions

- Use brute-force PSS correlation for frequency offset measurement — FFT the input, for each n_id_2 and bin shift compute correlation over 127 PSS subcarriers
- Avoid DC LO leakage — tune with a deliberate offset so the SSB appears away from DC
- Use wider bandwidth for scanning — 40 Ms/s, 28 MHz BW on USB3; 20 Ms/s, 14 MHz BW on USB2
- Add fill-rate filtering — only attempt PSS verification for detections with `fill_rate > 0.5`
- A deferred plan for RxStream API redesign exists at `examples/kalibrate-5g/.opencode/plans/rxstream-redesign.md` — do NOT implement this until kalibrate-5g produces correct end-to-end results
- Cross-correlation theorem does NOT work for sparse PSS — IFFT(R · conj(P)) produces flat/corrupted output
- FPGA scheduled retunes broken at 40 Ms/s — use sequential tune/collect/detect approach
- PSS verification in the wideband detector is **disabled** (`PSS_ENABLED = false`) because correlation is too weak at 40 Ms/s. CW rejection is done via the c2b discriminator in the narrowband second-pass PSS measurement instead.
- User wants proper profiling methodology (flamegraphs, benchmarks) before optimizing, and autovectorization planning for SIMD performance

## Discoveries

### PSS correlation at 40 Ms/s is too weak for reliable detection
- At 40 Ms/s with 65536-point FFT, `scs_bins = 24.58` (fractional). Lerp extraction only captures the narrow sinc peak, missing most subcarrier energy.
- PSS SNR was only 3.9 dB on real hardware — well below any reasonable threshold.

### Frequency-domain PSS correlation cannot discriminate n_id_2
- The phase of each PSS subcarrier in the FFT depends on the unknown time offset. `val * conj(pss)` gives random phase, summing 127 terms gives random-walk magnitude, NOT coherent sum.
- Both correct and incorrect n_id_2 produce same magnitude, so n_id_2 discrimination is ~0 dB in frequency domain.

### Correlation-to-band-power (c2b) is the key CW discriminator
- `c2b = 10·log10(best_correlation_mag / total_band_power_in_PSS_subcarriers)`
- For CW: c2b ≈ 0 dB; For real PSS: c2b > 3 dB. Threshold: `PSS_C2B_MIN = 3.0 dB`
- Works reliably in narrowband (4 Ms/s) second-pass PSS measurement where `scs_bins = 245.76`

### CW interferer dominates n3 band on real hardware
- A strong CW tone (~1870 MHz) consistently appears in n3 band with c2b ≈ 0 dB — correctly rejected
- It's actually DC LO leakage when retuning to SSB center frequency

### Coarse detector offset is biased
- The Welch PSD sliding-window detector consistently misestimates the SSB center by ~100-160 kHz

### libbladerf-rs API Safety Issues (9 issues documented in plan)
1. No `Drop` on streamers → device wedges on abnormal exit
2. `perform_format_deconfig` is a no-op → GPIO state leaks across sessions
3. `initialize()` must be called manually → no compile-time enforcement
4. Dropped `Buffer` starves the pool → silent deadlock
5. `SpiFlash::with_flash_alt_setting` panic leaves device in flash mode
6. `config_gpio_write` silently modifies the value
7. `cancel_scheduled_retunes` requires meaningless `Band::Low` parameter
8. `set_precalculated_frequency` error recovery leaves PLL in undefined state
9. No board state machine — missing capability checks and state guards

### nusb internals verified
- `Buffer::Drop` properly frees memory (no leak)
- `LinuxEndpoint::Drop` calls `cancel_all()` then frees
- `Pending::Drop` transitions to `STATE_ABANDONED` — kernel handler cleans up
- `EndpointBitSet` provides exclusivity — concurrent `endpoint()` calls fail properly
- Memory safety is fine, but **device state** is broken without proper cleanup

### C libbladeRF reference comparison
- C `bladerf1_open()` always calls `bladerf1_initialize()` — user never sees uninit device
- C `perform_format_deconfig()` also doesn't write GPIO but tracks `module_format[]` so `perform_format_config` can reconfigure correctly
- C `bladerf1_close()` calls `sync_deinit` + `cancel_scheduled_retunes` — full cleanup
- C `bladerf1_enable_module(false)` calls `perform_format_deconfig` — Rust deconfig is no-op

### Performance Analysis
- PSS correlator is the dominant cost (~9.6M `lerp_spectrum` calls per PSS measurement)
- `lerp_spectrum` uses f64 arithmetic → blocks AVX2 autovectorization (forces scalar `vcvtsd2ss`)
- Per-sample `Vec::push` in SC8Q7→Complex32 conversion prevents vectorization
- PSD `norm_sqr` accumulated as f64 halves SIMD throughput
- CPU: Intel i9-13980HX with AVX2, FMA, SSE4.2 — but Rust defaults to SSE2 only

### Autovectorization Strategy
- `Complex32` is `#[repr(C)]` — SIMD-friendly layout (4 × Complex32 = 1 AVX2 register)
- `target-cpu=native` gives 2-4× on f32 inner loops with zero code changes
- Key fixes: convert lerp_spectrum to f32, replace Vec::push with slice writes, batch lerp loads, accumulate PSD in f32
- Explicit SIMD (`core::simd` nightly or `std::arch` stable) only if autovectorization fails

## Accomplished

### Completed
1. PSS CW rejection via c2b discriminator — `c2b` metric in `pss_correlate()`, threshold 3 dB
2. Wideband PSS verification disabled (`PSS_ENABLED = false`) — too weak at 40 Ms/s
3. Narrowband second-pass PSS measurement — `measure_pss_offset()` at 4 Ms/s, c2b rejects CW
4. Top-5 candidate PSS measurement — tries real signals before CW-dominated ones
5. Dwell time doubled (200ms → 400ms) and PAR threshold lowered (10 dB → 6 dB) for better weak-signal detection
6. API safety plan written — `.opencode/plans/api-safety-and-lifecycle.md` with 9 issues, C reference evidence, fix proposals
7. Profiling and benchmarks plan written — `.opencode/plans/profiling-and-benchmarks.md` with flamegraph, criterion, I/O instrumentation methodology
8. Autovectorization section drafted — to be appended to profiling plan; covers target-cpu=native, f64→f32 fixes, Vec::push→slice, lerp batching, explicit SIMD escalation path
9. All clippy warnings fixed, formatting clean, 31 tests pass

### Not Yet Done
- Append autovectorization section (§9) to `.opencode/plans/profiling-and-benchmarks.md`
- Install profiling tools (`linux-perf`, `flamegraph`, `samply`), set `perf_event_paranoid=0`
- Actually run profiling and establish performance baselines
- Implement any of the API safety fixes from the plan
- Binary-search DAC calibration loop (Phase 3)
- Flash write support
- Band-specific CLI options
- Multi-dwell offset averaging
- RxStream API redesign (deferred)
- Validate narrowband PSS correlation with a real 5G signal
- Implement proper CW cancellation (current `cancel_dominant_tone()` is a no-op stub)

## Relevant files / directories

### kalibrate-5g (application)
- `examples/kalibrate-5g/src/main.rs` — Entry point, wideband scan → PSS measurement → offset computation
- `examples/kalibrate-5g/src/pss.rs` — PSS m-sequence generation, PSS correlation with lerp extraction, c2b CW discriminator, `cancel_dominant_tone()` stub
- `examples/kalibrate-5g/src/detect.rs` — Multi-detection SSB detector (PSS verification disabled via `PSS_ENABLED = false`), Welch PSD, cumulative sum sliding window
- `examples/kalibrate-5g/src/source.rs` — BladeRF1 Source wrapper, `streaming_ssb_scan()`, `measure_pss_offset()` narrowband second-pass PSS measurement, scan params (dwell=400ms, PAR=6dB)
- `examples/kalibrate-5g/src/gscn.rs` — GSCN tables + grouping + nearest mapping
- `examples/kalibrate-5g/src/offset.rs` — Standalone PSS offset measurement (no longer called from main)
- `examples/kalibrate-5g/src/factory.rs` — Factory DAC trim HTTP lookup
- `examples/kalibrate-5g/Cargo.toml` — Dependencies (rustfft, nusb, anyhow, minreq)

### libbladerf-rs (driver library)
- `src/bladerf1/board.rs` — BladeRf1 struct, Clone via Arc<Mutex<NiosInterface>>, initialize(), enable_module()
- `src/bladerf1/board/stream.rs` — BladeRf1RxStreamer/TxStreamer, BufferPool, read()/recycle(), no Drop impls
- `src/bladerf1/board/frequency.rs` — set_frequency(), schedule_retune(), cancel_scheduled_retunes()
- `src/bladerf1/board/dac_trim.rs` — set_dac_trim(), get_dac_trim(), get_vctcxo_trim()
- `src/bladerf1/board/calibration.rs` — read_flash_dac_trim(), save_dac_trim()
- `src/bladerf1/hardware/lms6002d/frequency.rs` — set_precalculated_frequency(), turn_off_dsms() error recovery
- `src/transport/usb.rs` — UsbTransport, NiosEndpoints, acquire_streaming_rx_endpoint()
- `src/bladerf1/hardware/spi_flash.rs` — with_flash_alt_setting() panic safety issue

### Plans
- `examples/kalibrate-5g/.opencode/plans/pss-in-detector.md` — PSS-in-detector integration plan
- `examples/kalibrate-5g/.opencode/plans/rxstream-redesign.md` — Deferred RxStream API redesign
- `.opencode/plans/api-safety-and-lifecycle.md` — 9 API safety issues with C reference evidence and fix proposals
- `.opencode/plans/profiling-and-benchmarks.md` — Profiling methodology, criterion benchmarks, I/O instrumentation (autovectorization section §9 still needs to be appended)
