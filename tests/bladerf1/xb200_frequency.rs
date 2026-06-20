use super::common::*;
use libbladerf_rs::bladerf1::TuningMode;
use libbladerf_rs::bladerf1::{ExpansionBoard, Xb200Path};
use libbladerf_rs::range::RangeItem;
use libbladerf_rs::{Channel, Result};

#[test]
fn frequency_tuning_with_xb200() -> Result<()> {
    logging_init("bladerf1_xb200_frequency");

    let mut sdr = sdr();
    let mut rf = sdr.rf_link_session()?;
    if rf.expansion_get_attached()? != ExpansionBoard::Xb200 {
        rf.expansion_attach(ExpansionBoard::Xb200)?;
    }

    let accepted_deviation = 1;
    let supported_frequencies = rf.get_frequency_range()?;

    log::trace!("supported_frequencies (XB200): {supported_frequencies:?}");
    for range_item in supported_frequencies.iter() {
        let (min, max, _step, _scale) = match range_item {
            RangeItem::Step(min, max, step, scale) => (min, max, step, scale),
            _ => panic!("frequency range item should be Variant of type \"Step\"!"),
        };

        let num_splits = 10.0;
        let offset = ((*max - *min) / num_splits).round() as u64;
        let mut desired = min.round() as u64;

        while desired <= max.round() as u64 {
            for channel in [Channel::Rx, Channel::Tx] {
                let current = rf.get_frequency(channel)?;
                log::trace!("Channel {channel:?} Frequency (CURRENT):\t{current}");
                log::trace!("Channel {channel:?} Frequency (DESIRED):\t{desired}");
                rf.set_frequency(channel, desired, TuningMode::Fpga)?;
                let new = rf.get_frequency(channel)?;
                log::trace!("Channel {channel:?} Frequency (NEW):\t{new}");

                let tolerable_deviation = (new as i64 - desired as i64).abs();
                assert!(tolerable_deviation <= accepted_deviation);

                rf.set_frequency(channel, current, TuningMode::Fpga)?;
            }

            desired += offset;
            desired = desired.clamp(min.round() as u64, max.round() as u64 + 1);
        }
    }

    Ok(())
}

#[test]
fn frequency_range_includes_zero_with_xb200() -> Result<()> {
    logging_init("bladerf1_xb200_frequency");

    let mut sdr = sdr();
    let mut rf = sdr.rf_link_session()?;
    if rf.expansion_get_attached()? != ExpansionBoard::Xb200 {
        rf.expansion_attach(ExpansionBoard::Xb200)?;
    }

    let range = rf.get_frequency_range()?;
    log::trace!("Frequency range with XB200: {range:?}");

    let min_freq = range
        .iter()
        .map(|item| match item {
            RangeItem::Step(min, _, _, _) => *min,
            RangeItem::Value(v) => *v,
            _ => f64::MAX,
        })
        .fold(f64::MAX, f64::min);

    log::trace!("Minimum frequency with XB200: {min_freq}");
    assert_eq!(min_freq, 0.0, "XB200 frequency range should start at 0 Hz");

    Ok(())
}

#[test]
fn frequency_mix_path_below_lms_min() -> Result<()> {
    logging_init("bladerf1_xb200_frequency");

    let mut sdr = sdr();
    let mut rf = sdr.rf_link_session()?;
    if rf.expansion_get_attached()? != ExpansionBoard::Xb200 {
        rf.expansion_attach(ExpansionBoard::Xb200)?;
    }

    let lms_min =
        libbladerf_rs::bladerf1::hardware::lms6002d::frequency::get_frequency_min() as u64;
    let test_freq = lms_min / 2;

    for channel in [Channel::Rx, Channel::Tx] {
        let original_freq = rf.get_frequency(channel)?;
        let original_path = rf.xb200_get_path(channel)?;

        rf.set_frequency(channel, test_freq, TuningMode::Fpga)?;

        let path = rf.xb200_get_path(channel)?;
        log::trace!("Channel {channel:?} at {test_freq}Hz (< LMS min {lms_min}): path = {path:?}");
        assert_eq!(
            path,
            Xb200Path::Mix,
            "Frequencies below LMS min should use Mix path"
        );

        let actual_freq = rf.get_frequency(channel)?;
        let deviation = (actual_freq as i64 - test_freq as i64).abs();
        assert!(deviation <= 1, "Frequency deviation too large: {deviation}");

        rf.xb200_set_path(channel, original_path)?;
        rf.set_frequency(channel, original_freq, TuningMode::Fpga)?;
    }

    Ok(())
}

#[test]
fn frequency_bypass_path_above_lms_min() -> Result<()> {
    logging_init("bladerf1_xb200_frequency");

    let mut sdr = sdr();
    let mut rf = sdr.rf_link_session()?;
    if rf.expansion_get_attached()? != ExpansionBoard::Xb200 {
        rf.expansion_attach(ExpansionBoard::Xb200)?;
    }

    let lms_min =
        libbladerf_rs::bladerf1::hardware::lms6002d::frequency::get_frequency_min() as u64;
    let test_freq = lms_min + 100_000;

    for channel in [Channel::Rx, Channel::Tx] {
        let original_freq = rf.get_frequency(channel)?;
        let original_path = rf.xb200_get_path(channel)?;

        rf.set_frequency(channel, test_freq, TuningMode::Fpga)?;

        let path = rf.xb200_get_path(channel)?;
        log::trace!("Channel {channel:?} at {test_freq}Hz (>= LMS min {lms_min}): path = {path:?}");
        assert_eq!(
            path,
            Xb200Path::Bypass,
            "Frequencies at or above LMS min should use Bypass path"
        );

        let actual_freq = rf.get_frequency(channel)?;
        let deviation = (actual_freq as i64 - test_freq as i64).abs();
        assert!(deviation <= 1, "Frequency deviation too large: {deviation}");

        rf.xb200_set_path(channel, original_path)?;
        rf.set_frequency(channel, original_freq, TuningMode::Fpga)?;
    }

    Ok(())
}
