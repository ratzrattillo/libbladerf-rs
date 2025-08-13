mod common;

use crate::common::*;
use libbladerf_rs::Result;
use libbladerf_rs::bladerf1::BladeRf1;
use libbladerf_rs::bladerf1::xb::ExpansionBoard;
// TODO: Detect attachable expansion board for testing!
// TODO: Currently XB200 is hardcoded!

// #[test]
// fn xb100_enabled() -> Result<()> {
//     logging_init("bladerf1_expansion_boards");
//
//     let enabled = BladeRf1::xb100_is_enabled(&BLADERF.interface)?;
//     log::trace!("XB100 enabled:\t{}", enabled);
//     // assert_eq!(enabled, false);
//     BLADERF.expansion_attach(Xb100)?;
//     let enabled = BladeRf1::xb100_is_enabled(&BLADERF.interface)?;
//     log::trace!("XB100 enabled:\t{}", enabled);
//     assert_eq!(enabled, true);
//
//     // BLADERF.device_reset()
//     Ok(())
// }

#[test]
fn xb200_enabled() -> Result<()> {
    logging_init("bladerf1_expansion_boards");

    let enabled = BladeRf1::xb200_is_enabled(&BLADERF.interface)?;
    log::trace!("XB200 enabled:\t{enabled}");
    // assert_eq!(enabled, false);
    BLADERF.expansion_attach(ExpansionBoard::Xb200)?;
    let enabled = BladeRf1::xb200_is_enabled(&BLADERF.interface)?;
    log::trace!("XB200 enabled:\t{enabled}");
    assert!(enabled);

    // BLADERF.device_reset()
    Ok(())
}

// #[test]
// fn xb300_enabled() -> Result<()> {
//     logging_init("bladerf1_expansion_boards");
//
//     let enabled = BladeRf1::xb300_is_enabled(&BLADERF.interface)?;
//     log::trace!("XB300 enabled:\t{}", enabled);
//     // assert_eq!(enabled, false);
//     BLADERF.expansion_attach(Xb300)?;
//     let enabled = BladeRf1::xb300_is_enabled(&BLADERF.interface)?;
//     log::trace!("XB300 enabled:\t{}", enabled);
//     assert_eq!(enabled, true);
//
//     // BLADERF.device_reset()
//     Ok(())
// }
