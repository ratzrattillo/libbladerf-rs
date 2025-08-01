mod common;

use crate::common::*;

use bladerf_globals::BladeRf1Loopback;
use libbladerf_rs::Result;

#[test]
fn rx_mux() -> Result<()> {
    logging_init("bladerf1_loopback");

    for desired in [
        BladeRf1Loopback::None,
        BladeRf1Loopback::Firmware,
        BladeRf1Loopback::BbTxlpfRxlpf,
        BladeRf1Loopback::BbTxlpfRxvga2,
        BladeRf1Loopback::BbTxvga1Rxlpf,
        BladeRf1Loopback::BbTxvga1Rxvga2,
        BladeRf1Loopback::Lna1,
        BladeRf1Loopback::Lna2,
        BladeRf1Loopback::Lna3,
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
