use super::{BulkEndpoint, require_length};
use crate::error::{Error, Result};
use crate::protocol::nios::NiosPacketError;
use nusb::transfer::Buffer;
use std::time::Duration;

const PACKET_SIZE: usize = 16;

enum Transaction {
    Idle { out: Buffer, input: Buffer },
    Sending { input: Buffer },
    Receiving { out: Buffer },
    Unsynchronized,
}

pub(super) struct NiosExchange<O: BulkEndpoint, I: BulkEndpoint> {
    out: O,
    input: I,
    transaction: Transaction,
}

impl<O: BulkEndpoint, I: BulkEndpoint> NiosExchange<O, I> {
    pub(super) fn new(out: O, input: I) -> Self {
        let transaction = Transaction::Idle {
            out: out.allocate(PACKET_SIZE),
            input: input.allocate(input.max_packet_size()),
        };
        Self {
            out,
            input,
            transaction,
        }
    }

    pub(super) async fn finish(&mut self, timeout: Duration) -> Result<()> {
        loop {
            match self.transaction {
                Transaction::Idle { .. } => return Ok(()),
                Transaction::Unsynchronized => return Err(Error::RecoveryRequired),
                Transaction::Sending { .. } => {
                    let completion =
                        crate::maybe_future::timeout(timeout, self.out.next_complete())
                            .await
                            .ok_or(Error::Timeout)?;
                    let previous =
                        std::mem::replace(&mut self.transaction, Transaction::Unsynchronized);
                    let Transaction::Sending { mut input } = previous else {
                        unreachable!()
                    };
                    completion.status?;
                    require_length(PACKET_SIZE, completion.actual_len)?;
                    input.clear();
                    input.set_requested_len(self.input.max_packet_size());
                    self.input.submit(input);
                    self.transaction = Transaction::Receiving {
                        out: completion.buffer,
                    };
                }
                Transaction::Receiving { .. } => {
                    let completion =
                        crate::maybe_future::timeout(timeout, self.input.next_complete())
                            .await
                            .ok_or(Error::Timeout)?;
                    let previous =
                        std::mem::replace(&mut self.transaction, Transaction::Unsynchronized);
                    let Transaction::Receiving { out } = previous else {
                        unreachable!()
                    };
                    completion.status?;
                    require_length(PACKET_SIZE, completion.actual_len)?;
                    require_length(PACKET_SIZE, completion.buffer.len())?;
                    validate_reply(&out, &completion.buffer)?;
                    self.transaction = Transaction::Idle {
                        out,
                        input: completion.buffer,
                    };
                }
            }
        }
    }

    pub(super) async fn exchange(
        &mut self,
        request: &[u8; PACKET_SIZE],
        timeout: Duration,
    ) -> Result<[u8; PACKET_SIZE]> {
        self.finish(timeout).await?;
        let previous = std::mem::replace(&mut self.transaction, Transaction::Unsynchronized);
        let Transaction::Idle { mut out, input } = previous else {
            unreachable!()
        };
        out.clear();
        out.extend_from_slice(request);
        self.out.submit(out);
        self.transaction = Transaction::Sending { input };
        self.finish(timeout).await?;
        let Transaction::Idle { input, .. } = &self.transaction else {
            unreachable!()
        };
        Ok(input[..]
            .try_into()
            .expect("validated NIOS response length"))
    }
}

