use super::common::*;
use libbladerf_rs::bladerf1::TuningMode;
use libbladerf_rs::range::RangeItem;
use libbladerf_rs::{Channel, Result};

#[test]
fn frequency_tuning() -> Result<()> {
    logging_init("bladerf1_frequency");

    let mut sdr = sdr();
    let mut rf = sdr.rf_link_session()?;
    let accepted_deviation = 1;
    let supported_frequencies = rf.get_frequency_range()?;

    log::trace!("supported_frequencies: {supported_frequencies:?}");
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
