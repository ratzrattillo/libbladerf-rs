# libbladerf-rs

Pure Rust driver for the Nuand BladeRF1 SDR. No C libbladeRF dependency. Based on [nusb] for USB transport.

[nusb]: https://github.com/kevinmehall/nusb

Edition 2024, MSRV 1.98.1.

## Commands

All commands run from the **repository root**:

| Action | Command |
|--------|---------|
| Build | `cargo build` |
| Test (no hardware) | `cargo test --lib` |
| Protocol tests (no hardware) | `cargo test --test unit` |
| Public API contract (no hardware) | `cargo test --test public_api --features bladerf1` |
| Run a single unit test | `cargo test --test unit -- <test_name>` |
| Test (with hardware) | `cargo test --features bladerf1 --tests -- --test-threads=1` |
| Async hardware tests only | `cargo test --features bladerf1,tokio --test bladerf1_async -- --test-threads=1` |
| Hardware tests, tokio only | `cargo test --no-default-features --features bladerf1,xb200,tokio --tests -- --test-threads=1` |
| wasm32 check | `cargo +stable check --target wasm32-unknown-unknown --features bladerf1 --lib` |
| Run a single hardware test | `cargo test --features bladerf1 --test bladerf1 -- <test_name>` |
| Clippy | `cargo clippy --all-targets -- -D warnings` |
| Format check | `cargo fmt --all --check` |
| Format (requires nightly) | `rustup run nightly -- cargo fmt` |
| Docs | `cargo doc --features bladerf1 --no-deps --lib --bins --examples` |
| Full CI check (local) | `bash scripts/check.sh` |
| Generate changelog | `bash scripts/changelog.sh` |
| Fuzz | `cargo +nightly fuzz run <target>` |

### Cross-compilation prerequisites

`check.sh` builds for all supported architectures. System linkers must be
installed manually before running the script:

```bash
# Arch Linux
sudo pacman -S aarch64-linux-gnu-gcc mingw-w64-gcc
# Ubuntu/Debian
sudo apt install gcc-aarch64-linux-gnu gcc-mingw-w64-x86-64
```

Rust targets are installed automatically by `check.sh`. The
`aarch64-linux-android` target needs no additional system linker: its build is
`--lib` only (rlib, no link step) and the dependency tree is pure Rust.

### scripts/check.sh vs CI

`scripts/check.sh` pins `RUSTUP_TOOLCHAIN=stable` (attempting `rustup update stable` first, warning if the mirror is unreachable) so results match CI's `@stable` regardless of the local default toolchain; formatting is checked with `+nightly` (rustfmt.toml uses nightly options); and clippy runs on **both** stable (the gate CI enforces) and nightly (early warning: warn-by-default nightly lints become failures on the next stable — fix or `#[allow]` deliberately, never dismiss as "nightly-only"). CI mirrors this with an advisory `clippy-nightly` job (`continue-on-error`). `scripts/check.sh` runs: test (`--features bladerf1 --examples --lib --tests --jobs=1 -- --test-threads=1`) → clippy → fmt → conventional-commits validation (`git-cliff --unreleased --output /dev/null`) → doc (`--lib --bins --examples`) → audit. CI does **not** run `scripts/check.sh` directly. CI runs these steps as individual jobs in `.github/workflows/ci.yml`: fmt → clippy (stable, both feature sets) → clippy-nightly (advisory) → audit → deny → doc → wasm check → build-examples → cross-compile (aarch64, android, windows-gnu) → conventional-commits (`git-cliff --unreleased`, same validation as check.sh) → unit tests on Linux/macOS/Windows (`--lib`, `--test unit`, `--test public_api`; Linux also builds the tokio feature set). `ci.yml` is also a **reusable workflow** (`workflow_call`) — `release.yml` calls it as its gate set instead of duplicating the jobs.

### Release pipeline

