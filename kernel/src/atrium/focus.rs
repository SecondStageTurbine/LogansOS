// SPDX-License-Identifier: MPL-2.0
//! The `InputFocus` kernel object (G1 kernel object #2): kernel-routed
//! atomic input-focus transfer.
//!
//! One global word — `ACTIVE_INPUT_FOCUS` — holds the id of the focus
//! object whose client owner may consume input. The compositor's disjoint
//! ACTIVATE-only sibling transfers focus with a single atomic swap; the client
//! RECEIVE root is gated by an id comparison on every poll.
//! Input security and input routing are literally the same comparison
//! (SYNTHESIS §2); slice-1 ships the object + transfer + gate — the fd-0
//! takeover and HID delivery arrive with the G1 input slice.
//!
//! Concurrency notes (the CP-bank consultation for this file):
//! - CP-11: the route is published `Release` (via `swap(AcqRel)` /
//!   `compare_exchange(AcqRel)`) and read `Acquire` — a poll that observes
//!   an activation happens-after everything the activator did before it.
//! - CP-13: the route is ONE aligned u64 — no multi-word state, torn reads
//!   are impossible by construction.
//! - CP-12: ids are minted from a monotonic u64 counter and never reused,
//!   so a stale leaf can never alias a later object (clean miss, ISC-14).

extern crate alloc;

use alloc::sync::{Arc, Weak};
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};

use crate::sync::irqsafe_mutex::{IrqSafeGuard, IrqSafeMutex};

use super::input_logic::InputEventV1;
use super::input_queue_logic::{Enqueued, InputQueue};
use super::pointer_route_logic::RecordRing;

/// Per-client bound. Overflow never waits (I-14b); what it drops is the
/// `input_queue_logic` policy: motion first, keys last (keystrokes are never
/// evicted by pointer traffic).
pub const INPUT_QUEUE_CAPACITY: usize = 64;
/// At most this many live client routes (Jarvis, #46 RETURN F3 / CP-54):
/// every id-to-route lookup under `DELIVERY` (flush, mint, forward, RECV)
/// scans a bounded set, however many focus objects user space creates. The
/// one pointer route is outside the cap, so a flood of client focuses can
/// never stop the WM from claiming it.
pub const INPUT_ROUTE_CAP: usize = 32;

struct RouteEntry {
    id: u64,
    /// The pointer route (outside [`INPUT_ROUTE_CAP`]). Stored, not read
    /// through `owner`: upgrading under `DELIVERY` could drop the last
    /// reference there, and `InputFocus::drop` takes `DELIVERY`.
    pointer: bool,
    owner: Weak<InputFocus>,
    queue: InputQueue,
    dropped: u64,
    /// L2b (`sak-lock-route`, Jarvis #160 items 1 and 6): the lock epoch
    /// (`Delivery::lock_epoch`) this client queue's contents were enqueued
    /// under. A queue from an earlier one holds input that crossed a lock
    /// and is emptied before it is next read or written ([`fresh`]).
    #[cfg(feature = "sak-lock-route")]
    lock_epoch: u64,
}

/// Everything the HID pump and RECV touch, behind ONE non-sleeping lock (no
/// new lock level for DI-1a): the per-focus queues, and the pointer route's
/// outstanding-record ring (`atrium-pointer-forward`).
///
/// CP-37/CP-56 (#55): an `IrqSafeMutex`, so the guard itself masks
/// interrupts and EVERY acquisition is IF=0 whoever the caller is — the
/// syscall paths, the HID pump, and the `InputFocus::drop` that process
/// teardown runs after re-enabling interrupts. Reached only through
/// [`delivery`] / [`try_delivery`], which feed the neo-test IF=1 witness.
struct Delivery {
    routes: Vec<RouteEntry>,
    ring: RecordRing,
    /// L2b: bumped, under this lock, by every end of a lock (an unlock, or a
    /// dying lock focus failing the session closed), so nothing a client's
    /// queue held across the lock reaches it afterwards.
    #[cfg(feature = "sak-lock-route")]
    lock_epoch: u64,
}

static DELIVERY: IrqSafeMutex<Delivery> = IrqSafeMutex::new(Delivery {
    routes: Vec::new(),
    ring: RecordRing::new(),
    #[cfg(feature = "sak-lock-route")]
    lock_epoch: 0,
});

/// Take `DELIVERY` (IF=0 by construction: see [`Delivery`]).
fn delivery() -> IrqSafeGuard<'static, Delivery> {
    let guard = DELIVERY.lock();
    #[cfg(feature = "neo-test")]
    note_delivery_acquisition();
    guard
}

/// Non-blocking [`delivery`]; on failure the caller's IF is restored.
fn try_delivery() -> Option<IrqSafeGuard<'static, Delivery>> {
    let guard = DELIVERY.try_lock()?;
    #[cfg(feature = "neo-test")]
    note_delivery_acquisition();
    Some(guard)
}

/// neo-test CP-56 witness: `DELIVERY` held with interrupts ON. Zero by
/// construction through [`delivery`]; only the
/// `neo-test-delivery-drop-control` build (today's pre-fix `Drop`) can
/// raise it, which is how the gate proves the witness is live.
#[cfg(feature = "neo-test")]
static DELIVERY_IF_ON: AtomicU64 = AtomicU64::new(0);

#[cfg(feature = "neo-test")]
fn note_delivery_acquisition() {
    if crate::arch::x86_64::interrupts_enabled() {
        DELIVERY_IF_ON.fetch_add(1, Ordering::Relaxed);
    }
}

