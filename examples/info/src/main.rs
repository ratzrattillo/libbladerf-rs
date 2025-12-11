use anyhow::Result;
use libbladerf_rs::bladerf1::xb::ExpansionBoard;
use libbladerf_rs::bladerf1::xb::ExpansionBoard::XbNone;
use libbladerf_rs::bladerf1::{BladeRf1, GainDb};
use libbladerf_rs::{BLADERF_MODULE_RX, BLADERF_MODULE_TX};

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

    let xb = bladerf.expansion_get_attached()?;
    log::debug!("XB: {xb:?}");
    if xb == XbNone {
        bladerf.expansion_attach(ExpansionBoard::Xb200)?;

        let xb = bladerf.expansion_get_attached()?;
        log::debug!("XB: {xb:?}");
    }

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

    Ok(())
}
