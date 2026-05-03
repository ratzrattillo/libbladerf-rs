# Implementation Plan — kalibrate completion + speedup

## Phase 0: Enable SC8_Q7Meta RX in libbladerf-rs (foundation)

Everything else depends on `Sc8Q7Meta` being a valid RX sample format. Currently blocked by `supports_format()` in the library crate.

### The change: `src/bladerf1/board/stream.rs`

Single-line change at `stream.rs:292-303`. Add `Sc8Q7Meta` to the RX match arm:

```rust
impl BladeRf1 {
    pub fn supports_format(&self, format: SampleFormat, direction: Channel) -> bool {
        match direction {
            Channel::Rx => matches!(
                format,
                SampleFormat::Sc8Q7Meta |  // <-- ADD THIS
                SampleFormat::Sc16Q11 | SampleFormat::Sc16Q11Meta | SampleFormat::PacketMeta
            ),
            Channel::Tx => matches!(
                format,
                SampleFormat::Sc16Q11 | SampleFormat::Sc16Q11Meta | SampleFormat::PacketMeta
            ),
        }
    }
}
```

**Why this is sufficient for the library side:**

The downstream plumbing already handles `Sc8Q7Meta` correctly:

| Component | Already correct? | Details |
|-----------|-----------------|---------|
| `SampleFormat::sample_size()` | ✅ | Returns 2 for `Sc8Q7` and `Sc8Q7Meta` (line 278) |
| `requires_timestamps()` | ✅ | Returns `true` for `Sc8Q7Meta` alongside `Sc16Q11Meta` and `PacketMeta` (line 286-289) |
| `perform_format_config()` | ✅ | Uses `requires_timestamps()` to set `BLADERF_GPIO_TIMESTAMP` + `BLADERF_GPIO_TIMESTAMP_DIV2` bits — identical GPIO setup to `Sc16Q11Meta` |
| `MetadataHeader` | ✅ | 16-byte header struct is format-agnostic; `is_valid_meta_format()`, `timestamp()`, `stream_flags()` all work identically for Sc8Q7Meta |
| `Stream::activate()` | ✅ | Calls `perform_format_config()` then `enable_module()` — no format-specific branching |
| `METADATA_HEADER_SIZE` | ✅ | 16 bytes, shared across all metadata formats |

The FPGA handles the Sc16→Sc8 quantization in hardware when the RX sample format is set to Sc8Q7. The USB message block layout is the same structure as Sc16Q11Meta: 16-byte metadata header followed by samples, repeated every `message_block_size` bytes. The only difference is 2 bytes/sample vs 4.

### Testing: `tests/integration_bladerf1_stream.rs`

Add a new integration test: `rx_stream_sc8q7meta_loopback()`.

**Test strategy — firmware loopback (TX→RX via FPGA, no analog path):**

1. Save original GPIO and loopback state
2. Set loopback to `Loopback::Firmware` (purely digital TX→RX in FPGA)
3. Create TX streamer with `Sc16Q11` (TX format unchanged), RX streamer with `Sc8Q7Meta` (new format)
4. TX sends known samples (e.g., a sine tone, 2 bytes/sample SC16 → FPGA downquantizes to SC8)
5. RX reads buffers, validates:
   - **Metadata**: Every `message_block_size` bytes, parse `MetadataHeader`, verify `is_valid_meta_format()`, timestamps are non-zero and monotonically increasing
   - **Sample alignment**: After the 16-byte header, payload stride must be 2 bytes/sample (even number of bytes)
   - **Data sanity**: Not all zeros (firmware loopback should carry signal), power > 0
   - **Bytes/sample ratio**: `(payload_bytes) % 2 == 0`, sample count matches expected
6. Deactivate streamers, restore GPIO and loopback state

**Why firmware loopback:**
- `Loopback::Firmware` routes TX samples directly to RX in the FPGA — no analog chain, no noise source needed
- Guarantees signal integrity regardless of antenna, RF conditions, or baseband filter settings
- The FPGA performs the Sc16→Sc8 quantization natively as part of the format pipeline
- Isolated: no external dependencies

**Why TX stays Sc16Q11:**
- We only need to verify RX can accept Sc8Q7Meta — TX format is orthogonal
- TX Sc8 support would require a separate `supports_format()` change for the TX arm + TX format configuration
- Not needed for kalibrate (receive-only)

**Test scaffolding to add:**

```
In tests/integration_bladerf1_stream.rs:

fn validate_sc8q7meta_metadata(buffer: &[u8], message_size: usize) -> Result<usize>
```
Returns the number of valid headers found. Walks the buffer in `message_size` strides, parses `MetadataHeader` at each offset, validates meta version, timestamp monotonicity, and counts samples in each payload.

```
#[test]
fn rx_stream_sc8q7meta_loopback() -> Result<()>
```
Full loopback test: set `Loopback::Firmware`, activate TX Sc16Q11 + RX Sc8Q7Meta, send known data, read and validate.

