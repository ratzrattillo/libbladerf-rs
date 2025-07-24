mod common;

use crate::common::*;

use bladerf_globals::range::RangeItem;
use bladerf_globals::{BLADERF_MODULE_RX, BLADERF_MODULE_TX};
use libbladerf_rs::{BladeRf1, Result};

#[test]
fn sample_rate() -> Result<()> {
    logging_init("bladerf1_sample_rate");

    let supported_sample_rates = BladeRf1::get_sample_rate_range();

    log::trace!("supported_sample_rates: {:?}", supported_sample_rates);
    for range_item in supported_sample_rates.items {
        let (min, max, step, scale) = match range_item {
            RangeItem::Step(min, max, step, scale) => (min, max, step, scale),
            _ => panic!("sample_rates range item should be Variant of type \"Step\"!"),
        };
        // let mut desired = range_item.min().round() as u32;
        // while desired <= range_item.max().round() as u32 {
        let mut desired = min.round() as u32;
        while desired <= max.round() as u32 {
            // TODO: What channels are supported?
            for channel in [BLADERF_MODULE_RX, BLADERF_MODULE_TX] {
                let current = BLADERF.get_sample_rate(channel)?;
                log::trace!("Channel {channel} Sample Rate (CURRENT):\t{current}");
                log::trace!("Channel {channel} Sample Rate (DESIRED):\t{desired}");

                BLADERF.set_sample_rate(channel, desired)?;
                // Give enough time for the OS to release the USB endpoint
                // sleep(Duration::from_millis(1000));

                let new = BLADERF.get_sample_rate(channel)?;
                log::trace!("Channel {channel} Sample Rate (NEW):\t\t{new}");
                assert_eq!(new, desired);
            }

            // if let Some(step) = range_item.step() && let Some(scale) = range_item.scale() {
            //     desired += (step * scale).round() as u32;
            // } else {
            //     break;
            // }

            desired += (step * scale).round() as u32;
        }
    }

    // BLADERF.device_reset()
    Ok(())
}
