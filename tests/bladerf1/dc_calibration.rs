use super::common::*;
use libbladerf_rs::MaybeFuture;
use libbladerf_rs::bladerf1::hardware::lms6002d::dc_calibration::{DcCalModule, DcCals};

#[test]
fn dc_cals_read() -> libbladerf_rs::Result<()> {
    logging_init("bladerf1_dc_calibration");

    let mut sdr = sdr();
    let mut rf = sdr.rf_link_session().wait()?;
    let dc_cals = rf.get_dc_cals().wait()?;
    log::trace!("Current DC cals: {dc_cals}");

    Ok(())
}

#[test]
fn dc_cals_roundtrip() -> libbladerf_rs::Result<()> {
    logging_init("bladerf1_dc_calibration");

    let mut sdr = sdr();
    let mut rf = sdr.rf_link_session().wait()?;
    let backup = rf.get_dc_cals().wait()?;

    let test_cals = DcCals {
        lpf_tuning: Some(20.try_into()?),
        tx_lpf_i: Some(10.try_into()?),
        tx_lpf_q: Some(15.try_into()?),
        rx_lpf_i: Some(25.try_into()?),
        rx_lpf_q: Some(30.try_into()?),
        dc_ref: Some(5.try_into()?),
        rxvga2a_i: Some(12.try_into()?),
        rxvga2a_q: Some(18.try_into()?),
        rxvga2b_i: Some(8.try_into()?),
        rxvga2b_q: Some(22.try_into()?),
    };

    rf.set_dc_cals(test_cals).wait()?;
    let readback = rf.get_dc_cals().wait();
    rf.set_dc_cals(backup).wait()?;
    let readback = readback?;

    log::trace!("DC cals (SET):\t\t{test_cals:?}");
    log::trace!("DC cals (READBACK):\t{readback:?}");
    assert_eq!(readback, test_cals, "DC cals roundtrip mismatch");

    Ok(())
}

#[test]
fn calibrate_all_modules() -> libbladerf_rs::Result<()> {
    logging_init("bladerf1_dc_calibration");

    let mut sdr = sdr();
    let mut rf = sdr.rf_link_session().wait()?;

    for &module in &[
        DcCalModule::LpfTuning,
        DcCalModule::RxLpf,
        DcCalModule::RxVga2,
    ] {
        log::debug!("Calibrating: {module:?}");
        rf.calibrate_dc(module).wait()?;
        log::debug!("Calibration complete: {module:?}");
    }

    log::debug!("Calibrating: {:?}", DcCalModule::TxLpf);
    rf.cal_tx_lpf().wait()?;
    log::debug!("Calibration complete: {:?}", DcCalModule::TxLpf);

    let dc_cals = rf.get_dc_cals().wait()?;
    log::trace!("DC cals after calibration: {dc_cals}");

    Ok(())
}