/// `DELIVERY` acquisitions made with interrupts ON, since boot.
#[cfg(feature = "neo-test")]
#[allow(dead_code)] // read by the scenarios that tear down a focus holder
pub fn debug_delivery_if_on_acquisitions() -> u64 {
    DELIVERY_IF_ON.load(Ordering::Relaxed)
}
/// DI-1a B-ref: the live pointer route's focus id (0 = none: the mouse takes
/// ADR-074's kind-2 route to the keyboard focus).
static POINTER_ROUTE_ID: AtomicU64 = AtomicU64::new(0);
/// The input session generation: bumped by every claim or reclaim, so a
/// route (and every record it minted) from an earlier generation is dead.
static SESSION_GENERATION: AtomicU64 = AtomicU64::new(0);

/// 0 = no focus active.
static ACTIVE_INPUT_FOCUS: AtomicU64 = AtomicU64::new(0);
/// S1-F1 telemetry, kept apart on purpose: a flush that found `DELIVERY`
/// busy (nothing lost: the events stay staged), versus an event the bounded
/// per-device stage itself had to drop (the counted overflow policy).
static FLUSH_CONTENDED: AtomicU64 = AtomicU64::new(0);
static STAGE_DROPPED: AtomicU64 = AtomicU64::new(0);
/// Jarvis (#44 RETURN F1): bumped, under `DELIVERY`, by every change of the
/// active route (activate, deactivate, a dying active focus). A staged batch
/// carries the epoch it was decoded under; a batch from an earlier epoch was
/// meant for a focus that no longer holds the keyboard and is dropped, never
/// flushed into the new owner (fail closed). Counted apart.
static ACTIVATION_EPOCH: AtomicU64 = AtomicU64::new(0);
static HANDOFF_DROPPED: AtomicU64 = AtomicU64::new(0);

/// A HID driver's staging area (S1-F1): its bounded queue plus the
/// activation epoch its events were decoded under.
pub struct Stage {
    queue: InputQueue,
    epoch: u64,
}

impl Stage {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            queue: InputQueue::with_capacity(capacity),
            epoch: ACTIVATION_EPOCH.load(Ordering::Acquire),
        }
    }

    /// Drop the batch decoded under an earlier epoch (fail closed).
    fn discard(&mut self) {
        let mut n = 0;
        while let Some(_event) = self.queue.pop_front() {
            #[cfg(feature = "freeze-watchdog")]
            crate::watchdog::note_key_dropped(&_event);
            n += 1;
        }
        HANDOFF_DROPPED.fetch_add(n, Ordering::Relaxed);
    }
}

/// SAK command latch (Jarvis F1): one bit per chord (`sak_logic::sak_bit`),
/// set with an atomic OR, so a chord can never be dropped by `DELIVERY`
/// contention or queue overflow. Repeats of one chord before the consumer
/// takes them coalesce into one pending action.
static SAK_PENDING: core::sync::atomic::AtomicU8 = core::sync::atomic::AtomicU8::new(0);
/// Chord presses latched, ever (test witness: "exactly one pending action").
static SAK_LATCHED: AtomicU64 = AtomicU64::new(0);
/// Raw-button reconciliation latch (Jarvis F2): the mouse's last DECODED
/// button bitmap, stored before the event is staged. A consumer that missed
/// a transition reconciles against this before acting on the next press.
static RAW_BUTTONS: core::sync::atomic::AtomicU8 = core::sync::atomic::AtomicU8::new(0);

/// Per-device staging bound (S1-F1). Same class-aware policy as a client
/// queue: motion coalesces and goes first, keys go last.
pub const INPUT_STAGE_CAPACITY: usize = 64;
/// Monotonic id mint; ids are never reused (CP-12 N-A by construction).
static NEXT_FOCUS_ID: AtomicU64 = AtomicU64::new(1);

/// The focus object: nothing but its never-reused identity. Route state is
/// the global word above; rights live on the capability handles.
pub struct InputFocus {
    id: u64,
    /// DI-1a: the WM's pointer route (receivable while it is the live route;
    /// never the keyboard focus).
    pointer_route: bool,
}

impl InputFocus {
    pub fn new() -> Self {
        Self {
            id: NEXT_FOCUS_ID.fetch_add(1, Ordering::Relaxed),
            pointer_route: false,
        }
    }

    /// Atomic route transfer (root/`ACTIVATE` only — gated at the syscall).
    /// Whatever was active before is deactivated by the same single swap;
    /// there is no window with two active routes.
    /// `false` (nothing changes) when [`INPUT_ROUTE_CAP`] live client routes
    /// already exist and this focus has none.
    pub fn activate(self: &Arc<Self>) -> bool {
        // This tiny non-sleeping critical section serializes the route swap with
        // enqueue. An event is therefore wholly before or after a handoff.
        if self.pointer_route {
            return false;
        }
        let mut delivery = delivery();
        if !insert_route(&mut delivery.routes, self) {
            return false;
        }
        ACTIVATION_EPOCH.fetch_add(1, Ordering::AcqRel);
        ACTIVE_INPUT_FOCUS.swap(self.id, Ordering::AcqRel);
        // Keys typed during the boot screens are not this route's.
        #[cfg(feature = "ps2-keyboard")]
        crate::drivers::ps2::i8042::note_route_activated();
        #[cfg(feature = "freeze-watchdog")]
        crate::watchdog::note_route_activated();
        true
    }

    /// Clear the route iff THIS object holds it. A root deactivating a
    /// focus that already lost the route must not clobber the current
    /// holder — hence compare-exchange, not a blind store. Returns whether
    /// the route was cleared.
    pub fn deactivate(&self) -> bool {
        // CP-37: `DELIVERY` is only ever held IF=0.
        crate::arch::x86_64::without_interrupts(|| {
            let _delivery = delivery();
            let cleared = ACTIVE_INPUT_FOCUS
                .compare_exchange(self.id, 0, Ordering::AcqRel, Ordering::Acquire)
                .is_ok();
            if cleared {
                ACTIVATION_EPOCH.fetch_add(1, Ordering::AcqRel);
            }
            cleared
        })
    }

