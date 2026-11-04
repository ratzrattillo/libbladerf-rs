use super::common::*;
use libbladerf_rs::bladerf2::BladeRf2;
use libbladerf_rs::{MaybeFuture, Result};

#[test]
fn open_close_repeatedly() -> Result<()> {
    logging_init("bladerf2_open");
    for _ in 0..3 {
        let dev = open();
        assert!(!dev.serial().wait()?.is_empty());
        drop(dev);
    }
    Ok(())
}

#[test]
fn open_by_serial() -> Result<()> {
    logging_init("bladerf2_open");
    let serial = open().serial().wait()?;
    let dev = BladeRf2::from_serial(&serial).wait()?;
    assert_eq!(dev.serial().wait()?, serial);
    Ok(())
}

#[test]
fn unknown_serial_is_not_found() {
    assert!(matches!(
        BladeRf2::from_serial("does-not-exist").wait(),
        Err(libbladerf_rs::Error::NotFound)
    ));
}
