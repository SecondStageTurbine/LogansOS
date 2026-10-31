// SPDX-License-Identifier: MPL-2.0
//! Phase 7 BUILD-3: capability-mediated tensor sharing + content-hash dedup — the
//! **global-view ownership** governance capability.
//!
//! The kernel sees, and enforces, that multiple principals reference **one physical
//! copy** of an immutable tensor. Two paths to the same fact:
//!
//! - **Explicit:** principal A *grants* a read-only tensor capability to principal B
//!   (Phase-2 derive/grant/revoke). One physical copy, kernel-enforced rights.
//! - **Implicit (dedup):** two principals load content-identical immutable model
//!   weights → the kernel keeps **one** physical copy, refcounted (`Arc<Tensor>`).
//!
//! Both produce the trace `tensor <id> shared RO {PID..} — 1 physical copy`. This is
//! a fact only the kernel can know: a per-process userspace runtime sees only its own
//! handles and would re-copy. It is the clearest "kernel does what a library
//! structurally can't."
//!
//! The refcount is the existing `Arc<Tensor>` (cross-process references are cloned
//! Arcs sharing one physical allocation — no new allocator). This module adds the
//! three layers on top: **visibility** (who references the copy — derived by scanning
//! handle tables, so revoke/drop need no hooking here), **dedup** (a content-hash →
//! `SlotKey` table since BUILD-O2/ADR-044 — the arena's I-13 generation check is
//! the liveness probe, so candidacy pins nothing and a deduped idle weight stays
//! claimable; the share survives movement because every sharer resolves through
//! the same slot), and the **§9.5 accounting rule** (count the physical bytes
//! once, to the owner — readers add zero; ISC-4). The pure attribution policy is in
//! [`sharing_logic`].
//!
//! Scope (THINK Q5, dispatch §1): **read-only** sharing of **CPU-resident** tensors,
//! reusing the capability machinery — no new authority primitive. Mutable coherent
//! sharing and concurrent *device-side* RO-vs-RW enforcement (the Intel PPGTT lift)
//! are BUILD-4 seams, noted not built.

// Entry points are exercised by the gated BUILD-3 gate scenario, the live model-load
// dedup path, and the cross-process grant hook; suppress dead-code noise in plain
// builds rather than gate the module.

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::ai::tensor::{Device, Tensor};
use crate::drivers::accel::accounting;
use crate::sched::process::{self, ProcessId};
use crate::serial_println;
use crate::sync::SwitchMutex;

#[path = "sharing_logic.rs"]
mod sharing_logic;
// total_resident_bytes rides the re-export for the §9.7 gate harness
// (feature neo-test-accel-tensor-sharing, test_harness::run_accel_tensor_sharing
// — sharing_logic is a private path-mod, so this is its only kernel path).
#[allow(unused_imports)] // total_resident_bytes is consumed only by the §9.7 gate feature
pub use sharing_logic::{SharedRecord, total_resident_bytes};

/// Shared CPU-resident tensor residency is attributed on the CPU device in §9.5
/// (BUILD-3 is CPU-resident first; device-resident shared tensors are a BUILD-4 seam).
const SHARE_DEVICE: Device = Device::Cpu;

/// Kernel-side record for one shared physical tensor: the pure accounted facts
/// (owner + bytes) plus the optional content hash that keyed it into the dedup table.
struct Entry {
    record: SharedRecord,
    hash: Option<[u8; 32]>,
}

/// Per-physical-tensor sharing registry, keyed by the stable `Tensor::id()`. Holds
/// the accounting facts (owner + physical bytes) and the dedup hash. The *reader set*
/// is intentionally NOT stored here — it is derived on demand by scanning handle
/// tables ([`readers`]), so explicit revoke / cap-drop / model-drop need no hook into
/// the generic capability machinery to stay consistent.
static REGISTRY: SwitchMutex<BTreeMap<u64, Entry>> = SwitchMutex::new(BTreeMap::new());