    /// The focus id (`lock_route_races`' watch).
    #[cfg(feature = "neo-test-lock-route-races")]
    pub fn debug_id(&self) -> u64 {
        self.id
    }

    /// The routing gate (leaf/`RECEIVE` side): is this object the active
    /// route right now? `Acquire` pairs with the activator's release.
    pub fn is_active(&self) -> bool {
        input_route() == self.id
    }

    /// May RECEIVE dequeue now? The active client focus, or the live
    /// pointer route.
    pub fn receivable(&self) -> bool {
        if self.pointer_route {
            POINTER_ROUTE_ID.load(Ordering::Acquire) == self.id
        } else {
            self.is_active()
        }
    }

    /// Non-blocking client dequeue. The caller must hold RECEIVE; the same
    /// `is_active` comparison gates both visibility and routing.
    pub fn try_recv(&self) -> Option<InputEventV1> {
        let mut delivery = delivery();
        if !self.receivable() {
            return None;
        }
        #[cfg(feature = "sak-lock-route")]
        let lock_epoch = delivery.lock_epoch;
        let entry = delivery
            .routes
            .iter_mut()
            .find(|entry| entry.id == self.id)?;
        #[cfg(feature = "sak-lock-route")]
        fresh(entry, lock_epoch);
        #[cfg(feature = "neo-test-lock-route-races")]
        race::point(race::RECV);
        let event = entry.queue.pop_front();
        // L2b (Jarvis #160 item 1): Super+L doesn't take `DELIVERY`, so it can
        // land between the check above and the pop. Check again after the pop:
        // a receive returns an event only if its last route read came before
        // the lock. The event it pops otherwise was queued before the lock,
        // and the unlock would empty it anyway ([`fresh`]): dropped.
        #[cfg(feature = "sak-lock-route")]
        if event.is_some() && !self.receivable() {
            HANDOFF_DROPPED.fetch_add(1, Ordering::Relaxed);
            return None;
        }
        #[cfg(feature = "prefill-perf")]
        if self.pointer_route
            && let Some(event) = event
        {
            crate::ai::prefill_perf::note_pointer_rx(event.code);
        }
        event
    }

    /// OT-22: does a RECEIVE poll have an event for this leaf now? Not
    /// receivable is *not* ready: every desktop client treats EPERM as "not
    /// focused, no input yet" and keeps waiting, and focus moves have no
    /// waker, so the tick re-checks (an input wait never parks past the
    /// next tick). The caller has run the pull pump first, as RECV does.
    /// The answer can be stale by the time the caller acts on it (Super+L
    /// takes no lock), which is harmless: it delivers nothing, and the
    /// RECEIVE that follows checks again.
    #[cfg(feature = "wait-multiplex")]
    pub fn wait_ready(&self) -> bool {
        let mut delivery = delivery();
        #[cfg(feature = "sak-lock-route")]
        let lock_epoch = delivery.lock_epoch;
        self.receivable()
            && delivery
                .routes
                .iter_mut()
                .find(|entry| entry.id == self.id)
                .is_some_and(|entry| {
                    #[cfg(feature = "sak-lock-route")]
                    fresh(entry, lock_epoch);
                    !entry.queue.is_empty()
                })
    }

    #[cfg(any(
        feature = "neo-test-atrium-input-routing",
        feature = "neo-test-input-queue-flood"
    ))]
    pub fn debug_queue_stats(&self) -> (usize, u64) {
        let mut delivery = delivery();
        #[cfg(feature = "sak-lock-route")]
        let lock_epoch = delivery.lock_epoch;
        delivery
            .routes
            .iter_mut()
            .find(|entry| entry.id == self.id)
            .map(|entry| {
                #[cfg(feature = "sak-lock-route")]
                fresh(entry, lock_epoch);
                (entry.queue.len(), entry.dropped)
            })
            .unwrap_or((0, 0))
    }
}

/// HID delivery entry point. No scheduler wake, allocation, DMA, or AI lock:
/// one bounded critical section, one active-id comparison, and the class-aware
/// overflow policy of `input_queue_logic` (bounded O(capacity) scan).
pub fn route_event(event: InputEventV1) -> bool {
    route_event_checked(event, None)
}

/// [`route_event`] for an event already checked against an activation epoch
/// (a key repeat, legacy path). The epoch is compared under `DELIVERY`,
/// where every route change bumps it, so the check and the enqueue are one
/// atomic decision: a repeat checked before a focus change or lock is
/// dropped, never given to the new owner.
#[cfg(feature = "key-autorepeat")]
pub fn route_event_at(event: InputEventV1, epoch: u64) -> bool {
    route_event_checked(event, Some(epoch))
}

