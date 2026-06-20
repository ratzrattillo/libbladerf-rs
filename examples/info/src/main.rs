use anyhow::Result;
use libbladerf_rs::Channel;

use libbladerf_rs::bladerf1::{BladeRf1, RfLinkSession, TuningMode};

fn main() -> Result<()> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .filter_module("nusb", log::LevelFilter::Info)
        .init();

    let mut bladerf = BladeRf1::from_first()?;
    log::debug!("FX3 Firmware: {}", bladerf.fx3_firmware_version()?);
    let mut rf = bladerf.rf_link_session()?;
    rf.initialize(false)?;
    log::debug!("FPGA: {}", rf.fpga_version()?);

    let xb = rf.expansion_get_attached()?;
    log::debug!("XB: {xb:?}");
    // if xb == XbNone {
    //     rf.expansion_attach(ExpansionBoard::Xb200)?;
    //
    //     let xb = rf.expansion_get_attached()?;
    //     log::debug!("XB: {xb:?}");
    // }

    let frequency_range = rf.get_frequency_range()?;
    log::debug!("Frequency Range: {frequency_range:?}");

    // Set Frequency to minimum frequency
    rf.set_frequency(
        Channel::Rx,
        frequency_range.min().unwrap() as u64,
        TuningMode::Fpga,
    )?;
    rf.set_frequency(
        Channel::Tx,
        frequency_range.min().unwrap() as u64,
        TuningMode::Fpga,
    )?;

    let frequency_rx = rf.get_frequency(Channel::Rx)?;
    let frequency_tx = rf.get_frequency(Channel::Tx)?;
    log::debug!("Frequency RX: {}", frequency_rx);
    log::debug!("Frequency TX: {}", frequency_tx);

    let sample_rate_range = RfLinkSession::get_sample_rate_range();
    log::debug!("Sample Rate: {sample_rate_range:?}");

    // Set Sample Rate to minimum Sample Rate
    rf.set_sample_rate(Channel::Rx, sample_rate_range.min().unwrap() as u32)?;
    rf.set_sample_rate(Channel::Tx, sample_rate_range.min().unwrap() as u32)?;

    let sample_rate_rx = rf.get_sample_rate(Channel::Rx)?;
    let sample_rate_tx = rf.get_sample_rate(Channel::Tx)?;
    log::debug!("Sample Rate RX: {}", sample_rate_rx);
    log::debug!("Sample Rate TX: {}", sample_rate_tx);

    let bandwidth_range = RfLinkSession::get_bandwidth_range();
    log::debug!("Bandwidth: {bandwidth_range:?}");

    // Set Sample Rate to minimum Sample Rate
    rf.set_bandwidth(Channel::Rx, bandwidth_range.min().unwrap() as u32)?;
    rf.set_bandwidth(Channel::Tx, bandwidth_range.min().unwrap() as u32)?;

    let bandwidth_rx = rf.get_bandwidth(Channel::Rx)?;
    let bandwidth_tx = rf.get_bandwidth(Channel::Tx)?;
    log::debug!("Bandwidth RX: {}", bandwidth_rx);
    log::debug!("Bandwidth TX: {}", bandwidth_tx);

    let gain_stages_rx = RfLinkSession::get_gain_stages(Channel::Rx);
    let gain_stages_tx = RfLinkSession::get_gain_stages(Channel::Tx);
    log::debug!("Gain Stages RX: {gain_stages_rx:?}");
    log::debug!("Gain Stages TX: {gain_stages_tx:?}");

    let gain_range_rx = RfLinkSession::get_gain_range(Channel::Rx);
    let gain_range_tx = RfLinkSession::get_gain_range(Channel::Tx);
    log::debug!("Gain Range RX: {gain_range_rx:?}");
    log::debug!("Gain Range TX: {gain_range_tx:?}");

    rf.set_gain(Channel::Rx, (gain_range_rx.min().unwrap() as i8).into())?;
    rf.set_gain(Channel::Tx, (gain_range_tx.min().unwrap() as i8).into())?;

    let gain_rx = rf.get_gain(Channel::Rx)?;
    let gain_tx = rf.get_gain(Channel::Tx)?;
    log::debug!("Gain RX: {}", gain_rx.db());
    log::debug!("Gain TX: {}", gain_tx.db());

    Ok(())
}
