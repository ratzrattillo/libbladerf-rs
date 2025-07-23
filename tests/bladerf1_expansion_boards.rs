mod common;

use crate::common::*;
use bladerf_globals::bladerf1::BladerfXb::BladerfXb200;
use libbladerf_rs::{BladeRf1, Result};
#[test]
fn xb200_enabled() -> Result<()> {
    logging_init("bladerf1_expansion_boards");

    let enabled = BladeRf1::xb200_is_enabled(&BLADERF.interface)?;
    log::trace!("XB200 enabled:\t{}", enabled);
    // assert_eq!(enabled, false);
    BLADERF.expansion_attach(BladerfXb200)?;
    let enabled = BladeRf1::xb200_is_enabled(&BLADERF.interface)?;
    log::trace!("XB200 enabled:\t{}", enabled);
    assert_eq!(enabled, true);

    // BLADERF.device_reset()
    Ok(())
}