fn route_event_checked(event: InputEventV1, epoch: Option<u64>) -> bool {
    #[cfg(feature = "freeze-watchdog")]
    crate::watchdog::note_key_produced(&event);
    #[cfg(feature = "sak-lock")]
    note_activity();
    // Producer side never spins behind a client/syscall holder. Contention is
    // an event drop, preserving the HID completion path's non-stall contract.
    let Some(mut delivery) = try_delivery() else {
        #[cfg(feature = "freeze-watchdog")]
        crate::watchdog::note_key_dropped(&event);
        return false;
    };
    if let Some(epoch) = epoch
        && epoch != ACTIVATION_EPOCH.load(Ordering::Acquire)
    {
        #[cfg(feature = "freeze-watchdog")]
        crate::watchdog::note_key_dropped(&event);
        STAGE_STALE.fetch_add(1, Ordering::Relaxed);
        return false;
    }
    let active = input_route();
    #[cfg(feature = "sak-lock-route")]
    let lock_epoch = delivery.lock_epoch;
    let Some(entry) = delivery.routes.iter_mut().find(|entry| entry.id == active) else {
        #[cfg(feature = "freeze-watchdog")]
        crate::watchdog::note_key_dropped(&event);
        return false;
    };
    #[cfg(feature = "sak-lock-route")]
    fresh(entry, lock_epoch);
    // Kill-switch: the legacy feature restores ADR-074's original drop-oldest.
    let outcome = if cfg!(feature = "atrium-input-queue-legacy") {
        entry.queue.enqueue_legacy(event)
    } else {
        entry.queue.enqueue(event)
    };
    if !matches!(outcome, Enqueued::Appended | Enqueued::Coalesced) {
        entry.dropped = entry.dropped.saturating_add(1);
    }
    true
}

/// S1-F1: a HID driver decodes into its own bounded stage (under its device
/// lock), never straight into `DELIVERY`, so a report it has consumed and
/// reposted can no longer vanish when `DELIVERY` is busy.
///
/// Jarvis F1: an event joins a batch only if the active route has not
/// changed since the batch began; otherwise the old batch (decoded for the
/// previous owner) is discarded first.
pub fn stage_event(stage: &mut Stage, event: InputEventV1) {
    stage_at(stage, event, ACTIVATION_EPOCH.load(Ordering::Acquire));
}

/// Stage `event` under `epoch`: a stage holding an earlier epoch's batch is
/// discarded first.
fn stage_at(stage: &mut Stage, event: InputEventV1, epoch: u64) {
    #[cfg(feature = "freeze-watchdog")]
    crate::watchdog::note_key_produced(&event);
    #[cfg(feature = "sak-lock")]
    note_activity();
    if stage.epoch != epoch {
        stage.discard();
        stage.epoch = epoch;
    }
    if !matches!(
        stage.queue.enqueue(event),
        Enqueued::Appended | Enqueued::Coalesced
    ) {
        STAGE_DROPPED.fetch_add(1, Ordering::Relaxed);
    }
}

/// [`stage_event`] for an event already checked against an activation epoch
/// (a key repeat: Jarvis, #61). It is staged under `epoch`, never the epoch
/// read now, so a focus change or lock between the check and this call
/// drops it, and one between this call and the flush discards it there
/// (`flush` compares under `DELIVERY`). Returns whether it was staged.
#[cfg(feature = "key-autorepeat")]
pub fn stage_event_at(stage: &mut Stage, event: InputEventV1, epoch: u64) -> bool {
    use super::key_repeat_logic::{StageAction, stage_action};
    match stage_action(stage.epoch, epoch, ACTIVATION_EPOCH.load(Ordering::Acquire)) {
        StageAction::Drop => {
            #[cfg(feature = "freeze-watchdog")]
            {
                crate::watchdog::note_key_produced(&event);
                crate::watchdog::note_key_dropped(&event);
            }
            #[cfg(feature = "sak-lock")]
            note_activity();
            STAGE_STALE.fetch_add(1, Ordering::Relaxed);
            false
        }
        StageAction::Restart | StageAction::Append => {
            stage_at(stage, event, epoch);
            true
        }
    }
}

/// Events checked under an activation epoch that had moved on by the time
/// they were staged or routed ([`stage_event_at`], [`route_event_at`]).
static STAGE_STALE: AtomicU64 = AtomicU64::new(0);

pub fn debug_stage_stale() -> u64 {
    STAGE_STALE.load(Ordering::Relaxed)
}

/// Move every staged event, in order, into the active focus's queue under ONE
/// `try_lock`. Contention moves nothing and loses nothing: the events wait
/// for the next pump. Never blocks (the producer's no-stall contract).
pub fn flush_staged(stage: &mut Stage) {
    flush(stage, false);
}

/// The mouse's flush. With a live pointer route (`atrium-pointer-forward`)
/// every staged report becomes a kernel-minted record in the WM's queue;
/// otherwise it is `flush_staged` (ADR-074's kind-2 route to the focus).
pub fn flush_pointer_staged(stage: &mut Stage) {
    flush(stage, cfg!(feature = "atrium-pointer-forward"));
}

fn flush(stage: &mut Stage, pointer: bool) {
    if stage.queue.is_empty() {
        return;
    }
    // CP-37: the fd-0 read path drains with interrupts ON while RECV takes
    // `DELIVERY` with them off; holding it across up to a full stage with
    // IF=1 would let a same-CPU RECV spin on a preempted holder. Keep the
    // whole critical section IF=0 whatever the caller's state.
    crate::arch::x86_64::without_interrupts(|| {
        let Some(mut delivery) = try_delivery() else {
            FLUSH_CONTENDED.fetch_add(1, Ordering::Relaxed);
            return;
        };
        let route = POINTER_ROUTE_ID.load(Ordering::Acquire);
        if pointer && route != 0 {
            // The WM's records, whatever the client focus: not epoch-bound.
            mint_records(&mut delivery, &mut stage.queue, route);
            return;
        }
        // Jarvis F1: the epoch is stable while `DELIVERY` is held (every
        // route change bumps it under this lock). A batch from an earlier
        // epoch never reaches the current owner.
        if stage.epoch != ACTIVATION_EPOCH.load(Ordering::Acquire) {
            stage.discard();
            return;
        }
        let active = input_route();
        #[cfg(feature = "sak-lock-route")]
        let lock_epoch = delivery.lock_epoch;
        let Some(entry) = delivery.routes.iter_mut().find(|entry| entry.id == active) else {
            // No active route: dropped, exactly as `route_event` drops it.
            while let Some(_event) = stage.queue.pop_front() {
                #[cfg(feature = "freeze-watchdog")]
                crate::watchdog::note_key_dropped(&_event);
            }
            return;
        };
        #[cfg(feature = "sak-lock-route")]
        fresh(entry, lock_epoch);
        while let Some(event) = stage.queue.pop_front() {
            enqueue(entry, event);
        }
    })
}

