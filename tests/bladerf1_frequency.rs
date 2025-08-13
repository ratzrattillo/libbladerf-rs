mod common;

use crate::common::*;

use libbladerf_rs::range::RangeItem;
use libbladerf_rs::{BLADERF_MODULE_RX, BLADERF_MODULE_TX, Result};
// use libbladerf_rs::hardware::lms6002d::LmsFreq;

// #[test]
// fn freq_to_lms_freq() -> Result<()> {
//     logging_init("bladerf1_frequency");
//     let frequencies = [2660000000u64];
//
//     for freq in frequencies {
//         let lms_freq: LmsFreq = freq.try_into()?;
//         let restored_freq: u64 = (&lms_freq).into();
//
//         log::error!("{lms_freq:?}");
//         log::error!("{restored_freq}");
//
//         assert_eq!(restored_freq, freq);
//     }
//     Ok(())
// }

#[test]
fn frequency_tuning() -> Result<()> {
    logging_init("bladerf1_frequency");

    let accepted_deviation = 1;
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
        let mut desired = min.round() as u64;

        while desired <= max.round() as u64 {
            for channel in [BLADERF_MODULE_RX, BLADERF_MODULE_TX] {
                // TODO: What channels are supported?
                let current = BLADERF.get_frequency(channel)?;
                log::trace!("Channel {channel} Frequency (CURRENT):\t{current}");
                log::trace!("Channel {channel} Frequency (DESIRED):\t{desired}");
                // TODO: Why is set frequency requiring a u64 while get frequency returns u32
                BLADERF.set_frequency(channel, desired)?;
                let new = BLADERF.get_frequency(channel)?;
                log::trace!("Channel {channel} Frequency (NEW):\t{new}");

                // The conversion from frequency (u64) to LMSFREQ struct (LmsFreq) is not 100% accurate
                // Minor deviations in frequency are thus expected and accepted...
                let tolerable_deviation = (new as i64 - desired as i64).abs();
                assert!(tolerable_deviation <= accepted_deviation);
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
            desired = desired.clamp(min.round() as u64, max.round() as u64 + 1);
        }
    }

    // BLADERF.device_reset()
    Ok(())
}
