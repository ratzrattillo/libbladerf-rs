# libbladerf-rs

Pure Rust driver for the Nuand BladeRF1 SDR. No C libbladeRF dependency. Based on [nusb] for USB transport.

[nusb]: https://github.com/kevinmehall/nusb

Edition 2024, MSRV 1.95.

## Commands

All commands run from the **repository root**:

| Action | Command |
|--------|---------|
| Build | `cargo build` |
| Test (no hardware) | `cargo test --lib` |
| Protocol tests (no hardware) | `cargo test --test unit` |
| Run a single unit test | `cargo test --test unit -- <test_name>` |
| Test (with hardware) | `cargo test --features bladerf1 --tests -- --test-threads=1` |
| Run a single hardware test | `cargo test --features bladerf1 --test bladerf1 -- <test_name>` |
| Clippy | `cargo clippy --all-targets -- -D warnings` |
| Format check | `cargo fmt --all --check` |
| Format (requires nightly) | `rustup run nightly -- cargo fmt` |
| Docs | `cargo doc --features bladerf1 --no-deps --lib --bins --examples` |
| Full CI check (local) | `bash check.sh` |
| Fuzz | `cargo +nightly fuzz run <target>` |

### check.sh vs CI

`check.sh` runs: test (`--features bladerf1 --examples --lib --tests --jobs=1 -- --test-threads=1`) → clippy → fmt → doc (`--lib --bins --examples`) → audit. CI does **not** run `check.sh` directly. CI runs these steps individually: build (`--features bladerf1`) → unit tests (`--lib`) → protocol tests (`--test unit`) → clippy (`--features bladerf1 --all-targets -- -D warnings`) → fmt check → audit → deny check (which check.sh omits) → docs (separate job: `--features bladerf1 --no-deps`).

### Examples

8 examples are workspace members (build with `-p`):

```bash
cargo build -p info -p calibrate -p rx_tx -p flash_firmware -p flash_fpga -p kalibrate -p kalibrate-5g -p bench-stream
```

Standalone examples (`diagnose`, `diag-xb200`) have their own `[workspace]` in `Cargo.toml` and must be built from their own directories.

### Benchmarks

Hardware benches must be run individually (they share one physical device). The stream build/teardown bench uses `BatchSize::PerIteration` because `iter_batched` with `SmallInput` pre-allocates multiple setups into a `Vec`, and only one `BladeRf1` can hold the USB interface claim at a time.

```bash
# No hardware needed:
cargo bench --bench nios_packet_bench --bench sample_format_bench --bench metadata_header_bench
# Requires hardware (run one at a time):
cargo bench --features bladerf1 --bench hardware_gpio_bench
cargo bench --features bladerf1 --bench hardware_tuning_bench
cargo bench --features bladerf1 --bench hardware_stream_latency_bench
cargo bench --features bladerf1 --bench hardware_stream_build_teardown_bench
cargo bench --features bladerf1 --bench hardware_gain_bench
cargo bench --features bladerf1 --bench hardware_calibration_bench
```

### Fuzz targets

`binkv_decode`, `nios_packet_decode`, `pack_sc16q11_packed`, `unpack_sc16q11_packed`, `metadata_header`

## Architecture

### Lock-free borrow-checker design

`BladeRf1` holds `nios: NiosCore` inline (no `Arc`, no `Mutex`). The borrow checker enforces exclusive access at compile time. Ephemeral wrappers provide subsystem namespacing:

```rust
pub struct BladeRf1 {
    device: Device,
    nios: NiosCore,
}

pub struct Lms6002d<'a>   { nios: &'a mut NiosCore }
pub struct Si5338<'a>     { nios: &'a mut NiosCore }
pub struct Dac161s055<'a> { nios: &'a mut NiosCore }
```

Note: there is **no** `SpiFlash` wrapper. Flash operations are `impl FlashSession` blocks in `spi_flash.rs`. `FlashMeta` is constructed and owned by `FlashSession` on creation, not stored on `BladeRf1`.

Calling convention: `self.lms.method()` → `self.lms().method()` (parentheses added). Direct `NiosCore` access: `self.nios.lock().unwrap().method()` → `self.nios.method()`.

