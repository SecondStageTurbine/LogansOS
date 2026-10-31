// SPDX-License-Identifier: MPL-2.0
//! The fenced DMA arena (Phase 7.5 BUILD-0, ADR-045): the I-14(c1) guard —
//! containment against OUR OWN errant descriptor programming, the realistic
//! near-certain failure class of a from-scratch xHCI/NIC bring-up.
//!
//! **The shape (the Linux restricted-DMA-pool / swiotlb pattern):** a
//! pre-registered contiguous physical region; connectivity drivers allocate
//! their DMA buffers ONLY from it ([`super::block::DmaBuffer::alloc_in_arena`]);
//! and the descriptor-programming boundary accepts ONLY a [`FencedDmaAddr`] —
//! a witness type with private fields whose single constructor is the
//! [`fence`] gate. A driver that skips validation cannot type-check a device
//! programming call: the same construction-proof as the tensor arena's
//! `mint` (I-4) and `SlotKey` (I-13).
//!
//! **Lock-free gate:** the fence bounds live in atomics — the containment
//! check can never become a contention point, and an uninitialized arena
//! fails CLOSED (`ArenaUnset`), never open. The span table takes one leaf
//! `SwitchMutex` (no new primitive; never held across any other acquisition
//! or park).
//!
//! **What this is NOT (stated per the dispatch's honesty clause):** I-14(c2)
//! — a *misbehaving device* DMA-ing wherever it likes — is NOT contained
//! here; that requires the IOMMU and ships at 7.5-V (VT-d). The arena stops
//! the kernel handing a bad address TO the device, not the device inventing
//! one. Existing DMA users (GPU/NVMe/AHCI/virtio) are untouched and outside
//! the arena: kill-switch (Rule 1) — arena-off is bit-for-bit today.

extern crate alloc;

use core::sync::atomic::{AtomicU64, Ordering};

use x86_64::PhysAddr;

use crate::mm;
use crate::sync::SwitchMutex;

#[path = "dma_arena_logic.rs"]
mod dma_arena_logic;
pub use dma_arena_logic::DmaFault;
// Re-exported so the graphics arena (OT-63 Slice 5) reuses the SAME pure
// span/validation logic without re-`mod`-including the file (which trips
// clippy::duplicate_mod). One home for the arena arithmetic, two arenas.
pub(crate) use dma_arena_logic::{SpanTable, validate};

const PAGE_SIZE: u64 = 4096;

/// Fence bounds. `ARENA_LEN == 0` ⇔ uninitialized ⇔ the gate fails closed.
static ARENA_BASE: AtomicU64 = AtomicU64::new(0);
static ARENA_LEN: AtomicU64 = AtomicU64::new(0);

/// The span book-keeping (offsets only; the region itself is owned by the
/// arena for the kernel's lifetime). Leaf lock: pure arithmetic inside,
/// never held across anything.
static SPANS: SwitchMutex<Option<SpanTable>> = SwitchMutex::new(None);

// --- I-14(c1) discriminator counters -----------------------------------------

static FENCE_ADMITS: AtomicU64 = AtomicU64::new(0);
static FENCE_REFUSALS: AtomicU64 = AtomicU64::new(0);
/// Spans whose release was refused (double free / out-of-fence / corrupt
/// fence) — a silent-leak signal that exists OUTSIDE neo-test builds (Forge
/// MINOR-1: the balance gauge must not vanish in production).
static LEAKED_SPANS: AtomicU64 = AtomicU64::new(0);

#[allow(dead_code)] // I-14(c1) discriminator surface — gate-consumed (Rule 5: never dead)
pub fn fence_admits() -> u64 {
    FENCE_ADMITS.load(Ordering::Relaxed)
}
#[allow(dead_code)] // I-14(c1) discriminator surface — gate-consumed (Rule 5: never dead)
pub fn fence_refusals() -> u64 {
    FENCE_REFUSALS.load(Ordering::Relaxed)
}
#[allow(dead_code)] // I-14(c1) discriminator surface — gate-consumed (Rule 5: never dead)
pub fn leaked_spans() -> u64 {
    LEAKED_SPANS.load(Ordering::Relaxed)
}
#[allow(dead_code)] // I-14(c1) discriminator surface — gate-consumed (Rule 5: never dead)
pub fn reset_counters() {
    FENCE_ADMITS.store(0, Ordering::Relaxed);
    FENCE_REFUSALS.store(0, Ordering::Relaxed);
}

/// A validated DMA address — the ONLY currency the connectivity
/// descriptor-programming surface accepts. Private fields + one constructor
/// (the [`fence`] gate). PRECISION (Forge MAJOR-2): the construction proof
/// covers this type — no `FencedDmaAddr` exists unvalidated. The system-wide
/// single-gate property additionally requires that every connectivity
/// descriptor-programming function take THIS type and never a raw
/// `PhysAddr` (`DmaBuffer::physical_addr` is pre-fence plumbing, legitimate
/// for legacy drivers and for FEEDING the gate). That signature discipline
/// is a named 7.5-A gate obligation (ADR-045), not yet a structural fact.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct FencedDmaAddr {
    phys: u64,
    len: u64,
}

