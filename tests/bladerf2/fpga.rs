use super::common::*;
use libbladerf_rs::bladerf2::FpgaSource;
use libbladerf_rs::{MaybeFuture, Result, SemanticVersion};

#[test]
fn versions_parsed() -> Result<()> {
    logging_init("bladerf2_fpga");
    let mut dev = open();
    let firmware = dev.firmware_version().wait()?;
    let mut rf = dev.rf_link_session().wait()?;
    assert!(rf.is_fpga_configured().wait()?, "FPGA must be loaded");
    let fpga: SemanticVersion = rf.fpga_version().wait()?.parse()?;
    println!("firmware v{firmware}, FPGA v{fpga}");
    Ok(())
}

#[test]
fn fpga_source_is_flash_or_host() -> Result<()> {
    logging_init("bladerf2_fpga");
    let mut dev = open();
    let mut rf = dev.rf_link_session().wait()?;
    let source = rf.get_fpga_source().wait()?;
    println!("FPGA source: {source:?}");
    assert!(matches!(source, FpgaSource::Flash | FpgaSource::Host));
    Ok(())
}