### Session-based USB alt setting model

Operations are grouped into sessions that switch the USB alternate setting:

```rust
pub struct RfLinkSession<'a>  { nios: &'a mut NiosCore }
pub struct FlashSession<'a>   { nios: &'a mut NiosCore, flash_meta: FlashMeta }
pub struct ConfigSession<'a>  { nios: &'a mut NiosCore }
```

`BladeRf1::rf_link_session()`, `flash_session()`, and `config_session()` each check and switch the USB alt setting before returning the session. `FlashMeta` is queried from the device and constructed inside `flash_session()` — it is not stored on `BladeRf1`. All flash-related queries (`size_bytes`, `total_pages`, `total_sectors`, `fpga_flash_sectors`, `fpga_flash_bytes`) live on `FlashSession`.

### Session transition rules

All transitions are unrestricted **except:** entering `FlashSession` or `ConfigSession` while `active_streams > 0` returns `Error::StreamsActive`. This prevents switching the USB alt setting away from RfLink while streaming endpoints are in use. All other transitions (RfLink↔Flash, RfLink↔Config, Flash↔Config) are allowed. `rf_link_session()` requires no special handling — if streams are active, the device is already in RfLink mode and the existing skip-if-already-correct optimization avoids a redundant `usb_change_setting` call.

### Directory layout

| Path | Purpose |
|------|---------|
| `src/bladerf1/board.rs` | `BladeRf1` struct, constructor, `Drop`, factory methods, session constructors |
| `src/bladerf1/board/*.rs` | Board-level operations (frequency, gain, bandwidth, stream, corrections, etc.) |
| `src/bladerf1/hardware/lms6002d/` | LMS6002D RF transceiver driver (frequency, gain, filters, loopback, DC cal) |
| `src/bladerf1/hardware/si5338.rs` | Si5338 clock generator (sample rate, VCTCXO trim) |
| `src/bladerf1/hardware/dac161s055.rs` | DAC161S055 VCTCXO trim DAC |
| `src/bladerf1/hardware/spi_flash.rs` | `FlashMeta` struct + `impl FlashSession` block (no `SpiFlash` wrapper) |
| `src/bladerf1/protocol/` | BladeRF1-specific NIOS packet encode/decode (retune) |
| `src/protocol/` | Generic NIOS packet encode/decode (8x8, 8x16, 8x32, 32x32) |
| `src/nios_client.rs` | `NiosCore` — all register I/O goes through here |
| `src/usb.rs` | `UsbTransport` — concrete nusb wrapper, USB vendor commands |
| `src/channel.rs`, `src/error.rs`, `src/version.rs`, `src/range.rs`, `src/flash.rs` | Pure data types |
| `src/bladerf2.rs` | **Stub only, not implemented** |

`src/hardware.rs` and `src/board.rs` are re-export modules only.

### Atomic read-modify-write

- `RfLinkSession::config_gpio_modify(&mut self, f)` — read config GPIO, apply closure, apply speed-dependent DMA mask, write back.
- `NiosCore::nios_config_modify(&mut self, f)` — same pattern at NiosCore level.

### Streaming model

- `RxStream::builder(&mut BladeRf1)` — borrows `&mut BladeRf1` during construction. Stream holds only the streaming `Endpoint` and buffer pool — no `NiosCore` reference.
- `close(&mut self, dev: &mut BladeRf1) -> Result<()>` — full teardown: cancel transfers, disable streaming module (RFFE + USB), drain cancelled, clear halt, deconfigure format GPIO bits. Takes `&mut BladeRf1` so the borrow checker prevents concurrent operations during teardown. Teardown logic is unified in `BladeRf1::close_stream()`, shared by both Rx and Tx.
- No `Drop` impl on streams. `close()` is the only way to cleanly tear down a stream. If a stream is dropped without closing, the `BufferPool` and its `Endpoint` drop naturally, but the streaming module stays enabled and format GPIO bits remain set. The next `initialize()` will recover.
- `TxStream` follows the same pattern.

### Drop for BladeRf1