/// Content-hash → the one owning SLOT (BUILD-O2, ADR-044): candidacy is the map
/// entry itself and pins nothing — the arena's generation check (I-13) is the
/// liveness probe, replacing `Weak::upgrade` (a freed slot is a clean miss,
/// removed lazily). N sharers → 1 `SlotKey` → 1 resting `Arc` → 1 physical
/// copy that can MOVE (demote/refetch) without breaking the share.
static DEDUP: SwitchMutex<BTreeMap<[u8; 32], crate::ai::arena::SlotKey>> =
    SwitchMutex::new(BTreeMap::new());

/// BLAKE3 content hash of `bytes` — the dedup key (same primitive NeoFS
/// content-addressing uses).
pub fn content_hash(bytes: &[u8]) -> [u8; 32] {
    *blake3::hash(bytes).as_bytes()
}

/// Register `tensor` as a shared physical copy owned by `owner`, if not already
/// registered. Idempotent: a second sharer (further grant, dedup hit) does NOT
/// re-account residency — the physical bytes are added to §9.5 exactly **once**, to
/// the owner (ISC-4). `hash` is `Some` for dedup-registered weights (so a later load
/// can find this copy) and `None` for explicit grants.
fn register(tensor: &Arc<Tensor>, owner: ProcessId, hash: Option<[u8; 32]>) {
    let id = tensor.id();
    let bytes = tensor.byte_len() as u64;
    let mut registry = REGISTRY.lock();
    if registry.contains_key(&id) {
        return;
    }
    registry.insert(
        id,
        Entry {
            record: SharedRecord {
                owner: owner.0,
                bytes,
            },
            hash,
        },
    );
    drop(registry);
    tensor.mark_shared();
    // The physical bytes become governed residency, attributed once to the owner.
    accounting::resident_add(SHARE_DEVICE, owner, bytes);
    // BUILD-O2: the dedup map is keyed by SlotKey, published separately by
    // [`publish_dedup_candidate`] once the caller has alloc'd the slot — the
    // registration itself no longer creates any reference (nothing pins).
}

/// Explicit cross-process RO share (ISC-2): a tensor capability was granted from
/// `owner` to `recipient`. Registers the physical tensor as shared (owner-attributed,
/// idempotent) and emits the ownership trace now that ≥2 principals reference it.
/// Called from the capability grant-completion path for `Handle::Tensor`.
pub fn on_tensor_shared(tensor: &Arc<Tensor>, owner: ProcessId, recipient: ProcessId) {
    register(tensor, owner, None);
    let _ = recipient; // the reader set is derived by scanning handle tables.
    trace_shared(tensor.id());
}

/// Probe the dedup table for an immutable tensor with content hash `hash`
/// (BUILD-O2 shape): copy the `SlotKey` out under the map lock, then `mint`
/// it OUTSIDE the lock — mint can park across an in-flight migration (the
/// recorded park-under-lock class), and a probe racing a demote must resolve
/// to the post-commit tensor, not deadlock. A hit returns the key (for the
/// caller's `retain`) plus the live `Arc`; a stale key (generation mismatch,
/// I-13 — the `Weak::upgrade`-failed analogue) is removed lazily, re-checking
/// the entry still maps to the SAME key (another loader may have already
/// re-published a fresh candidate under this hash).
pub fn dedup_probe(hash: [u8; 32]) -> Option<(crate::ai::arena::SlotKey, Arc<Tensor>)> {
    let key = { DEDUP.lock().get(&hash).copied() }?;
    match crate::ai::arena::mint(key) {
        Some(arc) => Some((key, arc)),
        None => {
            let mut dedup = DEDUP.lock();
            if dedup.get(&hash) == Some(&key) {
                dedup.remove(&hash);
            }
            None
        }
    }
}

/// Register a freshly-allocated, content-hashed immutable weight as a dedup
/// candidate owned by `owner` (dedup miss path): the §9.5 count-once
/// accounting + the registry entry. The hash→slot mapping is published
/// separately ([`publish_dedup_candidate`]) once the slot exists.
pub fn register_dedup_weight(tensor: &Arc<Tensor>, owner: ProcessId, hash: [u8; 32]) {
    register(tensor, owner, Some(hash));
}

