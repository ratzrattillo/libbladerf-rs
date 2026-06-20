[![Crates.io](https://img.shields.io/crates/v/libbladerf-rs.svg)](https://crates.io/crates/libbladerf-rs)
[![Documentation](https://docs.rs/libbladerf-rs/badge.svg)](https://docs.rs/libbladerf-rs)
[![License](https://img.shields.io/crates/l/libbladerf-rs.svg)](https://github.com/ratzrattillo/libbladerf-rs#license)
[![Build Status](https://github.com/ratzrattillo/libbladerf-rs/workflows/CI/badge.svg)](https://github.com/ratzrattillo/libbladerf-rs/actions)
[![Downloads](https://img.shields.io/crates/d/libbladerf-rs.svg)](https://crates.io/crates/libbladerf-rs)

Pure Rust driver for the Nuand BladeRF1 (x40/x115) SDR. No C libbladeRF dependency.
USB transport via [nusb]. Supports Windows, macOS, and Linux.

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

\* Enabled implicitly by `xb100`, `xb200`, or `xb300`.

## Usage

The device is accessed through **session types** that switch the FX3 USB alternate
setting and borrow `&mut NiosCore`. The borrow checker enforces exclusive access
at compile time.

```rust,ignore
use libbladerf_rs::bladerf1::{BladeRf1, RfLinkSession, RxStream, SampleFormat};

let mut dev = BladeRf1::from_first()?;
let mut sess = dev.rf_link_session()?;
sess.initialize(false)?;

let mut rx = RxStream::builder(&mut sess)
    .buffer_size(65536)
    .buffer_count(8)
    .format(SampleFormat::Sc16Q11)
    .build()?;
rx.start(&mut sess)?;

let buf = rx.read(None)?;
println!("Got {} bytes", buf.len());
rx.recycle(buf);

rx.close(&mut sess)?;
```

### Session types

| Session | USB alt setting | Capabilities |
|---------|----------------|--------------|
| `RfLinkSession` | RfLink (0x01) | Tuning, gain, sample rate, bandwidth, streaming, expansion boards, triggers, loopback, corrections |
| `FlashSession` | SpiFlash (0x02) | SPI flash erase/write/verify, calibration region access |
| `ConfigSession` | Config (0x03) | FPGA loading, device configuration |

`FlashSession` and `ConfigSession` return `Error::StreamsActive` if any stream is running.

## Examples

Git-tracked examples (build and run from the repository root):

| Package | Purpose |
|---------|---------|
| `info` | Basic device info and FPGA version |
| `rx_tx` | Streaming RX/TX with metadata headers |
| `calibrate` | DC calibration on LMS6002D |
| `dc_cal_table` | DC calibration table management |
| `flash_firmware` | FX3 firmware flashing |
| `flash_fpga` | FPGA bitstream flashing |

```bash
cargo run -p info
cargo run -p rx_tx
```

## Supported features

- **RF control**: frequency (host/FPGA tuning, quick-tune), gain (per-stage apportioning,
  gain modes, gain stage control), sample rate (integer and rational), bandwidth, LPF mode,
  RF port selection
- **Streaming**: zero-copy DMA via BufferPool (RX/TX), metadata headers, multiple sample
  formats (Sc16Q11, Sc8Q7, Sc16Q11Packed, *Meta variants), pack/unpack helpers
- **DC calibration**: on-demand LMS6002D calibration, flash-stored JSON calibration tables
  with auto-load on open and frequency-specific apply
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
