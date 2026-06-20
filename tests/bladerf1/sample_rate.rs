use super::common::*;
use libbladerf_rs::bladerf1::RfLinkSession;
use libbladerf_rs::range::RangeItem;
use libbladerf_rs::{Channel, Result};

#[test]
fn sample_rate() -> Result<()> {
    logging_init("bladerf1_sample_rate");

    let mut sdr = sdr();
    let mut rf = sdr.rf_link_session()?;
    let supported_sample_rates = RfLinkSession::get_sample_rate_range();

    log::trace!("supported_sample_rates: {supported_sample_rates:?}");
    for range_item in supported_sample_rates.iter() {
        let (min, max, step, scale) = match range_item {
            RangeItem::Step(min, max, step, scale) => (min, max, step, scale),
            _ => panic!("sample_rates range item should be Variant of type \"Step\"!"),
        };

        let num_splits = 10.0;
        let offset = ((*max - *min) / num_splits).round();

        let mut desired = min.round() as u32;
        while desired <= max.round() as u32 {
            for channel in [Channel::Rx, Channel::Tx] {
                let current = rf.get_sample_rate(channel)?;
                log::trace!("Channel {channel:?} Sample Rate (CURRENT):\t{current}");
                log::trace!("Channel {channel:?} Sample Rate (DESIRED):\t{desired}");

                rf.set_sample_rate(channel, desired)?;

                let new = rf.get_sample_rate(channel)?;
                log::trace!("Channel {channel:?} Sample Rate (NEW):\t\t{new}");
                assert_eq!(new, desired);

                rf.set_sample_rate(channel, current)?;
            }

            desired += (*step * *scale * offset).round() as u32;
        }
    }

    Ok(())
}
