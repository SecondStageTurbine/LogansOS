// SPDX-License-Identifier: MPL-2.0
//! OT-22 rung 1 (`wait-multiplex`): park a thread until any of a few objects
//! has something for it, or a deadline passes
//! (`docs/THINK-2026-10-19-ot22-readiness-multiplex.md` §3-§5).
//!
//! The shape is the futex's (`futex.rs`): one bounded waiter table, a park
//! token installed under the table guard, and wakes taken out under the guard
//! and delivered after it is dropped. What differs is that a waiter watches
//! several channel sides at once, so readiness cannot be re-checked under the
//! table guard (that would take `Channel.state` under it and invert the
//! order). Instead each waiter carries a `fired` flag:
//!
//! 1. `register`: the entry (its channel keys and deadline) goes into the
//!    table, with no token and `fired = 0`.
//! 2. The caller checks every item's readiness, each under that object's own
//!    lock. Anything ready: `unregister` and return. No park.
//! 3. `park`: under the table guard, an entry already fired returns at once;
//!    otherwise the token is installed and the guard is released on the new
//!    stack (`try_block_current_with_token_and_guard`). There is no instant
//!    where the thread is committed to sleep, lock-free and still on-CPU.
//! 4. A waker (a channel send/grant/close, the tick) sets `fired` under the
//!    table guard and takes the token if one is installed, drops the guard,
//!    then wakes it. A fire between steps 1 and 3 only sets `fired`, which
//!    step 3 sees (CP-14). A second fire finds no token: harmless.
//!
//! Channel readiness is published under `Channel.state`, and the channel's
//! fire runs under that same lock before its sender parks, so a waiter that
//! re-checks in step 2 either sees the sender or was registered before the
//! fire. Lock order: `Channel.state` → `WAIT_TABLE` → scheduler; the table
//! guard is never held while taking a channel lock (CP-1/CP-16). Every lock
//! here is an `IrqSafeMutex` (IF=0 by construction, CP-37/CP-56). The table
//! holds plain keys, never an `Arc`, so a waiter zombified while parked
//! leaks nothing (CP-30), and a stale key can only cause a spurious wake,
//! which a level-triggered wait absorbs.

use core::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use crate::arch::x86_64::timer::TICKS;
use crate::errno::EAGAIN;
use crate::sync::IrqSafeMutex;

use super::process::ProcessId;
use super::scheduler::{self, ThreadWakeToken};

/// At most this many items per wait (and channel keys per waiter).
pub const ITEMS_MAX: usize = 8;
/// Concurrent waiters. Three desktop processes park one thread each; a full
/// table fails closed with EAGAIN (the caller polls once), never a panic.
const WAITERS_MAX: usize = 16;
/// Entries one process may hold at once (AAG F2): no process can take the
/// whole table from the others.
const WAITERS_PER_PID: usize = 4;

/// Why a parked waiter woke (its syscall's internal return code).
pub const WHY_IPC: i64 = 1;
pub const WHY_TICK: i64 = 2;
/// A handle the waiter named was closed, revoked or drained (Jarvis rung-1
/// F1): the wait re-resolves its items and returns EBADF, or watches the
/// handle's new object.
pub const WHY_HANDLE: i64 = 3;

struct Waiter {
    ticket: u64,
    pid: ProcessId,
    keys: [u64; ITEMS_MAX],
    nkeys: usize,
    /// Absolute `TICKS` deadline; `u64::MAX` = none.
    deadline_ticks: u64,
    token: Option<ThreadWakeToken>,
    /// 0 = not fired; else `WHY_*`.
    fired: i64,
}

struct Table {
    slots: [Option<Waiter>; WAITERS_MAX],
}

static WAIT_TABLE: IrqSafeMutex<Table> = IrqSafeMutex::new(Table {
    slots: [const { None }; WAITERS_MAX],
});
static NEXT_TICKET: AtomicU64 = AtomicU64::new(1);
/// Registered waiters watching at least one channel: the send path's fast
/// gate (one relaxed load when no one waits).
static CHANNEL_WAITERS: AtomicUsize = AtomicUsize::new(0);
/// Waiters with a deadline (fired or not, until removed), and the earliest
/// deadline among the unfired ones: the tick's fast gate.
static TIMED_WAITERS: AtomicUsize = AtomicUsize::new(0);
static EARLIEST_DEADLINE_TICKS: AtomicU64 = AtomicU64::new(u64::MAX);

