# Async interface plan for libbladerf-rs

Status: implemented on branch `async-interface` (Phases 0–4); seify backend (§4) pending.
Target release: 0.5.0 (breaking).

This document is the working plan for adding an async interface to
libbladerf-rs so that seify can offer a bladeRF1 backend in both its sync
(`Device`) and async (`AsyncDevice`, WebUSB) worlds, which FutureSDR consumes.

Related repositories:

| Repo | Path | Role |
|------|------|------|
| libbladerf-rs | `/home/jl/sdr/libbladerf-rs` | bladeRF1 driver on nusb (this repo) |
| seify | `/home/jl/sdr/seify` | SDR HAL; `src/impls/bladerf1.rs` wraps this crate (sync only today) |
| FutureSDR | `/home/jl/sdr/FutureSDR` | consumes seify; `bladerf1 = ["seify/bladerf1"]` already wired |

A BladeRF1 is connected to the development machine; every phase is verified
against hardware.

---

## 1. Findings that drive the design

### 1.1 How seify consumes async drivers

* seify has **no `async` feature**. Async backends compile under
  `any(target_arch = "wasm32", feature = "smol", feature = "tokio")`. The
  `smol`/`tokio` features are *runtime* features forwarded to the upstream
  nusb-based drivers (`hackrf-nusb?/smol`, `hydrasdr-rs?/smol`, ...), which
  forward to `nusb/smol` / `nusb/tokio`.
* Both upstream drivers (`hackrf-nusb` 0.3, `hydrasdr-rs` 0.4) expose **one**
  API where every I/O method returns `impl MaybeFuture<Output = Result<T>>`
  (`nusb::MaybeFuture`: `.wait()` on native, `.await` anywhere).
* seify's `src/impls/hackrf/` and `src/impls/hydrasdr/` are four files each:
  `mod.rs`, `common.rs` (runtime-independent math/selectors/error mapping),
  `sync.rs` (`cfg(not(wasm32))`, calls `.wait()`), `asynchronous.rs`
  (`cfg(any(wasm32, smol, tokio))`, calls `.into_future().await`).
* The async adapters use a "take-out-to-lease" `AsyncSlot` (Mutex on native,
  RefCell on wasm) instead of holding a `MutexGuard` across `.await`.
  hydrasdr's shape fits libbladerf-rs best: the streamer **owns** the upstream
  stream (which owns its USB endpoint), the device handle lives in a
  `Shared<AsyncSlot<Box<Device>>>` and is only leased for control ops and
  `start`/`stop`.
* seify's async streamer traits pass a `timeout_us` but apply their own
  `with_timeout` around the upstream `read(..., None)`; the documented upstream
  contract is "blocking calls honor `timeout`; awaited calls ignore it and
  consume at most one USB completion per await".
* `libbladerf-rs` is currently a `cfg(not(target_arch = "wasm32"))` dependency
  in seify, and `async_registry::unavailable_driver()` explicitly reports that
  `Driver::BladeRf` has no async API.
* FutureSDR's wasm receivers (`examples/spectrum`, `web-spectrum`,
  `zigbee-wasm`, `wlan-wasm`) use only the async seify path:
  `AsyncRegistry::request_permission` → `AsyncBuilder` → `AsyncSource`
  (`AsyncRxStreamer::read().await`). Native FutureSDR blocks use the sync path.

### 1.2 libbladerf-rs today

* Fully blocking. 14 `MaybeFuture::wait()` calls, ~15
  `Endpoint::wait_next_complete` calls, 4 `thread::sleep`s, several
  `Instant`-based deadline loops.
* **All register I/O funnels through two choke points**:
  `UsbTransport::submit()` (paired OUT/IN bulk NIOS packet on EP 0x02/0x82)
  and the vendor control trio (`control_in`/`control_out`/`set_alt_setting`).
  Streaming touches nusb only through 5 calls in `BufferPool`.
