use anyhow::Result;
use libbladerf_rs::bladerf1::BladeRf1;
use libbladerf_rs::bladerf1::hardware::lms6002d::dc_calibration::DcCalModule;

fn main() -> Result<()> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .filter_module("nusb", log::LevelFilter::Info)
        .init();

    let mut bladerf = BladeRf1::from_first()?;
    let mut rf = bladerf.rf_link_session()?;
    rf.initialize(false)?;

    let dc_cals = rf.get_dc_cals()?;
    log::debug!("{dc_cals}");

    log::debug!("Calibrating: {:?}", DcCalModule::RxVga2);
    rf.calibrate_dc(DcCalModule::RxVga2)?;

    log::debug!("Calibrating: {:?}", DcCalModule::RxLpf);
    rf.calibrate_dc(DcCalModule::RxLpf)?;

    log::debug!("Calibrating: {:?}", DcCalModule::TxLpf);
    rf.cal_tx_lpf()?;

    log::debug!("Calibrating: {:?}", DcCalModule::LpfTuning);
    rf.calibrate_dc(DcCalModule::LpfTuning)?;

    Ok(())
}
