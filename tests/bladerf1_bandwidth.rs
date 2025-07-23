mod common;

use crate::common::*;

use bladerf_globals::bladerf1::{BLADERF_BANDWIDTH_MAX, BLADERF_BANDWIDTH_MIN};
use bladerf_globals::{BLADERF_MODULE_RX, BLADERF_MODULE_TX};
use libbladerf_rs::Result;

#[test]
fn enable_logging() {
    logging_init("bladerf1_bandwidth");
}

#[test]
fn bandwidth() -> Result<()> {
    //  logging_init("bladerf1_bandwidth");

    // TODO: The definition of allowed badwidths is still wrong.
    // TODO: Intermediate steps are not allowed, only fixed values
    // let desired = BladeRf1::get_bandwidth_range();

    for desired in [BLADERF_BANDWIDTH_MIN, BLADERF_BANDWIDTH_MAX] {
        for channel in [BLADERF_MODULE_RX, BLADERF_MODULE_TX] {
            // TODO: What channels are supported?
            let current = BLADERF.get_bandwidth(channel)?;
            log::trace!("Current Bandwidth:\t{current}");
            log::trace!("Desired Bandwidth:\t{desired}");

            BLADERF.set_bandwidth(channel, desired)?;

            let new = BLADERF.get_bandwidth(channel)?;
            log::trace!("New Bandwidth:\t\t{new}");
            assert_eq!(new, desired);
        }
    }

    // BLADERF.device_reset()
    Ok(())
}
