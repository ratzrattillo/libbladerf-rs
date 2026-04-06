#[path = "common/mod.rs"]
mod common;

use common::*;
use libbladerf_rs::Result;
use libbladerf_rs::bladerf1::hardware::lms6002d::loopback::Loopback;

#[test]
fn rx_mux() -> Result<()> {
    logging_init("bladerf1_loopback");

    for desired in [
        Loopback::None,
        Loopback::Firmware,
        Loopback::BbTxlpfRxlpf,
        Loopback::BbTxlpfRxvga2,
        Loopback::BbTxvga1Rxlpf,
        Loopback::BbTxvga1Rxvga2,
        Loopback::Lna1,
        Loopback::Lna2,
        Loopback::Lna3,
    ] {
        let current = BLADERF.get_loopback()?;
        log::trace!("Loopback (CURRENT):\t{current:?}");
        log::trace!("Loopback (DESIRED):\t{desired:?}");

        BLADERF.set_loopback(desired.clone())?;

        let new = BLADERF.get_loopback()?;
        log::trace!("Loopback (NEW):\t\t{new:?}");
        assert_eq!(new, desired);
    }

    // BLADERF.device_reset()
    Ok(())
}
