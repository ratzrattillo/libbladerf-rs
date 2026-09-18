use super::common::*;
use libbladerf_rs::Channel;
use libbladerf_rs::MaybeFuture;
use libbladerf_rs::Result;
use libbladerf_rs::bladerf1::TuningMode;
use libbladerf_rs::bladerf1::{ExpansionBoard, Xb200Filter, Xb200Path};

#[test]
fn xb200_enabled() -> Result<()> {
    logging_init("bladerf1_xb200");
    let mut sdr = sdr();
    let mut rf = sdr.rf_link_session().wait()?;
    if rf.expansion_get_attached().wait()? != ExpansionBoard::Xb200 {
        rf.expansion_attach(ExpansionBoard::Xb200).wait()?;
    }

    let attached = rf.expansion_get_attached().wait()?;
    log::trace!("XB200 enabled:\t{attached:?}");
    assert_eq!(attached, ExpansionBoard::Xb200);

    Ok(())
}

#[test]
fn xb200_path_set_get() -> Result<()> {
    logging_init("bladerf1_xb200");

    let mut sdr = sdr();
    let mut rf = sdr.rf_link_session().wait()?;
    if rf.expansion_get_attached().wait()? != ExpansionBoard::Xb200 {
        rf.expansion_attach(ExpansionBoard::Xb200).wait()?;
    }

    for channel in [Channel::Rx, Channel::Tx] {
        for desired in [Xb200Path::Bypass, Xb200Path::Mix] {
            let current = rf.xb200_get_path(channel).wait()?;
            log::trace!("Channel {channel:?} XB200 Path (CURRENT):\t{current:?}");
            log::trace!("Channel {channel:?} XB200 Path (DESIRED):\t{desired:?}");

            rf.xb200_set_path(channel, desired).wait()?;

            let new = rf.xb200_get_path(channel).wait()?;
            log::trace!("Channel {channel:?} XB200 Path (NEW):\t{new:?}");
            assert_eq!(new, desired);

            rf.xb200_set_path(channel, current).wait()?;
        }
    }

    Ok(())
}

#[test]
fn xb200_filterbank_set_get() -> Result<()> {
    logging_init("bladerf1_xb200");

    let mut sdr = sdr();
    let mut rf = sdr.rf_link_session().wait()?;
    if rf.expansion_get_attached().wait()? != ExpansionBoard::Xb200 {
        rf.expansion_attach(ExpansionBoard::Xb200).wait()?;
    }

    for channel in [Channel::Rx, Channel::Tx] {
        let current = rf.xb200_get_filterbank(channel).wait()?;

        for desired in [
            Xb200Filter::_50M,
            Xb200Filter::_144M,
            Xb200Filter::_222M,
            Xb200Filter::Custom,
        ] {
            log::trace!("Channel {channel:?} XB200 Filterbank (DESIRED):\t{desired:?}");

            rf.xb200_set_filterbank(channel, desired).wait()?;

            let new = rf.xb200_get_filterbank(channel).wait()?;
            log::trace!("Channel {channel:?} XB200 Filterbank (NEW):\t{new:?}");
            assert_eq!(new, desired);
        }

        rf.xb200_set_filterbank(channel, current).wait()?;
    }

    Ok(())
}

#[test]
fn xb200_auto_filter_selection() -> Result<()> {
    logging_init("bladerf1_xb200");

    let mut sdr = sdr();
    let mut rf = sdr.rf_link_session().wait()?;
    if rf.expansion_get_attached().wait()? != ExpansionBoard::Xb200 {
        rf.expansion_attach(ExpansionBoard::Xb200).wait()?;
    }

    let original_rx_filterbank = rf.xb200_get_filterbank(Channel::Rx).wait()?;
    let original_rx_freq = rf.get_frequency(Channel::Rx).wait()?;

    let test_frequencies = [40_000_000u64, 130_000_000u64, 200_000_000u64];
    let filter_modes = [Xb200Filter::Auto1db, Xb200Filter::Auto3db];

    for filter_mode in filter_modes {
        for freq in test_frequencies {
            let channel = Channel::Rx;

            rf.xb200_set_filterbank(channel, filter_mode).wait()?;
            rf.set_frequency(channel, freq, TuningMode::Fpga).wait()?;

            let selected = rf.xb200_get_filterbank(channel).wait()?;
            log::trace!(
                "Channel {channel:?} at {freq}Hz with {filter_mode:?} selected: {selected:?}"
            );
            assert_ne!(selected, filter_mode);
        }
    }

    rf.xb200_set_filterbank(Channel::Rx, original_rx_filterbank)
        .wait()?;
    rf.set_frequency(Channel::Rx, original_rx_freq, TuningMode::Fpga)
        .wait()?;

    Ok(())
}
