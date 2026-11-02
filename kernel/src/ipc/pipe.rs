// SPDX-License-Identifier: MPL-2.0
//! The kernel pipe object (Lane 1, L1a.1 step 2a; `docs/THINK-2026-11-02-l1a1-pipe-wiring.md`).
//!
//! One [`PipeObject`] holds the step-1 state machine ([`pipe_logic::Pipe`]),
//! its storage and its parked waiters under ONE lock, so the storage can't be
//! paired with the wrong state (Dexter #46 note 5.1).
//!
//! **Parking** is by wait token, as the futex does: the token is installed
//! while the pipe's guard is held, and the guard is released atomically with
//! the context switch (`try_block_current_with_token_and_guard`). A writer
//! that runs between "it was empty" and the park either sees the token and
//! wakes it, or runs before the guard is taken, and the reader then sees the
//! bytes (CP-14). A force-kill finds a token-parked thread by pid
//! (`zombify_parked_thread_by_pid`), so no `Box<Thread>` lives in the pipe
//! (CP-33); a stale token in the lists is harmless (`wake_thread_token` →
//! `Gone`).
//!
//! **Lock order:** `PipeObject.state` → scheduler (the park). Wakes happen
//! after the guard is dropped (CP-16, as the futex), and so do the
//! `SYS_WAIT_ANY` fires.
//!
//! **Endpoints** are counted per handle-table entry (`HandleTable`'s
//! `retain_pipe` / `release_pipe`), not per process: a granted end is one more
//! endpoint, and only the last close of an end wakes the other side.

use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec::Vec;

use super::pipe_logic::{self, End, Pipe, PipeError, ReadOutcome, Wake, WriteOutcome};
use crate::errno::{EAGAIN, EINVAL, ENOMEM, EPIPE};
use crate::sched::scheduler::{self, ThreadWakeToken};
use crate::sync::IrqSafeMutex;

/// Parked readers (and, separately, writers) per pipe. A terminal has one
/// of each; more parkers than this first wake and clear the list (the stale
/// tokens of killed threads go with it), then park.
pub const PARK_MAX: usize = 8;

struct Inner {
    logic: Pipe,
    storage: Box<[u8]>,
    readers: [Option<ThreadWakeToken>; PARK_MAX],
    writers: [Option<ThreadWakeToken>; PARK_MAX],
}

pub struct PipeObject {
    /// Fixed at creation; outside the lock, so the object-quota weight
    /// (computed under `CAP_GRAPH`) takes no pipe lock.
    capacity: usize,
    state: IrqSafeMutex<Inner>,
}

/// The tokens a state change must wake, collected under the guard and woken
/// after it is dropped.
struct Wakes {
    tokens: [Option<ThreadWakeToken>; 2 * PARK_MAX],
    n: usize,
    readers: bool,
    writers: bool,
}

impl Wakes {
    const fn none() -> Self {
        Self {
            tokens: [None; 2 * PARK_MAX],
            n: 0,
            readers: false,
            writers: false,
        }
    }

    fn take(inner: &mut Inner, wake: Wake) -> Self {
        let mut w = Self::none();
        w.readers = wake.readers;
        w.writers = wake.writers;
        if wake.readers {
            w.drain(&mut inner.readers);
        }
        if wake.writers {
            w.drain(&mut inner.writers);
        }
        w
    }

    fn drain(&mut self, list: &mut [Option<ThreadWakeToken>; PARK_MAX]) {
        for slot in list.iter_mut() {
            if let Some(token) = slot.take() {
                self.tokens[self.n] = Some(token);
                self.n += 1;
            }
        }
    }

    /// Wake everyone collected, then fire the wait keys. The caller has
    /// dropped the pipe's guard.
    #[cfg_attr(not(feature = "wait-multiplex"), allow(unused_variables))]
    fn fire(self, pipe: &PipeObject) {
        for token in self.tokens.iter().take(self.n).flatten() {
            let _ = scheduler::wake_thread_token(*token, 0);
        }
        #[cfg(feature = "wait-multiplex")]
        {
            if self.readers {
                crate::sched::wait_any::fire_channel(pipe.wait_key(End::Read));
            }
            if self.writers {
                crate::sched::wait_any::fire_channel(pipe.wait_key(End::Write));
            }
        }
    }
}

