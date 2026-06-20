# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.5.0] - 2026-06-20

### Changed
- Removed `Arc<Mutex<NiosCore>>`; `BladeRf1` holds `NiosCore` inline. Hardware wrappers (`Lms6002d`, `Si5338`, `Dac161s055`) are ephemeral `&'a mut NiosCore` borrows, enforcing exclusive access at compile time
- Stream API: `close(&mut self, dev: &mut BladeRf1)` replaces `Drop`-based teardown. Streams hold `Option<BufferPool<Dir>>` directly, no `Arc<Mutex<NiosCore>>`
- USB: `VendorRequest` and `UsbAltSetting` enums replace bare `u8` constants for all vendor commands and alt-setting values
- USB timeout increased from 1s to 3s
- Flash constants reorganized: hardware geometry (`PAGE_SIZE`, `ERASE_BLOCK_SIZE`) and BladeRF1 memory map (`ADDR_*`, `BYTE_LEN_*`) moved to `src/bladerf1/hardware/spi_flash.rs`; `MIN_FW_SIZE` moved to `src/bladerf1/board/firmware.rs`; `FPGA_SIZE_*` and `is_valid_fpga_size()` moved to `src/bladerf1/board/fpga.rs` (all re-exported from `flash.rs` for backward compatibility)
- Default features changed from `["bladerf1"]` to `["xb100", "xb200", "xb300"]`
- `SpiFlash` replaced by `FlashSession` with ephemeral borrow pattern; `FlashMeta` constructed and owned by `FlashSession` on creation, not stored on `BladeRf1`. Methods: `read_pages()`, `write_pages()`, `erase_sectors()`, `verify_pages()`, `size_bytes()`, `total_pages()`, `total_sectors()`
- `UsbTransport` now stores `current_alt_setting` and `speed`; `current_alt_setting()` and `speed()` query cached state; `usb_change_setting()` updates it
- `NiosCore::get_alt_setting()` returns `UsbAltSetting` instead of raw `u8`
- Removed `From<Interface>` impls for `NiosCore` and `UsbTransport`; use `NiosCore::new(UsbTransport::new(iface, speed))`
- `FpgaSize::variant_label()` added for FPGA build name lookup
- Si5338 driver refactored: `Multisynth` struct with `pack_regs()`/`unpack_regs()`/`calculate()` methods replacing free functions; `RationalRate::reduce()`/`double()` as methods instead of free functions
- LMS6002D driver refactored: functions converted from `&mut NiosClient`/`&mut LmsGuard` free functions to `&mut self` methods on `Lms6002d`
- `Range` gain queries now return `Result`: `step_checked()`, `scale_checked()`, `min_checked()`, `max_checked()` replace unwrapping `Option` methods
- `BladeRf1::speed()` delegates to `self.nios.transport().speed()` instead of cached field; `speed` field removed from `BladeRf1`
- Integration tests reorganized into `tests/bladerf1/` and `tests/unit/` with shared `common` module
- Unit tests moved from inline `#[cfg(test)]` modules to `tests/unit/`
- `check.sh`: commented out `cargo clean`
- `rx_tx` example: removed redundant debug logging and added nios_client/usb log filters
- `Error::HardwareState(&'static str)` renamed to `Error::BoardState(&'static str)`
- `BladeRf1::build()` now waits for FX3 firmware readiness and auto-loads DC calibration tables from `<serial>_dc_rx.json`/`<serial>_dc_tx.json`
- `NiosCore` tracks `active_streams: u8`; `flash_session()` and `config_session()` guard against alt-setting changes with `Error::StreamsActive`
- `RfLinkSession::initialize(force: bool)` — force flag to re-initialize an already-initialized device
- Added `serde` and `serde_json` dependencies for DC calibration table serialization

