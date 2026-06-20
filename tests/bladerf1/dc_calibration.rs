use super::common::*;
use libbladerf_rs::bladerf1::hardware::lms6002d::dc_calibration::{DcCalModule, DcCals};

#[test]
fn dc_cals_read() -> libbladerf_rs::Result<()> {
    logging_init("bladerf1_dc_calibration");

    let mut sdr = sdr();
    let mut rf = sdr.rf_link_session()?;
    let dc_cals = rf.get_dc_cals()?;
    log::trace!("Current DC cals: {dc_cals}");

    Ok(())
}

#[test]
fn dc_cals_roundtrip() -> libbladerf_rs::Result<()> {
    logging_init("bladerf1_dc_calibration");

    let mut sdr = sdr();
    let mut rf = sdr.rf_link_session()?;
    let backup = rf.get_dc_cals()?;

    let test_cals = DcCals::new(20, 10, 15, 25, 30, 5, 12, 18, 8, 22);

    rf.set_dc_cals(test_cals)?;
    let readback = rf.get_dc_cals()?;

    log::trace!("DC cals (SET):\t\t{test_cals:?}");
    log::trace!("DC cals (READBACK):\t{readback:?}");
    assert_eq!(readback, test_cals, "DC cals roundtrip mismatch");

    rf.set_dc_cals(backup)?;

    Ok(())
}

#[test]
fn calibrate_all_modules() -> libbladerf_rs::Result<()> {
    logging_init("bladerf1_dc_calibration");

    let mut sdr = sdr();
    let mut rf = sdr.rf_link_session()?;

    for &module in &[
        DcCalModule::LpfTuning,
        DcCalModule::RxLpf,
        DcCalModule::RxVga2,
    ] {
        log::debug!("Calibrating: {module:?}");
        rf.calibrate_dc(module)?;
        log::debug!("Calibration complete: {module:?}");
    }

    log::debug!("Calibrating: {:?}", DcCalModule::TxLpf);
    rf.cal_tx_lpf()?;
    log::debug!("Calibration complete: {:?}", DcCalModule::TxLpf);

    let dc_cals = rf.get_dc_cals()?;
    log::trace!("DC cals after calibration: {dc_cals}");

    Ok(())
}
