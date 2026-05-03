# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- `SampleFormat` enum with SC16Q11, SC16Q11Meta, SC16Q11Packed, SC8Q7, SC8Q7Meta, PacketMeta variants and pack/unpack helpers
- `MetadataHeader` parsing for timestamps and stream flags
- `SpiFlash` driver with page erase/read/write and calibration cache
- `FlashGuard` with `Drop` impl for panic-safe alt-setting restore
- `Error::TuningFailed` and `Error::MutexPoison` variants
- `khz()`/`mhz()`/`ghz()` const fns for frequency construction
- AGC gain mode support (`GainMode::Default` / `GainMode::Mgc`)
- `config_gpio_write` doc comment explaining `SMALL_DMA_XFER` bit behavior
- `.cargo/config.toml` with `target-cpu=native`
- Benchmarks: NIOS packet, sample format, metadata header, hardware (GPIO, NIOS, tuning, stream latency)
- `flash.rs` with `FpgaSize` enum and BINKV encode/decode
- XB100, XB200, XB300 integration tests
- `cargo-release` config (`release.toml`) with `check.sh` + Tier A benchmarks as pre-release hook
- `pre-release.sh` wrapper script
- `MAINTAINERS.md` with release workflow and pre-flight checklist
- VCTCXO calibration section in README.md

### Changed
- `perform_format_deconfig` now clears `PACKET | TIMESTAMP | TIMESTAMP_DIV2` bits in GPIO
- `Error::Invalid` → `Error::TuningFailed` in all `set_precalculated_frequency`/`tune_vcocap` error paths, with recovery log messages
- `BufferPool::submit_all_available()` replaces `refill_rx_pipeline()` (generalized name)
- Removed dead `completed` field from `BufferPool`
- `rx_tx` example updated to use builder-pattern streaming API
- Edition 2024, MSRV 1.95
- Updated README.md: replaced old manual streaming example with builder-pattern example, fixed `from_bus_addr` signature, trimmed outdated feature lists, fixed test commands

### Fixed
- Bad check in `get_frequency` (ce3ef47)
- Fixed bad device state after running integration tests

## [0.2.0] - 2026-04-06

### Changed
- Complete module restructure: `src/board/bladerf1/` → `src/bladerf1/board/`, new `bladerf1` feature gate
- LMS6002D driver split from monolithic `lms6002d.rs` into per-concern modules (frequency, gain, bandwidth, loopback, filters, dc_calibration)
- NIOS protocol refactored into `protocol/nios/` with generic packet types (8x8, 8x16, 8x32, 32x32, retune)
- New `NiosClient` wrapper over `NiosCore<UsbTransport>`, with `MockTransport` for unit testing
- Streaming API rewritten with `BufferPool` and builder pattern for `RxStream`/`TxStream`
- `Channel` and `Direction` enums introduced, replacing raw module constants
- `Error` type reworked with new variants
- USB transport rewritten (`usb.rs` → `transport/usb.rs`)
- `version` module added
- Removed `nios/packet_retune2.rs` (unused BladeRF2 retune2 protocol)
- Test reorganization: `tests/bladerf1_*.rs` → `tests/integration_bladerf1_*.rs` and `tests/unit_*.rs`

## [0.1.0] - 2026-03-15

### Added
- Initial release