/// Test witnesses: parks, and wakes by cause.
#[cfg(feature = "neo-test")]
pub static STATS: Stats = Stats::new();

#[cfg(feature = "neo-test")]
pub struct Stats {
    pub parks: AtomicU64,
    pub ipc_wakes: AtomicU64,
    pub tick_wakes: AtomicU64,
    pub handle_wakes: AtomicU64,
    pub fired_before_park: AtomicU64,
    /// Registrations dropped because an item's handle came to name another
    /// object between the two scans (Jarvis rung-1 F1).
    pub identity_retries: AtomicU64,
}

#[cfg(feature = "neo-test")]
impl Stats {
    const fn new() -> Self {
        Self {
            parks: AtomicU64::new(0),
            ipc_wakes: AtomicU64::new(0),
            tick_wakes: AtomicU64::new(0),
            handle_wakes: AtomicU64::new(0),
            fired_before_park: AtomicU64::new(0),
            identity_retries: AtomicU64::new(0),
        }
    }
}

/// The key a channel side is watched by.
pub fn channel_key(channel_ptr: *const (), side: usize) -> u64 {
    (channel_ptr as u64) | (side as u64 & 1)
}

/// A registered (not yet parked) waiter.
pub struct Registration {
    slot: usize,
    ticket: u64,
}

fn recompute_earliest(t: &Table) {
    let mut earliest = u64::MAX;
    for w in t.slots.iter().flatten() {
        if w.fired == 0 && w.deadline_ticks < earliest {
            earliest = w.deadline_ticks;
        }
    }
    EARLIEST_DEADLINE_TICKS.store(earliest, Ordering::Relaxed);
}

/// Step 1: enter the table. `None` when it is full, or `pid` already holds
/// `WAITERS_PER_PID` entries (the caller returns EAGAIN and polls once).
pub fn register(pid: ProcessId, keys: &[u64], deadline_ticks: u64) -> Option<Registration> {
    let mut t = WAIT_TABLE.lock();
    if t.slots.iter().flatten().filter(|w| w.pid == pid).count() >= WAITERS_PER_PID {
        return None;
    }
    let slot = t.slots.iter().position(|s| s.is_none())?;
    let ticket = NEXT_TICKET.fetch_add(1, Ordering::Relaxed);
    let mut stored = [0u64; ITEMS_MAX];
    let nkeys = keys.len().min(ITEMS_MAX);
    stored[..nkeys].copy_from_slice(&keys[..nkeys]);
    t.slots[slot] = Some(Waiter {
        ticket,
        pid,
        keys: stored,
        nkeys,
        deadline_ticks,
        token: None,
        fired: 0,
    });
    if nkeys != 0 {
        CHANNEL_WAITERS.fetch_add(1, Ordering::AcqRel);
    }
    if deadline_ticks != u64::MAX {
        TIMED_WAITERS.fetch_add(1, Ordering::Relaxed);
        EARLIEST_DEADLINE_TICKS.fetch_min(deadline_ticks, Ordering::Relaxed);
    }
    Some(Registration { slot, ticket })
}

/// Remove `reg`'s entry (if `purge_pid` has not already). Called with the
/// table guard held.
fn remove(t: &mut Table, reg: &Registration) {
    let mine = matches!(&t.slots[reg.slot], Some(w) if w.ticket == reg.ticket);
    if !mine {
        return;
    }
    let w = t.slots[reg.slot].take().expect("checked above");
    if w.nkeys != 0 {
        CHANNEL_WAITERS.fetch_sub(1, Ordering::AcqRel);
    }
    if w.deadline_ticks != u64::MAX {
        TIMED_WAITERS.fetch_sub(1, Ordering::Relaxed);
    }
    recompute_earliest(t);
}

