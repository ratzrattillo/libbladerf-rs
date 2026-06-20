use super::common::*;
use libbladerf_rs::Result;
use libbladerf_rs::bladerf1::board::RxMux::{
    Mux12BitCounter, Mux32BitCounter, MuxBaseband, MuxDigitalLoopback,
};

#[test]
fn rx_mux() -> Result<()> {
    logging_init("bladerf1_rx_mux");

    let mut sdr = sdr();
    let mut rf = sdr.rf_link_session()?;
    let original_rx_mux = rf.get_rx_mux()?;

    for desired in [
        Mux12BitCounter,
        Mux32BitCounter,
        MuxDigitalLoopback,
        MuxBaseband,
    ] {
        let current = rf.get_rx_mux()?;
        log::trace!("RX Mux (CURRENT):\t{current:?}");
        log::trace!("RX Mux (DESIRED):\t{desired:?}");

        rf.set_rx_mux(desired)?;

        let new = rf.get_rx_mux()?;
        log::trace!("RX Mux (NEW):\t{new:?}");
        assert_eq!(new, desired);
    }

    rf.set_rx_mux(original_rx_mux)?;

    Ok(())
}