Best-effort disable of RX/TX modules via `self.nios.usb_enable_module()`. `NiosCore` drops, which drops `UsbTransport`, which releases the nusb `Interface`.

## Feature flags

| Flag | Default | Effect |
|------|---------|--------|
| `bladerf1` | yes (via xb*) | BladeRF1 support (x40/x115) |
| `bladerf2` | no | BladeRF2 support (xA4/xA9) — **stub only, not implemented** |
| `xb100` | yes | XB-100 expansion board (implies `bladerf1`) |
| `xb200` | yes | XB-200 expansion board (implies `bladerf1`) |
| `xb300` | yes | XB-300 expansion board (implies `bladerf1`) |

Default features enable all three expansion board features (which each imply `bladerf1`).

## C reference implementation

`resources/bladeRF/` contains the C libbladeRF source. Key paths when porting logic:

| C file | Rust module | Purpose |
|--------|-------------|---------|
| `host/libraries/libbladeRF/src/board/bladerf1/bladerf1.c` | `src/bladerf1/board/*.rs` | Board-level API wrappers |
| `host/libraries/libbladeRF/src/driver/si5338.c` | `src/bladerf1/hardware/si5338.rs` | Si5338 clock generator |
| `host/libraries/libbladeRF/src/driver/smb_clock.c` | `src/bladerf1/hardware/si5338.rs` | SMB mode switching (register writes) |
| `host/libraries/libbladeRF/src/driver/spi_flash.c` | `src/bladerf1/hardware/spi_flash.rs` | SPI flash access |
| `fpga_common/src/lms.c` | `src/bladerf1/hardware/lms6002d/` | LMS6002D RF transceiver |
| `host/libraries/libbladeRF/src/board/bladerf1/flash.c` | `src/flash.rs` | Flash size decode, calibration |

## Design decisions

- **No `Transport` trait or `MockTransport`.** Valuable tests are the protocol encode/decode tests in `tests/unit/`.
- **`NiosCore` is concrete** (not generic over transport). Holds `UsbTransport` directly.
- **No `Arc<Mutex<>>`.** The borrow checker enforces NIOS protocol serialization. `BladeRf1` owns `NiosCore` directly; `&mut self` on `BladeRf1` gives exclusive access.
- **Each struct cleans up its own resources.** `BladeRf1::close_stream()` handles stream teardown. `BladeRf1::drop()` disables modules. No cross-struct teardown routing.
- **`speed: Speed` not stored.** Device speed is read from `self.nios.transport().speed()` when needed. It is immutable for the connection lifetime but not cached as a field.
- **`SuperPlus` handled same as `Super`.** Both clear the small DMA transfer bit in GPIO config.
- **No `SpiFlash` wrapper.** `spi_flash.rs` contains `FlashMeta` and an `impl FlashSession` block — there is no separate `SpiFlash<'a>` struct.
- **`FlashMeta` owned by `FlashSession`.** Constructed inside `flash_session()` from a USB vendor query, not stored on `BladeRf1`. Flash queries (`size_bytes`, `fpga_flash_sectors`, etc.) are on `FlashSession` only.
- **No `Drop` on streams.** `close(&mut self, dev: &mut BladeRf1)` is the only way to cleanly tear down a stream. This avoids doing hardware I/O in a `Drop` impl without access to `&mut BladeRf1`.
- **Unified `close_stream()`.** `BladeRf1::close_stream(channel, pool)` contains all teardown logic, shared by both `RxStream::close()` and `TxStream::close()`.
- **Stream-active counter.** `NiosCore` tracks `active_streams: u8`. `flash_session()` and `config_session()` return `Error::StreamsActive` if any stream is running. `rf_link_session()` requires no special handling — if streams are active, the device is already in RfLink mode and the existing skip-if-already-correct optimization avoids a redundant `usb_change_setting`. Streams increment the counter on `build()`, decrement on `close()`. If a stream is dropped without `close()`, the counter stays elevated — consistent with the existing "no `Drop` on streams" principle.
- **`perform_format_config` / `perform_format_deconfig` are global.** The format GPIO bits (PACKET, TIMESTAMP, 8BIT_MODE, HIGHLY_PACKED) are global, not per-channel. These methods do not take a `channel` parameter.
- **GPIO-based init state check, not a cached flag.** `RfLinkSession::require_initialized()` reads the config GPIO register and checks `(cfg & 0x7f) != 0`. This matches the C library's `CHECK_BOARD_STATE` pattern. A cached `initialized: bool` flag on `NiosCore` was tried and rejected because `initialize()` calls guarded methods internally (e.g. `set_frequency`, `set_gain_mode`), creating a circular dependency: the flag is `false` until the end of `initialize()`, but guarded sub-operations need it `true`. Working around this required setting the flag early and clearing on failure — a fragile pattern. The GPIO check eliminates the problem entirely: `initialize()` writes `0x57` to GPIO first, so subsequent `require_initialized()` calls naturally see the initialized state. No ordering issue, no flag management, no `mark_uninitialized()` needed at de-init sites (FPGA reload resets NIOS, which clears GPIO to `0x00`). The extra USB roundtrip per guard check is negligible — every guarded method already does USB I/O.