/// Leave the table without parking (something was ready at step 2).
pub fn unregister(reg: Registration) {
    let mut t = WAIT_TABLE.lock();
    remove(&mut t, &reg);
}

/// Step 3: park until fired. Returns `WHY_*`, or EAGAIN when the scheduler's
/// token table is full (nothing parked). The entry is always removed.
pub fn park(reg: Registration) -> i64 {
    #[cfg(feature = "neo-test-wait-any-race")]
    race_hook();
    let t = WAIT_TABLE.lock();
    let fired = match &t.slots[reg.slot] {
        Some(w) if w.ticket == reg.ticket => w.fired,
        // Purged by a forced teardown racing us: do not park.
        _ => WHY_TICK,
    };
    if fired != 0 {
        let mut t = t;
        remove(&mut t, &reg);
        #[cfg(feature = "neo-test")]
        STATS.fired_before_park.fetch_add(1, Ordering::Relaxed);
        return fired;
    }
    #[cfg(feature = "neo-test")]
    STATS.parks.fetch_add(1, Ordering::Relaxed);
    let slot = reg.slot;
    let ret = match scheduler::try_block_current_with_token_and_guard(t, |t, token| {
        if let Some(w) = t.slots[slot].as_mut() {
            w.token = Some(token);
        }
    }) {
        Ok(why) => why,
        Err(_guard) => EAGAIN,
    };
    let mut t = WAIT_TABLE.lock();
    remove(&mut t, &reg);
    ret
}

/// Park the calling kernel thread for `ticks` timer ticks (at least until
/// the next tick when `ticks` is 1). For kernel loops that would otherwise
/// yield-spin. Falls back to a yield when no slot or token is free.
pub fn sleep_ticks(ticks: u64) {
    let deadline = TICKS.load(Ordering::Relaxed).saturating_add(ticks.max(1));
    let Some(reg) = register(super::current_pid(), &[], deadline) else {
        super::yield_now();
        return;
    };
    if crate::arch::x86_64::without_interrupts(|| park(reg)) == EAGAIN {
        super::yield_now();
    }
}

/// Fire every waiter watching `key` (a channel side whose readiness just
/// changed). Called with that channel's `state` lock held, before its
/// sender parks (see the module docs for why that order is safe).
pub fn fire_channel(key: u64) {
    if cfg!(feature = "wait-multiplex-no-ipc-fire") {
        return;
    }
    fire_key(key, WHY_IPC);
}

/// Fire every waiter watching `key` because the handle naming it left a
/// handle table (close, revoke, process drain). Called from the
/// `HandleTable` removal chokepoints with that table's lock held; the order
/// is handle table → `WAIT_TABLE` → scheduler, and nothing takes a handle
/// table under either. A waiter registered before the removal is woken and
/// re-resolves its items; one that registers after it sees the change in
/// its second scan (`wait_any_items`), so no rebind or revoke strands it.
pub fn fire_handle_gone(key: u64) {
    if cfg!(feature = "wait-multiplex-no-handle-fire") {
        return;
    }
    fire_key(key, WHY_HANDLE);
}

fn fire_key(key: u64, why: i64) {
    if CHANNEL_WAITERS.load(Ordering::Acquire) == 0 {
        return;
    }
    let mut tokens: [Option<ThreadWakeToken>; WAITERS_MAX] = [None; WAITERS_MAX];
    let mut n = 0;
    {
        let mut t = WAIT_TABLE.lock();
        let mut changed = false;
        for w in t.slots.iter_mut().flatten() {
            if w.fired == 0 && w.keys[..w.nkeys].contains(&key) {
                w.fired = why;
                changed |= w.deadline_ticks != u64::MAX;
                if let Some(token) = w.token.take() {
                    tokens[n] = Some(token);
                    n += 1;
                }
            }
        }
        if changed {
            recompute_earliest(&t);
        }
    }
    for token in tokens.iter().take(n).flatten() {
        #[cfg(feature = "neo-test")]
        if why == WHY_IPC {
            STATS.ipc_wakes.fetch_add(1, Ordering::Relaxed);
        } else {
            STATS.handle_wakes.fetch_add(1, Ordering::Relaxed);
        }
        let _ = scheduler::wake_thread_token(*token, why);
    }
}

