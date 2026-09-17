use super::common::*;
use libbladerf_rs::MaybeFuture;
use libbladerf_rs::Result;
use libbladerf_rs::bladerf1::board::RxMux::{
    Mux12BitCounter, Mux32BitCounter, MuxBaseband, MuxDigitalLoopback,
};

#[test]
fn rx_mux() -> Result<()> {
    logging_init("bladerf1_rx_mux");

    let mut sdr = sdr();
    let mut rf = sdr.rf_link_session().wait()?;
    let original_rx_mux = rf.get_rx_mux().wait()?;

    for desired in [
        Mux12BitCounter,
        Mux32BitCounter,
        MuxDigitalLoopback,
        MuxBaseband,
    ] {
        let current = rf.get_rx_mux().wait()?;
        log::trace!("RX Mux (CURRENT):\t{current:?}");
        log::trace!("RX Mux (DESIRED):\t{desired:?}");

        rf.set_rx_mux(desired).wait()?;

        let new = rf.get_rx_mux().wait()?;
        log::trace!("RX Mux (NEW):\t{new:?}");
        assert_eq!(new, desired);
    }

    rf.set_rx_mux(original_rx_mux).wait()?;

    Ok(())
}
