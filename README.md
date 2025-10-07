A reimplementation of basic libbladeRF functions in Rust, based on [nusb] usb backend.
Currently supporting BladeRF1 on Windows, macOS and Linux only.

[nusb]: https://github.com/kevinmehall/nusb

Use [libbladerf-rs] to control your bladeRF1 from your Rust application. This software shall currently not be considered as a replacement for the official [libbladeRF]
due to several features not being available.

[libbladeRF]: https://github.com/Nuand/bladeRF
[libbladerf-rs]: https://github.com/ratzrattillo/libbladerf-rs


## Usage overview

After a BladeRF1 is connected via USB (High or SuperSpeed USB port required) and fully booted,
an instance to a BladeRF can be opened using [`bladerf1::BladeRf1::from_first`].
A handle to a specific BladeRF1 can also be obtained by its [`bladerf1::BladeRf1::from_bus_addr`] or [`bladerf1::BladeRf1::from_serial`].
<!-- or [`bladerf1::BladeRf1::from_fd`] on Android. -->

After obtaining an instance of a [`bladerf1::BladeRf1`], you can set basic parameters like Gain, Frequency
and Sample Rate or Bandwidth.

## Examples
An example exists to demonstrate the current functionality of [libbladerf-rs]:

```rust
use anyhow::Result;
use libbladerf_rs::bladerf1::xb::ExpansionBoard;
use libbladerf_rs::bladerf1::{BladeRf1, GainDb, SampleFormat};
use libbladerf_rs::{BLADERF_MODULE_RX, BLADERF_MODULE_TX, Direction};

fn main() -> Result<()> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .filter_module("nusb", log::LevelFilter::Info)
        .init();

    let bladerf = BladeRf1::from_first()?;

    log::debug!("Speed: {:?}", bladerf.speed());
    log::debug!("Serial: {}", bladerf.serial()?);
    log::debug!("Manufacturer: {}", bladerf.manufacturer()?);
    log::debug!("FX3 Firmware: {}", bladerf.fx3_firmware()?);
    log::debug!("Product: {}", bladerf.product()?);

    let languages = bladerf.get_supported_languages()?;
    log::debug!("Languages: {:x?}", languages);

    bladerf.initialize()?;

    log::debug!("FPGA: {}", bladerf.fpga_version()?);

    let xb = bladerf.expansion_get_attached();
    log::debug!("XB: {xb:?}");

    bladerf.expansion_attach(ExpansionBoard::Xb200)?;

    let xb = bladerf.expansion_get_attached();
    log::debug!("XB: {xb:?}");

    let frequency_range = bladerf.get_frequency_range()?;
    log::debug!("Frequency Range: {frequency_range:?}");

    // Set Frequency to minimum frequency
    bladerf.set_frequency(BLADERF_MODULE_RX, frequency_range.min().unwrap() as u64)?;
    bladerf.set_frequency(BLADERF_MODULE_TX, frequency_range.min().unwrap() as u64)?;

    let frequency_rx = bladerf.get_frequency(BLADERF_MODULE_RX)?;
    let frequency_tx = bladerf.get_frequency(BLADERF_MODULE_TX)?;
    log::debug!("Frequency RX: {}", frequency_rx);
    log::debug!("Frequency TX: {}", frequency_tx);

    let sample_rate_range = BladeRf1::get_sample_rate_range();
    log::debug!("Sample Rate: {sample_rate_range:?}");

    // Set Sample Rate to minimum Sample Rate
    bladerf.set_sample_rate(BLADERF_MODULE_RX, sample_rate_range.min().unwrap() as u32)?;
    bladerf.set_sample_rate(BLADERF_MODULE_TX, sample_rate_range.min().unwrap() as u32)?;

    let sample_rate_rx = bladerf.get_sample_rate(BLADERF_MODULE_RX)?;
    let sample_rate_tx = bladerf.get_sample_rate(BLADERF_MODULE_TX)?;
    log::debug!("Sample Rate RX: {}", sample_rate_rx);
    log::debug!("Sample Rate TX: {}", sample_rate_tx);

    let bandwidth_range = BladeRf1::get_bandwidth_range();
    log::debug!("Bandwidth: {bandwidth_range:?}");

    // Set Sample Rate to minimum Sample Rate
    bladerf.set_bandwidth(BLADERF_MODULE_RX, bandwidth_range.min().unwrap() as u32)?;
    bladerf.set_bandwidth(BLADERF_MODULE_TX, bandwidth_range.min().unwrap() as u32)?;

    let bandwidth_rx = bladerf.get_bandwidth(BLADERF_MODULE_RX)?;
    let bandwidth_tx = bladerf.get_bandwidth(BLADERF_MODULE_TX)?;
    log::debug!("Bandwidth RX: {}", bandwidth_rx);
    log::debug!("Bandwidth TX: {}", bandwidth_tx);

    let gain_stages_rx = BladeRf1::get_gain_stages(BLADERF_MODULE_RX);
    let gain_stages_tx = BladeRf1::get_gain_stages(BLADERF_MODULE_TX);
    log::debug!("Gain Stages RX: {gain_stages_rx:?}");
    log::debug!("Gain Stages TX: {gain_stages_tx:?}");

    let gain_range_rx = BladeRf1::get_gain_range(BLADERF_MODULE_RX);
    let gain_range_tx = BladeRf1::get_gain_range(BLADERF_MODULE_TX);
    log::debug!("Gain Range RX: {gain_range_rx:?}");
    log::debug!("Gain Range TX: {gain_range_tx:?}");

    // Set Sample Rate to minimum Sample Rate
    bladerf.set_gain(
        BLADERF_MODULE_RX,
        GainDb {
            db: gain_range_rx.min().unwrap() as i8,
        },
    )?;
    bladerf.set_gain(
        BLADERF_MODULE_TX,
        GainDb {
            db: gain_range_tx.min().unwrap() as i8,
        },
    )?;

    let gain_rx = bladerf.get_gain(BLADERF_MODULE_RX)?;
    let gain_tx = bladerf.get_gain(BLADERF_MODULE_TX)?;
    log::debug!("Gain RX: {}", gain_rx.db);
    log::debug!("Gain TX: {}", gain_tx.db);

    bladerf.perform_format_config(Direction::Rx, SampleFormat::Sc16Q11)?;

    bladerf.enable_module(BLADERF_MODULE_RX, true)?;

    bladerf.experimental_control_urb()?;

    // bladerf.run_stream()?;

    bladerf.perform_format_deconfig(Direction::Rx)?;

    bladerf.enable_module(BLADERF_MODULE_RX, false)?;

    Ok(())
}
```

