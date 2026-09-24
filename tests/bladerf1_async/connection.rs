use super::*;
use libbladerf_rs::Error;

#[tokio::test(flavor = "current_thread")]
async fn shutdown_preserves_live_claims_and_reopens_cleanly() -> Result<()> {
    let mut device = open().await?;
    let serial = device.serial().await?;
    let mut rx = {
        let mut rf = device.rf_link_session().await?;
        RxStream::builder(&mut rf).buffer_count(2).build().await?
    };
    assert!(matches!(device.shutdown().await, Err(Error::StreamsActive)));
    assert!(matches!(
        device.device_reset().await,
        Err(Error::StreamsActive)
    ));
    rx.close(&mut device.rf_link_session().await?).await?;
    if let Ok(result) = tokio::time::timeout(Duration::ZERO, device.shutdown()).await {
        result?;
    }
    device.shutdown().await?;
    assert!(matches!(
        device.rf_link_session().await,
        Err(Error::DeviceClosed)
    ));
    device.close().await?;
    let mut device = BladeRf1::from_serial(&serial).await?;
    device.rf_link_session().await?.initialize(false).await?;
    device.close().await
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "resets and re-enumerates the connected BladeRF1"]
async fn reset_recovers_an_abandoned_stream_and_invalidates_the_old_connection() -> Result<()> {
    let mut device = open().await?;
    let serial = device.serial().await?;
    {
        let mut rf = device.rf_link_session().await?;
        rf.set_loopback(Loopback::None).await?;
        let mut rx = RxStream::builder(&mut rf).buffer_count(2).build().await?;
        rx.start(&mut rf).await?;
    }
    assert!(matches!(
        device.flash_session().await,
        Err(Error::RecoveryRequired)
    ));
    device.device_reset().await?;
    assert!(matches!(
        device.device_reset().await,
        Err(Error::DeviceClosed)
    ));
    assert!(matches!(
        device.rf_link_session().await,
        Err(Error::DeviceClosed)
    ));
    drop(device);
    let mut last = Error::NotFound;
    for _ in 0..30 {
        tokio::time::sleep(Duration::from_secs(1)).await;
        match BladeRf1::from_serial(&serial).await {
            Ok(mut device) => {
                device.rf_link_session().await?.initialize(false).await?;
                return device.close().await;
            }
            Err(error) => last = error,
        }
    }
    Err(last)
}
