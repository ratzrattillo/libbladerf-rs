use anyhow::Result;
use libbladerf_rs::bladerf1::BladeRf1;
use libbladerf_rs::hardware::lms6002d::dc_calibration::DcCalModule;

fn main() -> Result<()> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .filter_module("nusb", log::LevelFilter::Info)
        .init();

    let bladerf = BladeRf1::from_first()?;
    bladerf.initialize()?;

    let dc_cals = bladerf.get_dc_cals()?;
    //log::debug!("Dc Calibration: {:?}", dc_cals);
    log::debug!("{dc_cals}");

    log::debug!("Calibrating: {:?}", DcCalModule::RxVga2);
    bladerf.calibrate_dc(DcCalModule::RxVga2)?;
    //
    // log::debug!("Calibrating: {:?}", DcCalModule::RxLpf);
    // bladerf.calibrate_dc(DcCalModule::RxLpf)?;
    //
    // log::debug!("Calibrating: {:?}", DcCalModule::TxLpf);
    // // bladerf.calibrate_dc(DcCalModule::TxLpf)?;
    // bladerf.cal_tx_lpf()?;
    //
    // log::debug!("Calibrating: {:?}", DcCalModule::LpfTuning);
    // bladerf.calibrate_dc(DcCalModule::LpfTuning)?;

    Ok(())
}
