mod common;

use crate::common::*;

use bladerf_globals::bladerf1::{BLADERF_SAMPLERATE_MIN, BLADERF_SAMPLERATE_REC_MAX};
use bladerf_globals::{BLADERF_MODULE_RX, BLADERF_MODULE_TX};
use libbladerf_rs::Result;

#[test]
fn sample_rate() -> Result<()> {
    logging_init("bladerf1_sample_rate");

    // TODO: The definition of allowed sample rates is still wrong.
    // TODO: Intermediate steps are not allowed, only fixed values
    // let desired = BladeRf1::get_sample_rate_range();

    for desired in [BLADERF_SAMPLERATE_MIN, BLADERF_SAMPLERATE_REC_MAX] {
        for channel in [BLADERF_MODULE_RX, BLADERF_MODULE_TX] {
            // TODO: What channels are supported?
            let current = BLADERF.get_sample_rate(channel)?;
            log::trace!("Current Sample rate:\t{current}");
            log::trace!("Desired Sample rate:\t{desired}");

            BLADERF.set_sample_rate(channel, desired)?;

            let new = BLADERF.get_sample_rate(channel)?;
            log::trace!("New Sample rate:\t{new}");
            assert_eq!(new, desired);
        }
    }

    // BLADERF.device_reset()
    Ok(())
}