/// Publish the hash → slot mapping (BUILD-O2): the candidacy itself. Pins
/// nothing — a later load probes it through the arena's generation check.
pub fn publish_dedup_candidate(hash: [u8; 32], key: crate::ai::arena::SlotKey) {
    DEDUP.lock().insert(hash, key);
}

/// A dedup hit just shared the existing copy with `recipient` (ISC-3): emit the
/// ownership trace now that ≥2 principals reference it.
pub fn on_dedup_hit(tensor_id: u64, recipient: ProcessId) {
    let _ = recipient;
    trace_shared(tensor_id);
}

/// The last `Arc<Tensor>` to this physical copy has dropped (called from
/// `Tensor::Drop` only when the tensor was shared). Releases the §9.5 residency
/// attributed to its owner and removes the registry + dedup entries — counted once,
/// released once (ISC-4). No-op if not registered.
pub fn on_tensor_drop(tensor_id: u64, _bytes: u64) {
    let entry = REGISTRY.lock().remove(&tensor_id);
    if let Some(entry) = entry {
        accounting::resident_sub(
            SHARE_DEVICE,
            ProcessId(entry.record.owner),
            entry.record.bytes,
        );
        if let Some(h) = entry.hash {
            DEDUP.lock().remove(&h);
        }
    }
}

/// The owning principal of a shared physical tensor, if registered.
#[allow(dead_code)] // §9.7 sharing surface — scenario/syscall probe (owner attribution)
pub fn owner_of(tensor_id: u64) -> Option<ProcessId> {
    REGISTRY
        .lock()
        .get(&tensor_id)
        .map(|e| ProcessId(e.record.owner))
}

/// Physical byte size of a shared tensor (counted once), if registered.
pub fn physical_bytes(tensor_id: u64) -> Option<u64> {
    REGISTRY.lock().get(&tensor_id).map(|e| e.record.bytes)
}

/// Whether a physical tensor is currently in the sharing registry.
pub fn is_registered(tensor_id: u64) -> bool {
    REGISTRY.lock().contains_key(&tensor_id)
}

/// The set of principals that currently reference physical tensor `tensor_id` — its
/// **readers**. Derived by scanning every process's capability table for a tensor
/// capability (a `Handle::Tensor`) or a model (`Handle::Model`, whose weights it holds)
/// referencing this id. Because the scan reads live handle tables, a revoked or
/// dropped capability is reflected immediately with **no** hook into revoke/drop —
/// that is the point of deriving rather than storing the reader set (ISC-2/5).
/// Sorted + deduplicated.
pub fn readers(tensor_id: u64) -> Vec<ProcessId> {
    process::tensor_reader_pids(tensor_id)
}

/// Emit the ownership trace — the load-bearing governance line — for `tensor_id`:
/// `tensor <id> shared RO {PID..} — 1 physical copy`. Fires only when ≥2 principals
/// reference the copy (an actual share). Snapshots the registry, then scans handle
/// tables (never holding the registry lock across the scan).
pub fn trace_shared(tensor_id: u64) {
    if !is_registered(tensor_id) {
        return;
    }
    let pids = readers(tensor_id);
    if pids.len() < 2 {
        return;
    }
    let bytes = physical_bytes(tensor_id).unwrap_or(0);
    let mut out = alloc::string::String::new();
    use core::fmt::Write as _;
    let _ = write!(out, "tensor {tensor_id} shared RO {{");
    for (i, pid) in pids.iter().enumerate() {
        if i > 0 {
            let _ = write!(out, ", ");
        }
        let _ = write!(out, "PID{}", pid.0);
    }
    let _ = write!(out, "}} — 1 physical copy ({bytes} bytes)");
    serial_println!("accel-share: {}", out);
    crate::log::flush();
}