**Also add `Sc8Q7Meta` to the GPIO format cycle test** (`gpio_state_format_cycle`, line 794): add `SampleFormat::Sc8Q7Meta` to the format array and verify it sets the same timestamp/div2 bits as `Sc16Q11Meta`.

### After Phase 0 succeeds

Once the test passes, the kalibrate crate no longer needs dual-format `compute_power_raw()` — it can switch directly to Sc8Q7Meta with a single 2-byte decoder path.

---

## Phase A: CLI Wiring (makes the tool actually functional)

**File: `main.rs`**

Rewrite `main()` to dispatch on parsed CLI args instead of unconditionally running the power scan. The flow matches `kal.cc`:

| CLI args | Code path |
|----------|-----------|
| `-s BAND` | `scan::scan(source, band, mult)` |
| `-f FREQ` | `calibrate(source, freq, args)` |
| `-c ARFCN` (+ optional `-b BAND`) | → resolve frequency, then `calibrate(source, freq, args)` |
| `-a` | Power scan all bands → pick strongest → `calibrate(source, freq, args)` |

Changes needed:
- Add `-a` band selection to `scan` flow — when `-a` set, skip band filter in power scan, pick top result
- Wire `-C` (initial DAC trim) → `source.set_dac_trim()` before scan/calibrate
- Wire `-m` (scan multiplier) → pass to `scan::scan()` as `mult` (already accepted)
- Wire `-b` (band disambiguation) → used when resolving `-c ARFCN` 512–810
- Wire `-w` (write flash) → already handled inside `calibrate()`

**Bugs to fix in existing `calibrate()` (main.rs:156):**
1. Line 183: Calls `offset::offset_detect()` for every binary search iteration — should use `offset::offset_detect_quick()` for binary search iterations, reserving `offset_detect()` (100 detections) only for the final measurement. Saves ~80% calibration time during binary search.

---

## Phase B: scan.rs — Replace Phase 1 with streaming power scan

**File: `scan.rs`**

Phase 1 (`scan.rs:37-56`) iterates all ARFCNs in the band sequentially: tune → flush → read → compute. Replace it with a single call to `source.streaming_power_scan()` for the band's frequencies, then map results back to `(arfcn, freq, power)` tuples.

```
Before: tune → flush → read → norm_sqr (per ARFCN, ~50ms overhead each)
After:  source.streaming_power_scan(band_frequencies, ...) → 0 per-channel overhead
```

Phase 3 (FCCH detection) **remains sequential** — the detector needs guaranteed contiguous samples per channel.

---

## Phase C: SC8_Q7Meta migration + parameter derivation in kalibrate

**File: `source.rs` (kalibrate crate)**

With Phase 0 complete (library accepts Sc8Q7Meta), switch the kalibrate power scan to use Sc8 as the only format:

1. **Create `derive_scan_params(usb_speed) -> ScanParams`** — derives all scan parameters from USB speed (bandwidth, sample rate, samples/freq, bytes/sample, retune gap)
2. **Replace `compute_power_raw()`** — single 2-byte SC8 decoder path, no dual-format branching
3. **`streaming_power_scan()` format change** — use `Sc8Q7Meta` directly, no fallback. Drop `sample_rate` and `samples_per_freq` from the signature, derive from `derive_scan_params()`
4. **Update `main.rs` call site** — simplified: `source.streaming_power_scan(&frequencies, args.gain)`

---

## Phase D: Batch FCCH detection (Phase 3 optimization)

**File: `source.rs` — new `streaming_fcch_scan()` method**

Phase 3 in `scan.rs` re-tunes, flushes, and reads separately for each above-threshold ARFCN. Replace with streaming:

1. Create `streaming_fcch_scan(frequencies, dwell_ms)` that activates Rx once, schedules retunes for all frequencies via the FPGA queue, streams continuously, and uses timestamps to bucket samples per frequency
2. After receiving all buckets, run `FcchDetector::scan()` per bucket on CPU — zero USB overhead during FCCH detection
3. Re-schedule missed frequencies if no FCCH burst was captured

**Trade-off:** Larger USB window during scan (50 channels × 15 ms × 4 MS/s × 4 bytes = 12 MB with SC16), but eliminates per-channel flush overhead (~40% of current time).

---

## Priority & ordering

| Phase | Impact | Risk | Depends on |
|-------|--------|------|------------|
| **0** — SC8Q7Meta in lib | Unblocks everything | Low (1-line change) | — |
| **A** — CLI wiring | Tool becomes functional | Low | — |
| **B** — scan.rs streaming | 10–50× faster power scan | Low | A |
| **C** — SC8 in kalibrate | 2× less USB data | Low | 0 |
| **D** — Batch FCCH | 5–10× faster scan Phase 3 | Medium | B, C |
