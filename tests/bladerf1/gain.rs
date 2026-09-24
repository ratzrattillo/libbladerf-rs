use super::common::*;
use libbladerf_rs::MaybeFuture;
use libbladerf_rs::{Channel, Result};

#[test]
fn set_gain() -> Result<()> {
    logging_init("bladerf1_gain");

    let mut sdr = sdr();
    let mut rf = sdr.rf_link_session().wait()?;

    let mut mismatches = Vec::new();
    for (channel, min, max) in [(Channel::Tx, 17, 73), (Channel::Rx, -1, 60)] {
        let current = rf.get_gain(channel).wait()?;
        let result: Result<()> = (|| {
            for desired in (min..=max).chain([i8::MIN, i8::MAX]) {
                rf.set_gain(channel, desired.into()).wait()?;
                let new = rf.get_gain(channel).wait()?.db();
                if new != desired.clamp(min, max) {
                    mismatches.push((channel, desired, new));
                }
            }
            Ok(())
        })();
        let restoration = rf.set_gain(channel, current).wait();
        result?;
        restoration?;
    }
    assert!(
        mismatches.is_empty(),
        "unexpected gain readbacks: {mismatches:?}"
    );
    Ok(())
}
