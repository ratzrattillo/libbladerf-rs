mod common;

use crate::common::*;

use bladerf_globals::range::RangeItem;
use bladerf_globals::{BLADERF_MODULE_RX, BLADERF_MODULE_TX};
use libbladerf_rs::{BladeRf1, Result};

// #[test]
// fn enable_logging() {
//     logging_init("bladerf1_bandwidth");
// }

#[test]
fn bandwidth() -> Result<()> {
    logging_init("bladerf1_bandwidth");

    let supported_bandwidths = BladeRf1::get_bandwidth_range();

    log::trace!("supported_bandwidths: {:?}", supported_bandwidths);
    for range_item in supported_bandwidths.items {
        let desired = match range_item {
            RangeItem::Value(v) => v.round() as u32,
            _ => panic!("bandwidth range item should be Variant of type \"Value\"!"),
        };
        // TODO: What channels are supported?
        for channel in [BLADERF_MODULE_RX, BLADERF_MODULE_TX] {
            let current = BLADERF.get_bandwidth(channel)?;
            log::trace!("Channel {channel} Bandwidth (CURRENT):\t{current}");
            log::trace!("Channel {channel} Bandwidth (DESIRED):\t{desired}");

            BLADERF.set_bandwidth(channel, desired)?;

            let new = BLADERF.get_bandwidth(channel)?;
            log::trace!("Channel {channel} Bandwidth (NEW):\t{new}");
            assert_eq!(new, desired);
        }
    }

    // BLADERF.device_reset()
    Ok(())
}
