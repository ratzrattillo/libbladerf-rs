[![Crates.io](https://img.shields.io/crates/v/libbladerf-rs.svg)](https://crates.io/crates/libbladerf-rs)
[![Documentation](https://docs.rs/libbladerf-rs/badge.svg)](https://docs.rs/libbladerf-rs)
[![License](https://img.shields.io/crates/l/libbladerf-rs.svg)](https://github.com/ratzrattillo/libbladerf-rs#license)
[![Build Status](https://github.com/ratzrattillo/libbladerf-rs/workflows/CI/badge.svg)](https://github.com/ratzrattillo/libbladerf-rs/actions)
[![Downloads](https://img.shields.io/crates/d/libbladerf-rs.svg)](https://crates.io/crates/libbladerf-rs)

Pure Rust driver for the Nuand BladeRF1 (x40/x115) SDR. No C libbladeRF dependency.
USB transport via [nusb]. Supports Windows, macOS, Linux, Android (via file
descriptor) and WebUSB (`wasm32-unknown-unknown`). Every I/O method can be
used synchronously or asynchronously.

Requires Rust 1.98.1 or newer.

The current development API targets the coordinated 0.6 migration; package
metadata remains 0.5.2 until release. See [MIGRATION.md](MIGRATION.md).

[nusb]: https://github.com/kevinmehall/nusb
[libbladeRF]: https://github.com/Nuand/bladeRF

## Feature flags

| Flag        | Default | Effect                                  |
|-------------|---------|-----------------------------------------|
| `bladerf1`  | yes\*   | BladeRF1 support (x40/x115)            |
| `bladerf2`  | no      | BladeRF2 support — **stub only**       |
| `xb100`     | yes     | XB-100 LED expansion board             |
| `xb200`     | yes     | XB-200 transverter board               |
| `xb300`     | yes     | XB-300 amplifier board                 |
| `smol`      | yes     | nusb `smol` integration (`blocking` thread pool) |
| `tokio`     | no      | nusb `tokio` integration (`spawn_blocking`)     |

Exactly like every nusb-based driver, one of `smol`/`tokio` is required on
native targets: nusb resolves device open, interface claim, alternate-setting
switches and clear-halt through the selected runtime's blocking pool. Neither
is needed on wasm32. If both are enabled nusb uses `smol`.

\* Enabled implicitly by `xb100`, `xb200`, or `xb300`.

## Usage

The device is accessed through **session types** that switch the FX3 USB alternate
setting and borrow `&mut NiosCore`. The borrow checker enforces exclusive access
at compile time.

### Session types

| Session | USB alt setting | Capabilities |
|---------|----------------|--------------|
| `RfLinkSession` | RfLink (0x01) | Tuning, gain, sample rate, bandwidth, streaming, expansion boards, triggers, loopback, corrections |
| `FlashSession` | SpiFlash (0x02) | SPI flash erase/write/verify, calibration region access |
| `ConfigSession` | Config (0x03) | FPGA loading, device configuration |

`FlashSession` and `ConfigSession` return `Error::StreamsActive` while any stream
owns an endpoint, including prepared and stopped streams. Close streams before
switching modes. An abandoned active stream requires connection recovery.

### Sync or async

Every I/O method returns an [`nusb::MaybeFuture`] (re-exported as
`libbladerf_rs::MaybeFuture`). Call `.wait()` to block the current thread, or
`.await` it from async code:

```rust,ignore
use libbladerf_rs::MaybeFuture;
use libbladerf_rs::bladerf1::{BladeRf1, RxStream, TuningMode};
use libbladerf_rs::Channel;

// Blocking (native targets only)
let mut dev = BladeRf1::from_first().wait()?;
let mut rf = dev.rf_link_session().wait()?;
rf.initialize(false).wait()?;
rf.set_frequency(Channel::Rx, 915_000_000, TuningMode::Fpga).wait()?;
let mut rx = RxStream::builder(&mut rf).build().wait()?;
rx.start(&mut rf).wait()?;
let buffer = rx.read(Some(std::time::Duration::from_secs(1))).wait()?;
rx.recycle(buffer);
rx.close(&mut rf).wait()?;
dev.close().wait()?;

// Async — identical calls, `.await` instead of `.wait()`
let mut dev = BladeRf1::from_first().await?;
let mut rf = dev.rf_link_session().await?;
rf.initialize(false).await?;
let mut rx = RxStream::builder(&mut rf).build().await?;
rx.start(&mut rf).await?;
let buffer = rx.read(None).await?;
rx.recycle(buffer);
rx.close(&mut rf).await?;
dev.close().await?;
```

The crate mirrors [nusb]'s semantics exactly: transfers are real futures
completed by nusb's event thread, and the handful of blocking syscalls (device
open, interface claim, alternate-setting switch, clear halt) are offloaded
through nusb's `smol` or `tokio` integration. With the default `smol` feature
both `.wait()` and `.await` work under any executor. With `tokio` instead,
`.await` must run inside a tokio runtime; `.wait()` works anywhere (the crate
enters a private runtime context for callers outside tokio).

Streaming timeouts (`RxStream::read`, `TxStream::get_buffer`,
`TxStream::wait_completion`) apply to the blocking path only. The awaited
futures ignore the timeout argument and retain pending transfers when cancelled,
so wrap them in your executor's timeout
(`tokio::time::timeout`, `gloo_timers`, ...) if you need a deadline.
RX reads deliver every completion in order; TX completion waits drain the queued
transfers. No ordinary RX read discards completed buffers.

### Stream lifecycle and recovery

- `build()` validates/claims an endpoint, clears halt, and allocates the pool.
- `start()` configures the shared format and enables the module; RX submits buffers.
- `stop()` drains and disables the direction, retaining the endpoint and pool.
- `close()` releases the endpoint after confirmed teardown. Compatible duplex
  peers keep their shared format until the last user stops.

Keep streams after an interrupted start/stop/close and retry the operation with a
session from the same device. A timeout is not evidence of USB completion.
`shutdown(&mut self)` provides the same retryable ownership for device shutdown;
`close(self)` is a consuming convenience. See the migration guide for recovery
boundaries, strict flash inputs, and typed calibration/retune results.

### Metadata

Use `rx.metadata_layout()?.messages(&buffer)?` to visit each timestamped SC16
message. Every message has its own 16-byte little-endian header and sample
payload; a USB buffer usually contains several messages.

| Matched firmware / FPGA | High-Speed message | Super/SuperPlus message |
|---|---:|---:|
| firmware ≥ 2.5, FPGA ≥ 0.16 | 4,096 bytes | 8,192 bytes |
| firmware < 2.5, FPGA < 0.16 | 1,024 bytes | 2,048 bytes |

Mixed generations are rejected for timestamp streaming. `MetadataPacket` uses
the packet header's 32-bit-word payload count and exposes transport padding
separately. `MetadataHeader::to_bytes()` serializes fields portably.

### WebUSB

The crate compiles for `wasm32-unknown-unknown`. nusb's WebUSB backend needs
web-sys unstable APIs, so consumers must add to their `.cargo/config.toml`:

```toml
[target.wasm32-unknown-unknown]
rustflags = ["--cfg=web_sys_unstable_apis"]
```

Obtain a device with `nusb::request_device` (from a user gesture) or
`nusb::list_devices`, then open it with `BladeRf1::from_device(device).await`.
There is no `.wait()` on wasm, `Drop` performs no I/O, and transfers cannot be
cancelled, so always `close()` streams and the device explicitly.
RX teardown must keep its data source available until existing transfers finish.
For firmware loopback, send enough TX data; for triggered RX, release the trigger.
`Error::StreamDrainIncomplete { pending }` retains the stream for retry after a
five-second completion deadline. `pending_transfers()` reports uncollected
transfers. Dropping a stream does not prove that browser requests have stopped.

### Android and calibration storage

Open with `BladeRf1::from_fd(OwnedFd)` or `from_device`; Android has no enumeration
constructors. Duplicate the Java connection's FD before transferring ownership.
Choose an application directory explicitly with `load_dc_cal_tables_from_dir`.
For WebUSB, deserialize validated `DcCalTable` values and install them with
`set_dc_cal_table`; filesystem helpers require an available native filesystem.

## Examples

Git-tracked examples (build and run from the repository root):

| Package | Purpose |
|---------|---------|
| `info` | Basic device info and FPGA version |
| `rx-tx` | Plain SC16 RX and a TX helper with explicit lifecycle |
| `rx-async` | RX streaming with the awaited API on tokio (`--features tokio`) |
| `calibrate` | DC calibration on LMS6002D |
| `dc-cal-table` | DC calibration table management |
| `flash-firmware` | FX3 firmware flashing |
| `flash-fpga` | FPGA bitstream flashing |

```bash
cargo run -p info
cargo run -p rx-tx
cargo run -p rx-async
```

## Supported features

- **RF control**: frequency (host/FPGA tuning, quick-tune), gain (per-stage apportioning,
  gain modes, gain stage control), sample rate (integer and rational), bandwidth, LPF mode,
  RF port selection
- **Streaming**: zero-copy buffers (RX/TX), SC16 and timestamped SC16, version-gated
  `PacketMeta`, and pure packed-SC16 conversion helpers. Stock FPGA streaming
  rejects SC8 and packed SC16.
- **DC calibration**: on-demand LMS6002D calibration, validated host JSON tables
  with optional filesystem auto-load and frequency-specific apply
- **Flash**: erase/write/verify, calibration region (DAC trim, FPGA size)
- **FPGA**: host-based loading, flash autoload, source query, firmware log reading
- **Expansion boards**: XB-100 (GPIO/LED), XB-200 (filter bank, upconverter, auto filter),
  XB-300 (amplifier, TRX, output power)
- **Other**: SMB clock, VCTCXO tamer, triggers, loopback (LMS + FPGA), corrections (DC/phase),
  RX mux, retune scheduling, timestamps, firmware flashing

## Not supported (vs C libbladeRF)

- **BladeRF2** — stub only, not implemented
- **Synchronous API** — `bladerf_sync_config/rx/tx` not implemented
- **Bootloader** — jump to bootloader, load firmware from bootloader
- **OTP (one-time programmable)** — read/write/lock
- **Image helpers** — flash image allocate/free/read/write
- **Byte-level flash** — byte-addressed erase/read/write (page/sector only)
- **Wishbone** — master read/write
- **USB reset on open** — configuration option
- **Multi-device / MIMO** — clock sync helpers
- **Tuning mode get** — missing getter
- **Gain calibration tables** — bladeRF2-specific (not applicable to BladeRF1)

## Developers

Contributions are welcome. The architecture is documented in [`AGENTS.md`](AGENTS.md).
The release and maintenance workflow is documented in [`MAINTAINERS.md`](MAINTAINERS.md).

### Commit messages

Commits must follow [Conventional Commits](https://www.conventionalcommits.org/).
A husky-rs `commit-msg` hook (`.husky/commit-msg`) validates each message with
[`git-cliff`](https://git-cliff.org) (`cargo install git-cliff`). The hook
installs automatically on `cargo build` / `cargo test`; set `NO_HUSKY_HOOKS=1`
to skip installation.

For debugging, compare USB traffic between [libbladeRF] and [libbladerf-rs] using
[Wireshark](https://www.wireshark.org/download.html):

```bash
sudo usermod -a -G wireshark <your_user>
sudo modprobe usbmon
sudo setfacl -m u:<your_user>:r /dev/usbmon*
```

Filter example:

```wireshark
usb.bus_id == 1 and usb.device_address == 2
```

### Datasheets

- [SI5338 Datasheet](https://www.skyworksinc.com/-/media/Skyworks/SL/documents/public/data-sheets/Si5338.pdf)
- [SI5338 Reference Manual](https://www.skyworksinc.com/-/media/Skyworks/SL/documents/public/reference-manuals/Si5338-RM.pdf)
- [LMS6002D Datasheet](https://cdn.sanity.io/files/yv2p7ubm/production/47449c61cd388c058561bfd3121b8a10b3d2c987.pdf)
- [LMS6002D Programming and Calibration Guide](https://cdn.sanity.io/files/yv2p7ubm/production/d20182c51057add570a74bd51d9c1336e814ea90.pdf)
- [DAC161S055 Datasheet](https://www.ti.com/lit/ds/symlink/dac161s055.pdf)

## Documentation

```bash
cargo doc --features bladerf1 --no-deps --lib --bins --examples
```

## Testing

### Unit tests (no hardware)

```bash
cargo test --lib
cargo test --test unit
```

### Integration tests (requires BladeRF1)

```bash
cargo test --features bladerf1 --tests -- --test-threads=1
```

### Specific test

```bash
cargo test --features bladerf1 --test bladerf1 -- frequency -- --test-threads=1
```
