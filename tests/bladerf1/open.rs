use super::common::*;
use libbladerf_rs::MaybeFuture;
use libbladerf_rs::Result;
use libbladerf_rs::bladerf1::board::Loopback;

#[test]
fn firmware_loopback_set_get() -> Result<()> {
    logging_init("bladerf1_open");

    let mut sdr = sdr();
    let mut rf = sdr.rf_link_session().wait()?;

    rf.set_loopback(Loopback::Firmware).wait()?;
    let lb = rf.get_loopback().wait()?;
    assert_eq!(lb, Loopback::Firmware, "expected Firmware, got {lb:?}");

    rf.set_loopback(Loopback::None).wait()?;
    let lb = rf.get_loopback().wait()?;
    assert_eq!(lb, Loopback::None, "expected None, got {lb:?}");

    Ok(())
}
