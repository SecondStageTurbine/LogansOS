// SPDX-License-Identifier: MPL-2.0
//! The generational slot arena (ANSPEC-001 §9.12 ownership, BUILD-O1a,
//! ADR-042): the kernel as the unique strong owner of a tensor at rest.
//!
//! Holders keep a [`SlotKey`] — never an `Arc`. *Use* mints a short-lived
//! `Arc` through the P4a reservation gate ([`mint`] = our `fget`: generation
//! check first, then `guarded_clone`, which parks across an in-flight
//! migration and fails cleanly on a drained slot — the `inc_not_zero`
//! discipline). *Freeing* drains via the P4a take protocol (waits for an
//! in-flight migration to commit, never tears) and bumps the slot's
//! generation, so a stale key can never reach the next occupant (invariant
//! I-13). With the slot holding the only resting `Arc`, the I-4 claim gate
//! (`strong == 1 && weak == 0`) now means exactly "no use in flight".
//!
//! The table lock (`lock_order::ARENA`, 12, below `TENSOR_SLOT`) is never
//! held across a park: `mint` snapshots the occupant's [`SlotRef`] and drops
//! the lock before touching the tensor slot. A racing free-then-realloc
//! drains the *snapshotted* slot (the mint observes `None`); the new
//! occupant is unreachable because the stale path never re-derefs the table.
//!
//! Kill-switch (Rule 1): [`set_reuse_enabled`]`(false)` makes `free` drain
//! and bump but never recycle the index — the arena degenerates to the
//! append-only registry it replaced (BUILD-P4b shape, bit-for-bit; the pure
//! equality lives in `arena_logic` + the host suite, the behavioral proof is
//! the unchanged P4b family).
//!
//! Decisions live in the pure twin `arena_logic.rs` (host-tested); this file
//! owns the `Arc`s, the table lock, and the reservation integration.
//! Zero `unsafe`.

extern crate alloc;

use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::ai::reservation::{self, TensorSlot};
use crate::ai::tensor::{Device, Tensor};
#[cfg(feature = "neo-test-arena-mixed-if-control")]
use crate::sync::SwitchMutex;

#[path = "arena_logic.rs"]
mod arena_logic;
use arena_logic::AllocPick;
pub use arena_logic::SlotKey;

/// A tensor the policy layer may manage: the P4a tensor slot plus the reuse
/// signal and the device the tensor belongs on when hot. Moved here from
/// `residency_daemon` by BUILD-O1a (the arena owns the slot type; the daemon
/// re-exports it, so scenario code is source-identical).
pub struct ManagedSlot {
    slot: TensorSlot,
    pub(super) hot: AtomicBool,
    /// Set at prefetch commit, cleared by `mark_used`: a device-resident
    /// tensor with this flag and a withdrawn prediction was prefetched for
    /// nothing — the daemon rolls it back and counts the misprediction.
    pub(super) prefetched: AtomicBool,
    pub(super) home: Device,
}

impl ManagedSlot {
    pub const fn new(home: Device) -> Self {
        Self {
            slot: TensorSlot::new(),
            hot: AtomicBool::new(false),
            prefetched: AtomicBool::new(false),
            home,
        }
    }

    pub fn slot(&self) -> &TensorSlot {
        &self.slot
    }

    /// Install a tensor into the managed slot (tenant-side).
    pub fn put(&self, tensor: Arc<Tensor>) {
        self.slot.put(tensor);
    }

    /// Drain the slot (tenant-side teardown); retries while reserved.
    pub fn take(&self) -> Option<Arc<Tensor>> {
        self.slot.take()
    }

    /// Mint a reference through the P4a park protocol (tenant-side use).
    pub fn guarded_clone(&self) -> Option<Arc<Tensor>> {
        reservation::guarded_clone(&self.slot)
    }
}