/// Enqueue under the client policy; `false` if anything was dropped.
fn enqueue(entry: &mut RouteEntry, event: InputEventV1) -> bool {
    // A9-A (`prefill-perf`): a record minted for the WM's pointer route,
    // counted for the overlay's `ptr in` (Janus, #91 RETURN items 1-2).
    #[cfg(feature = "prefill-perf")]
    if entry.pointer {
        crate::ai::prefill_perf::note_pointer_minted(event.code);
    }
    let outcome = if cfg!(feature = "atrium-input-queue-legacy") {
        entry.queue.enqueue_legacy(event)
    } else {
        entry.queue.enqueue(event)
    };
    let kept = matches!(outcome, Enqueued::Appended | Enqueued::Coalesced);
    if !kept {
        entry.dropped = entry.dropped.saturating_add(1);
    }
    kept
}

/// Add `focus`'s route if it has none. `false` when it would be a client
/// route beyond [`INPUT_ROUTE_CAP`] (the pointer route is never refused).
fn insert_route(routes: &mut Vec<RouteEntry>, focus: &Arc<InputFocus>) -> bool {
    routes.retain(|entry| entry.owner.strong_count() != 0);
    if routes.iter().any(|entry| entry.id == focus.id) {
        return true;
    }
    let clients = routes.iter().filter(|entry| !entry.pointer).count();
    if !focus.pointer_route && clients >= INPUT_ROUTE_CAP {
        return false;
    }
    {
        routes.push(RouteEntry {
            id: focus.id,
            pointer: focus.pointer_route,
            owner: Arc::downgrade(focus),
            queue: InputQueue::with_capacity(INPUT_QUEUE_CAPACITY),
            dropped: 0,
            // Empty: whatever epoch it starts in, the first `fresh` loses nothing.
            #[cfg(feature = "sak-lock-route")]
            lock_epoch: 0,
        });
    }
    true
}

/// DI-1a (THINK §8.3): one record per staged report, into the pointer
/// route's queue. A record the queue drops forces the next one to RESYNC.
/// Sequence exhaustion starts a new generation with an empty ring.
fn mint_records(delivery: &mut Delivery, stage: &mut InputQueue, route: u64) {
    let Delivery { routes, ring, .. } = delivery;
    let Some(entry) = routes.iter_mut().find(|entry| entry.id == route) else {
        while stage.pop_front().is_some() {}
        return;
    };
    while let Some(event) = stage.pop_front() {
        if event.kind != super::input_logic::INPUT_EVENT_POINTER {
            continue;
        }
        // A spent route (every tag used: Jarvis, #46 RETURN F1) mints no
        // more: it sends one ROUTE_EXPIRED record so the WM reclaims a new
        // route (new handle, new ring), and drops reports until then. The
        // mouse never falls back to the raw route.
        // A9-C2: one record per delivery, the wheel record before the motion
        // record, so a loss while the wheel is handled reaches the motion
        // record as RESYNC (Chad, #104 RETURN F2; `RecordRing::mint_report`).
        #[cfg(feature = "atrium-pointer-wheel")]
        ring.mint_report(&event, |record| enqueue(entry, record));
        // Without `atrium-pointer-wheel` the drivers pass no wheel, so this
        // mints one motion record per report, as before A9-C2.
        #[cfg(not(feature = "atrium-pointer-wheel"))]
        {
            let wheel = event.wheel();
            let motion =
                wheel == 0 || event.x != 0 || event.y != 0 || event.flags != ring.buttons();
            let records = [
                motion.then(|| ring.mint(event.x, event.y, event.flags)),
                (wheel != 0).then(|| ring.mint_wheel(wheel)),
            ];
            for minted in records.into_iter().flatten() {
                let Some(record) = minted else {
                    if let Some(expiry) = ring.expiry() {
                        let _ = enqueue(entry, expiry);
                    }
                    continue;
                };
                if !enqueue(entry, record) {
                    ring.note_lost();
                }
            }
        }
    }
}

/// Why a claim failed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClaimError {
    /// A pointer route is live and this was not a reclaim.
    Exists,
}

/// DI-1a (THINK §8.2/§8.3): claim the pointer route for a new session
/// generation. `reclaim` replaces a live route (WM restart): the old route
/// stops being receivable and every record it minted is dead.
#[cfg(feature = "atrium-pointer-forward")]
pub fn claim_pointer_route(reclaim: bool) -> Result<Arc<InputFocus>, ClaimError> {
    let focus = Arc::new(InputFocus {
        id: NEXT_FOCUS_ID.fetch_add(1, Ordering::Relaxed),
        pointer_route: true,
    });
    let mut delivery = delivery();
    let live = POINTER_ROUTE_ID.load(Ordering::Acquire);
    if live != 0 && !reclaim {
        return Err(ClaimError::Exists);
    }
    delivery.routes.retain(|entry| entry.id != live);
    delivery.ring = RecordRing::new();
    let _ = insert_route(&mut delivery.routes, &focus);
    SESSION_GENERATION.fetch_add(1, Ordering::AcqRel);
    POINTER_ROUTE_ID.store(focus.id, Ordering::Release);
    Ok(focus)
}