`.github/workflows/release.yml` is `workflow_dispatch`-only (a `concurrency` group serializes triggers). Its single gate is a `ci` job that calls `.github/workflows/ci.yml` via `workflow_call` — the exact gate set that runs on every push/PR to main, defined once, no duplication. `bump_and_tag` runs only after `ci` passes and does no checks itself — there is no pre-release hook (`release.toml` has none), so keep all checks in `ci.yml`. It runs `cargo release <level> --execute --no-verify --no-publish --no-push --no-confirm`, refreshes `CHANGELOG.md` (drops the stale `## [unreleased]` section, prepends the just-tagged version's section via `git-cliff --latest --prepend`, amends both into the release commit and re-points the tag, still annotated with its original message — so the tag carries the updated changelog), and then pushes the bump commit **and** the new tag itself (`git push origin HEAD <tag>`, job-level `contents: write`); do not rely on cargo-release's own push. The `chore: Release` housekeeping commit is skipped in the changelog by a `cliff.toml` parser rule. **`--no-confirm` is mandatory in CI**: without it cargo-release prints `Release libbladerf-rs X.Y.Z? [y/N]` (step 1, before any change), reads EOF on the closed stdin as "no", and a declined confirmation is `Err(0)` — it exits **0 without doing anything**. The job goes green, nothing is bumped/committed/tagged, and `github_release`'s `git describe` then resolves the *previous* tag (which is how `gh release create` 422s with "tag_name already exists"). Tag naming: `release.toml` keeps the default empty `tag-prefix` because cargo-release's default tag name template is `{{prefix}}v{{version}}` — setting a prefix there would double the `v` (`vv0.5.0`). The "disabled by user, skipping <example> which has files changed since v0.1.0" warnings during a release are cosmetic: cargo-release releases only the root package by default (the example members are always skipped), and its prior-tag resolution picks the first glob-matching tag (alphabetically `v0.1.0`) rather than the newest. `github_release` and `publish` check out the **branch tip** (`ref: ${{ github.ref_name }}`), not the run's SHA — the bump commit was just pushed, so the run SHA is stale and `git describe` would resolve the *previous* tag. `github_release` creates the GitHub release for the new tag; `publish` checks out the tag and runs `cargo publish` with `CARGO_REGISTRY_TOKEN`. A local `cargo release` has no automatic gate — run `bash scripts/check.sh` first.

### scripts/changelog.sh

`scripts/changelog.sh` runs `git-cliff --unreleased --prepend CHANGELOG.md`, generating changelog entries for unreleased commits and prepending them to `CHANGELOG.md`. It mutates `CHANGELOG.md` in place, so it is a separate on-demand script — not part of `check.sh`. Both scripts `cd` to the repo root, so they work from any directory. It is a local *preview* tool: at release time the pipeline supersedes it by replacing the `## [unreleased]` section with the versioned section (amended into the release commit), so the committed file holds released sections plus at most one `## [unreleased]` preview on top.

### Examples

7 examples are workspace members (build with `-p`):

```bash
cargo build -p info -p calibrate -p dc-cal-table -p rx-tx -p rx-async -p flash-firmware -p flash-fpga
```

Standalone examples (`bench-stream`, `kalibrate`, `kalibrate-5g`) have their own `[workspace]` in `Cargo.toml` and must be built from their own directories.

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
    dc_rx_table: Option<DcCalTable>,
    dc_tx_table: Option<DcCalTable>,
}

pub struct Lms6002d<'a>   { nios: &'a mut NiosCore }
pub struct Si5338<'a>     { nios: &'a mut NiosCore }
pub struct Dac161s055<'a> { nios: &'a mut NiosCore }
```

The chip drivers are `&mut self` wrapper structs (not free functions), constructed per call by `RfLinkSession::lms()`, `si()`, `dac()`. Note: there is **no** `SpiFlash` wrapper. Flash operations are `impl FlashSession` blocks in `spi_flash.rs`. `FlashMeta` is constructed and owned by `FlashSession` on creation, not stored on `BladeRf1`.

Calling convention: `self.lms.method()` → `self.lms().method()` (parentheses added). Direct `NiosCore` access: `self.nios.lock().unwrap().method()` → `self.nios.method()`.

### Session-based USB alt setting model

Operations are grouped into sessions that switch the USB alternate setting:

```rust
pub struct RfLinkSession<'a>  { nios: &'a mut NiosCore, dc_rx_table: Option<&'a DcCalTable>, dc_tx_table: Option<&'a DcCalTable> }
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

`src/bladerf1.rs` is the public re-export surface for the board API. `src/usb.rs` and `src/nios_client.rs` are `pub(crate)`: nothing outside the crate can reach a `NiosCore` or `UsbTransport`. Chip-driver structs (`Lms6002d`, `Si5338`, `Dac161s055`) and their I/O methods are `pub(crate)`; only their data types (`GainDb`, `GainStage`, `Band`, `Loopback`, `DcCalModule`, `RationalRate`, ...) are public.

### Atomic read-modify-write

- `RfLinkSession::config_gpio_modify(&mut self, f)` — read config GPIO, apply closure, apply speed-dependent DMA mask, write back.
- `NiosCore::nios_config_modify(&mut self, f)` — same pattern at NiosCore level.