* The borrow-checker/session architecture (`BladeRf1 { device, nios }`,
  sessions borrowing `&mut NiosCore`, ephemeral chip wrappers) does not need
  to change; asyncness can live inside the transport with the session layer
  converted mechanically (~170 public methods, almost all 1–3 transport calls).
* No `Rc`/`RefCell`, no recursive functions (async recursion would need
  boxing), no threads. `std::fs` only in DC-cal-table file loading.
* Repo `AGENTS.md` is stale in places: `rust-version = "1.96"`, stream
  builders take `&mut RfLinkSession` and streams have `start()`/`stop()`,
  chip drivers are `&mut self` wrapper structs, `dc_rx_table`/`dc_tx_table`
  fields exist on `BladeRf1`.

### 1.3 nusb 0.2.7 realities

| Fact | Consequence |
|------|-------------|
| Bulk (`Endpoint::submit`/`next_complete`) and control transfers are real wakeable futures, completed by nusb's own event thread on native (control transfers carry their own timeout). | A runtime-free `block_on` (thread-park waker) can drive them. No tokio/smol needed for `.wait()`. |
| `DeviceInfo::open`, `Device::from_fd`, `claim_interface`, `detach_and_claim_interface`, `set_alt_setting`, `clear_halt`, `set_configuration`, `reset`, `release` are `Blocking` wrappers. `.await`-ing them **panics** unless `nusb/smol` or `nusb/tokio` is enabled; with `tokio` it also panics outside a runtime. | These must be resolved with `.wait()` on native (fast ioctls, config-time only) and `.await` on wasm. Never `.await` them on native. |
| `MaybeFuture::wait()`, `Endpoint::cancel_all()`, `wait_next_complete()`, `transfer_blocking()` are `cfg(not(target_arch = "wasm32"))`. | wasm is async-only and has **no transfer cancellation**. |
| `Device::speed()` returns `None` on WebUSB. | Need a speed heuristic on wasm (bulk endpoint `max_packet_size()`: 1024 = Super, 512 = High). |
| `std::thread::sleep` and `Instant::now()` panic on `wasm32-unknown-unknown`. | Async timer needed; deadline loops must not use `Instant` in shared code. |
| `MaybeFuture: IntoFuture<IntoFuture: NonWasmSend> + NonWasmSend` (`Send` on native, blanket on wasm). | Futures must be `Send` on native. `Interface`, `Endpoint`, `Buffer`, `Device` are `Send + Sync`, so this should hold. |
| `MaybeFuture` is a public, unsealed trait; hackrf-nusb implements it for its own types. | We can implement it for a crate-private wrapper. |

---

## 2. Target design

### 2.1 Uniform `MaybeFuture` API, one definition per method

Every I/O method at every layer (`BladeRf1`, `RfLinkSession`, `FlashSession`,
`ConfigSession`, `RxStream`, `TxStream`, `NiosCore`, `Lms6002d`, `Si5338`,
`Dac161s055`, flash primitives) becomes:

```rust
pub fn set_frequency(
    &mut self,
    channel: Channel,
    frequency: u64,
    mode: TuningMode,
) -> impl MaybeFuture<Output = Result<()>> + '_ {
    Op::new(async move {
        // body unchanged, internal calls just `.await`
        self.lms().set_frequency(channel, frequency).await?;
        // ...
        Ok(())
    })
}
```

* Internal callers `.await` the public method (via `IntoFuture`), external
  sync callers `.wait()`. No `_impl` twins, no drift, no doc duplication.
* Method names stay unchanged (no `_async` suffix).
* `nusb::MaybeFuture` is re-exported from the crate root
  (`pub use nusb::MaybeFuture;`) so users can call `.wait()` without importing
  nusb, exactly like hackrf-nusb.
* Pure functions (`SampleFormat::*`, `MetadataHeader::*`, `Range`, gain
  math, `supports_format`, `speed()`, `current_alt_setting()`,
  `get_alt_setting()`, `buffer_size()`, `recycle()`, `try_get_completed()`
  when implemented via `poll_next_complete`) keep their plain signatures.
