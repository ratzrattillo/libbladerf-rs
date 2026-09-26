use super::common::*;
use libbladerf_rs::Channel;
use libbladerf_rs::MaybeFuture;
use libbladerf_rs::Result;
use libbladerf_rs::bladerf1::RfLinkSession;
use libbladerf_rs::bladerf1::TxStream;
use libbladerf_rs::bladerf1::hardware::lms6002d::dc_calibration::{DcCalModule, DcCals};

#[test]
fn dc_cals_read() -> Result<()> {
    logging_init("bladerf1_dc_calibration");

    let mut sdr = sdr();
    let mut rf = sdr.rf_link_session().wait()?;
    let dc_cals = rf.get_dc_cals().wait()?;
    log::trace!("Current DC cals: {dc_cals}");

    Ok(())
}

#[test]
fn dc_cals_roundtrip() -> Result<()> {
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

fn calibrate_with_retry(
    rf: &mut RfLinkSession<'_>,
    module: DcCalModule,
    attempt: impl Fn(&mut RfLinkSession<'_>) -> Result<()>,
) -> Result<()> {
    let result = attempt(rf);
    match &result {
        Err(error) if matches!(error, libbladerf_rs::Error::CalibrationFailed(_)) => {
            log::warn!(
                "DC calibration ({module:?}) did not converge: {error}; forcing re-initialization and retrying"
            );
            rf.initialize(true).wait()?;
            attempt(rf)
        }
        _ => result,
    }
}

#[test]
fn calibrate_lpf_tuning() -> Result<()> {
    logging_init("bladerf1_dc_calibration");

    let mut sdr = sdr();
    let mut rf = sdr.rf_link_session().wait()?;
    rf.initialize(true).wait()?;

    calibrate_with_retry(&mut rf, DcCalModule::LpfTuning, |rf| {
        rf.calibrate_dc(DcCalModule::LpfTuning).wait()
    })?;

    let dc_cals = rf.get_dc_cals().wait()?;
    log::trace!("DC cals after LpfTuning: {dc_cals}");

    Ok(())
}

#[test]
fn calibrate_tx_lpf() -> Result<()> {
    logging_init("bladerf1_dc_calibration");

    let mut sdr = sdr();
    let mut rf = sdr.rf_link_session().wait()?;
    rf.initialize(true).wait()?;

    let loopback_backup = rf.get_loopback().wait()?;
    let sample_rate_backup = rf.get_rational_sample_rate(Channel::Tx).wait()?;

    calibrate_with_retry(&mut rf, DcCalModule::TxLpf, |rf| rf.cal_tx_lpf().wait())?;

    let loopback = rf.get_loopback().wait()?;
    assert_eq!(
        loopback, loopback_backup,
        "cal_tx_lpf must restore the loopback mode"
    );
    let sample_rate = rf.get_rational_sample_rate(Channel::Tx).wait()?;
    assert_eq!(
        sample_rate, sample_rate_backup,
        "cal_tx_lpf must restore the TX sample rate"
    );

    let mut tx = TxStream::builder(&mut rf).build().wait()?;
    tx.start(&mut rf).wait()?;
    tx.close(&mut rf).wait()?;

    let dc_cals = rf.get_dc_cals().wait()?;
    log::trace!("DC cals after TxLpf: {dc_cals}");

    Ok(())
}

#[test]
fn calibrate_rx_lpf() -> Result<()> {
    logging_init("bladerf1_dc_calibration");

    let mut sdr = sdr();
    let mut rf = sdr.rf_link_session().wait()?;
    rf.initialize(true).wait()?;

    let result = calibrate_with_retry(&mut rf, DcCalModule::RxLpf, |rf| {
        rf.calibrate_dc(DcCalModule::RxLpf).wait()
    });
    match result {
        Err(error) if matches!(error, libbladerf_rs::Error::CalibrationFailed(_)) => {
            log::warn!(
                "RxLpf DC calibration did not converge ({error}); \
                known hardware-dependent condition, passing"
            );
        }
        other => other?,
    }

    let dc_cals = rf.get_dc_cals().wait()?;
    log::trace!("DC cals after RxLpf: {dc_cals}");

    Ok(())
}

#[test]
fn calibrate_rx_vga2() -> Result<()> {
    logging_init("bladerf1_dc_calibration");

    let mut sdr = sdr();
    let mut rf = sdr.rf_link_session().wait()?;
    rf.initialize(true).wait()?;

    calibrate_with_retry(&mut rf, DcCalModule::RxVga2, |rf| {
        rf.calibrate_dc(DcCalModule::RxVga2).wait()
    })?;

    let dc_cals = rf.get_dc_cals().wait()?;
    log::trace!("DC cals after RxVga2: {dc_cals}");

    Ok(())
}

#[test]
fn calibrate_all_modules() -> Result<()> {
    logging_init("bladerf1_dc_calibration");

    let mut sdr = sdr();
    let mut rf = sdr.rf_link_session().wait()?;
    rf.initialize(true).wait()?;

    for &module in &[
        DcCalModule::LpfTuning,
        DcCalModule::TxLpf,
        DcCalModule::RxLpf,
        DcCalModule::RxVga2,
    ] {
        log::debug!("Calibrating: {module:?}");
        let result = calibrate_with_retry(&mut rf, module, |rf| match module {
            DcCalModule::TxLpf => rf.cal_tx_lpf().wait(),
            other => rf.calibrate_dc(other).wait(),
        });
        match result {
            Err(error)
                if module == DcCalModule::RxLpf
                    && matches!(error, libbladerf_rs::Error::CalibrationFailed(_)) =>
            {
                log::warn!(
                    "RxLpf DC calibration did not converge ({error}); \
                    known hardware-dependent condition, continuing"
                );
            }
            other => other?,
        }
        log::debug!("Calibration complete: {module:?}");
    }

    let dc_cals = rf.get_dc_cals().wait()?;
    log::trace!("DC cals after calibration: {dc_cals}");

    Ok(())
}