### Streaming model

- `RxStream::builder(&mut RfLinkSession)` — borrows the session during construction. `build()` claims the streaming endpoint, configures the format GPIO bits, allocates the buffer pool and clears the endpoint halt. It does **not** enable the RF module or submit buffers. The stream holds only the streaming `Endpoint` and buffer pool — no `NiosCore` reference.
- `start(&mut self, dev: &mut RfLinkSession)` — enables the module, increments the stream counter and (RX only) submits all buffers. `stop(&mut self, dev)` — tears down but keeps the pool so the stream can be restarted.
- `close(&mut self, dev: &mut RfLinkSession) -> Result<()>` — full teardown: cancel transfers, disable streaming module (RFFE + USB), drain cancelled, clear halt, deconfigure format GPIO bits. Takes `&mut RfLinkSession` so the borrow checker prevents concurrent operations during teardown. Teardown logic is unified in `RfLinkSession::close_stream()`, shared by both Rx and Tx and by `stop()`.
- No `Drop` impl on streams. `close()` is the only way to cleanly tear down a stream. If a stream is dropped without closing, the `BufferPool` and its `Endpoint` drop naturally, but the streaming module stays enabled and format GPIO bits remain set. The next `initialize()` will recover.
- `TxStream` follows the same pattern (`get_buffer` → fill → `submit` → `wait_completion`).

### Drop for BladeRf1

Best-effort disable of RX/TX modules via `self.nios.usb_enable_module(..).wait()` on native targets only, skipped if `close()` already ran (`closed` flag). `NiosCore` drops, which drops `UsbTransport`, which releases the nusb `Interface`. `BladeRf1::close(self)` is the non-blocking counterpart and the only shutdown path on wasm.

### Sync/async model (`MaybeFuture`)

Every I/O method, at every layer (`BladeRf1`, sessions, streams, `NiosCore`, chip drivers, flash), returns `impl MaybeFuture<Output = Result<T>>` (`nusb::MaybeFuture`, re-exported at the crate root). Callers `.wait()` (native) or `.await`. There is no `_async` twin API.

**Principle: mirror nusb's semantics exactly.** Expose nusb's `MaybeFuture` shape, forward nusb's runtime features (`smol`, `tokio`) under the same names, and never add adapters that change how a nusb operation resolves. If nusb's own example is `list_devices().wait()?.find(..)`, ours should read the same way.

#### Why `Op::new(async move { .. })` — and what it is not

- **It is not a lock or an atomicity mechanism.** Ordering inside a method comes from sequential `.await`s, exactly like sequential blocking calls. Exclusivity against other device I/O comes from the borrow checker: a method's future holds `&mut RfLinkSession` (→ `&mut NiosCore`) until it completes, so no other NIOS packet can be issued meanwhile, from sync or async callers. Other *tasks* may run between our awaits (that is the point of async); they cannot touch this device.
- **The async block is just the state machine** for "several USB round-trips with suspension points". `Op` is the ~20-line adapter that lets the same state machine be driven by `.await` (any executor) or by `.wait()` (`block_on`). There is no second implementation of any method.
- **Why the control plane is async at all, not only streaming:** WebUSB has no blocking calls (`MaybeFuture::wait()` does not exist on wasm; every USB call is a JS promise), so a blocking `set_frequency` cannot exist there — and the sequencing logic (VCO cap search, DC calibration, `initialize`'s ~30 steps, flash erase/verify retries, `wait_until_ready`'s 1 s sleeps) lives in this crate, so it must be awaitable here. "Blocking bodies in libbladerf-rs, async wrapping in seify" works on native only (via `spawn_blocking` + a mutex) and is exactly what the pre-0.5 seify backend did. Even on native, blocking control calls inside a single-threaded runtime (browser tab, FutureSDR scheduler retuning from a message handler) stall everything else for tens of ms to seconds. hackrf-nusb and hydrasdr-rs expose every control method as a `MaybeFuture` for the same reason.
- **Cost:** none measurable. Async blocks compile to unboxed state machines; the sync `.wait()` path measured 170 → 172 µs per NIOS round-trip against the pre-async code.
- **One real semantic difference:** an async caller can cancel a future between awaits (`select!`, timeout, drop), leaving the device where a mid-sequence `Err` + `?` would have left it. Sync callers cannot trigger this deliberately. Methods must therefore not rely on "the rest of me always runs"; anything that needs restoration on early exit already handles the `?` path and thereby the cancel path.
- **Why not combinators (`and_then`) for sequencing:** our operations are dependent through one `&mut self` borrow; the first future holds it, so a continuation closure cannot capture `self` again — `and_then` chains over `&mut self` methods do not type-check. hackrf-nusb can chain because its methods take `&self` over an `Arc`-backed backend. Combinators are used only where they fit: `map`/`map_ok`/`map_err` for a single call plus pure post-processing (see the rule below).

