mod common;

use crate::common::*;

use libbladerf_rs::bladerf1::BladeRf1;
use libbladerf_rs::range::RangeItem;
use libbladerf_rs::{Channel, Result};

#[test]
fn sample_rate() -> Result<()> {
    logging_init("bladerf1_sample_rate");

    let supported_sample_rates = BladeRf1::get_sample_rate_range();

    log::trace!("supported_sample_rates: {supported_sample_rates:?}");
    for range_item in supported_sample_rates.items {
        let (min, max, step, scale) = match range_item {
            RangeItem::Step(min, max, step, scale) => (min, max, step, scale),
            _ => panic!("sample_rates range item should be Variant of type \"Step\"!"),
        };

        // To not go through each possible frequency, we split the range in num_splits parts
        // and tune to each of the desired frequencies.
        let num_splits = 10.0;
        let offset = ((max - min) / num_splits).round();

        let mut desired = min.round() as u32;
        while desired <= max.round() as u32 {
            // TODO: What channels are supported?
            for channel in [Channel::Rx, Channel::Tx] {
                let current = BLADERF.get_sample_rate(channel)?;
                log::trace!("Channel {channel:?} Sample Rate (CURRENT):\t{current}");
                log::trace!("Channel {channel:?} Sample Rate (DESIRED):\t{desired}");

                BLADERF.set_sample_rate(channel, desired)?;
                // Give enough time for the OS to release the USB endpoint
                // sleep(Duration::from_millis(1000));

                let new = BLADERF.get_sample_rate(channel)?;
                log::trace!("Channel {channel:?} Sample Rate (NEW):\t\t{new}");
                assert_eq!(new, desired);
            }

            // if let Some(step) = range_item.step() && let Some(scale) = range_item.scale() {
            //     desired += (step * scale).round() as u32;
            // } else {
            //     break;
            // }

            desired += (step * scale * offset).round() as u32;
        }
    }

    // BLADERF.device_reset()
    Ok(())
}
