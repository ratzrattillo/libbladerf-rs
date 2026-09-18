use anyhow::Result;
use libbladerf_rs::MaybeFuture;
use libbladerf_rs::bladerf1::BladeRf1;
use libbladerf_rs::bladerf1::hardware::lms6002d::dc_calibration::DcCalModule;

fn main() -> Result<()> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .filter_module("nusb", log::LevelFilter::Info)
        .init();

    let mut bladerf = BladeRf1::from_first().wait()?;
    let mut rf = bladerf.rf_link_session().wait()?;
    rf.initialize(false).wait()?;

    let dc_cals = rf.get_dc_cals().wait()?;
    log::debug!("{dc_cals}");

    log::debug!("Calibrating: {:?}", DcCalModule::RxVga2);
    rf.calibrate_dc(DcCalModule::RxVga2).wait()?;

    log::debug!("Calibrating: {:?}", DcCalModule::RxLpf);
    rf.calibrate_dc(DcCalModule::RxLpf).wait()?;

    log::debug!("Calibrating: {:?}", DcCalModule::TxLpf);
    rf.cal_tx_lpf().wait()?;

    log::debug!("Calibrating: {:?}", DcCalModule::LpfTuning);
    rf.calibrate_dc(DcCalModule::LpfTuning).wait()?;

    Ok(())
}
