mod common;

use crate::common::*;

use bladerf_globals::BladerfLoopback;
use libbladerf_rs::Result;

#[test]
fn rx_mux() -> Result<()> {
    logging_init("bladerf1_loopback");

    for desired in [
        BladerfLoopback::None,
        BladerfLoopback::Firmware,
        BladerfLoopback::BbTxlpfRxlpf,
        BladerfLoopback::BbTxlpfRxvga2,
        BladerfLoopback::BbTxvga1Rxlpf,
        BladerfLoopback::BbTxvga1Rxvga2,
        BladerfLoopback::Lna1,
        BladerfLoopback::Lna2,
        BladerfLoopback::Lna3,
    ] {
        let current = BLADERF.get_loopback()?;
        log::trace!("Current Loopback:\t{current:?}");
        log::trace!("Desired Loopback:\t{desired:?}");

        BLADERF.set_loopback(desired.clone())?;

        let new = BLADERF.get_loopback()?;
        log::trace!("New Loopback:\t\t{new:?}");
        assert_eq!(new, desired);
    }

    // BLADERF.device_reset()
    Ok(())
}