* Sync ergonomics: `dev.rf_link_session().wait()?.set_frequency(..).wait()?`.

### 2.2 `Op<F>` — the crate-private MaybeFuture wrapper (`src/maybe_future.rs`)

```rust
pub(crate) struct Op<F>(F);

impl<F> Op<F> { pub(crate) fn new(f: F) -> Self }

impl<F: Future> IntoFuture for Op<F> {
    type Output = F::Output;
    type IntoFuture = F;              // zero-cost, no boxing
    fn into_future(self) -> F { self.0 }
}

impl<F: Future + NonWasmSend> MaybeFuture for Op<F> {
    #[cfg(not(target_arch = "wasm32"))]
    fn wait(self) -> F::Output { block_on(self.0) }
}
```

`block_on` is an in-crate ~30-line executor: `std::task::Wake` impl that
`unpark`s the current thread, poll loop with `thread::park()`. It is correct
because nusb completes transfers from its own event thread and `Blocking`
ops are never awaited on native (see 2.3). No `pollster`/`futures` dependency.

Also in this module:

```rust
#[cfg(not(target_arch = "wasm32"))] pub(crate) use std::marker::Send as NonWasmSend;
#[cfg(target_arch = "wasm32")]      pub(crate) trait NonWasmSend {}  // blanket impl
```

### 2.3 nusb's blocking-class operations — mirror nusb, forward its features

(Revised after implementation; the first version used a crate-private
`blocking_op` adapter that ran these syscalls inline on native. It was
removed because it diverged from nusb's semantics and forced async blocks
around single nusb calls.)

`DeviceInfo::open`, `Device::from_fd`, `detach_and_claim_interface`,
`set_alt_setting`, `Endpoint::clear_halt` (and `list_devices` on Windows)
are nusb `Blocking` operations: awaiting them requires nusb's `smol` or
`tokio` feature. libbladerf-rs forwards both under the same names, exactly
like hackrf-nusb and hydrasdr-rs:

* `smol = ["nusb/smol"]` — **default**; the `blocking` thread pool is
  executor-agnostic, so both `.wait()` and `.await` work anywhere.