/// How an arena entry holds its slot: legacy `'static` registrations (the
/// daemon-gate pool — never freed, the kill-switch shape by construction) or
/// arena-owned allocations. Cloning is cheap (a ref copy or an `Arc` bump);
/// a clone keeps the *slot* alive, never the tensor — tensor lifetime stays
/// with the resting `Arc` inside the `TensorSlot` (I-4).
#[derive(Clone)]
pub(super) enum SlotRef {
    Static(&'static ManagedSlot),
    Owned(Arc<ManagedSlot>),
}

impl SlotRef {
    pub(super) fn managed(&self) -> &ManagedSlot {
        match self {
            SlotRef::Static(slot) => slot,
            SlotRef::Owned(slot) => slot,
        }
    }
}

struct ArenaEntry {
    generation: u32,
    /// How many handles name this slot (the GEM `handle_count` analogue).
    /// `release` frees on the transition to zero. Static registrations hold
    /// a permanent count of 1 that nothing releases.
    handles: u32,
    occupant: Option<SlotRef>,
}

struct ArenaTable {
    entries: Vec<ArenaEntry>,
    free_list: Vec<u32>,
}

/// The arena table. Ordered at `ARENA` (12): a slot acquire MAY nest under it
/// (12 < 15), but no arena path parks while holding it.
///
/// IRQ-safe (CP-37/CP-56): the forward on the inference stack (IF=1,
/// preemptible) and the tensor syscalls and handle table (IF=0) all take it,
/// so a spin lock held IF-on could be preempted while a same-CPU IF=0 taker
/// spun on it forever. Every section is short and park-free (`free`/`release`
/// drain the occupant after the lock), so holding it with interrupts masked
/// costs nothing. No ISR takes it (I-14(a)); masking only affects the holder.
/// Kill-switch: `neo-test-arena-mixed-if-control` restores the plain
/// `SwitchMutex` (test only).
#[cfg(not(feature = "neo-test-arena-mixed-if-control"))]
type ArenaLock = crate::sync::IrqSafeMutex<ArenaTable>;
#[cfg(feature = "neo-test-arena-mixed-if-control")]
type ArenaLock = SwitchMutex<ArenaTable>;

static ARENA: ArenaLock = ArenaLock::new_ordered(
    ArenaTable {
        entries: Vec::new(),
        free_list: Vec::new(),
    },
    crate::sync::lock_order::ARENA,
);

/// `ARENA` acquisitions that found interrupts enabled inside the section
/// (the CP-56 witness: must stay 0).
#[cfg(feature = "neo-test-arena-if-off")]
static HELD_IF_ON: AtomicU64 = AtomicU64::new(0);

#[cfg(feature = "neo-test-arena-if-off")]
pub fn held_if_on_for_test() -> u64 {
    HELD_IF_ON.load(Ordering::Acquire)
}

/// Every `ARENA` acquisition goes through here.
fn arena_lock() -> impl core::ops::DerefMut<Target = ArenaTable> {
    let table = ARENA.lock();
    #[cfg(feature = "neo-test-arena-if-off")]
    if crate::arch::x86_64::interrupts_enabled() {
        HELD_IF_ON.fetch_add(1, Ordering::AcqRel);
    }
    table
}

/// Rule-1 kill-switch: reuse disabled ⇒ freed indices are never reissued —
/// the append-only pre-O1a registry, bit-for-bit.
static REUSE_ENABLED: AtomicBool = AtomicBool::new(true);

#[allow(dead_code)] // Rule-1 kill-switch entry — gate + host-equality consumer
pub fn set_reuse_enabled(enabled: bool) {
    REUSE_ENABLED.store(enabled, Ordering::Release);
}

// --- I-13 / slot-balance discriminator counters --------------------------------

static ALLOCS: AtomicU64 = AtomicU64::new(0);
static FREES: AtomicU64 = AtomicU64::new(0);
/// Keyed operations refused on a stale or out-of-range key. The reuse-attack
/// gate demands this is > 0 (stale keys really were generated) while the
/// scenario-side count of stale mints that *returned a tensor* is exactly 0.
static GEN_MISMATCHES: AtomicU64 = AtomicU64::new(0);

#[allow(dead_code)] // I-13 discriminator surface — gate-consumed (Rule 5: never dead)
pub fn reset_counters() {
    ALLOCS.store(0, Ordering::Relaxed);
    FREES.store(0, Ordering::Relaxed);
    GEN_MISMATCHES.store(0, Ordering::Relaxed);
}

#[allow(dead_code)] // I-13 discriminator surface — gate-consumed (Rule 5: never dead)
pub fn allocs() -> u64 {
    ALLOCS.load(Ordering::Relaxed)
}
#[allow(dead_code)] // I-13 discriminator surface — gate-consumed (Rule 5: never dead)
pub fn frees() -> u64 {
    FREES.load(Ordering::Relaxed)
}
#[allow(dead_code)] // I-13 discriminator surface — gate-consumed (Rule 5: never dead)
pub fn gen_mismatches() -> u64 {
    GEN_MISMATCHES.load(Ordering::Relaxed)
}

// --- The arena protocol ---------------------------------------------------------

/// Install a tensor into a fresh (or recycled) slot; the slot becomes the
/// unique resting owner. Returns the key at the slot's current generation.
#[allow(dead_code)] // O1a mechanism surface — gate-consumed; O1b wires Handle/Model onto it
pub fn alloc(tensor: Arc<Tensor>, home: Device) -> SlotKey {
    let managed = Arc::new(ManagedSlot::new(home));
    // Populate before publishing into the table: no racer can see the slot
    // until the entry lands under the lock below.
    managed.put(tensor);
    let mut table = arena_lock();
    let ArenaTable { entries, free_list } = &mut *table;
    let key = match arena_logic::alloc_pick(free_list, entries.len()) {
        AllocPick::Reuse(index) => {
            let entry = &mut entries[index as usize];
            debug_assert!(entry.occupant.is_none(), "free list pointed at a live slot");
            entry.handles = 1;
            entry.occupant = Some(SlotRef::Owned(managed));
            SlotKey {
                index,
                generation: entry.generation,
            }
        }
        AllocPick::Grow(index) => {
            entries.push(ArenaEntry {
                generation: 0,
                handles: 1,
                occupant: Some(SlotRef::Owned(managed)),
            });
            SlotKey {
                index,
                generation: 0,
            }
        }
    };
    ALLOCS.fetch_add(1, Ordering::Relaxed);
    key
}

/// Adopt a `'static` slot into the arena's iteration surface (the legacy
/// daemon registration path). Permanent: handles = 1 that nothing releases,
/// no key handed out, never counted toward the alloc/free balance.
pub(super) fn register_static(slot: &'static ManagedSlot) {
    let mut table = arena_lock();
    let ArenaTable { entries, free_list } = &mut *table;
    match arena_logic::alloc_pick(free_list, entries.len()) {
        AllocPick::Reuse(index) => {
            let entry = &mut entries[index as usize];
            debug_assert!(entry.occupant.is_none(), "free list pointed at a live slot");
            entry.handles = 1;
            entry.occupant = Some(SlotRef::Static(slot));
        }
        AllocPick::Grow(_) => {
            entries.push(ArenaEntry {
                generation: 0,
                handles: 1,
                occupant: Some(SlotRef::Static(slot)),
            });
        }
    }
}

/// The generation-checked snapshot every keyed *use* path starts with: a
/// stale, out-of-range, or vacant key counts a mismatch and yields `None`;
/// a live key yields a cloned [`SlotRef`] — and the table lock is NOT held
/// by the caller afterward, so a racing free-then-realloc can only drain
/// the snapshotted slot, never alias the new occupant (I-13).
fn lookup(key: SlotKey) -> Option<SlotRef> {
    let table = arena_lock();
    let Some(entry) = table.entries.get(key.index as usize) else {
        GEN_MISMATCHES.fetch_add(1, Ordering::Relaxed);
        return None;
    };
    if !arena_logic::key_matches(entry.generation, key, key.index) {
        GEN_MISMATCHES.fetch_add(1, Ordering::Relaxed);
        return None;
    }
    match &entry.occupant {
        Some(slot_ref) => Some(slot_ref.clone()),
        None => {
            GEN_MISMATCHES.fetch_add(1, Ordering::Relaxed);
            None
        }
    }
}

/// Mint a short-lived `Arc` for *use* — the `fget` analogue. Generation
/// check first (stale/reused ⇒ `None`, never the new occupant), then the
/// P4a `guarded_clone` (parks across a migration; `None` if the slot was
/// drained between the snapshot and the clone — the free won that race).
#[allow(dead_code)] // O1a mechanism surface — gate-consumed; O1b wires Handle/Model onto it
pub fn mint(key: SlotKey) -> Option<Arc<Tensor>> {
    let slot_ref = lookup(key)?;
    reservation::guarded_clone(slot_ref.managed().slot())
}

// --- Race-gate probe seams (the O1a gate's claimer role + deterministic park) --

/// The daemon role in one keyed call: the same generation-checked snapshot
/// `mint` takes, then `try_migrate` on the slot. `None` on a stale key.
#[allow(dead_code)] // O1a race-gate probe seam — feature-consumed
pub fn debug_migrate(
    key: SlotKey,
    target: Device,
    owner: u64,
) -> Option<reservation::MigrateOutcome> {
    let slot_ref = lookup(key)?;
    Some(reservation::try_migrate(
        slot_ref.managed().slot(),
        target,
        owner,
    ))
}

/// Keyed wrapper over the P4a held-open claim probe (the deterministic-park
/// phase: hold a claim until an accessor is observed parked, then release).
#[allow(dead_code)] // O1a race-gate probe seam — feature-consumed
pub fn debug_claim(key: SlotKey, owner: u64) -> Option<reservation::ClaimDecision> {
    let slot_ref = lookup(key)?;
    Some(reservation::debug_claim_only(
        slot_ref.managed().slot(),
        owner,
    ))
}

/// Keyed wrapper over the P4a probe release (wakes every parked accessor).
#[allow(dead_code)] // O1a race-gate probe seam — feature-consumed
pub fn debug_release(key: SlotKey, tensor_id: u64) {
    if let Some(slot_ref) = lookup(key) {
        reservation::debug_release(slot_ref.managed().slot(), tensor_id);
    }
}

/// Free a slot: generation-checked (stale ⇒ no-op), drains via the P4a take
/// protocol (a free racing an in-flight migration waits for the commit,
/// never tears), bumps the generation, and recycles the index (unless the
/// kill-switch disabled reuse). Static registrations are not freeable.
#[allow(dead_code)] // O1a mechanism surface — gate-consumed; O1b wires Handle/Model onto it
pub fn free(key: SlotKey) -> bool {
    let taken = {
        let mut table = arena_lock();
        let reuse = REUSE_ENABLED.load(Ordering::Acquire);
        let ArenaTable { entries, free_list } = &mut *table;
        let Some(entry) = entries.get_mut(key.index as usize) else {
            GEN_MISMATCHES.fetch_add(1, Ordering::Relaxed);
            return false;
        };
        let refused = !arena_logic::key_matches(entry.generation, key, key.index)
            || matches!(entry.occupant, Some(SlotRef::Static(_)) | None);
        if refused {
            GEN_MISMATCHES.fetch_add(1, Ordering::Relaxed);
            return false;
        }
        entry.generation = arena_logic::bump_generation(entry.generation);
        entry.handles = 0;
        let occupant = entry.occupant.take();
        arena_logic::recycle_slot(free_list, key.index, reuse);
        occupant
    };
    if let Some(slot_ref) = taken {
        drain(slot_ref.managed());
    }
    FREES.fetch_add(1, Ordering::Relaxed);
    true
}

/// One more handle names the slot. Stale key, a static registration (whose
/// permanent count of 1 is kernel bookkeeping no key may touch — the same
/// refusal `free`/`release` apply), or counter overflow ⇒ refused.
#[allow(dead_code)] // O1a mechanism surface — gate-consumed; O1b wires Handle/Model onto it
pub fn retain(key: SlotKey) -> bool {
    let mut table = arena_lock();
    let Some(entry) = table.entries.get_mut(key.index as usize) else {
        GEN_MISMATCHES.fetch_add(1, Ordering::Relaxed);
        return false;
    };
    if !arena_logic::key_matches(entry.generation, key, key.index)
        || matches!(entry.occupant, Some(SlotRef::Static(_)) | None)
    {
        GEN_MISMATCHES.fetch_add(1, Ordering::Relaxed);
        return false;
    }
    match arena_logic::retain_count(entry.handles) {
        Some(next) => {
            entry.handles = next;
            true
        }
        None => false,
    }
}

/// One handle dropped; the last release frees the slot (drain + bump +
/// recycle — the same path as [`free`]). Returns whether this call freed.
/// Stale key or a release below zero ⇒ refused no-op.
#[allow(dead_code)] // O1a mechanism surface — gate-consumed; O1b wires Handle/Model onto it
pub fn release(key: SlotKey) -> bool {
    let taken = {
        let mut table = arena_lock();
        let reuse = REUSE_ENABLED.load(Ordering::Acquire);
        let ArenaTable { entries, free_list } = &mut *table;
        let Some(entry) = entries.get_mut(key.index as usize) else {
            GEN_MISMATCHES.fetch_add(1, Ordering::Relaxed);
            return false;
        };
        let refused = !arena_logic::key_matches(entry.generation, key, key.index)
            || matches!(entry.occupant, Some(SlotRef::Static(_)) | None);
        if refused {
            GEN_MISMATCHES.fetch_add(1, Ordering::Relaxed);
            return false;
        }
        let Some((next, free_now)) = arena_logic::release_count(entry.handles) else {
            return false;
        };
        entry.handles = next;
        if !free_now {
            return false;
        }
        entry.generation = arena_logic::bump_generation(entry.generation);
        let occupant = entry.occupant.take();
        arena_logic::recycle_slot(free_list, key.index, reuse);
        occupant
    };
    if let Some(slot_ref) = taken {
        drain(slot_ref.managed());
    }
    FREES.fetch_add(1, Ordering::Relaxed);
    true
}

/// Snapshot the live slots' KEYS in index order — the daemon's walk surface
/// (BUILD-O1b, the ISC-6 fix): pure u32 copies under the table lock, zero
/// `Arc` bumps. The walker resolves each key lazily via [`slot_ref`] outside
/// the lock; a slot freed between snapshot and resolve simply misses (and
/// counts a generation mismatch — a legitimate stale-key event). With
/// never-freed slots, index order IS registration order: the append-only
/// registry's iteration, unchanged (the O1a ISC-2 equivalence).
pub(super) fn live_keys() -> Vec<SlotKey> {
    arena_lock()
        .entries
        .iter()
        .enumerate()
        .filter(|(_, entry)| entry.occupant.is_some())
        .map(|(index, entry)| SlotKey {
            index: index as u32,
            generation: entry.generation,
        })
        .collect()
}

/// Resolve one walk key to its slot — the per-candidate `Arc` bump, taken
/// outside the walk's snapshot lock (`None` for a since-freed slot).
pub(super) fn slot_ref(key: SlotKey) -> Option<SlotRef> {
    lookup(key)
}

/// How many handles currently name this slot — the §3.7 `sys_tensor_query`
/// logical-refcount source (BUILD-O1b): with install-retains/remove-releases,
/// this equals the number of live handle-table entries for the tensor,
/// exactly the pre-O1b `Arc`-derived figure at any syscall-quiescent point.
#[allow(dead_code)] // O1b syscall surface — query path consumer
pub fn handle_count(key: SlotKey) -> Option<u32> {
    let table = arena_lock();
    let entry = table.entries.get(key.index as usize)?;
    if !arena_logic::key_matches(entry.generation, key, key.index) || entry.occupant.is_none() {
        return None;
    }
    Some(entry.handles)
}

/// The home device for production tensor allocations (BUILD-O1b): the first
/// capacity-bounded accelerator (the only kind the daemon manages), else
/// CPU. `home` gates daemon victim/prefetch eligibility only — placement
/// still decides where ops actually run.
#[allow(dead_code)] // O1b syscall surface — create/load path consumer
pub fn default_home() -> Device {
    for index in 0..crate::drivers::accel::device_count() {
        if let Some(driver) = crate::drivers::accel::get_device(index)
            && driver.capacity_bytes().is_some()
        {
            return driver.descriptor().device;
        }
    }
    Device::Cpu
}

/// Drain a slot that has already left the table: take the resting `Arc` out,
/// parking (via the P4a accessor protocol) across any in-flight migration
/// and retrying — a free never tears a migration and never spins IRQ-off.
/// Terminates: the entry is gone from the table, so no new walk picks the
/// slot up; only an already-snapshotted daemon op can re-reserve it, and
/// each such op commits or rolls back in bounded time.
fn drain(managed: &ManagedSlot) {
    loop {
        if managed.take().is_some() {
            return; // the resting Arc drops here — storage accounting follows
        }
        // take() refuses while Reserved: park until the mover commits, then
        // retry the take. A true Empty (already drained) ends the loop.
        if reservation::guarded_clone(managed.slot()).is_none() {
            return;
        }
        crate::sched::yield_now();
    }
}

/// Test seam (`i2c_hid_absent`, I-14(a)): run `f` while this thread holds
/// `ARENA`, so an ISR that wanted it would never get it.
/// Must not run alongside concurrent AI reservation or accounting activity:
/// the governance locks `f` nests under `ARENA` are plain `SwitchMutex`es, not
/// IRQ-safe (Jarvis's review of #125, relay #62).
#[cfg(feature = "neo-test-i2c-hid-absent")]
pub fn debug_with_arena_held<R>(f: impl FnOnce() -> R) -> R {
    let _arena = arena_lock();
    f()
}