impl FencedDmaAddr {
    pub fn phys(&self) -> u64 {
        self.phys
    }
    pub fn len(&self) -> u64 {
        self.len
    }
}

/// Reserve the fenced region: one contiguous buddy block, registered once.
/// Idempotent-refusing: a second init is an error (the fence is supposed to
/// be a boot-time fact, not a movable target).
#[allow(dead_code)] // 7.5-A boot-path consumer; gate-consumed until then
pub fn init(pages: u64) -> Result<(), DmaFault> {
    if pages == 0 {
        return Err(DmaFault::ZeroLength);
    }
    if ARENA_LEN.load(Ordering::Acquire) != 0 {
        return Err(DmaFault::OutsideArena); // already fenced — refuse to move it
    }
    let mut order = 0usize;
    let mut covered = 1u64;
    while covered < pages {
        covered = covered.checked_mul(2).ok_or(DmaFault::Overflow)?;
        order += 1;
    }
    let frame = mm::with_frame_alloc(|fa| fa.allocate_blocks(order)).ok_or(DmaFault::Overflow)?;
    let base = frame.start_address().as_u64();
    let len = covered.checked_mul(PAGE_SIZE).ok_or(DmaFault::Overflow)?;
    *SPANS.lock() = Some(SpanTable::new(covered));
    ARENA_BASE.store(base, Ordering::Release);
    ARENA_LEN.store(len, Ordering::Release);
    Ok(())
}

/// The fence bounds, if initialized — `(base, len)`.
pub fn bounds() -> Option<(u64, u64)> {
    let len = ARENA_LEN.load(Ordering::Acquire);
    if len == 0 {
        return None;
    }
    Some((ARENA_BASE.load(Ordering::Acquire), len))
}

/// THE GATE (I-14(c1)): validate `[phys, phys+len)` against the fence and
/// mint the witness. Refusals are counted; an uninitialized arena refuses
/// everything (fails closed). Lock-free.
pub fn fence(phys: u64, len: u64) -> Result<FencedDmaAddr, DmaFault> {
    let base = ARENA_BASE.load(Ordering::Acquire);
    let arena_len = ARENA_LEN.load(Ordering::Acquire);
    match validate(phys, len, base, arena_len) {
        Ok(()) => {
            FENCE_ADMITS.fetch_add(1, Ordering::Relaxed);
            Ok(FencedDmaAddr { phys, len })
        }
        Err(fault) => {
            FENCE_REFUSALS.fetch_add(1, Ordering::Relaxed);
            Err(fault)
        }
    }
}

/// Allocate a page-span from the arena: the physical address of the span's
/// first page, or `None` (unset arena / exhausted). Used by
/// `DmaBuffer::alloc_in_arena`; the matching [`release_span`] runs on Drop.
pub(crate) fn alloc_span(pages: u64) -> Option<PhysAddr> {
    let (base, _) = bounds()?;
    let mut spans = SPANS.lock();
    let table = spans.as_mut()?;
    let offset_pages = table.alloc(pages)?;
    // Total arithmetic, like the gate's (Forge MAJOR-1): an offset that
    // cannot be checked is a span that cannot be handed out.
    let phys = offset_pages
        .checked_mul(PAGE_SIZE)
        .and_then(|off| base.checked_add(off))?;
    Some(PhysAddr::new(phys))
}

/// Return a span to the arena (never to the buddy — the region stays
/// reserved). `false` on a forged/double free (refused, counted nowhere —
/// the balance gauge catches imbalance at teardown).
pub(crate) fn release_span(phys: PhysAddr) -> bool {
    let Some((base, len)) = bounds() else {
        return false;
    };
    // Defensive: a phys outside the fence must refuse, never alias offset 0
    // (unreachable today — only arena-allocated buffers call this — but the
    // fence does not rely on its callers being right).
    let raw = phys.as_u64();
    let Some(fence_end) = base.checked_add(len) else {
        LEAKED_SPANS.fetch_add(1, Ordering::Relaxed);
        return false; // a corrupt fence refuses (total arithmetic, Forge MAJOR-1)
    };
    if raw < base || raw >= fence_end {
        LEAKED_SPANS.fetch_add(1, Ordering::Relaxed);
        return false;
    }
    let offset_pages = (raw - base) / PAGE_SIZE;
    let mut spans = SPANS.lock();
    match spans.as_mut() {
        Some(table) => table.free(offset_pages),
        None => false,
    }
}

/// Pages currently allocated from the arena — the teardown balance gauge.
#[allow(dead_code)] // I-14(c1) discriminator surface — gate-consumed (Rule 5: never dead)
pub fn used_pages() -> u64 {
    SPANS.lock().as_ref().map(|t| t.used_pages()).unwrap_or(0)
}
