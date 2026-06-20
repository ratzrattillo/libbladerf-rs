use super::common::*;
use libbladerf_rs::bladerf1::RfLinkSession;
use libbladerf_rs::range::RangeItem;
use libbladerf_rs::{Channel, Result};

#[test]
fn bandwidth() -> Result<()> {
    logging_init("bladerf1_bandwidth");

    let mut sdr = sdr();
    let mut rf = sdr.rf_link_session()?;
    let supported_bandwidths = RfLinkSession::get_bandwidth_range();

    log::trace!("supported_bandwidths: {supported_bandwidths:?}");
    for range_item in supported_bandwidths.iter() {
        let desired = match range_item {
            RangeItem::Value(v) => v.round() as u32,
            _ => panic!("bandwidth range item should be Variant of type \"Value\"!"),
        };
        for channel in [Channel::Rx, Channel::Tx] {
            let current = rf.get_bandwidth(channel)?;
            log::trace!("Channel {channel:?} Bandwidth (CURRENT):\t{current}");
            log::trace!("Channel {channel:?} Bandwidth (DESIRED):\t{desired}");

            let actual = rf.set_bandwidth(channel, desired)?;
            log::trace!("Channel {channel:?} Bandwidth (ACTUAL):\t{actual}");
            assert_eq!(actual, desired);

            rf.set_bandwidth(channel, current)?;
        }
    }

    Ok(())
}