/// Why a forward or drop failed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ForwardError {
    /// The named route is not the live pointer route.
    NotRoute,
    /// Never minted, or overwritten since.
    Stale,
    /// Already forwarded or dropped.
    Already,
    /// The target has no delivery route (never activated, or dead).
    NoTarget,
}

/// DI-1a (THINK §8.3): consume record `tag` exactly once. With a target,
/// deliver a kind-3 event whose buttons and phase come from the record and
/// whose coordinates the WM chose; without one, drop it. Validated before
/// anything is consumed. Process context: `delivery()` (IF=0), with no USB or
/// handle-table lock held (Jarvis checklist 3).
#[cfg(feature = "atrium-pointer-forward")]
pub fn forward_record(
    route: &InputFocus,
    tag: u16,
    target: Option<&InputFocus>,
    x: i16,
    y: i16,
) -> Result<(), ForwardError> {
    use super::pointer_route_logic::ConsumeError;
    let mut delivery = delivery();
    // L2b: while locked, a record for any focus but the lock's is consumed
    // and delivered to no one. Checked under `DELIVERY` (Jarvis #160 item
    // 6), so an unlock can't come between it and the enqueue; a Super+L that
    // does (it takes no lock) leaves the record in a client queue that the
    // next unlock empties ([`fresh`]), and the client can't read it meanwhile.
    #[cfg(feature = "sak-lock-route")]
    let target = target.filter(|target| lock_admits(target.id));
    #[cfg(feature = "sak-lock-route")]
    let lock_epoch = delivery.lock_epoch;
    if !route.pointer_route || POINTER_ROUTE_ID.load(Ordering::Acquire) != route.id {
        return Err(ForwardError::NotRoute);
    }
    let map = |e| match e {
        ConsumeError::Stale => ForwardError::Stale,
        ConsumeError::Already => ForwardError::Already,
    };
    delivery.ring.peek(tag).map_err(map)?;
    let Delivery { routes, ring, .. } = &mut *delivery;
    let entry = match target {
        Some(target) => Some(
            routes
                .iter_mut()
                .find(|entry| entry.id == target.id && !target.pointer_route)
                .ok_or(ForwardError::NoTarget)?,
        ),
        None => None,
    };
    let record = ring.consume(tag).map_err(map)?;
    if let Some(entry) = entry {
        #[cfg(feature = "sak-lock-route")]
        fresh(entry, lock_epoch);
        #[cfg(feature = "neo-test-lock-route-races")]
        race::point(race::FORWARD);
        // A9-C2: a wheel record reaches the client as kind 5, its wheel
        // from the record (never the WM).
        let (kind, code) = if record.wheel != 0 {
            (
                super::pointer_route_logic::INPUT_EVENT_POINTER_WHEEL,
                record.wheel as u16,
            )
        } else {
            (
                super::pointer_route_logic::INPUT_EVENT_POINTER_ABS,
                record.phase,
            )
        };
        enqueue(
            entry,
            InputEventV1 {
                kind,
                flags: record.buttons,
                code,
                x,
                y,
            },
        );
    }
    Ok(())
}

/// The keyboard focus's activation epoch: it changes whenever the active
/// route does. Key auto-repeat stops when it moves.
#[cfg(feature = "key-autorepeat")]
pub fn activation_epoch() -> u64 {
    ACTIVATION_EPOCH.load(Ordering::Acquire)
}

/// The current input session generation (test witness).
#[cfg(feature = "neo-test-di1-forward")]
pub fn debug_session_generation() -> u64 {
    SESSION_GENERATION.load(Ordering::Acquire)
}

/// Latch a SAK chord (`key` is a `sak_logic::SAK_KEYS` member). Lock-free:
/// never waits on, or is lost to, `DELIVERY`.
pub fn latch_sak(key: u8) {
    SAK_PENDING.fetch_or(super::sak_logic::sak_bit(key), Ordering::AcqRel);
    SAK_LATCHED.fetch_add(1, Ordering::Relaxed);
    #[cfg(feature = "sak-lock-route")]
    if key == super::sak_logic::KEY_L {
        engage_lock_route();
    }
}

/// OT-66 L2 (`sak-lock`): a key or pointer event was produced. One atomic
/// OR into the latch ([`super::sak_logic::ACTIVITY_BIT`]), lock-free like a
/// chord (CP-51), so the WM's idle timer sees input that went to a client's
/// route as well as its own, and no client can feign or hide it.
#[cfg(feature = "sak-lock")]
fn note_activity() {
    SAK_PENDING.fetch_or(super::sak_logic::ACTIVITY_BIT, Ordering::AcqRel);
}

/// OT-22: is a SAK chord pending? (Read only; `take_sak` clears it.)
#[cfg(feature = "wait-multiplex")]
pub fn sak_pending() -> bool {
    SAK_PENDING.load(Ordering::Acquire) != 0
}

/// Take (and clear) the pending chord bits. DI-1a's system-route consumer.
#[allow(dead_code)] // consumer arrives with DI-1a; the scenario reads it now
pub fn take_sak() -> u8 {
    SAK_PENDING.swap(0, Ordering::AcqRel)
}

/// Record the mouse's raw button state (every decoded report, delivered or not).
pub fn note_raw_buttons(buttons: u8) {
    RAW_BUTTONS.store(buttons, Ordering::Release);
}

/// The last decoded raw button bitmap (the reconciliation latch).
#[allow(dead_code)] // consumer arrives with DI-1a; the scenario reads it now
pub fn raw_buttons() -> u8 {
    RAW_BUTTONS.load(Ordering::Acquire)
}

