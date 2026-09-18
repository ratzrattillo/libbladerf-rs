# Code quality follow-up plan

Status: in progress on branch `async-interface`. All five items done (surface, delegation/unsafe, stream seam + lifecycle tests, ErrorKind, deny(missing_docs) + public_api test).

Goal: close the engineering-hygiene gaps identified when comparing
libbladerf-rs with hackrf-nusb / hydrasdr-rs, while keeping the crate's
strengths (layering, borrow-checker ownership, hardware coverage) and
shaping the core so a future bladeRF2 board module can share it.

Guiding constraint: the transport, NIOS protocol, error and streaming core
are board-agnostic and will be shared with bladeRF2. Board modules
(`bladerf1`, later `bladerf2`) sit on top. Public API should therefore be the
board APIs plus stable data types, not the plumbing.

## Order of work

| # | Item | Why this order |
|---|------|----------------|
| 1 | Public surface | Everything after this becomes an internal change, not a semver event. |
| 2 | Collapse the triple `UsbInterfaceCommands` delegation, remove the `unsafe` | Small, mechanical, easier once the traits are crate-private. |
| 3 | Backend seam for streams + lifecycle tests without hardware | The most valuable item; would have caught the stop/close underflow. |
| 4 | `ErrorKind` and targeted variants | Independent; touches many files, so after the refactors above. |
| 5 | `#![deny(missing_docs)]` + `public_api` test | Last, so it locks in the final surface. |

Each step: `cargo test --lib`, `cargo test --test unit`, hardware suites
(default `smol` and `tokio`-only), clippy (stable), fmt, wasm32 check, one
commit.

## 1. Public surface

Measured (before): 348 `pub fn`, 62 `pub(crate) fn`. External users of
internals (examples, tests, benches, seify): `protocol::nios` (unit tests,
benches, fuzz), `range`, `flash`, `bladerf1::protocol::{RetuneTimestamp,
NiosPktRetuneRequest}`, and hardware *data types* only:
`lms6002d::{gain::GainStage, loopback::Loopback, dc_calibration::{DcCalModule,
DcCals}, frequency::get_frequency_min}`, `si5338::RationalRate`.

Changes:

* `usb` and `nios_client` → `pub(crate)`. Nothing outside the crate can reach
  a `NiosCore` or `UsbTransport` anyway (`BladeRf1` has no accessor).
* Chip drivers (`Lms6002d`, `Si5338`, `Dac161s055`): the wrapper structs and
  their I/O methods → `pub(crate)`; their data types, constants and pure
  helpers stay public and are re-exported from `bladerf1::` (already the case
  for most: `GainDb`, `GainStage`, `Band`, `Tune`, `Loopback`, `DcCalModule`,
  `DcCals`, `RationalRate`, …).
* `FlashSession` flash primitives stay public (flash programming is a user
  feature); `FlashMeta` stays `pub(crate)`.
* `protocol`, `bladerf1::protocol`, `range`, `flash`, `channel`, `error`,
  `version` stay public (pure data, stable).
* `pub use nusb;` stays (callers need `nusb::Device` for `from_device`).

## 2. Delegation and `unsafe`

* `UsbInterfaceCommands` / `BladeRf1UsbInterfaceCommands` are implemented for
  `Interface`, `UsbTransport` and `NiosCore` — 28 pure delegation methods.
  Keep the impls on `Interface` only; `UsbTransport` and `NiosCore` expose
  `interface()` and keep the two behaviours that are genuinely theirs
  (`usb_change_setting` releasing NIOS endpoints, `usb_set_firmware_loopback`
  cycling the alt setting) as inherent methods. Call sites change from
  `self.nios.usb_x(..)` to `self.nios.interface().usb_x(..)`.
* `MetadataHeader::from_bytes`: kept as the `#[repr(C, packed)]` +
  `ptr::read_unaligned` read by maintainer preference (a `from_le_bytes`
  version was tried and reverted; it measured identically in
  `metadata_header_full_parse`). This is the crate's single `unsafe` block.

## 3. Stream backend seam and lifecycle tests

