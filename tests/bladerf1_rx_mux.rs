mod common;

use crate::common::*;

use bladerf_globals::bladerf1::BladeRf1RxMux::{
    Mux12BitCounter, Mux32BitCounter, MuxBaseband, MuxDigitalLoopback,
};
use libbladerf_rs::Result;

#[test]
fn rx_mux() -> Result<()> {
    logging_init("bladerf1_rx_mux");

    for desired in [
        Mux12BitCounter,
        Mux32BitCounter,
        MuxDigitalLoopback,
        MuxBaseband,
    ] {
        let current = BLADERF.get_rx_mux()?;
        log::trace!("RX Mux (CURRENT):\t{current:?}");
        log::trace!("RX Mux (DESIRED):\t{desired:?}");

        BLADERF.set_rx_mux(desired.clone())?;

        let new = BLADERF.get_rx_mux()?;
        log::trace!("RX Mux (NEW):\t{new:?}");
        assert_eq!(new, desired);
    }

    // BLADERF.device_reset()
    Ok(())
}
