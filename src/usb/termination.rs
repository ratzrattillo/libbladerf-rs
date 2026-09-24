use super::pending::Pending;
use crate::maybe_future::NonWasmSend;
use crate::{Error, Result};
use std::future::Future;
use std::time::Duration;

#[derive(Default)]
pub(super) enum Termination {
    #[default]
    Open,
    Shutdown(Pending<()>),
    Closed,
    Reset(Pending<()>),
    ResetIssued,
}

impl Termination {
    pub(super) fn require_open(&self) -> Result<()> {
        match self {
            Self::Open => Ok(()),
            Self::Shutdown(_) => Err(Error::ShutdownInProgress),
            Self::Reset(_) => Err(Error::ResetInProgress),
            Self::Closed | Self::ResetIssued => Err(Error::DeviceClosed),
        }
    }

    pub(super) async fn shutdown(
        &mut self,
        operation: impl Future<Output = Result<()>> + NonWasmSend + 'static,
        timeout: Duration,
    ) -> Result<()> {
        match self {
            Self::Open => *self = Self::Shutdown(Pending::default()),
            Self::Shutdown(_) => {}
            Self::Closed => return Ok(()),
            Self::Reset(_) => return Err(Error::ResetInProgress),
            Self::ResetIssued => return Err(Error::DeviceClosed),
        }
        let Self::Shutdown(pending) = self else {
            unreachable!()
        };
        if !pending.is_pending() {
            pending.begin(operation);
        }
        pending.finish(timeout).await?;
        *self = Self::Closed;
        Ok(())
    }

    pub(super) async fn reset(
        &mut self,
        operation: impl Future<Output = Result<()>> + NonWasmSend + 'static,
        timeout: Duration,
    ) -> Result<()> {
        match self {
            Self::Shutdown(pending) => {
                pending.finish(timeout).await?;
            }
            Self::ResetIssued => return Err(Error::DeviceClosed),
            _ => {}
        }
        if !matches!(self, Self::Reset(_)) {
            let mut pending = Pending::default();
            pending.begin(operation);
            *self = Self::Reset(pending);
        }
        let Self::Reset(pending) = self else {
            unreachable!()
        };
        let result = pending.finish(timeout).await.map(|_| ());
        if !pending.is_pending() {
            *self = Self::ResetIssued;
        }
        result
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use crate::maybe_future::block_on;
    use std::sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    };
    use std::task::{Context, Poll, Waker};

    #[test]
    fn shutdown_resumes_the_retained_operation_and_blocks_regular_io() {
        let ready = Arc::new(AtomicBool::new(false));
        let gate = ready.clone();
        let calls = Arc::new(AtomicUsize::new(0));
        let count = calls.clone();
        let operation = async move {
            count.fetch_add(1, Ordering::Relaxed);
            std::future::poll_fn(|_| {
                if gate.load(Ordering::Relaxed) {
                    Poll::Ready(Ok(()))
                } else {
                    Poll::Pending
                }
            })
            .await
        };
        let mut termination = Termination::Open;
        {
            let mut wait = std::pin::pin!(termination.shutdown(operation, Duration::MAX));
            assert!(
                wait.as_mut()
                    .poll(&mut Context::from_waker(Waker::noop()))
                    .is_pending()
            );
        }
        assert!(matches!(
            termination.require_open(),
            Err(Error::ShutdownInProgress)
        ));
        assert!(matches!(
            block_on(termination.shutdown(async { panic!("replacement was run") }, Duration::ZERO)),
            Err(Error::Timeout)
        ));
        ready.store(true, Ordering::Relaxed);
        block_on(termination.shutdown(async { panic!("replacement was run") }, Duration::ZERO))
            .unwrap();
        assert_eq!(calls.load(Ordering::Relaxed), 1);
        assert!(matches!(
            termination.require_open(),
            Err(Error::DeviceClosed)
        ));
        block_on(termination.shutdown(async { panic!("closed device was used") }, Duration::ZERO))
            .unwrap();
    }

    #[test]
    fn failed_shutdown_is_retryable_but_an_observed_reset_is_not_replayed() {
        let mut termination = Termination::Open;
        assert!(
            block_on(termination.shutdown(async { Err(Error::Timeout) }, Duration::MAX)).is_err()
        );
        assert!(matches!(
            termination.require_open(),
            Err(Error::ShutdownInProgress)
        ));
        block_on(termination.shutdown(async { Ok(()) }, Duration::MAX)).unwrap();
        assert!(block_on(termination.reset(async { Err(Error::Timeout) }, Duration::MAX)).is_err());
        assert!(matches!(
            termination.require_open(),
            Err(Error::DeviceClosed)
        ));
        assert!(matches!(
            block_on(termination.reset(async { panic!("reset was replayed") }, Duration::MAX)),
            Err(Error::DeviceClosed)
        ));
    }

    #[test]
    fn reset_timeout_keeps_the_original_request_until_observed() {
        let ready = Arc::new(AtomicBool::new(false));
        let gate = ready.clone();
        let operation = std::future::poll_fn(move |_| {
            if gate.load(Ordering::Relaxed) {
                Poll::Ready(Ok(()))
            } else {
                Poll::Pending
            }
        });
        let mut termination = Termination::Open;
        assert!(matches!(
            block_on(termination.reset(operation, Duration::ZERO)),
            Err(Error::Timeout)
        ));
        assert!(matches!(
            termination.require_open(),
            Err(Error::ResetInProgress)
        ));
        ready.store(true, Ordering::Relaxed);
        block_on(termination.reset(async { panic!("replacement reset") }, Duration::ZERO)).unwrap();
        assert!(matches!(
            termination.require_open(),
            Err(Error::DeviceClosed)
        ));
    }
}