* `tokio = ["nusb/tokio", "dep:tokio"]` — `spawn_blocking`; `.await` must run
  inside a tokio runtime. Because our `.wait()` is `block_on` over an async
  block (unlike nusb's inline `Blocking::wait`), `Op::wait()` enters a lazily
  created private runtime context when the caller is outside tokio
  (`tokio_context()`), so sync callers keep working with `tokio` alone.
* Neither on native → `compile_error!`. Neither needed on wasm32.

Single nusb calls are returned as combinator chains
(`nusb::list_devices().map_ok(..).map_err(Error::from)`), not wrapped in
`Op::new(async move { .. })`; the async block is reserved for bodies with two
or more awaits.

### 2.4 Timers (`sleep()` helper)

Dependency: `futures-timer = "3"`; under
`[target.'cfg(target_arch = "wasm32")'.dependencies]` with
`features = ["wasm-bindgen"]`.

```rust
pub(crate) async fn sleep(d: Duration) {
    #[cfg(not(target_arch = "wasm32"))]
    if d < Duration::from_millis(1) { std::thread::sleep(d); return; }
    futures_timer::Delay::new(d).await
}
```

Sleeps to convert:

| Location | Today | Becomes |
|----------|-------|---------|
| `src/bladerf1/board.rs` `wait_until_ready` | `thread::sleep(1s)` ×30 | `sleep(1s).await` |
| `src/bladerf1/board/fpga.rs` status poll | `thread::sleep(200ms)` | `sleep(200ms).await` |
| `src/bladerf1/hardware/lms6002d/frequency.rs` `get_vtune` | `sleep(µs)` | `sleep(µs).await` (thread::sleep on native) |
| `src/bladerf1/hardware/lms6002d/frequency.rs` VTUNE poll | `sleep(10µs)` | same |

Deadline loops (`release_endpoints`, `drain_cancelled`, `TxStream::get_buffer`,
`TxStream::wait_completion`) are restructured per 2.5/2.6; any remaining
`Instant` use must be in `cfg(not(wasm32))` code. If a shared-code `Instant`
survives, add `web-time` (drop-in shim) rather than cfg-forking.

### 2.5 Transport (`src/usb.rs`)

* `UsbTransport::submit(timeout)` → async:
  `ep_out.submit(buf); ep_out.next_complete().await; ep_in.submit(buf);
  ep_in.next_complete().await`.
  * Native: race against `futures_timer::Delay(timeout)`; on timeout
    `cancel_all()` both NIOS endpoints, drain, return `Error::Timeout`
    (preserves today's 3 s semantics and keeps the request/response pairing
    intact).
  * wasm: plain await, no timeout (WebUSB has no cancellation; a timed-out
    but still-pending transfer would desync the NIOS protocol).
* Vendor control helpers (`vendor_cmd_in`, `usb_vendor_cmd_out_w_index`,
  device reset) → `.await` the nusb `MaybeFuture` (timeout built in).
* `usb_change_setting` → `release_endpoints().await` then
  `interface.set_alt_setting(..).await`.
* `release_endpoints` → native: `cancel_all` + await completions until
  `pending() == 0` (bounded by a 5 s timer race); wasm: NIOS endpoints never
  have pending transfers (strict request/response, no timeouts), so drop the
  cached endpoints and return.
* `usb_bulk_out` → `submit` + `next_complete().await` (native: timer race +
  cancel).
* `DeviceCommands` string-descriptor helpers → `.await`.
* The `UsbInterfaceCommands` / `BladeRf1UsbInterfaceCommands` traits get
  `-> impl MaybeFuture<Output = Result<..>> + '_` signatures; the three
  impls (`Interface`, `UsbTransport`, `NiosCore`) delegate as today.
* `UsbTransport::new(interface, speed)` unchanged; speed is computed by the
  caller (2.8).

### 2.6 Streaming (`src/bladerf1/board/stream.rs`) — no sync regression

`BufferPool<Dir>` gains async counterparts next to the existing sync ones:

| Sync (native only) | Async (all targets) |
|--------------------|---------------------|
| `wait_completion(timeout)` → `wait_next_complete` | `next_completion()` → `endpoint.next_complete().await` |
| `drain_extras()` (`Duration::ZERO` polls) | `reap_ready()` → `poll_next_complete(&mut Context::from_waker(Waker::noop()))` loop |
| `drain_cancelled()` (`cancel_all` + 5 s deadline) | native: `cancel_all` + await completions with timer race; wasm: await completions until `pending() == 0` |
| `clear_halt()` → `.wait()` | `endpoint.clear_halt().await` |

Hot-path operations get **hand-written dual futures** (not `Op`), so the
sync path runs today's code verbatim and keeps its timeout semantics:

```rust
pub struct RxRead<'a> {
    pool: &'a mut BufferPool<In>,
    timeout: Option<Duration>,
    submitted: bool,
}

impl Future for RxRead<'_> {            // Unpin; hand-written poll
    type Output = Result<Buffer>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if !self.submitted { self.pool.submit_all_available(); self.submitted = true; }
        let completion = ready!(self.pool.endpoint.poll_next_complete(cx));
        completion.status?;
        self.pool.reap_ready();
        Poll::Ready(Ok(completion.buffer))
    }
}
// IntoFuture comes from std's blanket impl for `F: Future`.
impl MaybeFuture for RxRead<'_> {
    #[cfg(not(target_arch = "wasm32"))]
    fn wait(self) -> Result<Buffer> { /* exact current read() body, wait_next_complete(timeout) */ }
}
```

(`impl Trait` in an associated type is nightly-only, so these futures are
named structs with a manual `poll`. They are `Unpin` and hold only
`&mut BufferPool`, so they are `Send` on native.)

Applies to: `RxStream::read`, `TxStream::get_buffer`,
`TxStream::wait_completion`. Everything else on streams (`build`, `start`,
`stop`, `close`) uses `Op`.

Async contract (documented, matches hackrf-nusb/hydrasdr-rs): awaited
`read`/`get_buffer`/`wait_completion` **ignore `timeout`** and consume at most
one completion per await. `next_complete()` is cancel-safe, so callers may
wrap the future in their own timeout (seify's `with_timeout`) and drop it.

`try_read` / `try_get_buffer` / `try_get_completed` become plain sync
functions built on `poll_next_complete` with `Waker::noop()` (work on wasm
too).

`close_stream` (in `board.rs`) ordering:

* native: `cancel_all` → `enable_module(false)` → drain cancelled → clear halt
  → format deconfig (unchanged).
* wasm: `enable_module(false)` → await remaining pending completions → clear
  halt → format deconfig. **Risk:** whether FX3 completes/stalls pending IN
  transfers after `RF_RX` disable is unverified; validate in a browser during
  the seify/FutureSDR step. Mitigation if it hangs: keep at most one RX
  transfer in flight on wasm, or issue a device-side stop that produces a
  short packet.

`RxStream::recycle`, `buffer_size`, `buffer_count` stay sync.

### 2.7 Board and sessions

* `rf_link_session()`, `flash_session()`, `config_session()` →
  `impl MaybeFuture<Output = Result<Session<'_>>> + '_` (they may switch alt
  setting and, for flash, query the flash ID).
* All `impl RfLinkSession` / `FlashSession` / `ConfigSession` methods in
  `src/bladerf1/board/*.rs`, `src/bladerf1/hardware/**`, `spi_flash.rs`
  converted mechanically to `Op::new(async move { .. })`.
* `require_initialized()` stays a GPIO read (async), per the AGENTS.md
  rationale.
* `config_gpio_modify(f)` / `nios_config_modify(f)`: closure bound must be
  `FnOnce(u32) -> u32 + Send` (native) for the future to be `Send`.

### 2.8 `BladeRf1` lifecycle

* Constructors → `impl MaybeFuture<Output = Result<BladeRf1>>`:
  `list_bladerf1()`, `from_first()`, `from_serial()`, `from_bus_addr()`
  (`cfg(not(android))`), `from_fd()` (`cfg(linux|android)`), and **new**
  `from_device(nusb::Device)` (all targets; wasm callers pass
  `nusb::Device::from_js(js_device).await?`, or use `nusb::request_device` /
  `list_devices`).
* `build()`: speed = `device.speed()`; if `None` (WebUSB) infer from the
  claimed interface's bulk IN endpoint `max_packet_size()`
  (1024 → `Super`, 512 → `High`, else `Error::UnsupportedSpeed`).
* `wait_until_ready()` uses `sleep()`.
* DC-cal-table auto-load (`std::fs`) gated `cfg(not(target_arch = "wasm32"))`.
* **`Drop for BladeRf1`**: keep the best-effort blocking module disable on
  native only (`cfg(not(wasm32))`, via `.wait()`). Add
  `pub fn close(self) -> impl MaybeFuture<Output = Result<()>>` that disables
  RX/TX modules and suppresses the `Drop` I/O (e.g. `ManuallyDrop` or a
  `closed` flag) — the non-blocking alternative required by C-DTOR-BLOCK.
  On wasm `close()` is the only teardown path.

### 2.9 Public visibility

`NiosCore`, `Lms6002d`, `Si5338`, `Dac161s055` and the flash primitives stay
public with the same `MaybeFuture` shape (uniform API; seify imports types
from `hardware::lms6002d`). Revisit narrowing to `pub(crate)` only if the
`Send` bounds or doc burden become a problem.

### 2.10 Not in scope

* No `spawn_blocking` facade over the sync API.
* No `Complex32` conversion in `RxStream::read` — stays zero-copy `Buffer`;
  seify keeps its conversion/`pending` carry-over logic.
* No bladeRF2, no OTP, no gain-calibration tables (unchanged policy).

---

## 3. Phases

Each phase ends green on:

```bash
cargo test --lib
cargo test --test unit
cargo test --features bladerf1 --tests -- --test-threads=1     # hardware
cargo clippy --all-targets -- -D warnings
cargo check --target wasm32-unknown-unknown --features bladerf1 --lib
bash scripts/check.sh
```

Commits follow Conventional Commits (husky hook). Suggested one commit per
phase step below; use `feat!:`/`refactor!:` with a `BREAKING CHANGE:` footer
where signatures change.

### Phase 0 — Preparation

- [x] Commit the pending `husky-rs 0.3.3 → 0.4` bump in `Cargo.toml`
      (`chore(deps): ...`).
- [x] Fix stale statements in `AGENTS.md` (rust-version 1.96, stream builder
      takes `&mut RfLinkSession`, `start()/stop()`, chip drivers are wrapper
      structs, `dc_rx_table`/`dc_tx_table`, workspace members). The async
      section is added in Phase 4.
- [x] `rustup target add wasm32-unknown-unknown` (already present on the
      `stable` toolchain; the default `nightly` toolchain cannot fetch it from
      the configured mirror, so run wasm checks with `cargo +stable`). The
      `cargo check --target wasm32-unknown-unknown --features bladerf1 --lib`
      line is added to `scripts/check.sh` and `.github/actions/check` at the
      end of Phase 3, when it turns green.
- [x] nusb's WebUSB backend needs web-sys unstable APIs:
      `.cargo/config.toml` now sets
      `[target.wasm32-unknown-unknown] rustflags = ["--cfg=web_sys_unstable_apis"]`
      (same convention as seify/FutureSDR; downstream crates must do the
      same — document in README in Phase 4).
- [x] Baseline: with that flag nusb compiles for wasm32 and libbladerf-rs
      fails at exactly the 26 `.wait()`/`wait_next_complete`/`cancel_all`
      sites plus `DeviceInfo::bus_id`/`device_address` (absent on WebUSB →
      `from_bus_addr` is `cfg(not(target_arch = "wasm32"))`).
- [x] Dependencies added: `futures-timer = "3"` (+ wasm32 target block with
      `features = ["wasm-bindgen"]`); dev-dependency `tokio` (`rt`, `macros`,
      `time`). `cargo deny check licenses` passes.

### Phase 1 — Core primitives and transport

Files: new `src/maybe_future.rs`, `src/usb.rs`, `src/nios_client.rs`,
`src/lib.rs`.

- [x] `src/maybe_future.rs`: `Op<F>`, `block_on`, `NonWasmSend`,
      `sleep()`, unit tests for `block_on` (ready future,
      future woken from another thread) and `Op` (`wait()` and `.await` via
      a tiny hand-rolled poll).
- [x] `src/lib.rs`: `pub use nusb::MaybeFuture;` (+ `pub use nusb;` if
      wasm callers need `Device::from_js` without adding nusb themselves —
      decide; hackrf-nusb re-exports selectively).
- [x] `src/usb.rs`: convert per 2.5. Keep the constants, enums, and packet
      decode logic untouched. Timer-race helper for native timeouts lives
      here or in `maybe_future.rs`.
- [x] `src/nios_client.rs`: every I/O method → `Op::new(async move { .. })`.
      `nios_read`/`nios_write` generic bounds: add `Send` where needed
      (`NiosNum + Send` already present).
- [x] Done together with Phases 2–3 on the `async-interface` branch
      (`main` never broken). Conversion was tool-assisted: a script wrapped
      `Result`-returning methods in `Op::new(async move {..})` and inserted
      `.await`/`.wait()` at the byte spans rustc reported for opaque-type
      mismatches, iterating to a fixpoint; branch arms and pattern scrutinees
      were fixed by hand.

### Phase 2 — Streams

Files: `src/bladerf1/board/stream.rs`, `close_stream` in
`src/bladerf1/board.rs`.

- [x] `BufferPool`: add async counterparts (2.6); `cfg(not(wasm32))` on the
      sync ones and on `cancel_all`.
- [x] `RxRead`, `TxGetBuffer`, `TxWaitCompletion` dual futures.
- [x] `build`/`start`/`stop`/`close` via `Op`; `close_stream` wasm branch.
- [x] `try_*` functions via `poll_next_complete` + `Waker::noop()`.
- [x] Hardware check: `cargo test --features bladerf1 --test bladerf1 -- stream`
      (existing tests, now with `.wait()`), plus a new async RX/TX smoke test
      under `#[tokio::test]`.
- [x] Measured on hardware (baseline `main` worktree vs. branch):
      `rx_read_latency` 16.384 ms → 16.384 ms, `tx_write_latency` 5.23 µs →
      5.20 µs (stream path identical); `config_gpio_read` 169.7 µs →
      170.2 µs, `config_gpio_write` 168.3 µs → 172.2 µs, `config_gpio_modify`
      336 µs → 345 µs (NIOS round trip via `block_on` + timer race, ≈1–2 %,
      within noise).

### Phase 3 — Board, sessions, chip drivers

Files: `src/bladerf1/board.rs`, `src/bladerf1/board/*.rs` (~20 files),
`src/bladerf1/hardware/lms6002d/*.rs`, `si5338.rs`, `dac161s055.rs`,
`spi_flash.rs`, `src/bladerf1/calibration.rs`.

- [x] Mechanical conversion, file by file, bottom-up: `dac161s055` →
      `si5338` → `lms6002d/*` → `spi_flash` → `board/*.rs` → `board.rs`.
- [x] Replace the 4 sleeps with `sleep().await`.
- [x] Constructors, `from_device`, speed fallback, `close()`, `Drop` cfg
      (2.8). Gate `std::fs` DC-cal loading off wasm.
- [x] Session constructors → `MaybeFuture`.
- [x] `cargo check --target wasm32-unknown-unknown --features bladerf1 --lib`
      is green; enabled in `check.sh` and as a CI job.
- [x] Watch for: futures that are not `Send` (compiler error points at the
      held value); very large futures (`initialize`, `calibrate_dc`,
      `load_fpga`) — if any exceeds a few KB, `Box::pin` just that body
      inside `Op::new`.

### Phase 4 — Call-site migration, tests, docs

- [x] Append `.wait()` at all sync call sites (~386): `examples/*` (workspace
      members and standalone `bench-stream`, `kalibrate`, `kalibrate-5g`,
      `diagnose`, `diag-xb200`), `tests/bladerf1/*`, `tests/common/mod.rs`,
      `benches/hardware_*`.
- [x] New `tests/bladerf1/async_*.rs` (or a module) driven by
      `#[tokio::test(flavor = "current_thread")]` covering: open via
      `from_first().await`, `rf_link_session().await`, `initialize`,
      `set_frequency`/`get_frequency`, sample rate, gain, RX stream
      build/start/read/recycle/stop/close, TX stream build/start/get_buffer/
      submit/wait_completion/close, `close().await`. Reuse the shared device
      singleton pattern (`--test-threads=1` still mandatory); the async tests
      must not hold the sync `MutexGuard` across `.await` — open a separate
      handle or run them in a dedicated test binary that owns the device.
- [x] New example `examples/rx-async` (tokio) mirroring `rx-tx` RX half.
- [x] Docs: README (sync `.wait()` and async `.await` examples, executor
      notes, wasm notes), crate-level docs, `AGENTS.md` async section
      (design rules from §2: `Op`, nusb feature forwarding, dual futures,
      no timeout in async streaming, wasm cfgs), `CHANGELOG.md` via
      `scripts/changelog.sh` with a BREAKING CHANGE entry, `TODO.md`
      (drop the WASM transport item — superseded).
- [x] `bash scripts/check.sh` fully green; `cargo doc --features bladerf1 --no-deps`
      clean; `cargo build` for every `examples/*/Cargo.toml`.
- [ ] Release 0.5.0 per `MAINTAINERS.md`.

---

## 4. Step 2 (separate plan): seify `AsyncBladeRf`

To be planned in detail after 0.5.0 lands; outline so the libbladerf-rs API
is shaped correctly for it:

* Split `src/impls/bladerf1.rs` → `src/impls/bladerf1/{mod,common,sync,asynchronous}.rs`
  following `src/impls/hydrasdr/`.
  * `common.rs`: gain math, ranges, `bladerf_err`, sample-format conversion
    (`Buffer` → `Complex32`, `pending` carry-over), selector/probe helpers.
  * `sync.rs`: today's code with `.wait()` appended.
  * `asynchronous.rs`: `AsyncBladeRf { device_slot: Shared<AsyncSlot<Box<BladeRf1>>>, abandoned_rx/tx slots, .. }`;
    streamers own `RxStream`/`TxStream` and only lease the device for
    `start`/`stop` (they need `&mut RfLinkSession`); `read` = `stream.read(None).await`
    inside seify's `with_timeout`.
* `Cargo.toml`: move `libbladerf-rs` out of the `cfg(not(wasm32))` block once
  the browser teardown risk (2.6) is validated; no feature forwarding needed.
* Register in `AsyncRegistry::default()`, remove `Driver::BladeRf` from
  `unavailable_driver()`, add `webusb_filters` (`0x2CF0:0x5246`).
* FutureSDR: no changes required (`bladerf1 = ["seify/bladerf1"]`).

---

## 4b. Notes from implementation

* A pre-existing bug surfaced by the async tests: `stop()` followed by
  `close()` decremented `active_streams` twice. Streams now carry a
  `started` flag.
* `let _ = io_method();` silently drops the future. All such sites in
  examples were fixed; `#[must_use]` on `MaybeFuture` catches bare
  statements only.
* `UsbTransport::submit` now restores the OUT buffer before checking the
  transfer status and resets the NIOS endpoints on timeout, so a timed-out
  NIOS transaction no longer leaves `buf_out` empty for the next call.
* nightly clippy (1.100) flags `chunks_exact` with constant sizes in
  pre-existing `flash.rs`/`stream.rs`/`spi_flash.rs` code
  (`clippy::chunks_exact_to_as_chunks`); stable (CI) is clean. Follow-up.
* Minimum nusb raised to 0.2.7.

## 5. Risks and open items

| Item | Status / mitigation |
|------|---------------------|
| wasm stream teardown without cancellation (does FX3 complete pending IN URBs after `RF_RX` disable?) | Unverified. Ship compile-clean + design-correct; validate in browser in Step 2. Fallback options in 2.6. |
| Large async state machines on the stack | Box individual bodies at the `Op::new` boundary if needed. |
| `Send` bound failures on native | Fix at the source (closure bounds, no `MutexGuard`/raw pointers across `.await`). |
| `Blocking`-class ops block the executor briefly on native | Accepted; config-time only; documented. |
| `futures-timer` maintenance cadence | Wrapped behind `sleep()`; swappable for `async-io`/`gloo-timers` later. |
| Breaking change for all downstream sync users | 0.5.0, CHANGELOG entry, mechanical `.wait()` migration. |
| Native async NIOS timeout uses timer race + `cancel_all` | Keep a hardware test that provokes a timeout (e.g. NIOS access while FPGA unconfigured) to confirm the endpoints recover. |
