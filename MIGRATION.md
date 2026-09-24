# Migration from 0.5.2 to the 0.6 development API

This checkout contains intentional breaking changes for a coordinated 0.6 release.
The package version remains 0.5.2 until the release workflow runs. Rust 1.98.1 is
the minimum supported compiler.

## Streams remain independently owned

Continue constructing streams with a borrowed `RfLinkSession`; the resulting
RX/TX handles are independent of that borrow. All I/O still uses one
`MaybeFuture` API: `.wait()` on native targets or `.await` on every target.

| Operation | Resource and hardware effects |
|---|---|
| `build()` | Validate sizes/capabilities, claim the direction, clear halt, allocate buffers. |
| `start(&mut rf)` | Establish compatible global format bits, enable the module, submit RX buffers. |
| `stop(&mut rf)` | Finish teardown but retain the endpoint claim and pool for restart. |
| `close(&mut rf)` | Finish teardown and release the endpoint. Further calls report `StreamClosed`. |

Prepared/stopped streams block Flash/Config sessions, forced initialization,
firmware-loopback changes, and device shutdown. A second stream for the same
direction fails early. A session from another device returns `WrongDevice`.
Low-level GPIO/module/format operations respect the same ownership checks.

RX and TX share all format GPIO bits. Compatible peers may run together; an
incompatible start returns `IncompatibleStreamFormat` before changing hardware.
Stopping one peer preserves the other's format. Stopped peers re-establish the
format on restart.

Every successful RX completion is delivered once in order. Recycle every buffer
after processing it. Failed TX completions and rejected submissions return their
buffers to the live pool. Plain SC16 TX lengths must describe complete four-byte
samples, timestamp TX lengths complete messages, and packet TX lengths exactly
one declared packet.

## Cancellation, timeouts, and recovery

- Dropping an RX/TX waiter preserves its transfer and buffer. Data-path timeout
  arguments apply only to `.wait()`; use the executor's timeout for `.await`.
- An interrupted start/stop/close retains its pool, endpoint, and remaining work.
  Keep the stream and retry with the originating device's RF-link session.
  `StreamTransition` prevents data-path use during an incomplete transition.
- Native teardown cancels before disabling, then collects every completion.
  WebUSB cannot cancel: it drains while the module and data source remain active,
  then disables. Trigger-gated RX needs its trigger released; firmware-loopback
  RX needs TX data. No new reads are submitted during teardown.
- After a five-second completion deadline, `StreamDrainIncomplete { pending }`
  reports unfinished teardown. `pending_transfers()` exposes the remaining
  uncollected transfer count. Restore the source and retry; dropping the handle
  does not establish browser transfer completion.
- NIOS transactions, vendor controls, alternate-setting changes, flash page
  operations, and FPGA uploads retain their original backend operations across
  interrupted waits. Conflicting work first settles those operations.
- If synchronization cannot be established, `RecoveryRequired` requests a
  connection recovery boundary. Clearing halt or finding a matching response
  address is not proof that an abandoned NIOS response belongs to a new request.
- Calibration and host tuning preserve temporary-register restoration in the
  device owner. A later operation finishes restoration first. `OperationAndCleanup`
  preserves both the primary error and a failed cleanup.

Already-applied writes remain applied after cancellation. This is not rollback
for persistent configuration or flash programming.

## Shutdown and reset

Use `BladeRf1::shutdown(&mut self)` when shutdown must be retryable after errors
or cancellation. Close all streams first. Success disables the USB modules and
selects the Null alternate setting; the handle can then be dropped. Repeating
successful shutdown is harmless.

`BladeRf1::close(self)` remains a consuming convenience. Its waiter owns the
device, so cancelling it releases the handle. Native `Drop` attempts shutdown
subject to ownership/recovery checks; WebUSB `Drop` performs no I/O.

`device_reset(&mut self)` requires all live stream handles to be gone. It can
recover an abandoned stream, retains an outstanding reset across interrupted
waits, and never automatically repeats an observed reset. Drop the invalidated
handle and reopen the re-enumerated device, including after a reset error whose
effect is uncertain. Normal connection I/O reports `DeviceClosed` afterward.

`is_fpga_configured()` now takes `&mut self` to join serialized transport recovery.

## Calibration values and tables

`DcCals` uses named public `Option<DcCalValue>` fields. `None` leaves that register
unchanged; `Some(DcCalValue::try_from(value)?)` accepts only `0..=63`.

```rust
use libbladerf_rs::bladerf1::{DcCals, DcCalValue};

let updates = DcCals {
    lpf_tuning: Some(DcCalValue::try_from(12)?),
    tx_lpf_i: Some(DcCalValue::try_from(0)?),
    ..DcCals::default()
};
```

`set_dc_cals` takes this value. Replace positional constructors and `-1` sentinels.
JSON accepts null/omitted fields or six-bit integers; unknown fields and legacy
`-1` values are rejected. `get_dc_cals()` returns present values for every field.

