use crate::error::{Error, Result};
use crate::maybe_future::NonWasmSend;
use crate::protocol::nios::{
    NiosPkt8x8Target, nios_decode_write, nios_encode_write, validate_response_address,
};
use crate::usb::UsbTransport;
use std::collections::VecDeque;
use std::future::Future;
use std::sync::{Arc, Weak};

pub(crate) struct RestorationLease(Arc<()>);

pub(super) struct Restoration {
    owner: Weak<()>,
    writes: VecDeque<(u8, u8)>,
}

trait RegisterWriter {
    fn restore_register(
        &mut self,
        address: u8,
        value: u8,
    ) -> impl Future<Output = Result<()>> + NonWasmSend;
}

impl RegisterWriter for UsbTransport {
    async fn restore_register(&mut self, address: u8, value: u8) -> Result<()> {
        let mut request = [0; 16];
        let target = u8::from(NiosPkt8x8Target::Lms6);
        nios_encode_write::<u8, u8>(&mut request, target, address, value)?;
        let response = self.exchange(&request, None).await?;
        validate_response_address(&response, target, address)?;
        nios_decode_write::<u8, u8>(&response)
    }
}

impl Restoration {
    pub(super) fn new(writes: VecDeque<(u8, u8)>) -> (Self, RestorationLease) {
        let owner = RestorationLease(Arc::new(()));
        (
            Self {
                owner: Arc::downgrade(&owner.0),
                writes,
            },
            owner,
        )
    }

    async fn drive(&mut self, writer: &mut impl RegisterWriter) -> Result<bool> {
        if self.owner.strong_count() != 0 {
            return Ok(false);
        }
        while let Some(&(address, value)) = self.writes.front() {
            writer.restore_register(address, value).await?;
            self.writes.pop_front();
        }
        Ok(true)
    }

    pub(super) async fn restore(&mut self, transport: &mut UsbTransport) -> Result<bool> {
        self.drive(transport)
            .await
            .map_err(|error| Error::RestorationFailed(Box::new(error)))
    }
}

