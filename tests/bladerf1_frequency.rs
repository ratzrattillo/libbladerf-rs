mod common;

use crate::common::*;

use bladerf_globals::{BLADERF_MODULE_RX, BLADERF_MODULE_TX};
use libbladerf_rs::Result;

#[test]
fn frequency_tuning() -> Result<()> {
    logging_init("bladerf1_frequency");

    // TODO: Test several frequencys by dividing the whole spectrum in a certain amount of frequencies
    // TODO: E.g. offset = ((range_max - range_min) / 10); desired_freq = current_freq + offset;
    // let desired = BLADERF.get_frequency_range()?;

    for channel in [BLADERF_MODULE_RX, BLADERF_MODULE_TX] {
        // TODO: What channels are supported?
        let old_freq = BLADERF.get_frequency(channel)?;
        log::trace!("Current Frequency:\t{}", old_freq);
        let desired_freq = old_freq + 1000;
        log::trace!("Desired Frequency:\t{}", desired_freq);
        // TODO: Why is set frequency requiring a u64 while get frequency returns u32
        BLADERF.set_frequency(channel, desired_freq as u64)?;
        let new_freq = BLADERF.get_frequency(channel)?;
        log::trace!("New Frequency:\t{}", new_freq);
        assert_eq!(new_freq, desired_freq);
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
