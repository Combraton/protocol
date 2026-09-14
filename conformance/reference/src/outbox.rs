//! Pending output for one connection, written by its own thread (CORE section 16.5, backpressure).
//!
//! The session thread never blocks on a consumer that stops reading. It queues frames here and
//! can see how many bytes are produced but not yet written, which is the bound backpressure
//! enforces.

use std::collections::VecDeque;
use std::io::Write;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

#[derive(Default)]
struct State {
    queue: VecDeque<Vec<u8>>,
    pending: usize,
    queued: u64,
    written: u64,
    failed: bool,
    closed: bool,
    abandoned: bool,
}

pub struct Outbox {
    state: Mutex<State>,
    changed: Condvar,
}

impl Outbox {
    /// Start the writer thread for `out`.
    pub fn start<W: Write + Send + 'static>(mut out: W) -> Arc<Self> {
        let outbox = Arc::new(Self {
            state: Mutex::new(State::default()),
            changed: Condvar::new(),
        });
        let writer = outbox.clone();
        std::thread::spawn(move || {
            loop {
                let frame = {
                    let mut state = writer.lock();
                    while state.queue.is_empty() && !state.closed {
                        state = writer
                            .changed
                            .wait(state)
                            .unwrap_or_else(std::sync::PoisonError::into_inner);
                    }
                    if state.abandoned {
                        return;
                    }
                    match state.queue.front() {
                        Some(frame) => frame.clone(),
                        None => return,
                    }
                };
                let ok = out.write_all(&frame).and_then(|()| out.flush()).is_ok();
                let mut state = writer.lock();
                if state.abandoned {
                    return;
                }
                if !ok {
                    state.failed = true;
                    writer.changed.notify_all();
                    return;
                }
                state.queue.pop_front();
                state.pending -= frame.len();
                state.written += 1;
                writer.changed.notify_all();
            }
        });
        outbox
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Queue a frame. Returns false once writing has failed.
    pub fn push(&self, frame: Vec<u8>) -> bool {
        let mut state = self.lock();
        if state.failed {
            return false;
        }
        state.pending += frame.len();
        state.queued += 1;
        state.queue.push_back(frame);
        self.changed.notify_all();
        true
    }

    /// Bytes produced but not yet written.
    pub fn pending(&self) -> usize {
        self.lock().pending
    }

    pub fn queued(&self) -> u64 {
        self.lock().queued
    }

    /// Wait until `count` frames have been written, writing failed, or `timeout` passed.
    pub fn wait_written(&self, count: u64, timeout: Option<Duration>) -> bool {
        let deadline = timeout.map(|t| Instant::now() + t);
        let mut state = self.lock();
        loop {
            if state.written >= count {
                return true;
            }
            if state.failed {
                return false;
            }
            match deadline {
                None => {
                    state = self
                        .changed
                        .wait(state)
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                }
                Some(deadline) => {
                    let now = Instant::now();
                    if now >= deadline {
                        return false;
                    }
                    state = self
                        .changed
                        .wait_timeout(state, deadline - now)
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .0;
                }
            }
        }
    }

    /// Wait until everything queued has been written. Returns false if writing failed or
    /// `timeout` passed first.
    pub fn wait_drained(&self, timeout: Duration) -> bool {
        let queued = self.queued();
        self.wait_written(queued, Some(timeout))
    }

    /// Discard everything not yet written; the writer stops after any write in progress.
    pub fn abandon(&self) {
        let mut state = self.lock();
        state.abandoned = true;
        state.closed = true;
        state.queue.clear();
        state.pending = 0;
        self.changed.notify_all();
    }

    /// Stop the writer once the queue is empty.
    pub fn close(&self) {
        self.lock().closed = true;
        self.changed.notify_all();
    }
}