pub(super) fn with_cleanup<T>(result: Result<T>, cleanup: Result<()>) -> Result<T> {
    match (result, cleanup) {
        (result, Ok(())) => result,
        (Ok(_), Err(error)) => Err(error),
        (Err(operation), Err(cleanup)) => Err(Error::OperationAndCleanup {
            operation: Box::new(operation),
            cleanup: Box::new(cleanup),
        }),
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use crate::maybe_future::block_on;
    use std::task::{Context, Poll, Waker};

    #[derive(Default)]
    struct Writer {
        completed: Vec<(u8, u8)>,
        pause: Option<u8>,
        fail: Option<u8>,
    }

    impl RegisterWriter for Writer {
        async fn restore_register(&mut self, address: u8, value: u8) -> Result<()> {
            if self.pause == Some(address) {
                std::future::pending::<()>().await;
            }
            if self.fail == Some(address) {
                return Err(Error::Timeout);
            }
            self.completed.push((address, value));
            Ok(())
        }
    }

    #[test]
    fn restoration_is_deferred_until_owner_drops_and_resumes_without_losing_registers() {
        for paused in [1, 2, 3] {
            let (mut restoration, owner) = Restoration::new([(1, 10), (2, 20), (3, 30)].into());
            let mut writer = Writer::default();
            assert!(!block_on(restoration.drive(&mut writer)).unwrap());
            assert!(writer.completed.is_empty());
            drop(owner);
            writer.pause = Some(paused);
            {
                let mut operation = std::pin::pin!(restoration.drive(&mut writer));
                assert!(matches!(
                    operation
                        .as_mut()
                        .poll(&mut Context::from_waker(Waker::noop())),
                    Poll::Pending
                ));
            }
            writer.pause = None;
            writer.fail = Some(paused);
            assert!(block_on(restoration.drive(&mut writer)).is_err());
            writer.fail = None;
            assert!(block_on(restoration.drive(&mut writer)).unwrap());
            assert_eq!(writer.completed, [(1, 10), (2, 20), (3, 30)]);
        }
    }

    #[test]
    fn cleanup_does_not_hide_the_primary_failure() {
        let error = with_cleanup::<()>(Err(Error::TuningFailed), Err(Error::Timeout)).unwrap_err();
        let Error::OperationAndCleanup { operation, cleanup } = error else {
            panic!("missing context");
        };
        assert!(matches!(*operation, Error::TuningFailed));
        assert!(matches!(*cleanup, Error::Timeout));
    }

    #[tokio::test(flavor = "current_thread")]
    #[ignore = "requires a connected BladeRF1; run hardware tests sequentially"]
    async fn hardware_temporary_registers_survive_calibration_tuning_and_cancellation() -> Result<()>
    {
        use crate::Channel;
        use crate::bladerf1::hardware::lms6002d::dc_calibration::DcCalModule;
        use crate::bladerf1::{BladeRf1, TuningMode};
        use std::time::Duration;

        const REGISTERS: &[u8] = &[
            0x02, 0x03, 0x09, 0x32, 0x33, 0x36, 0x3f, 0x52, 0x53, 0x5f, 0x62, 0x63, 0x64, 0x65,
            0x68, 0x6e, 0x72, 0x75, 0x76,
        ];

        let mut device = BladeRf1::from_first().await?;
        let mut rf = device.rf_link_session().await?;
        rf.initialize(false).await?;
        let dc_cals = rf.get_dc_cals().await?;
        let frequency = rf.get_frequency(Channel::Rx).await?;
        let mut baseline = Vec::new();
        for &address in REGISTERS {
            baseline.push(
                rf.nios
                    .nios_read::<u8, u8>(NiosPkt8x8Target::Lms6, address)
                    .await?,
            );
        }
        let mut outcomes = Vec::new();
        let mut snapshots = Vec::new();
        let result: Result<()> = async {
            for module in [
                DcCalModule::LpfTuning,
                DcCalModule::TxLpf,
                DcCalModule::RxLpf,
                DcCalModule::RxVga2,
            ] {
                outcomes.push(rf.calibrate_dc(module).await);
                let mut snapshot = Vec::new();
                for &address in REGISTERS {
                    snapshot.push(
                        rf.nios
                            .nios_read::<u8, u8>(NiosPkt8x8Target::Lms6, address)
                            .await?,
                    );
                }
                snapshots.push(snapshot);
            }
            rf.set_frequency(Channel::Rx, 915_000_000, TuningMode::Host)
                .await?;
            let dsm: u8 = rf.nios.nios_read(NiosPkt8x8Target::Lms6, 0x09u8).await?;
            assert_eq!(dsm, baseline[2]);
            for delay in [5, 10, 20] {
                let _outcome = tokio::time::timeout(
                    Duration::from_millis(delay),
                    rf.calibrate_dc(DcCalModule::RxVga2),
                )
                .await;
                let mut snapshot = Vec::new();
                for &address in REGISTERS {
                    snapshot.push(
                        rf.nios
                            .nios_read::<u8, u8>(NiosPkt8x8Target::Lms6, address)
                            .await?,
                    );
                }
                snapshots.push(snapshot);
            }
            Ok(())
        }
        .await;
        let restore_cals = rf.set_dc_cals(dc_cals).await;
        let restore_frequency = rf
            .set_frequency(Channel::Rx, frequency, TuningMode::Host)
            .await;
        result?;
        restore_cals?;
        restore_frequency?;
        for outcome in outcomes {
            if let Err(error) = outcome {
                assert!(matches!(error, Error::CalibrationFailed(_)), "{error:?}");
            }
        }
        for snapshot in snapshots {
            assert_eq!(snapshot, baseline);
        }
        device.close().await
    }
}
