mod common;

use common::*;

use bladerf_globals::bladerf1::{BLADERF1_USB_PID, BLADERF1_USB_VID};
use libbladerf_rs::{BladeRf1, Error, Result};
use nusb::MaybeFuture;
// use std::sync::LazyLock;

// pub static LOGGING: LazyLock<()> = LazyLock::new(|| logging_init("bladerf1_open"));

#[test]
fn from_first() -> Result<()> {
    logging_init("bladerf1_open");
    BladeRf1::from_first()?;
    Ok(())
}

#[test]
fn from_bus_addr() -> Result<()> {
    logging_init("bladerf1_open");
    let bladerf = nusb::list_devices()
        .wait()?
        .find(|dev| dev.vendor_id() == BLADERF1_USB_VID && dev.product_id() == BLADERF1_USB_PID)
        .map(|dev| (dev.busnum(), dev.device_address()));
    if let Some((bus_number, bus_address)) = bladerf {
        BladeRf1::from_bus_addr(bus_number, bus_address)?;
        Ok(())
    } else {
        Err(Error::NotFound)
    }
}

#[test]
fn from_serial() -> Result<()> {
    logging_init("bladerf1_open");
    let serial = "0617f60964e8f3efcbf78adc8ed94c26";
    BladeRf1::from_serial(serial)?;
    Ok(())
}

// #[test]
// fn from_fd() -> Result<()> {
//     Ok(())
// }