Implementation rules (see `src/maybe_future.rs` and `ASYNC_PLAN.md`):

- Single-call methods return the combinator chain directly: `nusb_op().map_ok(..).map_err(Error::from)`, `self.nios.nios_read(..).map_ok(..)`. `Op::new(async move { .. })` is only for bodies with two or more awaits or control flow between them. Direct delegations return the inner `MaybeFuture` unchanged.
- Inside async blocks, internal calls `.await` the public method directly (`Op<F>: IntoFuture<IntoFuture = F>`, zero cost, no boxing).
- `Op::wait()` runs a crate-private thread-parking `block_on`. It works because nusb completes transfers on its own event thread, `futures-timer` on its own timer thread, and nusb's blocking syscalls (open, claim, alt setting, clear halt, `list_devices` on Windows) on the `smol` (`blocking` crate) or `tokio` (`spawn_blocking`) pool. That is why one of the two features is mandatory on native (`compile_error!` otherwise); `smol` is the default. With `tokio` alone, `Op::wait()` enters a lazily created private runtime context for callers outside tokio (`tokio_context()`), because `spawn_blocking` needs one; awaited use must already be inside a tokio runtime.
- Sleeps use `crate::maybe_future::sleep` (futures-timer; `thread::sleep` for sub-millisecond delays on native). No `std::thread::sleep`, no `Instant` outside `cfg(not(target_arch = "wasm32"))` blocks.
- Closures held across an await need `+ Send` (`config_gpio_modify`, `nios_config_modify`).
- Streaming hot paths (`RxStream::read`, `TxStream::get_buffer`, `TxStream::wait_completion`) are hand-written `Future` structs that also implement `MaybeFuture`: `wait()` runs the original blocking code with timeouts; `poll()` uses `Endpoint::poll_next_complete`. Awaited variants ignore `timeout` and consume one completion per await (matches hackrf-nusb/hydrasdr-rs; seify applies its own timeout).
- Non-blocking probes (`try_read`, `try_get_buffer`, `try_get_completed`, `drain_extras`) use `poll_next_complete` with `Waker::noop()`.
- `let _ = some_io_method();` is a silent no-op (the future is never driven). Always `.wait()`/`.await`. `#[must_use]` on `MaybeFuture` catches bare statements but not `let _`.
- Native-only nusb API: `wait()`, `wait_next_complete`, `cancel_all`. On wasm `close_stream`/`drain` await in-flight transfers instead of cancelling; `Device::speed()` is `None` and is inferred from the bulk endpoint max packet size (`BladeRf1::infer_speed`); `from_bus_addr` is unavailable.
- `cargo +stable check --target wasm32-unknown-unknown --features bladerf1 --lib` must stay green (`.cargo/config.toml` supplies `--cfg=web_sys_unstable_apis`; the default nightly toolchain cannot fetch the wasm target from the configured mirror).
- Streams track `started`; `start()` rejects a started stream, `stop()` requires one, `close()` decrements the active-stream counter only if started, and all data-path calls (`read`, `try_read`, `get_buffer`, `submit`, `wait_completion`) require a started stream so no transfer is ever submitted while the module is off.
- `BufferPool::drain_extras` is bounded by `buffer_count`; a device that completes resubmitted buffers immediately (or a mock) must not be able to livelock the caller.

## Feature flags

| Flag | Default | Effect |
|------|---------|--------|
| `bladerf1` | yes (via xb*) | BladeRF1 support (x40/x115) |
| `bladerf2` | no | BladeRF2 support (xA4/xA9) — **stub only, not implemented** |
| `xb100` | yes | XB-100 expansion board (implies `bladerf1`) |
| `xb200` | yes | XB-200 expansion board (implies `bladerf1`) |
| `xb300` | yes | XB-300 expansion board (implies `bladerf1`) |
| `smol` | yes | `nusb/smol` — blocking syscalls on the `blocking` thread pool |
| `tokio` | no | `nusb/tokio` — blocking syscalls via `spawn_blocking` |

