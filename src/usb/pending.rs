use crate::error::{Error, Result};
use crate::maybe_future::NonWasmSend;
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;
use sync_wrapper::SyncWrapper;

#[cfg(not(target_arch = "wasm32"))]
type OwnedFuture<T> = Pin<Box<dyn Future<Output = Result<T>> + Send>>;
#[cfg(target_arch = "wasm32")]
type OwnedFuture<T> = Pin<Box<dyn Future<Output = Result<T>>>>;

pub(crate) struct Pending<T>(SyncWrapper<Option<OwnedFuture<T>>>);

impl<T> Default for Pending<T> {
    fn default() -> Self {
        Self(SyncWrapper::new(None))
    }
}

impl<T> Pending<T> {
    pub(crate) fn is_pending(&mut self) -> bool {
        self.0.get_mut().is_some()
    }

    pub(crate) fn begin(
        &mut self,
        operation: impl Future<Output = Result<T>> + NonWasmSend + 'static,
    ) {
        assert!(!self.is_pending());
        *self.0.get_mut() = Some(Box::pin(operation));
    }

    pub(crate) async fn finish(&mut self, timeout: Duration) -> Result<Option<T>> {
        let Some(operation) = self.0.get_mut().as_mut() else {
            return Ok(None);
        };
        let result = crate::maybe_future::timeout(timeout, operation)
            .await
            .ok_or(Error::Timeout)?;
        *self.0.get_mut() = None;
        result.map(Some)
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use crate::maybe_future::block_on;
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    use std::task::{Context, Poll, Waker};

    #[test]
    fn cancelled_wait_keeps_the_original_operation_until_completion() {
        let ready = Arc::new(AtomicBool::new(false));
        let gate = ready.clone();
        let mut pending = Pending::default();
        pending.begin(std::future::poll_fn(move |_| {
            if gate.load(Ordering::Relaxed) {
                Poll::Ready(Ok(42))
            } else {
                Poll::Pending
            }
        }));
        {
            let mut wait = std::pin::pin!(pending.finish(Duration::MAX));
            assert!(
                wait.as_mut()
                    .poll(&mut Context::from_waker(Waker::noop()))
                    .is_pending()
            );
        }
        assert!(pending.is_pending());
        assert!(matches!(
            block_on(pending.finish(Duration::ZERO)),
            Err(Error::Timeout)
        ));
        assert!(pending.is_pending());
        ready.store(true, Ordering::Relaxed);
        assert_eq!(block_on(pending.finish(Duration::ZERO)).unwrap(), Some(42));
        assert!(!pending.is_pending());
    }
}