Build this example by executing the following command in your shell:
```bash
cargo run --package info
```

## Limitations

[libbladerf-rs] currently only supports the BladeRF1. Support for BladeRF2 is currently not
possible, as I am not in the possession of named SDR.

### Implemented Features
- Getting/Setting gain levels of individual stages like rxvga1, rxvga2, lna, txvga1 and txvga2.
- Getting/Setting RX/TX frequency
- Getting/Setting Bandwidth
- Getting/Setting Sample Rate
- Support for BladeRF1 Expansion boards (XB100, XB200, XB300)
- Interface for sending and receiving I/Q samples

### Missing Features
- Support for BladeRF2
- Support for Firmware and FPGA flashing/validation
- Support for different I/Q sample formats and timestamps
- DC calibration table support
- Usage from both async and blocking contexts (currently sync only)
- Extensive documentation
- AGC enablement

## Developers
Contributions of any kind are welcome!

If possible, method names should adhere to the documented methods in [libbladeRF-doc]

[libbladeRF-doc]: https://www.nuand.com/libbladeRF-doc/v2.5.0/modules.html
[Wireshark]: https://www.wireshark.org/download.html

For debugging purposes, it is useful to compare the communication between the SDR and
the original [libbladeRF] with the communication of [libbladerf-rs].
Hand tooling for this purpose is [Wireshark]. Allow wireshark to monitor USB traffic:

```bash
sudo usermod -a -G wireshark <your_user>
sudo modprobe usbmon
sudo setfacl -m u:<your_user>:r /dev/usbmon*
```

Filter out unwanted traffic by using a Wireshark filter like e.g.

```wireshark
usb.bus_id == 1 and usb.device_address == 2
```

Datasheets for the BladeRF1 hardware are available at the following resources:
### SI5338
[SI5338 Datasheet](https://www.skyworksinc.com/-/media/Skyworks/SL/documents/public/data-sheets/Si5338.pdf)

[SI5338 Reference Manual](https://www.skyworksinc.com/-/media/Skyworks/SL/documents/public/reference-manuals/Si5338-RM.pdf)

### LMS6002D
[LMS6002D Datasheet](https://cdn.sanity.io/files/yv2p7ubm/production/47449c61cd388c058561bfd3121b8a10b3d2c987.pdf)

[LMS6002D Programming and Calibration Guide](https://cdn.sanity.io/files/yv2p7ubm/production/d20182c51057add570a74bd51d9c1336e814ea90.pdf)

### DAC161S055
[DAC Datasheet](https://www.ti.com/lit/ds/symlink/dac161s055.pdf?ts=1739140548819&ref_url=https%253A%252F%252Fwww.ti.com%252Fproduct%252Fde-de%252FDAC161S055)

libbladerf-rs is a pure rust implementation for interacting with a Nuand BladeRF1.


To view the documentation, build it with:
```bash
cargo doc --open
```

Examples on how to use libbladerf-rs can be found in the `examples/` directory

# Testing

### Run tests using the following command

```bash
cargo test -- --test-threads=1
```

### Run tests and display output

```bash
cargo test -- --nocapture --test-threads=1
```

### Run a specific test and display output

```bash
cargo test --test bladerf1_tuning -- --nocapture --test-threads=1
```
