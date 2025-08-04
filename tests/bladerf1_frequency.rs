mod common;

use crate::common::*;

use bladerf_globals::range::RangeItem;
use bladerf_globals::{BLADERF_MODULE_RX, BLADERF_MODULE_TX};
use libbladerf_rs::Result;

#[test]
fn frequency_tuning() -> Result<()> {
    logging_init("bladerf1_frequency");

    let supported_frequencies = BLADERF.get_frequency_range()?;

    log::trace!("supported_frequencies: {supported_frequencies:?}");
    for range_item in supported_frequencies.items {
        let (min, max, _step, _scale) = match range_item {
            RangeItem::Step(min, max, step, scale) => (min, max, step, scale),
            _ => panic!("frequency range item should be Variant of type \"Step\"!"),
        };

        // To not go through each possible frequency, we split the range in num_splits parts
        // and tune to each of the desired frequencies.
        let num_splits = 10.0;
        let offset = ((max - min) / num_splits).round() as u64;
        // let mut desired = range_item.min().round() as u32;
        // while desired <= range_item.max().round() as u32 {
        let mut desired = min.round() as u64;
        while desired <= max.round() as u64 {
            for channel in [BLADERF_MODULE_RX, BLADERF_MODULE_TX] {
                // TODO: What channels are supported?
                let current = BLADERF.get_frequency(channel)?;
                log::trace!("Channel {channel} Frequency (CURRENT):\t{current}");
                // let desired = current + 1000;
                log::trace!("Channel {channel} Frequency (DESIRED):\t{desired}");
                // TODO: Why is set frequency requiring a u64 while get frequency returns u32
                BLADERF.set_frequency(channel, desired)?;
                let new = BLADERF.get_frequency(channel)?;
                log::trace!("Channel {channel} Frequency (NEW):\t{new}");
                assert_eq!(new, desired);
            }

            // if let Some(step) = range_item.step() && let Some(scale) = range_item.scale() {
            //     desired += (step * scale).round() as u32;
            // } else {
            //     break;
            // }

            // This adjustment of desired value can be used,
            // when we want to tune to each possible frequency
            // desired += (step * scale).round() as u32;
            desired += offset;
            desired = desired.clamp(min.round() as u64, max.round() as u64);
        }
    }

    // BLADERF.device_reset()
    Ok(())
}

// #[test]
// fn frequency_tuning_xb200() -> Result<()> {
//     logging_init("bladerf1_tuning");
//     for channel in [BLADERF_MODULE_RX, BLADERF_MODULE_TX] {
//         // TODO: What channels are supported?
//         let old_freq = BLADERF.get_frequency(channel)?;
//         log::trace!("Current Frequency:\t{}", old_freq);
//         let desired_freq = 925e5 as u32;
//         log::trace!("Desired Frequency:\t{}", desired_freq);
//         // TODO: Why is set frequency requiring a u64 while get frequency returns u32
//         BLADERF.set_frequency(channel, desired_freq as u64)?;
//         let new_freq = BLADERF.get_frequency(channel)?;
//         log::trace!("New Frequency:\t{}", new_freq);
//         assert_eq!(new_freq, desired_freq);
//     }
//
//     // BLADERF.device_reset()
//     Ok(())
// }