### Added
- Board-level operations: `flash_firmware()`, `device_reset()`, `load_fpga()`, `flash_fpga()`, `erase_stored_fpga()`, `read_fw_log()`, `is_fpga_configured()`, `flash_erase_write_verify()`, `get_fpga_bytes()`, `get_flash_size()`, `fpga_flash_sectors()`, `fpga_flash_bytes()`, `get_fpga_source()`
- New board modules: `firmware`, `flash`, `fpga`, `lpf_mode`, `rf_port`, `smb`, `timestamp`, `trigger`, `vctcxo_tamer`
- New public types: `FpgaSource` enum, `QuickTune`, `TriggerRole`, `TriggerState`, `VctcxoTamerMode`, `FwLogFile`/`FwLogEntry`
- NIOS protocol: `NiosPkt8x64Target::Timestamp`, `NiosPkt8x64TimestampAddr`, `nios_get_timestamp()`
- USB: `usb_is_fpga_configured()`, `usb_begin_fpga_prog()`, `usb_bulk_out()`
- Flash: `FpgaSize::variant_label()`, `pad_to_page()`, `decode_flash_size()`, `is_valid_fpga_size()`, flash memory map constants
- `Error::FlashVerificationFailed`, `Error::BoardState`, `Error::StreamsActive`, `Error::Json` variants
- Benchmarks: `nios_packet_bench`, `sample_format_bench`, `metadata_header_bench`, `hardware_gpio_bench`, `hardware_tuning_bench`, `hardware_stream_latency_bench`, `hardware_stream_build_teardown_bench`, `hardware_gain_bench`, `hardware_calibration_bench`
- Examples: `flash_firmware`, `flash_fpga`, `dc_cal_table`
- Integration tests: `bandwidth`, `correction`, `dc_calibration`, `flash`, `fpga`, `frequency`, `gain`, `loopback`, `open`, `rx_mux`, `sample_rate`, `xb200`, `xb200_frequency`
- Unit tests: `bladerf1_nios_retune`, `dc_cal_table`, `flash`, `nios_packet`, `range`, `sample_format`
- `DcCalTable` struct with JSON file load/save, frequency interpolation lookup, and auto-load at device open
- `BladeRf1::load_dc_cal_table()`, `clear_dc_cal_table()`, `set_dc_cal_table()` — DC calibration table management
- `BladeRf1::from_fd()` — Linux file descriptor constructor
- `BladeRf1::fx3_firmware_version()` — FX3 firmware version query
- `PATTERNS.md` — architectural documentation

### Removed
- `Error::FlashError(u32)` variant — replaced by `Error::FlashVerificationFailed`
- `Error::HardwareState(&'static str)` variant — replaced by `Error::BoardState`
- `Arc<Mutex<NiosCore>>` from `BladeRf1` and all hardware wrappers
- `SpiFlashGuard` and `Deref`-based guard pattern
- `SpiFlash` struct — replaced by `FlashSession` with `FlashMeta`
- `RxStreamInner`/`TxStreamInner` wrapper structs
- `From<Interface>` impls for `NiosCore` and `UsbTransport`
- `BLADERF_FLASH_TOTAL_PAGES`, `BLADERF_FLASH_TOTAL_SECTORS`, `BLADERF_FLASH_CAL_PAGE_START`, `BLADERF_FLASH_CAL_PAGE_END` constants (replaced by runtime `FlashMeta` fields)
- Raw USB command constants (`BLADE_USB_CMD_*`, `USB_IF_*`) — replaced by typed enums
- `speed` field from `BladeRf1` — speed queried via `self.nios.transport().speed()`

### Fixed
- Bitwise operator precedence: `val & mask != 0` patterns corrected to `(val & mask) != 0` throughout protocol, NIOS, and flash code
- `set_sample_rate` now correctly computes the Si5338 multisynth divider values

## [0.4.0] - 2026-05-10

### Changed
- Removed `NiosClient` wrapper; functionality merged into `NiosCore`
- Removed `Transport` trait and `MockTransport`; `NiosCore` is no longer generic, holds `UsbTransport` directly
- Removed `transport` module; `usb.rs` moved from `src/transport/usb.rs` to `src/usb.rs`
- Hardware drivers refactored to guard pattern — `Lms6002d`, `Si5338`, `Dac161s055`, `SpiFlash` each hold `Arc<Mutex<NiosCore>>` with dedicated guards (`LmsGuard`, `Si5338Guard`, `DacGuard`, `SpiFlashGuard`) implementing `Deref<Target = NiosCore>`
- Internal LMS6002D module functions refactored from `&mut NiosClient` to `&mut LmsGuard<'_>`
- `RxStream`/`TxStream` refactored to hold `Arc<Mutex<NiosCore>>` clones — streams manage their own hardware teardown in `close()` and `Drop`
- `RxStream::close()` and `TxStream::close()` no longer require a separate `&mut BladeRf1` argument
- `RxStream::buffer_size()` and `TxStream::buffer_size()` now return `Result`

### Added
- `Error::StreamClosed` variant
- `nios_config_modify()` atomic read-modify-write on `NiosCore`
- Guard pattern for all hardware subsystems

### Removed
- `NiosClient` struct
- `Transport` trait and `MockTransport`
- `transport` module and `common` test helper module

## [0.3.0] - 2026-04-27

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
- `flash.rs` with `FpgaSize` enum and BINKV encode/decode
- XB100, XB200, XB300 integration tests
- `cargo-release` config (`release.toml`) with `check.sh` as pre-release hook
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