Modelled on hackrf-nusb's `BulkInBackend`/`AsyncBulkInBackend`:

```rust
pub(crate) trait BulkEndpoint: Send {
    fn address(&self) -> u8;
    fn max_packet_size(&self) -> usize;
    fn allocate(&self, len: usize) -> Buffer;
    fn submit(&mut self, buffer: Buffer);
    fn pending(&self) -> usize;
    fn poll_next_complete(&mut self, cx: &mut Context<'_>) -> Poll<Completion>;
    #[cfg(not(target_arch = "wasm32"))]
    fn wait_next_complete(&mut self, timeout: Duration) -> Option<Completion>;
    #[cfg(not(target_arch = "wasm32"))]
    fn cancel_all(&mut self);
    fn clear_halt(&mut self) -> impl MaybeFuture<Output = Result<(), nusb::Error>>;
}
impl<Dir: EndpointDirection> BulkEndpoint for nusb::Endpoint<Bulk, Dir> { .. }
```

* `BufferPool<E: BulkEndpoint>`; `usb::drain_pending` generic over `E`.
* The stream state machine moves into `pub(crate) struct StreamCore<E>`
  (pool + `started`), with the read/get_buffer/wait_completion futures and
  start/stop/close logic written against `E` and a `pub(crate) trait
  StreamHost` (implemented by `RfLinkSession`: `enable_module`,
  `stream_started/stopped`, `perform_format_config/deconfig`,
  `require_initialized`, `close_stream`). Public `RxStream`/`TxStream` become
  thin wrappers over `StreamCore<nusb::Endpoint<Bulk, In/Out>>`; their public
  signatures do not change.
* Tests (`#[cfg(test)]` in `stream.rs`, no hardware): a `MockEndpoint`
  (scripted completions, records submits/cancels/clear_halts) and a
  `MockHost` (records module enables, format config, counter). Cover:
  start → read → stop → start → close; close without start; double start
  rejected; stop when not started rejected; counter never underflows;
  read with all buffers held returns `NoTransfersInFlight`; error
  completion recycles the buffer; RX cancel-safety (drop the read future
  mid-flight, next read still completes); TX get_buffer/submit/
  wait_completion round trip; `submit` with `len != buf.len()` recycles and
  errors; teardown order (cancel → disable → drain → clear_halt → deconfig)
  on native and (disable → drain → clear_halt → deconfig) on wasm.
* A small executable lifecycle model (like hackrf's `lifecycle_model.rs`):
  enumerate action sequences over {Start, Read, Stop, Close, Drop, Restart}
  and check the mock against the model (module state, counter, pool
  invariants: `available + pending + held == buffer_count`).

## 4. Errors

* Add `#[non_exhaustive] pub enum ErrorKind { Usb, Transfer, Protocol,
  Timeout, InvalidArgument, Unsupported, NotFound, DeviceState, Stream,
  Calibration, Flash, Io }` and `Error::kind()`; document per variant.
* Replace the string catch-alls that encode program state with variants:
  `NotInitialized`, `StreamAlreadyStarted`, `StreamNotStarted`,
  `NoTransfersInFlight`, `TriggerNotArmed`, `TriggerRoleMismatch`.
  `BoardState(&'static str)` stays for genuine hardware anomalies (VTUNE
  mismatch, invalid register values) and is documented as such.
* `#[non_exhaustive]` on `Error` (already breaking at 0.5).

## 5. Docs and API pinning

* `#![deny(missing_docs)]` (54 items to document, mostly `dc_calibration`,
  `xb300`, `flash`, `error`, `calibration`, `version`, `trigger`).
* `tests/public_api.rs`: compile-time assertions that the documented entry
  points exist with the documented shapes (constructors, sessions, stream
  builders, `MaybeFuture` return, feature-gated items), plus `Send` asserts
  for the main futures.
* `AGENTS.md`: update the "no MockTransport" design decision to "no mock for
  NIOS register I/O; streams have a backend seam", surface rules, error rules.

## Not in scope

* Rewriting the async core as combinators (see discussion: 54 loops in
  I/O paths; async blocks are the right tool).
* bladeRF2 itself.
