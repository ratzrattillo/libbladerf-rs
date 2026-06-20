use super::common::*;
use libbladerf_rs::Channel;
use libbladerf_rs::bladerf1::board::Correction;
use libbladerf_rs::bladerf1::calibration::{DcCalEntry, DcCalTable};
use libbladerf_rs::bladerf1::hardware::lms6002d::dc_calibration::DcCals;
use libbladerf_rs::bladerf1::{DcPair, TuningMode};

#[test]
fn load_and_lookup() -> libbladerf_rs::Result<()> {
    logging_init("bladerf1_dc_cal_table");

    let table = DcCalTable::new(
        DcCals::new(-1, -1, -1, -1, -1, -1, -1, -1, -1, -1),
        vec![
            DcCalEntry::new(1_000_000_000, DcPair::new(100, 200)),
            DcCalEntry::new(2_000_000_000, DcPair::new(200, 400)),
        ],
    );

    let dir = std::env::temp_dir().join("libbladerf_rs_dc_cal_table_test");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("test_dc_rx.json");
    table.save(&path)?;

    let mut sdr = sdr();
    sdr.load_dc_cal_table(Channel::Rx, &path)?;

    let mut rf = sdr.rf_link_session()?;
    rf.initialize(false)?;

    rf.set_frequency(Channel::Rx, 1_000_000_000, TuningMode::Fpga)?;
    let i = rf.get_correction(Channel::Rx, &Correction::DcOffI)?;
    let q = rf.get_correction(Channel::Rx, &Correction::DcOffQ)?;
    log::debug!("At 1GHz: dc_i={i}, dc_q={q}");

    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}
