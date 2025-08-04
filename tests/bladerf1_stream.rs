mod common;

use crate::common::*;
use num_complex::Complex32;

use libbladerf_rs::{BladeRf1RxStreamer, BladeRf1TxStreamer, Result};

#[test]
fn rx_stream() -> Result<()> {
    logging_init("bladerf1_stream");
    let buffer_size = 65535;
    let num_pending_transfers = Some(8);
    let timeout = None;

    let mut samples = [Complex32::new(0.0, 0.0); 8192];

    let mut streamer =
        BladeRf1RxStreamer::new(BLADERF.clone(), buffer_size, num_pending_transfers, timeout)?;
    streamer.activate()?;
    let n = streamer.read_sync(&mut [&mut samples], 200000)?;

    assert_eq!(samples.len(), n);
    log::trace!("Read {n} samples!");

    // BLADERF.device_reset()
    Ok(())
}

#[test]
fn tx_stream() -> Result<()> {
    logging_init("bladerf1_stream");
    let buffer_size = 65535;
    let num_pending_transfers = Some(8);
    let timeout = None;

    let mut samples = [Complex32::new(0.0, 0.0); 8192];

    let mut streamer =
        BladeRf1TxStreamer::new(BLADERF.clone(), buffer_size, num_pending_transfers, timeout)?;
    streamer.activate()?;
    // TODO: should we return number of written bytes?
    // TODO: Test the write method instead of write_all only
    streamer.write_all(&[&mut samples], None, true, 20000)?;

    // assert_eq!(samples.len(), n);
    log::trace!("Wrote samples!");

    // BLADERF.device_reset()
    Ok(())
}
