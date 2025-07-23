mod common;

use crate::common::*;

use bladerf_globals::{BLADERF_MODULE_RX, BLADERF_MODULE_TX};
use libbladerf_rs::{BladeRf1, Result};

#[test]
fn sample_rate() -> Result<()> {
    logging_init("bladerf1_gain");

    // TODO: The definition of allowed gain rates is still wrong.
    // TODO: Intermediate steps are not allowed, only fixed values

    for channel in [BLADERF_MODULE_RX, BLADERF_MODULE_TX] {
        let range = BladeRf1::get_gain_range(channel);
        for desired in [range.min, range.max] {
            // TODO: What channels are supported?
            let current = BLADERF.get_gain(channel)?;
            log::trace!("Current Gain:\t{current}");
            log::trace!("Desired Gain:\t{desired}");

            BLADERF.set_gain(channel, desired)?;

            let new = BLADERF.get_gain(channel)?;
            log::trace!("New Gain:\t{new}");
            assert_eq!(new, desired);
        }
    }

    // BLADERF.device_reset()
    Ok(())
}
