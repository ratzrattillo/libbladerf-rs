//! Sync/async bridging primitives.
//!
//! Every I/O method in this crate returns an [`nusb::MaybeFuture`], which
//! callers either `.wait()` (blocking, native only) or `.await`. Internally
//! the driver is written as plain `async` code; [`Op`] wraps such a future
//! so it satisfies `MaybeFuture` with a runtime-free executor for `wait()`.
//!
//! nusb's semantics are mirrored exactly: bulk and control transfers are
//! real futures completed by nusb's event thread, while device open,
//! interface claim, alternate setting and clear halt are blocking syscalls
//! that nusb offloads through its `smol` or `tokio` feature when awaited.
//! Enable one of this crate's `smol`/`tokio` features for native async use.

use nusb::MaybeFuture;
use std::future::{Future, IntoFuture};
use std::time::Duration;

#[cfg(not(target_arch = "wasm32"))]
pub(crate) use std::marker::Send as NonWasmSend;

#[cfg(target_arch = "wasm32")]
pub(crate) trait NonWasmSend {}
#[cfg(target_arch = "wasm32")]
impl<T: ?Sized> NonWasmSend for T {}

/// Wraps a future so it implements [`MaybeFuture`].
///
/// `Op` is never named in public signatures; methods return
/// `impl MaybeFuture<Output = T>` and the concrete future stays hidden.
pub(crate) struct Op<F>(F);

impl<F> Op<F> {
    pub(crate) fn new(future: F) -> Self {
        Self(future)
    }
}

impl<F: Future> IntoFuture for Op<F> {
    type Output = F::Output;
    type IntoFuture = F;

    fn into_future(self) -> F {
        self.0
    }
}

impl<F: Future + NonWasmSend> MaybeFuture for Op<F> {
    #[cfg(not(target_arch = "wasm32"))]
    fn wait(self) -> F::Output {
        #[cfg(feature = "tokio")]
        let _context = tokio_context();
        block_on(self.0)
    }
}

/// Enters a tokio runtime context if the current thread has none.
///
/// With the `tokio` feature nusb resolves blocking syscalls through
/// `tokio::task::spawn_blocking`, which requires a runtime context even
/// when the future is driven by [`block_on`]. Synchronous callers outside
/// tokio get a lazily created runtime whose blocking pool serves them.
#[cfg(all(feature = "tokio", not(target_arch = "wasm32")))]
fn tokio_context() -> Option<tokio::runtime::EnterGuard<'static>> {
    use std::sync::LazyLock;

    static RUNTIME: LazyLock<tokio::runtime::Runtime> = LazyLock::new(|| {
        tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("failed to build the fallback tokio runtime")
    });

    if tokio::runtime::Handle::try_current().is_ok() {
        None
    } else {
        Some(RUNTIME.enter())
    }
}

/// Drives a future to completion on the current thread.
///
/// Wakeups are delivered by unparking this thread, which works for any
/// future whose readiness is signalled from another thread (nusb's event
/// thread, `futures-timer`'s timer thread) or that is immediately ready.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn block_on<F: Future>(future: F) -> F::Output {
    use std::pin::pin;
    use std::sync::Arc;
    use std::task::{Context, Poll, Wake, Waker};
    use std::thread::{self, Thread};

    struct ThreadWaker(Thread);

    impl Wake for ThreadWaker {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }

        fn wake_by_ref(self: &Arc<Self>) {
            self.0.unpark();
        }
    }

    let waker = Waker::from(Arc::new(ThreadWaker(thread::current())));
    let mut cx = Context::from_waker(&waker);
    let mut future = pin!(future);
    loop {
        match future.as_mut().poll(&mut cx) {
            Poll::Ready(output) => return output,
            Poll::Pending => thread::park(),
        }
    }
}

/// Sleeps for `duration` without blocking the executor.
///
/// Sub-millisecond delays on native targets use `thread::sleep`; the
/// timer-thread round trip would dominate such short waits.
pub(crate) async fn sleep(duration: Duration) {
    #[cfg(not(target_arch = "wasm32"))]
    if duration < Duration::from_millis(1) {
        std::thread::sleep(duration);
        return;
    }
    futures_timer::Delay::new(duration).await
}

/// Awaits `future` for at most `timeout`, returning `None` on expiry.
///
/// The inner future is dropped on timeout. Callers that must not leave a
/// USB transfer pending cancel it afterwards.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) async fn timeout<F: Future>(timeout: Duration, future: F) -> Option<F::Output> {
    use std::pin::pin;
    use std::task::Poll;

    if timeout == Duration::MAX {
        return Some(future.await);
    }
    let mut future = pin!(future);
    let mut delay = pin!(futures_timer::Delay::new(timeout));
    std::future::poll_fn(|cx| {
        if let Poll::Ready(output) = future.as_mut().poll(cx) {
            return Poll::Ready(Some(output));
        }
        if delay.as_mut().poll(cx).is_ready() {
            return Poll::Ready(None);
        }
        Poll::Pending
    })
    .await
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use std::pin::Pin;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::task::{Context, Poll, Waker};

    #[test]
    fn block_on_ready_future() {
        assert_eq!(block_on(async { 42 }), 42);
    }

    #[test]
    fn block_on_future_woken_from_another_thread() {
        let flag = Arc::new(AtomicBool::new(false));
        let mut waker_slot: Option<Waker> = None;
        let flag2 = Arc::clone(&flag);
        let fut = std::future::poll_fn(move |cx| {
            if flag2.load(Ordering::Acquire) {
                Poll::Ready(7)
            } else {
                if waker_slot.is_none() {
                    let w = cx.waker().clone();
                    let f = Arc::clone(&flag2);
                    std::thread::spawn(move || {
                        std::thread::sleep(Duration::from_millis(20));
                        f.store(true, Ordering::Release);
                        w.wake();
                    });
                    waker_slot = Some(cx.waker().clone());
                }
                Poll::Pending
            }
        });
        assert_eq!(block_on(fut), 7);
    }

    #[test]
    fn op_wait_and_await_agree() {
        let sync = Op::new(async { 1 + 1 }).wait();
        let mut cx = Context::from_waker(Waker::noop());
        let mut fut = Box::pin(Op::new(async { 1 + 1 }).into_future());
        let Poll::Ready(asynchronous) = Pin::new(&mut fut).poll(&mut cx) else {
            panic!("ready future returned Pending");
        };
        assert_eq!(sync, asynchronous);
    }

    #[test]
    fn sleep_elapses() {
        let start = std::time::Instant::now();
        block_on(sleep(Duration::from_millis(15)));
        assert!(start.elapsed() >= Duration::from_millis(15));
    }

    #[test]
    fn timeout_returns_none_on_expiry() {
        let never = std::future::pending::<()>();
        assert!(block_on(timeout(Duration::from_millis(10), never)).is_none());
    }

    #[test]
    fn timeout_returns_some_when_ready() {
        assert_eq!(
            block_on(timeout(Duration::from_secs(1), async { 5 })),
            Some(5)
        );
    }

    #[test]
    fn op_is_send_when_future_is_send() {
        fn assert_send<T: Send>(_: &T) {}
        let op = Op::new(async { 0u8 });
        assert_send(&op);
    }
}
