mod common;

use crate::common::*;

use bladerf_globals::bladerf1::BladerfRxMux;
use libbladerf_rs::Result;

#[test]
fn rx_mux() -> Result<()> {
    logging_init("bladerf1_rx_mux");

    for desired in [
        BladerfRxMux::Mux12BitCounter,
        BladerfRxMux::Mux32BitCounter,
        BladerfRxMux::MuxDigitalLoopback,
        BladerfRxMux::MuxBaseband,
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
