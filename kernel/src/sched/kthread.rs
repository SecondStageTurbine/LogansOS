// SPDX-License-Identifier: MPL-2.0
//! Scheduler-owned kernel-thread lifecycle handles.
//!
//! `KthreadHandle` owns lifecycle authority for a worker, not the worker's
//! `Thread`. The scheduler continues to own every `Box<Thread>` through the
//! ready/current/blocked/zombie queues. Handles and stop tokens only share a
//! small control block containing identity, stop/completion state, and
//! scheduler wake tokens.
//!
//! Failure model: NeoPlutarch currently aborts on kernel panic rather than
//! unwinding, so a panicking worker cannot report a join-visible failure. Worker
//! code that wants surfaced failure should return `KthreadExitState::Failed`.

#![allow(dead_code)]

extern crate alloc;

use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use spin::Lazy;

use crate::arch::x86_64::per_cpu;
use crate::sync::SwitchMutex;

use super::scheduler::{self, ThreadWakeToken, WakeTokenResult};
use super::thread::Thread;

const INVALID_THREAD_ID: u64 = u64::MAX;
const KTHREAD_STOP_WAKE: i64 = 0x33a;
const KTHREAD_JOIN_WAKE: i64 = 0x30a;

/// Process/kthread context registry: spawn, trampoline lookup, detach, and
/// unregister are not reached from interrupt handlers.
static REGISTRY: Lazy<SwitchMutex<KthreadRegistry>> =
    Lazy::new(|| SwitchMutex::new(KthreadRegistry { slots: Vec::new() }));

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct KthreadId {
    slot: u32,
    generation: u32,
}

impl KthreadId {
    pub const fn slot(self) -> u32 {
        self.slot
    }

