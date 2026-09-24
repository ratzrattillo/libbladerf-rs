use super::*;
use libbladerf_rs::Error;
use libbladerf_rs::bladerf1::RxMux;
use libbladerf_rs::bladerf1::board::TriggerRole;

#[tokio::test(flavor = "current_thread")]
async fn trigger_release_completes_the_original_cancelled_read() -> Result<()> {
    let mut device = open().await?;
    let mut rf = device.rf_link_session().await?;
    let trigger = rf.trigger_state(Channel::Rx).await?;
    let mux = rf.get_rx_mux().await?;
    let rate = rf.get_sample_rate(Channel::Rx).await?;
    let loopback = rf.get_loopback().await?;
    rf.set_loopback(Loopback::None).await?;
    rf.set_rx_mux(RxMux::MuxBaseband).await?;
    rf.set_sample_rate(Channel::Rx, 1_000_000).await?;
    rf.arm_trigger(Channel::Rx, TriggerRole::Master).await?;
    let mut rx = RxStream::builder(&mut rf)
        .buffer_size(8192)
        .buffer_count(4)
        .build()
        .await?;
    let mut timed_out = false;
    let mut pending = 0;
    let result: Result<()> = async {
        rx.start(&mut rf).await?;
        match tokio::time::timeout(Duration::from_millis(20), rx.read(None)).await {
            Err(_) => timed_out = true,
            Ok(buffer) => rx.recycle(buffer?),
        }
        pending = rx.pending_transfers()?;
        rf.fire_trigger(Channel::Rx).await?;
        let buffer = tokio::time::timeout(Duration::from_secs(2), rx.read(None))
            .await
            .map_err(|_| Error::Timeout)??;
        let length = buffer.len();
        rx.recycle(buffer);
        if length != 8192 {
            return Err(Error::UsbTransferLength {
                expected: 8192,
                actual: length,
            });
        }
        Ok(())
    }
    .await;
    let close = rx.close(&mut rf).await;
    let disarm = rf.disarm_trigger(Channel::Rx).await;
    let restore_mux = rf.set_rx_mux(mux).await;
    let restore_rate = rf.set_sample_rate(Channel::Rx, rate).await;
    let restore_loopback = rf.set_loopback(loopback).await;
    if let Some(role) = trigger.role() {
        rf.arm_trigger(Channel::Rx, role).await?;
        if trigger.fire_requested() && role == TriggerRole::Master {
            rf.fire_trigger(Channel::Rx).await?;
        }
    }
    result?;
    close?;
    disarm?;
    restore_mux?;
    restore_rate?;
    restore_loopback?;
    assert!(
        timed_out,
        "armed trigger should gate the original RX transfer"
    );
    assert_eq!(pending, 4);
    device.close().await
}