/// Timer-tick expiry (BSP, IF=0). One relaxed load when no waiter has a
/// deadline. No allocation, no HID work, no `DELIVERY`: the tick only wakes;
/// the woken thread pumps input in its own syscall.
pub fn on_tick_expire() {
    if TIMED_WAITERS.load(Ordering::Relaxed) == 0 {
        return;
    }
    let now = TICKS.load(Ordering::Relaxed);
    if now < EARLIEST_DEADLINE_TICKS.load(Ordering::Relaxed) {
        return;
    }
    let mut tokens: [Option<ThreadWakeToken>; WAITERS_MAX] = [None; WAITERS_MAX];
    let mut n = 0;
    {
        let mut t = WAIT_TABLE.lock();
        for w in t.slots.iter_mut().flatten() {
            if w.fired == 0 && w.deadline_ticks <= now {
                w.fired = WHY_TICK;
                if let Some(token) = w.token.take() {
                    tokens[n] = Some(token);
                    n += 1;
                }
            }
        }
        recompute_earliest(&t);
    }
    for token in tokens.iter().take(n).flatten() {
        #[cfg(feature = "neo-test")]
        STATS.tick_wakes.fetch_add(1, Ordering::Relaxed);
        let _ = scheduler::wake_thread_token(*token, WHY_TICK);
    }
}

/// Forced teardown: drop every entry `pid` owns. Its parked thread is
/// zombified by the caller (`zombify_parked_thread_by_pid`); this keeps a
/// late fire from waking it and frees the slots.
pub fn purge_pid(pid: ProcessId) {
    let mut t = WAIT_TABLE.lock();
    let mut any = false;
    for slot in t.slots.iter_mut() {
        if matches!(slot, Some(w) if w.pid == pid) {
            let w = slot.take().expect("checked above");
            if w.nkeys != 0 {
                CHANNEL_WAITERS.fetch_sub(1, Ordering::AcqRel);
            }
            if w.deadline_ticks != u64::MAX {
                TIMED_WAITERS.fetch_sub(1, Ordering::Relaxed);
            }
            any = true;
        }
    }
    if any {
        recompute_earliest(&t);
    }
}

/// Test witness: entries in the table (all of them, or `pid`'s).
#[cfg(feature = "neo-test")]
#[allow(dead_code)]
pub fn debug_entries(pid: Option<ProcessId>) -> usize {
    let t = WAIT_TABLE.lock();
    t.slots
        .iter()
        .flatten()
        .filter(|w| pid.is_none_or(|p| w.pid == p))
        .count()
}

/// Rebind leg (Jarvis rung-1 F1): a waiter of this pid stops between its
/// first scan and its registration until a sibling has closed its handle
/// and reused the slot for another channel. Test-only.
#[cfg(feature = "neo-test-wait-any-core")]
pub static REBIND_PID: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(u32::MAX);
#[cfg(feature = "neo-test-wait-any-core")]
pub static REBIND_IN_WINDOW: core::sync::atomic::AtomicBool =
    core::sync::atomic::AtomicBool::new(false);
#[cfg(feature = "neo-test-wait-any-core")]
pub static REBIND_DONE: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);

#[cfg(feature = "neo-test-wait-any-core")]
pub fn rebind_window_hook(pid: ProcessId) {
    if REBIND_PID
        .compare_exchange(pid.0, u32::MAX, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return;
    }
    REBIND_IN_WINDOW.store(true, Ordering::Release);
    while !REBIND_DONE.load(Ordering::Acquire) {
        core::hint::spin_loop();
    }
}

/// CP-14 race leg: widen the window between the caller's readiness check
/// (step 2) and the park (step 3), so a sender on another CPU fires into
/// it. Test-only.
#[cfg(feature = "neo-test-wait-any-race")]
pub static RACE_SPIN: AtomicU64 = AtomicU64::new(0);

#[cfg(feature = "neo-test-wait-any-race")]
fn race_hook() {
    for _ in 0..RACE_SPIN.load(Ordering::Relaxed) {
        core::hint::spin_loop();
    }
}
