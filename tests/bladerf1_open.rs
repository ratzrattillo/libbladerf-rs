mod common;

use common::*;

use libbladerf_rs::bladerf1::{BLADERF1_USB_PID, BLADERF1_USB_VID, BladeRf1};
use libbladerf_rs::{Error, Result};
use nusb::MaybeFuture;

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

// #[test]
// fn from_serial() -> Result<()> {
//     logging_init("bladerf1_open");
//     let serial = "0123456789abcdef";
//     BladeRf1::from_serial(serial)?;
//     Ok(())
// }

// #[test]
// fn from_fd() -> Result<()> {
//     Ok(())
// }
