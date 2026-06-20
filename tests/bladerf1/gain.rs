use super::common::*;
use libbladerf_rs::bladerf1::RfLinkSession;
use libbladerf_rs::range::RangeItem;
use libbladerf_rs::{Channel, Result};

#[test]
fn set_gain() -> Result<()> {
    logging_init("bladerf1_gain");

    let mut sdr = sdr();
    let mut rf = sdr.rf_link_session()?;

    for channel in [Channel::Tx, Channel::Rx] {
        let current = rf.get_gain(channel)?;

        let supported_gains = RfLinkSession::get_gain_range(channel);

        for range_item in supported_gains.iter() {
            let (min, max, step, scale) = match range_item {
                RangeItem::Step(min, max, step, scale) => (min, max, step, scale),
                _ => panic!("gain range item should be Variant of type \"Step\"!"),
            };

            let mut desired = min.round() as i8;

            while desired <= max.round() as i8 {
                log::trace!("Channel {channel:?} Gain (DESIRED):\t{desired}");

                rf.set_gain(channel, desired.into())?;

                let new = rf.get_gain(channel)?.db();
                log::trace!("Channel {channel:?} Gain (NEW):\t{new}");
                assert_eq!(new, desired);

                desired += (*step * *scale).round() as i8;
            }
        }

        rf.set_gain(channel, current)?;
    }

    Ok(())
}