/// How a blocking call ended without completing.
enum Parked {
    /// Woken (or the list was cleared): try again.
    Retry,
    /// Can't park: `EAGAIN` (token table full) or `ESRCH` (the process is
    /// being torn down).
    Fail(i64),
}

impl PipeObject {
    /// A new pipe of `capacity` bytes with one read end and one write end
    /// (their handles are installed by the caller). `EINVAL` for a capacity
    /// outside `1..=MAX_PIPE_CAPACITY`; `ENOMEM` if the storage can't be had.
    pub fn new(capacity: usize) -> Result<Arc<Self>, i64> {
        let logic = Pipe::new_unattached(capacity).map_err(|_| EINVAL)?;
        let mut storage = Vec::new();
        storage.try_reserve_exact(capacity).map_err(|_| ENOMEM)?;
        storage.resize(capacity, 0);
        Ok(Arc::new(Self {
            capacity,
            state: IrqSafeMutex::new(Inner {
                logic,
                storage: storage.into_boxed_slice(),
                readers: [None; PARK_MAX],
                writers: [None; PARK_MAX],
            }),
        }))
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// The `SYS_WAIT_ANY` key for a waiter on `end`: the channel key's
    /// shape (an aligned pointer with the end in bit 0).
    pub fn wait_key(&self, end: End) -> u64 {
        (self as *const Self as u64) | (end_bit(end) & 1)
    }

    /// `SYS_WAIT_ANY` readiness for a handle on `end`: a read end is ready
    /// with bytes or at EOF, a write end with room or at EPIPE (Dexter #46
    /// note 5.7). An EOF'd read end stays ready.
    pub fn ready(&self, end: End) -> bool {
        let g = self.state.lock();
        match end {
            End::Read => g.logic.readable(),
            End::Write => g.logic.writable(),
        }
    }

    /// One more handle-table entry on `end`.
    pub fn retain(&self, end: End) {
        let r = self.state.lock().logic.add_endpoint(end);
        // A handle is only ever copied from a live entry, so its end is
        // open, and a u32 count can't overflow a bounded handle table.
        debug_assert!(r.is_ok(), "pipe retain on a closed end: {r:?}");
    }

    /// A handle-table entry on `end` left (close, revoke, drain). The last
    /// one of an end wakes everyone parked on the other.
    pub fn release(&self, end: End) {
        let wakes = {
            let mut g = self.state.lock();
            match g.logic.close_endpoint(end) {
                Ok(wake) => Wakes::take(&mut g, wake),
                Err(PipeError::NoSuchEndpoint) => {
                    debug_assert!(false, "pipe release of an end with no endpoints");
                    Wakes::none()
                }
                Err(_) => Wakes::none(),
            }
        };
        wakes.fire(self);
    }

    /// One non-blocking read into `dst`: `Ok(n)` (n == 0 at EOF, or for an
    /// empty `dst`), or `Err(EAGAIN)`.
    pub fn try_read(&self, dst: &mut [u8]) -> Result<usize, i64> {
        let (result, wakes) = {
            let mut g = self.state.lock();
            let Inner { logic, storage, .. } = &mut *g;
            match logic.read(storage, dst) {
                Ok((ReadOutcome::Read(n), wake)) => (Ok(n), Wakes::take(&mut g, wake)),
                Ok((ReadOutcome::Eof, _)) => (Ok(0), Wakes::none()),
                Ok((ReadOutcome::WouldBlock, _)) => (Err(EAGAIN), Wakes::none()),
                Err(_) => (Err(EINVAL), Wakes::none()),
            }
        };
        wakes.fire(self);
        result
    }

    /// One non-blocking write of `src` (atomic when it is at most
    /// `PIPE_ATOMIC` bytes and fits the capacity): `Ok(n)`, `Err(EAGAIN)` or
    /// `Err(EPIPE)`.
    pub fn try_write(&self, src: &[u8]) -> Result<usize, i64> {
        let (result, wakes) = {
            let mut g = self.state.lock();
            let Inner { logic, storage, .. } = &mut *g;
            match logic.write(storage, src) {
                Ok((WriteOutcome::Written(n), wake)) => (Ok(n), Wakes::take(&mut g, wake)),
                Ok((WriteOutcome::BrokenPipe, _)) => (Err(EPIPE), Wakes::none()),
                Ok((WriteOutcome::WouldBlock, _)) => (Err(EAGAIN), Wakes::none()),
                Err(_) => (Err(EINVAL), Wakes::none()),
            }
        };
        wakes.fire(self);
        result
    }

    /// A blocking read into `dst`: parks until there are bytes or EOF.
    /// `Ok(n)` or `Err(EAGAIN | ESRCH)` when it can't park.
    pub fn read_blocking(&self, dst: &mut [u8]) -> Result<usize, i64> {
        loop {
            let mut g = self.state.lock();
            let Inner { logic, storage, .. } = &mut *g;
            match logic.read(storage, dst) {
                Ok((ReadOutcome::Read(n), wake)) => {
                    let wakes = Wakes::take(&mut g, wake);
                    drop(g);
                    wakes.fire(self);
                    return Ok(n);
                }
                Ok((ReadOutcome::Eof, _)) => return Ok(0),
                Ok((ReadOutcome::WouldBlock, _)) => {}
                Err(_) => return Err(EINVAL),
            }
            match self.park(g, End::Read) {
                Parked::Retry => continue,
                Parked::Fail(e) => return Err(e),
            }
        }
    }

    /// A blocking write of `src` (one chunk; atomic when at most
    /// `PIPE_ATOMIC` bytes and within the capacity): parks until some of it
    /// (or, atomically, all of it) is stored, or the last reader is gone.
    /// `Ok(n)` with `n > 0`, `Err(EPIPE)`, or `Err(EAGAIN | ESRCH)`.
    pub fn write_blocking(&self, src: &[u8]) -> Result<usize, i64> {
        loop {
            let mut g = self.state.lock();
            let Inner { logic, storage, .. } = &mut *g;
            match logic.write(storage, src) {
                Ok((WriteOutcome::Written(n), wake)) => {
                    let wakes = Wakes::take(&mut g, wake);
                    drop(g);
                    wakes.fire(self);
                    return Ok(n);
                }
                Ok((WriteOutcome::BrokenPipe, _)) => return Err(EPIPE),
                Ok((WriteOutcome::WouldBlock, _)) => {}
                Err(_) => return Err(EINVAL),
            }
            match self.park(g, End::Write) {
                Parked::Retry => continue,
                Parked::Fail(e) => return Err(e),
            }
        }
    }

    /// Park the current thread as a waiter for `side` (`Read`: waiting for
    /// bytes; `Write`: waiting for room). Takes the guard of the failed
    /// attempt, so nothing changes between the check and the park (CP-14).
    fn park(
        &self,
        mut g: crate::sync::irqsafe_mutex::IrqSafeGuard<'_, Inner>,
        side: End,
    ) -> Parked {
        let list = match side {
            End::Read => &mut g.readers,
            End::Write => &mut g.writers,
        };
        let Some(slot) = list.iter().position(Option::is_none) else {
            // Full: wake and clear everyone on this side (stale tokens of
            // killed threads included), then retry. Live waiters re-check.
            let mut wakes = Wakes::none();
            wakes.drain(list);
            drop(g);
            wakes.fire(self);
            return Parked::Retry;
        };
        match scheduler::try_block_current_with_token_and_guard(g, |inner: &mut Inner, token| {
            match side {
                End::Read => inner.readers[slot] = Some(token),
                End::Write => inner.writers[slot] = Some(token),
            }
        }) {
            // Woken by a wake (0): try again. `ESRCH`: the process is being
            // retired and must not park.
            Ok(0) => Parked::Retry,
            Ok(e) => Parked::Fail(e),
            Err(_g) => Parked::Fail(EAGAIN),
        }
    }

    /// Test witness: (readers, writers, buffered).
    #[cfg(feature = "neo-test")]
    pub fn counts_for_test(&self) -> (u32, u32, usize) {
        let g = self.state.lock();
        (g.logic.readers(), g.logic.writers(), g.logic.buffered())
    }
}

const fn end_bit(end: End) -> u64 {
    match end {
        End::Read => 0,
        End::Write => 1,
    }
}

/// The capacity limits, re-exported for the syscall layer.
pub use pipe_logic::PIPE_ATOMIC;