`DcCalTable::new(reg_vals, entries)` and `lookup(frequency)` now return `Result`.
Construction and deserialization sort entries and reject duplicate frequencies.
Empty tables yield zero corrections; lookup outside the `u32` frequency domain
returns an argument error rather than wrapping. Entries remain immutable after
validation.

Calibration reports convergence failures separately from USB/protocol failures.
The documented retry from initial code 31 to code 0 is restored. Do not treat
nonconvergence as successful calibration.

## PLL selectors and retune outcomes

`LmsFreq` has validated construction and derives the PLL divisor from its selector.
Its invalid `Default` implementation and independent `x` field are removed.
Convert raw `QuickTune` parameters with `LmsFreq::try_from(quick_tune)?` rather than
`From`; selector, divider width, capacitor value, and flags are checked.

`schedule_retune_with_duration()` returns `(LmsFreq, RetuneResult)` instead of a
tuple containing an unconditional duration. Immediate measured results use
`TimestampTicks`; scheduled/queue-clear acknowledgements carry no measurement.
Only consume duration/VCOCAP where the outcome exposes valid measurements.
`schedule_retune()` continues to return just the validated `LmsFreq`.

`BLADERF1_RX_GAIN_OFFSET` and `BLADERF1_TX_GAIN_OFFSET` are now `i8` constants.
Gain allocation uses bounded integer arithmetic; channel requests outside the
advertised range clamp to its endpoints.

## Metadata and supported formats

Stock BladeRF1 streams `Sc16Q11`, `Sc16Q11Meta`, and version-gated `PacketMeta`.
`Sc8Q7`, `Sc8Q7Meta`, and `Sc16Q11Packed` remain available as data-layout variants
and conversion helpers, but stream construction rejects them. Packet streaming
requires FPGA ≥ 0.12 and firmware ≥ 2.4. Query `supports_format(...).await?`.

Every timestamped SC16 FPGA message has its own 16-byte little-endian header:

| Firmware / FPGA generation | High-Speed | Super/SuperPlus |
|---|---:|---:|
| firmware ≥ 2.5 and FPGA ≥ 0.16 | 4,096 bytes | 8,192 bytes |
| firmware < 2.5 and FPGA < 0.16 | 1,024 bytes | 2,048 bytes |

Mixed generations are unsupported for timestamp streaming. These sizes include
the header; each payload contains `(message_size - 16) / 4` complex samples.
Buffers are rounded up to complete messages. Do not strip just the first header
of a USB transfer.

Obtain `MetadataLayout` from `rf.metadata_layout().await?` or
`rx.metadata_layout()?`, then iterate `layout.messages(&buffer)?`. Each
`MetadataMessage` provides `header()` and `payload()` without sample copying.
`MetadataHeader::to_bytes()` is the portable encoding path. Raw version/flag
accessors retain their wire values; SC16's `0x12344321` prefix is a diagnostic
marker, not a negotiated protocol version.

`MetadataPacket::parse_prefix()` uses the packet length in 32-bit words, excluding
the header, and returns trailing transport bytes separately. Existing
`bladerf1`, `board`, and `board::stream` re-exports of `SampleFormat` and
`MetadataHeader` remain available after their module extraction.

## Flash and protocol validation

- Single-page reads/writes require exactly 256 bytes; multi-page operations
  require complete pages. Empty operations are validated no-ops.
- Every complete page/sector range is checked before side effects. A one-past-end
  start is valid only for an empty range.
- `erase_write_verify()` accepts a page start only at a 64-KiB sector boundary.
  It erases the entire final sector, including an unwritten tail. To preserve
  neighbors, read/modify/program a complete replacement sector first.
- Programming makes one initial attempt and at most three retries, only after
  byte verification mismatches. USB, firmware, and range errors are propagated.
  Verification failures retain their actual offset and expected/actual bytes.
- Completed flash side effects survive cancellation. Retained page-buffer
  transactions settle before reuse; restarting a larger programming request is
  an explicit whole-sector decision.
- Fixed-length USB/NIOS replies and OUT completions are checked exactly. Response
  target/address/direction and command-specific success semantics are validated.
  `NiosNum` is sealed to its supported integer representations.

## Platform integration

Native builds need `smol` (default) or `tokio`; wasm needs neither. With tokio-only,
awaited nusb syscalls run inside a tokio runtime. Driver `.wait()` methods supply
a runtime context for callers outside tokio. Native public handles/futures retain
their thread-safety bounds; WebUSB futures may be local.

Android opens an app-owned connection through `OwnedFd` or `from_device`; duplicate
an FD still owned by Java. Enumeration constructors are unavailable there.
WebUSB needs `--cfg=web_sys_unstable_apis` and browser device permission.

Calibration tables are validated in-memory values. Native filesystem helpers use
host JSON files; Android should select an application directory. WebUSB should
deserialize and call `set_dc_cal_table` directly.