## Style conventions

- **`&mut self` methods on hardware structs, not free functions.** If a function takes its "subject" as the first `&mut T` parameter, it should be `&mut self` on `T`. Keep it in the same module.
- **`&mut self` for in-place mutation (especially `Copy` types), `self -> T` for pure queries.** E.g. `GainStage::gain_range(self) -> Range`, `SampleFormat::requires_timestamps(self) -> bool`.
- **No comments** unless explicitly requested.

## Integration test pattern

All hardware integration tests share a single `LazyLock<Mutex<BladeRf1>>` in `tests/common/mod.rs`. This is why `--test-threads=1` is mandatory — tests run sequentially against one physical device. The mutex recovers from poison (a panicked test logs a warning and recovers the guard).

Test helpers (backup/restore) use extension traits defined in each test file since `BladeRf1` is a foreign type. E.g. `trait CalBackup { fn backup_cal(&mut self) -> ...; fn restore_cal(&mut self, ...); }` implemented for `BladeRf1`.

## Gotchas

- **Rust bitwise operator precedence.** `&` and `|` bind looser than `==`/`!=`. `val & mask != 0` parses as `val & (mask != 0)` — always use `(val & mask) != 0`. Same for `|` with comparisons.
- **Formatting requires nightly.** `rustfmt.toml` enables `format_code_in_doc_comments=true`, which is nightly-only. Use `rustup run nightly -- cargo fmt`.
- **`.cargo/config.toml` sets `target-cpu=native`.** Builds are machine-specific. Remove this flag if distributing binaries.
- **`bladerf2` feature is a stub.** `src/bladerf2.rs` exists but is not implemented. Don't try to use it.
- **Integration tests require `--test-threads=1`.** `.cargo/config.toml` sets `test.threads = 1` but explicit `-- --test-threads=1` is still recommended.
- **`check.sh` has `cargo clean` commented out.** It does NOT delete `Cargo.lock` or clean the target dir by default.
- **Release process uses `cargo-release`.** Config in `release.toml`. Pre-release hook runs `check.sh`. See `MAINTAINERS.md` for full workflow. Publish is manual (`publish = false` in `release.toml`).
- **No OTP region access.** Manufacturing-only operations. Don't add them.
- **No gain calibration.** The table-based gain calibration API is bladeRF2-specific. BladeRF1 has no calibration tables.
- **Stream build/teardown bench requires `BatchSize::PerIteration`.** `iter_batched` with `SmallInput` (the default) calls setup multiple times before running any routine, collecting results into a `Vec`. Since `BladeRf1::from_first()` claims the USB interface exclusively, the second setup call fails with EBUSY. `PerIteration` forces batch_size=1 so each setup→routine→close cycle completes before the next setup.
- **Cached state flags that guard methods called during initialization create circular dependencies.** If `require_initialized()` checks a flag that `initialize()` sets, and `initialize()` calls guarded methods, the flag must be set before those calls — but then must be cleared on failure. This is fragile. Prefer hardware-readable state (e.g. GPIO register) when the hardware already tracks the state.
