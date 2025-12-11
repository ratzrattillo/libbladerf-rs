mod common;

use crate::common::*;

use libbladerf_rs::bladerf1::BladeRf1;
use libbladerf_rs::range::RangeItem;
use libbladerf_rs::{Channel, Result};

#[test]
fn bandwidth() -> Result<()> {
    logging_init("bladerf1_bandwidth");

    let supported_bandwidths = BladeRf1::get_bandwidth_range();

    log::trace!("supported_bandwidths: {supported_bandwidths:?}");
    for range_item in supported_bandwidths.items {
        let desired = match range_item {
            RangeItem::Value(v) => v.round() as u32,
            _ => panic!("bandwidth range item should be Variant of type \"Value\"!"),
        };
        // TODO: What channels are supported?
        for channel in [Channel::Rx, Channel::Tx] {
            let current = BLADERF.get_bandwidth(channel)?;
            log::trace!("Channel {channel:?} Bandwidth (CURRENT):\t{current}");
            log::trace!("Channel {channel:?} Bandwidth (DESIRED):\t{desired}");

            BLADERF.set_bandwidth(channel, desired)?;

            let new = BLADERF.get_bandwidth(channel)?;
            log::trace!("Channel {channel:?} Bandwidth (NEW):\t{new}");
            assert_eq!(new, desired);
        }
    }

    // BLADERF.device_reset()
    Ok(())
}
