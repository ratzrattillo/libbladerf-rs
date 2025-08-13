mod common;

use crate::common::*;

use libbladerf_rs::bladerf1::{BladeRf1, GainDb};
use libbladerf_rs::range::RangeItem;
use libbladerf_rs::{BLADERF_MODULE_RX, BLADERF_MODULE_TX, Result};

#[test]
fn set_gain() -> Result<()> {
    logging_init("bladerf1_gain");

    // TODO: The definition of allowed gain rates is still wrong.
    // TODO: Intermediate steps are not allowed, only fixed values

    for channel in [BLADERF_MODULE_TX, BLADERF_MODULE_RX] {
        let supported_gains = BladeRf1::get_gain_range(channel);

        for range_item in supported_gains.items {
            let (min, max, step, scale) = match range_item {
                RangeItem::Step(min, max, step, scale) => (min, max, step, scale),
                _ => panic!("gain range item should be Variant of type \"Step\"!"),
            };

            let mut desired = min.round() as i8;

            while desired <= max.round() as i8 {
                // TODO: What channels are supported?
                let current = BLADERF.get_gain(channel)?.db;
                log::trace!("Channel {channel} Gain (CURRENT):\t{current}");
                log::trace!("Channel {channel} Gain (DESIRED):\t{desired}");

                BLADERF.set_gain(channel, GainDb { db: desired })?;

                let new = BLADERF.get_gain(channel)?.db;
                log::trace!("Channel {channel} Gain (NEW):\t{new}");
                assert_eq!(new, desired);

                desired += (step * scale).round() as i8;
            }
        }
    }

    // BLADERF.device_reset()
    Ok(())
}