fn validate_reply(request: &[u8], response: &[u8]) -> Result<()> {
    if request[0] != response[0] {
        return Err(NiosPacketError::MagicMismatch {
            expected: request[0],
            actual: response[0],
        }
        .into());
    }
    let address_size = match request[0] {
        0x41..=0x44 => 1,
        0x45 => 2,
        0x4b => 4,
        0x54 => return Ok(()),
        _ => return Err(NiosPacketError::InvalidTypeCombination.into()),
    };
    if request[1] != response[1]
        || ((request[2] ^ response[2]) & 1) != 0
        || request[4..4 + address_size] != response[4..4 + address_size]
    {
        return Err(NiosPacketError::ResponseMismatch.into());
    }
    Ok(())
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use crate::maybe_future::{Op, block_on};
    use nusb::MaybeFuture;
    use nusb::transfer::{Completion, TransferError};
    use std::collections::VecDeque;
    use std::future::Future;
    use std::sync::{Arc, Mutex};
    use std::task::{Context, Poll, Waker};

    #[derive(Default)]
    struct State {
        pending: VecDeque<Buffer>,
        ready: VecDeque<(Vec<u8>, std::result::Result<(), TransferError>)>,
        submitted: Vec<Vec<u8>>,
    }

    struct Endpoint {
        state: Arc<Mutex<State>>,
        input: bool,
        cancellable: bool,
    }

    impl BulkEndpoint for Endpoint {
        fn max_packet_size(&self) -> usize {
            512
        }
        fn allocate(&self, len: usize) -> Buffer {
            Buffer::new(len)
        }
        fn submit(&mut self, buffer: Buffer) {
            let mut state = self.state.lock().unwrap();
            state.submitted.push(buffer.to_vec());
            state.pending.push_back(buffer);
        }
        fn pending(&self) -> usize {
            self.state.lock().unwrap().pending.len()
        }
        fn poll_next_complete(&mut self, _: &mut Context<'_>) -> Poll<Completion> {
            let mut state = self.state.lock().unwrap();
            let Some((bytes, status)) = state.ready.pop_front() else {
                return Poll::Pending;
            };
            let mut buffer = state.pending.pop_front().unwrap();
            if self.input {
                buffer.clear();
                buffer.extend_from_slice(&bytes);
            }
            Poll::Ready(Completion {
                actual_len: buffer.len(),
                buffer,
                status,
            })
        }
        fn wait_next_complete(&mut self, _: Duration) -> Option<Completion> {
            match self.poll_next_complete(&mut Context::from_waker(Waker::noop())) {
                Poll::Ready(completion) => Some(completion),
                Poll::Pending => None,
            }
        }
        fn can_cancel(&self) -> bool {
            self.cancellable
        }
        fn cancel_all(&mut self) {
            panic!("a timed-out NIOS request must retain completion ownership");
        }
        fn clear_halt(
            &mut self,
        ) -> impl MaybeFuture<Output = std::result::Result<(), nusb::Error>> + 'static {
            Op::new(async { Ok(()) })
        }
    }

    fn request(address: u8) -> [u8; 16] {
        let mut request = [0; 16];
        request[0] = 0x43;
        request[1] = 1;
        request[4] = address;
        request
    }

    fn response(address: u8, value: u8) -> Vec<u8> {
        let mut response = request(address).to_vec();
        response[2] = 2;
        response[5] = value;
        response
    }

    #[test]
    fn cancellation_preserves_both_transaction_phases_and_settles_old_response() {
        for cancellable in [false, true] {
            for cancel_after_send in [false, true] {
                let out = Arc::new(Mutex::new(State::default()));
                let input = Arc::new(Mutex::new(State::default()));
                let mut exchange = NiosExchange::new(
                    Endpoint {
                        state: out.clone(),
                        input: false,
                        cancellable,
                    },
                    Endpoint {
                        state: input.clone(),
                        input: true,
                        cancellable,
                    },
                );
                let first = request(7);
                let second = request(7);
                let mut cx = Context::from_waker(Waker::noop());
                {
                    let mut future =
                        std::pin::pin!(exchange.exchange(&first, Duration::from_secs(1)));
                    assert!(future.as_mut().poll(&mut cx).is_pending());
                    if cancel_after_send {
                        out.lock().unwrap().ready.push_back((vec![], Ok(())));
                        assert!(future.as_mut().poll(&mut cx).is_pending());
                    }
                }
                if !cancel_after_send {
                    out.lock().unwrap().ready.push_back((vec![], Ok(())));
                }
                let mut future = std::pin::pin!(exchange.exchange(&second, Duration::from_secs(1)));
                assert!(future.as_mut().poll(&mut cx).is_pending());
                assert_eq!(out.lock().unwrap().submitted.len(), 1);
                input
                    .lock()
                    .unwrap()
                    .ready
                    .push_back((response(7, 11), Ok(())));
                assert!(future.as_mut().poll(&mut cx).is_pending());
                assert_eq!(out.lock().unwrap().submitted.len(), 2);
                out.lock().unwrap().ready.push_back((vec![], Ok(())));
                assert!(future.as_mut().poll(&mut cx).is_pending());
                input
                    .lock()
                    .unwrap()
                    .ready
                    .push_back((response(7, 22), Ok(())));
                let Poll::Ready(Ok(reply)) = future.as_mut().poll(&mut cx) else {
                    panic!("missing response");
                };
                assert_eq!(reply[5], 22);
                assert!(out.lock().unwrap().pending.is_empty());
                assert!(input.lock().unwrap().pending.is_empty());
            }
        }
    }

    #[test]
    fn timeout_keeps_pending_request_and_mismatched_reply_blocks_reuse() {
        let out = Arc::new(Mutex::new(State::default()));
        let input = Arc::new(Mutex::new(State::default()));
        let mut exchange = NiosExchange::new(
            Endpoint {
                state: out.clone(),
                input: false,
                cancellable: false,
            },
            Endpoint {
                state: input.clone(),
                input: true,
                cancellable: false,
            },
        );
        assert!(matches!(
            block_on(exchange.exchange(&request(7), Duration::ZERO)),
            Err(Error::Timeout)
        ));
        assert_eq!(out.lock().unwrap().pending.len(), 1);
        out.lock().unwrap().ready.push_back((vec![], Ok(())));
        input
            .lock()
            .unwrap()
            .ready
            .push_back((response(8, 1), Ok(())));
        assert!(matches!(
            block_on(exchange.exchange(&request(9), Duration::ZERO)),
            Err(Error::NiosPacket(_))
        ));
        assert!(matches!(
            block_on(exchange.exchange(&request(9), Duration::ZERO)),
            Err(Error::RecoveryRequired)
        ));
        assert_eq!(out.lock().unwrap().submitted.len(), 1);
    }
}