/// Chord presses latched since boot.
#[cfg(feature = "neo-test-hid-contention")]
pub fn debug_sak_latched() -> u64 {
    SAK_LATCHED.load(Ordering::Relaxed)
}

/// Test seam: run `hold` while this CPU owns `DELIVERY`, so another CPU's
/// pump meets real contention (and, for `i2c_hid_absent`, so the touchpad ISR
/// runs with it held).
#[cfg(any(
    feature = "neo-test-hid-contention",
    feature = "neo-test-i2c-hid-absent"
))]
pub fn debug_with_delivery_held(hold: impl FnOnce()) {
    let _delivery = delivery();
    hold();
}

/// (flushes that found `DELIVERY` busy, events dropped by a full stage).
#[cfg(feature = "neo-test-hid-contention")]
pub fn debug_stage_stats() -> (u64, u64) {
    (
        FLUSH_CONTENDED.load(Ordering::Relaxed),
        STAGE_DROPPED.load(Ordering::Relaxed),
    )
}

/// Events discarded because the active route changed after they were
/// decoded (Jarvis F1).
#[cfg(feature = "neo-test-hid-contention")]
pub fn debug_handoff_dropped() -> u64 {
    HANDOFF_DROPPED.load(Ordering::Relaxed)
}

impl Drop for InputFocus {
    /// A focus object dying while active clears the route — a revoked or
    /// dropped focus can never keep consuming the input gate (ISC-14).
    fn drop(&mut self) {
        // Witness: `DELIVERY` is never taken under `CAP_GRAPH`, so no focus
        // drops there (a refused install hands its handle out first).
        #[cfg(feature = "neo-test")]
        assert!(
            !crate::sched::process::cap_graph_held_here(),
            "an InputFocus dropped under CAP_GRAPH (DELIVERY would nest in it)"
        );
        // CP-56 (#55): the last `Arc` is often released by process teardown
        // AFTER it re-enables interrupts (the drained handle vector), so this
        // must not rely on its caller's IF: `delivery()` masks it itself.
        // The control build restores the pre-fix shape (IF as the caller has
        // it) so the teardown discriminator can show its witness firing.
        #[cfg(feature = "neo-test-delivery-drop-control")]
        let mut delivery = {
            let guard = DELIVERY.lock_as_caller();
            note_delivery_acquisition();
            guard
        };
        #[cfg(not(feature = "neo-test-delivery-drop-control"))]
        let mut delivery = delivery();
        delivery.routes.retain(|entry| entry.id != self.id);
        if ACTIVE_INPUT_FOCUS
            .compare_exchange(self.id, 0, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            ACTIVATION_EPOCH.fetch_add(1, Ordering::AcqRel);
        }
        // A dying pointer route (the WM exited) stops minting records; the
        // mouse falls back to ADR-074 until the next claim.
        let _ = POINTER_ROUTE_ID.compare_exchange(self.id, 0, Ordering::AcqRel, Ordering::Acquire);
        #[cfg(feature = "sak-lock-route")]
        drop_lock_route(&mut delivery, self.id);
    }
}

/// The keyboard route input is delivered to: the active focus.
#[cfg(not(feature = "sak-lock-route"))]
#[inline(always)]
fn input_route() -> u64 {
    ACTIVE_INPUT_FOCUS.load(Ordering::Acquire)
}

/// The keyboard route input is delivered to: the active focus, or, while
/// the session is locked, the WM's lock focus.
#[cfg(feature = "sak-lock-route")]
#[inline(always)]
fn input_route() -> u64 {
    lock_word::route(
        LOCK_ROUTE.load(Ordering::Acquire),
        ACTIVE_INPUT_FOCUS.load(Ordering::Acquire),
    )
}

/// OT-66 L2b (`sak-lock-route`): the kernel's half of the lock. One word
/// (CP-13: no torn read of two; its transitions are `sak_logic::lock_word`,
/// host-tested): the lock focus the WM registered, and whether the session
/// is locked. While locked,
/// [`input_route`] is the lock focus whatever `ACTIVE_INPUT_FOCUS` says, so
/// every client's RECEIVE fails its id check and every event goes to the
/// lock focus. Super+L sets the bit in the latch itself, with no lock and no
/// wait, so the input locks even while the WM is parked in a send to a
/// client that never receives (CP-57). Only the input-system holder
/// registers, engages or releases ([`SYS_INPUT_LOCK`]).
///
/// [`SYS_INPUT_LOCK`]: crate::arch::x86_64::syscall
#[cfg(feature = "sak-lock-route")]
static LOCK_ROUTE: AtomicU64 = AtomicU64::new(0);
#[cfg(feature = "sak-lock-route")]
use super::sak_logic::lock_word;

/// Lock now, if a lock focus is registered. Lock-free (the latch calls it):
/// one compare-exchange loop on [`LOCK_ROUTE`], whose only other writers are
/// register, release and a dying lock focus. `false`: nothing registered.
///
/// The epoch bump discards every staged batch decoded before the lock and
/// stops key repeat. It is made outside `DELIVERY`, so a flush holding it may
/// already have passed its epoch check; that flush's events were decoded
/// before the lock on their own device (a device's decode and flush are
/// serialized by its own state), and it reads the route after the check, so
/// they go to the client only if it read the route before this store.
#[cfg(feature = "sak-lock-route")]
pub fn engage_lock_route() -> bool {
    let engaged = LOCK_ROUTE
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, lock_word::engage)
        .is_ok();
    if engaged {
        ACTIVATION_EPOCH.fetch_add(1, Ordering::AcqRel);
        #[cfg(feature = "freeze-watchdog")]
        crate::watchdog::note_route_activated();
    }
    engaged || lock_word::id(LOCK_ROUTE.load(Ordering::Acquire)) != 0
}