Default features enable all three expansion board features (which each imply `bladerf1`) and `smol`. One of `smol`/`tokio` is required on native targets (compile error otherwise); neither on wasm32.

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

- **`#![deny(missing_docs)]` and `tests/public_api.rs`.** Every public item is documented (the lint is an error). `tests/public_api.rs` is a compile-time contract: it pins constructors, sessions, stream builders, the `MaybeFuture` shape, feature gating and `Send`-ness of the main futures without touching hardware. Update it deliberately when the public API changes.
- **No `MockTransport` for NIOS register I/O.** Register traffic is covered by the protocol encode/decode tests in `tests/unit/`. Streams are different: their lifecycle lives in `StreamCore<E: BulkEndpoint>` driven through a `StreamHost` trait, and `stream.rs` has a `#[cfg(test)]` module with a scripted `MockEndpoint`/`MockHost` plus an exhaustive lifecycle model (`lifecycle_model_holds_for_all_short_sequences`). Any change to start/stop/close/read/teardown semantics must keep those tests green and should add a case.
- **`NiosCore` is concrete** (not generic over transport). Holds `UsbTransport` directly.
- **No `Arc<Mutex<>>`.** The borrow checker enforces NIOS protocol serialization. `BladeRf1` owns `NiosCore` directly; `&mut self` on `BladeRf1` gives exclusive access.
- **Each struct cleans up its own resources.** `RfLinkSession::close_stream()` handles stream teardown. `BladeRf1::drop()` disables modules. No cross-struct teardown routing.
- **Errors: variants for program state, `ErrorKind` for applications.** `Error::kind()` gives the stable coarse category (`ErrorKind::{Usb, Protocol, Timeout, WouldBlock, NotFound, InvalidArgument, Unsupported, State, Hardware, Calibration, Flash, Io, Internal}`); both enums are `#[non_exhaustive]`. Anything that describes *our* state (not initialized, stream not started/already started, nothing in flight, trigger not armed) is its own variant. `BoardState(&'static str)` is reserved for hardware-reported anomalies (VTUNE mismatch, invalid register values); `Internal(&'static str)` for violated invariants (incomplete static tables, overflowing computed register values); `FlashData(&'static str)` for malformed stored data. Do not add new string-typed catch-all uses for state.
- **`speed: Speed` not stored.** Device speed is read from `self.nios.transport().speed()` when needed. It is immutable for the connection lifetime but not cached as a field.
- **`SuperPlus` handled same as `Super`.** Both clear the small DMA transfer bit in GPIO config.
- **No `SpiFlash` wrapper.** `spi_flash.rs` contains `FlashMeta` and an `impl FlashSession` block — there is no separate `SpiFlash<'a>` struct.
- **`FlashMeta` owned by `FlashSession`.** Constructed inside `flash_session()` from a USB vendor query, not stored on `BladeRf1`. Flash queries (`size_bytes`, `fpga_flash_sectors`, etc.) are on `FlashSession` only.
- **No `Drop` on streams.** `close(&mut self, dev: &mut RfLinkSession)` is the only way to cleanly tear down a stream. This avoids doing hardware I/O in a `Drop` impl without access to the session.
- **Unified teardown.** `StreamCore::teardown()` (cancel → disable module → drain → clear halt → deconfigure format bits; no cancel on wasm) is shared by `stop()` and `close()` for both directions.
- **Stream-active counter.** `NiosCore` tracks `active_streams: u8`. `flash_session()` and `config_session()` return `Error::StreamsActive` if any stream is running. `rf_link_session()` requires no special handling — if streams are active, the device is already in RfLink mode and the existing skip-if-already-correct optimization avoids a redundant `usb_change_setting`. Streams increment the counter on `build()`, decrement on `close()`. If a stream is dropped without `close()`, the counter stays elevated — consistent with the existing "no `Drop` on streams" principle.
- **Single `MaybeFuture` API instead of sync + `_async` twins.** Matches hackrf-nusb / hydrasdr-rs so seify's bladerf1 backend can be two thin adapters (`.wait()` / `.await`) over one API. One definition per method, no drift.
- **Runtime agnostic, with nusb's one exception mirrored.** Transfers complete on nusb's own event thread, sleeps use `futures-timer`'s thread, `.wait()` uses our `block_on`; none of this depends on an executor. The single non-agnostic point is nusb's *blocking syscalls* (device open, interface claim, alternate setting, clear halt, `list_devices` on Windows), which nusb offloads through exactly two integrations: the `blocking` crate (nusb feature `smol`) or `tokio::spawn_blocking` (nusb feature `tokio`). We forward both under the same names, like hackrf-nusb and hydrasdr-rs, and require one on native (`compile_error!` otherwise).
  - **`smol` is the default and pulls no runtime.** Despite the name it only adds the `blocking` crate, an executor-agnostic thread pool that works under tokio, smol, async-std, `futures::executor` and `block_on`. `cargo tree -e no-dev --features bladerf1` shows no tokio/smol/async-io. A tokio application can use the defaults and never enable our `tokio` feature.
  - **`tokio` is optional and exists for parity.** It lets a tokio user run nusb's syscalls on tokio's blocking pool. Our only direct tokio dependency (`rt`, optional, native-only) is the ~15-line `tokio_context()` guard in `Op::wait`: `spawn_blocking` needs a runtime context, so sync callers outside tokio get a lazily created private runtime. The dev-dependency on tokio serves `tests/bladerf1_async` and `examples/rx-async` only. Dropping the `tokio` feature entirely (keeping `blocking`) would lose nothing functionally; it is kept for consistency with nusb and the other two drivers. Revisit if "no tokio in the driver's tree" becomes a requirement.
  - A `blocking_op` adapter that ran the syscalls inline on native was tried and removed: it diverged from nusb's semantics and forced async blocks around single nusb calls.
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
- **`target-cpu=native` is a local-only optimization; CI neutralizes it via env.** `.cargo/config.toml` sets `target-cpu=native` for `x86_64-unknown-linux-gnu` so local builds and hardware benches get real performance. But those rustflags also apply to build scripts and proc macros, which run inside the compiler process, and an LLVM codegen bug (rust-lang/rust#141099) makes rustc SIGILL on some CPU models in GitHub's runner pool — a different job fails on every run. Every workflow therefore keeps `CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS: "-C target-cpu=x86-64"` in its top-level `env:`; cargo appends env rustflags *after* the config's and rustc honors the *last* `-C target-cpu`. Keep that env var in any new workflow and do not change the append/last-wins ordering.
- **`bladerf2` feature is a stub.** `src/bladerf2.rs` exists but is not implemented. Don't try to use it.
- **Integration tests require `--test-threads=1`.** `.cargo/config.toml` sets `test.threads = 1` but explicit `-- --test-threads=1` is still recommended.
- **`scripts/check.sh` has `cargo clean` commented out.** It does NOT delete `Cargo.lock` or clean the target dir by default.
- **Release process uses `cargo-release`.** Config in `release.toml`. No pre-release hook — the reusable `ci.yml` workflow (called from `release.yml` via `workflow_call`) must pass before `bump_and_tag`. For a local release, run `scripts/check.sh` first. See `docs/MAINTAINERS.md` for full workflow. `cargo-release` itself does not publish (`publish = false` in `release.toml`) and the pipeline does not rely on its git push either (`--no-push`; `bump_and_tag` pushes the bump commit and the new tag itself); the `.github/workflows/release.yml` `workflow_dispatch` pipeline (`ci` → `bump_and_tag` → `github_release` → `publish`) runs `cargo publish` for CI-driven releases.
- **Conventional Commits enforced by a `commit-msg` hook.** `.husky/commit-msg` validates each message with `git-cliff` (`cliff.toml` sets `require_conventional=true`) via husky-rs (`core.hooksPath = .husky`, installs on `cargo build`/`cargo test`, skip with `NO_HUSKY_HOOKS=1`). `scripts/check.sh` and the `conventional-commits` job in `ci.yml` (which runs on every push/PR to main and gates releases) re-check all unreleased commits as a backstop.
- **No OTP region access.** Manufacturing-only operations. Don't add them.
- **No gain calibration.** The table-based gain calibration API is bladeRF2-specific. BladeRF1 has no calibration tables.
- **Stream build/teardown bench requires `BatchSize::PerIteration`.** `iter_batched` with `SmallInput` (the default) calls setup multiple times before running any routine, collecting results into a `Vec`. Since `BladeRf1::from_first()` claims the USB interface exclusively, the second setup call fails with EBUSY. `PerIteration` forces batch_size=1 so each setup→routine→close cycle completes before the next setup.
- **Cached state flags that guard methods called during initialization create circular dependencies.** If `require_initialized()` checks a flag that `initialize()` sets, and `initialize()` calls guarded methods, the flag must be set before those calls — but then must be cleared on failure. This is fragile. Prefer hardware-readable state (e.g. GPIO register) when the hardware already tracks the state.