    pub const fn generation(self) -> u32 {
        self.generation
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum KthreadExitState {
    Success,
    Failed(u32),
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum KthreadJoinResult {
    Exited(KthreadExitState),
    Detached,
    AlreadyJoined,
    JoinAlreadyInProgress,
}

pub trait KthreadJob: Send + 'static {
    fn run(self: Box<Self>, stop: KthreadStopToken) -> KthreadExitState;
}

impl<F> KthreadJob for F
where
    F: FnOnce(KthreadStopToken) -> KthreadExitState + Send + 'static,
{
    fn run(self: Box<Self>, stop: KthreadStopToken) -> KthreadExitState {
        (*self)(stop)
    }
}

pub struct KthreadHandle {
    control: Arc<KthreadControl>,
}

impl KthreadHandle {
    pub fn id(&self) -> KthreadId {
        self.control.id
    }

    pub fn request_stop(&self) {
        request_stop_control(&self.control);
    }

    pub fn join(&self) -> KthreadJoinResult {
        join_control(&self.control)
    }

    pub fn detach(self) -> KthreadId {
        let id = self.id();
        detach_control(&self.control);
        id
    }
}

impl Drop for KthreadHandle {
    fn drop(&mut self) {
        detach_control(&self.control);
    }
}

#[derive(Clone)]
pub struct KthreadStopToken {
    control: Arc<KthreadControl>,
}

impl KthreadStopToken {
    pub fn id(&self) -> KthreadId {
        self.control.id
    }

    pub fn is_stop_requested(&self) -> bool {
        self.control.stop_requested.load(Ordering::Acquire)
    }

    pub fn park_until_stop(&self) -> bool {
        if self.is_stop_requested() {
            return true;
        }

        let guard = self.control.state.lock();
        if self.is_stop_requested() {
            return true;
        }

        scheduler::block_current_with_token_and_guard(guard, |state, token| {
            debug_assert!(
                state.stop_waiter.is_none(),
                "kthread stop park supports one parked worker"
            );
            state.stop_waiter = Some(token);
        });

        self.is_stop_requested()
    }
}

struct KthreadControl {
    id: KthreadId,
    name: &'static str,
    thread_id: AtomicU64,
    stop_requested: AtomicBool,
    /// Scheduler/context-switch special: stop and join waiters park through
    /// `block_current_with_token_and_guard`, requiring release-token semantics.
    state: SwitchMutex<KthreadControlState>,
}

struct KthreadControlState {
    job: Option<Box<dyn KthreadJob>>,
    exit_state: Option<KthreadExitState>,
    detached: bool,
    joined: bool,
    stop_waiter: Option<ThreadWakeToken>,
    join_waiter: Option<ThreadWakeToken>,
}

struct KthreadRegistry {
    slots: Vec<KthreadRegistrySlot>,
}

struct KthreadRegistrySlot {
    generation: u32,
    control: Option<Arc<KthreadControl>>,
}

impl KthreadRegistry {
    fn reserve_id(&mut self) -> KthreadId {
        if let Some((idx, slot)) = self
            .slots
            .iter_mut()
            .enumerate()
            .find(|(_, slot)| slot.control.is_none())
        {
            slot.generation = next_generation(slot.generation);
            return KthreadId {
                slot: idx as u32,
                generation: slot.generation,
            };
        }

        let id = KthreadId {
            slot: self.slots.len() as u32,
            generation: 1,
        };
        self.slots.push(KthreadRegistrySlot {
            generation: id.generation,
            control: None,
        });
        id
    }

    fn install(&mut self, control: Arc<KthreadControl>) {
        let idx = control.id.slot as usize;
        let slot = self
            .slots
            .get_mut(idx)
            .expect("kthread registry slot must exist before install");
        assert_eq!(
            slot.generation, control.id.generation,
            "kthread registry generation mismatch"
        );
        assert!(
            slot.control.is_none(),
            "kthread registry slot installed twice"
        );
        slot.control = Some(control);
    }

    fn get(&self, id: KthreadId) -> Option<Arc<KthreadControl>> {
        let slot = self.slots.get(id.slot as usize)?;
        if slot.generation != id.generation {
            return None;
        }
        slot.control.as_ref().cloned()
    }

    fn get_by_thread_id(&self, thread_id: u64) -> Option<Arc<KthreadControl>> {
        self.slots
            .iter()
            .filter_map(|slot| slot.control.as_ref())
            .find(|control| control.thread_id.load(Ordering::Acquire) == thread_id)
            .cloned()
    }

    fn remove(&mut self, id: KthreadId) -> bool {
        let Some(slot) = self.slots.get_mut(id.slot as usize) else {
            return false;
        };
        if slot.generation != id.generation {
            return false;
        }
        slot.control.take().is_some()
    }
}

fn next_generation(current: u32) -> u32 {
    let next = current.wrapping_add(1);
    if next == 0 { 1 } else { next }
}

pub fn spawn_owned<J>(name: &'static str, job: J) -> KthreadHandle
where
    J: KthreadJob,
{
    let id = REGISTRY.lock().reserve_id();
    let control = Arc::new(KthreadControl {
        id,
        name,
        thread_id: AtomicU64::new(INVALID_THREAD_ID),
        stop_requested: AtomicBool::new(false),
        state: SwitchMutex::new(KthreadControlState {
            job: Some(Box::new(job)),
            exit_state: None,
            detached: false,
            joined: false,
            stop_waiter: None,
            join_waiter: None,
        }),
    });

    let thread = Thread::new(name, kthread_trampoline);
    control.thread_id.store(thread.id.0, Ordering::Release);
    REGISTRY.lock().install(control.clone());
    scheduler::push_ready_kthread(thread);

    KthreadHandle { control }
}

pub fn spawn_detached<J>(name: &'static str, job: J) -> KthreadId
where
    J: KthreadJob,
{
    let handle = spawn_owned(name, job);
    handle.detach()
}

pub fn request_stop(handle: &KthreadHandle) {
    handle.request_stop();
}

pub fn join(handle: &KthreadHandle) -> KthreadJoinResult {
    handle.join()
}

pub fn detach(handle: KthreadHandle) -> KthreadId {
    handle.detach()
}

extern "C" fn kthread_trampoline() -> ! {
    run_current()
}

fn run_current() -> ! {
    let thread_id = per_cpu::current_thread_id();
    let control = REGISTRY
        .lock()
        .get_by_thread_id(thread_id)
        .expect("kthread trampoline entered without registry control");

    let job = {
        let mut state = control.state.lock();
        state
            .job
            .take()
            .expect("kthread trampoline entered without job")
    };

    let exit_state = job.run(KthreadStopToken {
        control: control.clone(),
    });
    complete_control(&control, exit_state);
    scheduler::exit();
}

fn request_stop_control(control: &Arc<KthreadControl>) {
    control.stop_requested.store(true, Ordering::Release);
    let waiter = control.state.lock().stop_waiter.take();
    if let Some(token) = waiter {
        let _ = scheduler::wake_thread_token(token, KTHREAD_STOP_WAKE);
    }
}

fn join_control(control: &Arc<KthreadControl>) -> KthreadJoinResult {
    loop {
        let guard = control.state.lock();
        if guard.joined {
            return KthreadJoinResult::AlreadyJoined;
        }
        if guard.detached {
            return KthreadJoinResult::Detached;
        }
        if let Some(exit_state) = guard.exit_state {
            drop(guard);
            mark_joined_and_unregister(control);
            return KthreadJoinResult::Exited(exit_state);
        }
        if guard.join_waiter.is_some() {
            return KthreadJoinResult::JoinAlreadyInProgress;
        }

        scheduler::block_current_with_token_and_guard(guard, |state, token| {
            state.join_waiter = Some(token);
        });
        scheduler::reap_nested();
    }
}

fn complete_control(control: &Arc<KthreadControl>, exit_state: KthreadExitState) {
    let (join_waiter, unregister) = {
        let mut state = control.state.lock();
        state.exit_state = Some(exit_state);
        (state.join_waiter.take(), state.detached)
    };

    if let Some(token) = join_waiter {
        match scheduler::wake_thread_token(token, KTHREAD_JOIN_WAKE) {
            WakeTokenResult::Woken | WakeTokenResult::Gone | WakeTokenResult::Stale => {}
        }
    }

    if unregister {
        unregister_control(control.id);
    }
}

fn detach_control(control: &Arc<KthreadControl>) {
    let unregister = {
        let mut state = control.state.lock();
        if state.joined {
            return;
        }
        state.detached = true;
        state.exit_state.is_some()
    };

    if unregister {
        unregister_control(control.id);
    }
}

fn mark_joined_and_unregister(control: &Arc<KthreadControl>) {
    {
        let mut state = control.state.lock();
        state.joined = true;
        state.join_waiter = None;
        state.stop_waiter = None;
    }
    unregister_control(control.id);
}

fn unregister_control(id: KthreadId) {
    REGISTRY.lock().remove(id);
}

#[cfg(feature = "neo-test")]
#[allow(dead_code)]
pub fn debug_registered(id: KthreadId) -> bool {
    REGISTRY.lock().get(id).is_some()
}

#[cfg(feature = "neo-test")]
#[allow(dead_code)]
pub fn debug_stop_waiter_present(handle: &KthreadHandle) -> bool {
    handle.control.state.lock().stop_waiter.is_some()
}

#[cfg(feature = "neo-test")]
pub struct KthreadLifetimeDebug {
    pub handle_is_control_arc: bool,
    pub registered: bool,
    pub thread_id_recorded: bool,
}

#[cfg(feature = "neo-test")]
#[allow(dead_code)]
pub fn debug_lifetime_shape(handle: &KthreadHandle) -> KthreadLifetimeDebug {
    KthreadLifetimeDebug {
        handle_is_control_arc: core::mem::size_of::<KthreadHandle>()
            == core::mem::size_of::<Arc<KthreadControl>>(),
        registered: debug_registered(handle.id()),
        thread_id_recorded: handle.control.thread_id.load(Ordering::Acquire) != INVALID_THREAD_ID,
    }
}