/// Register `focus` (the WM's own) as the lock focus, keeping the locked bit.
/// `false`: a pointer route, or the client route cap is full.
#[cfg(feature = "sak-lock-route")]
pub fn register_lock_route(focus: &Arc<InputFocus>) -> bool {
    if focus.pointer_route {
        return false;
    }
    let mut delivery = delivery();
    if !insert_route(&mut delivery.routes, focus) {
        return false;
    }
    let _ = LOCK_ROUTE.fetch_update(Ordering::AcqRel, Ordering::Acquire, |word| {
        Some(lock_word::register(word, focus.id))
    });
    true
}

/// Unlock: `target` becomes the active route, then the bit clears, both
/// under `DELIVERY` with the epoch bumped (a batch decoded for the lock
/// focus never reaches `target`). `false`: no lock focus is registered, or
/// `target` can't be a route (a pointer route, or the cap is full).
#[cfg(feature = "sak-lock-route")]
pub fn release_lock_route(target: &Arc<InputFocus>) -> bool {
    if target.pointer_route || lock_word::id(LOCK_ROUTE.load(Ordering::Acquire)) == 0 {
        return false;
    }
    let mut delivery = delivery();
    if !insert_route(&mut delivery.routes, target) {
        return false;
    }
    // Nothing a client's queue holds now was meant for it after the lock.
    delivery.lock_epoch += 1;
    ACTIVATION_EPOCH.fetch_add(1, Ordering::AcqRel);
    ACTIVE_INPUT_FOCUS.swap(target.id, Ordering::AcqRel);
    let _ = LOCK_ROUTE.fetch_update(Ordering::AcqRel, Ordering::Acquire, |word| {
        Some(lock_word::release(word))
    });
    #[cfg(feature = "freeze-watchdog")]
    crate::watchdog::note_route_activated();
    true
}

/// May a pointer record be forwarded to focus `id` now? Unlocked, any; locked,
/// only the lock focus.
#[cfg(feature = "sak-lock-route")]
fn lock_admits(id: u64) -> bool {
    lock_word::admits(LOCK_ROUTE.load(Ordering::Acquire), id)
}

/// A dying lock focus (the WM exited), under `DELIVERY`. Unlocked: nothing
/// is registered any more. Locked: the session stays closed to clients (fail
/// closed): no route at all, until a WM activates one or registers and
/// releases.
///
/// Jarvis #160 item 7: the active route is cleared BEFORE the unlocked word
/// is published, so no lock-free route read sees the client's route with the
/// lock gone. Only Super+L (unlocked to locked) can change the word while
/// `DELIVERY` is held, so a failed compare-exchange is retried as locked.
#[cfg(feature = "sak-lock-route")]
fn drop_lock_route(delivery: &mut Delivery, id: u64) {
    let mut closed = false;
    loop {
        let word = LOCK_ROUTE.load(Ordering::Acquire);
        let Some(next) = lock_word::dropped(word, id) else {
            return;
        };
        if word & lock_word::ENGAGED != 0 && !closed {
            closed = true;
            ACTIVE_INPUT_FOCUS.store(0, Ordering::Release);
            ACTIVATION_EPOCH.fetch_add(1, Ordering::AcqRel);
            delivery.lock_epoch += 1;
        }
        #[cfg(feature = "neo-test-lock-route-races")]
        race::point(race::DROP);
        if LOCK_ROUTE
            .compare_exchange(word, next, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            return;
        }
    }
}

/// L2b: empty a client queue holding input from before the last end of a
/// lock (`Delivery::lock_epoch`), so input that crossed a lock never reaches
/// a client afterwards. The pointer route (the WM's) is never locked out.
#[cfg(feature = "sak-lock-route")]
fn fresh(entry: &mut RouteEntry, lock_epoch: u64) {
    if entry.pointer || entry.lock_epoch == lock_epoch {
        return;
    }
    entry.lock_epoch = lock_epoch;
    let mut n = 0;
    while entry.queue.pop_front().is_some() {
        n += 1;
    }
    HANDOFF_DROPPED.fetch_add(n, Ordering::Relaxed);
}

/// `lock_route_races` (Jarvis #160 items 1, 6, 7): one armed point in the
/// receive, forward or drop path, between a route check and its effect, runs
/// what another CPU could run there, so each schedule is reproduced on one
/// CPU. RECV and FORWARD engage the lock (Super+L takes no lock); DROP
/// records whether the focus [`race::WATCH`] names was receivable then.
#[cfg(feature = "neo-test-lock-route-races")]
pub mod race {
    use core::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering};

    pub const RECV: u8 = 1;
    pub const FORWARD: u8 = 2;
    pub const DROP: u8 = 3;
    /// The armed point (0: none). One-shot.
    pub static ARMED: AtomicU8 = AtomicU8::new(0);
    /// DROP: the focus id to read the route of.
    pub static WATCH: AtomicU64 = AtomicU64::new(0);
    /// DROP: was [`WATCH`] the route when the point ran?
    pub static WATCHED_ROUTE: AtomicBool = AtomicBool::new(false);
    /// Times an armed point ran.
    pub static HITS: AtomicU64 = AtomicU64::new(0);

    pub(super) fn point(at: u8) {
        if ARMED
            .compare_exchange(at, 0, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return;
        }
        HITS.fetch_add(1, Ordering::Relaxed);
        if at == DROP {
            let watch = WATCH.load(Ordering::Acquire);
            WATCHED_ROUTE.store(super::input_route() == watch, Ordering::Release);
        } else {
            super::engage_lock_route();
        }
    }
}
