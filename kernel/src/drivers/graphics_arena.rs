// SPDX-License-Identifier: MPL-2.0
//! The named **graphics** DMA arena (OT-63 GPU P1 Slice 5, ADR-077 §Slice-5 +
//! amendment 5): the contiguous physical region every **GPU-reachable**
//! allocation is drawn from, so the graphics VT-d domain (`iommu::graphics`)
//! can map exactly this region — the GPU's whole working set — **and nothing
//! else**. A GPU DMA outside it faults (the I-14(c2) hardware guard for the
//! iGPU), the physical-memory analogue of the Slice-3 out-of-arena scanout
//! refusal.
//!
//! **Why a SECOND arena, distinct from the connectivity arena
//! (`dma_arena`):** amendment 5 (Chad) — granting the iGPU its working set
//! must NOT expose the NIC/USB DMA buffers. Two separately-reserved buddy
//! blocks are disjoint by construction, so the graphics domain maps the GPU's
//! pages and the connectivity fence domain maps the connectivity pages, with
//! neither reachable from the other's device. The two arenas share the *pure*
//! span/validation logic (`dma_arena_logic`, host-tested) but keep independent
//! statics — the connectivity path is untouched (zero regression).
//!
//! **The kill-switch (Rule-1) lives here, always compiled.** Graphics-VT-d
//! narrowing is a runtime toggle (`GRAPHICS_NARROWING_ENABLED`, default OFF).
//! OFF ⇒ [`narrowing_enabled`] is false ⇒ the accel driver's GPU-reachable
//! allocations use `DmaBuffer::alloc_zeroed` (buddy) exactly as before Slice 5,
//! and the iGPU stays in the identity VT-d domain — **bit-for-bit the Slice-4
//! state**. The toggle is a plain flag (not `vtd`-gated) so the accel path can
//! read it without pulling the `vtd` feature; the DOMAIN machinery that
//! *consumes* this arena is `vtd`-gated in `iommu::graphics`.

extern crate alloc;

use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use x86_64::PhysAddr;

use crate::mm;
use crate::sync::SwitchMutex;

// The graphics arena reuses the connectivity arena's PURE logic (the span
// allocator + the total fence-bounds validation), host-tested independently of
// either arena's statics — one home for the arena arithmetic, two arenas over
// disjoint regions. (Re-`mod`-including the file instead would trip
// clippy::duplicate_mod, so we import the re-exports from `dma_arena`.)
pub use crate::drivers::dma_arena::DmaFault;
use crate::drivers::dma_arena::{SpanTable, validate};

const PAGE_SIZE: u64 = 4096;

/// Graphics-arena fence bounds. `GFX_ARENA_LEN == 0` ⇔ uninitialized ⇔ the
/// gate fails closed (no GPU-reachable arena allocation possible).
static GFX_ARENA_BASE: AtomicU64 = AtomicU64::new(0);
static GFX_ARENA_LEN: AtomicU64 = AtomicU64::new(0);

/// Span book-keeping (offsets only; the region is owned by the arena for the
/// kernel's lifetime). Leaf lock — pure arithmetic inside, never held across
/// anything (the `dma_arena` discipline).
static GFX_SPANS: SwitchMutex<Option<SpanTable>> = SwitchMutex::new(None);

/// The Slice-5 kill-switch (Rule-1). Default OFF: the iGPU stays
/// identity-domained and GPU allocations stay buddy — bit-for-bit Slice-4.
static GRAPHICS_NARROWING_ENABLED: AtomicBool = AtomicBool::new(false);

/// I-14(c) containment state for the iGPU (Slice-4 AAG bounce, 2026-08-01).
///
/// `GFX_DOMAIN_ARMED` is set **only** by a successful `iommu::narrow_graphics`
/// — i.e. the graphics VT-d domain actually holds the requester. It is the
/// difference between "narrowing was requested" (`GRAPHICS_NARROWING_ENABLED`,
/// a policy bit) and "device DMA is actually contained" (a hardware fact).
/// I-14(c2) rests on the latter, so the fail-closed gates below read it.
static GFX_DOMAIN_ARMED: AtomicBool = AtomicBool::new(false);

/// Set once a **real bus-mastering** accelerator is bound. A simulated
/// accelerator (`accel-sim`) never sets it and therefore keeps the
/// pre-Slice-5 buddy path bit-for-bit (the Rule-1 equality is about the
/// simulated/no-device path; it was never a licence for an uncontained
/// hardware DMA master).
static GFX_HW_ACCEL_BOUND: AtomicBool = AtomicBool::new(false);

// ── discriminator counters (gate-consumed; the arena-gate balance signal) ─────
static GFX_ADMITS: AtomicU64 = AtomicU64::new(0);
static GFX_REFUSALS: AtomicU64 = AtomicU64::new(0);
static GFX_LEAKED_SPANS: AtomicU64 = AtomicU64::new(0);

/// Whether Slice-5 graphics-VT-d narrowing is enabled. When false (default),
/// the accel driver allocates GPU-reachable buffers from the buddy allocator
/// exactly as before Slice 5 (the kill-switch equality). Read on every
/// GPU-reachable allocation (`DmaBuffer::alloc_gpu`) and by the production
/// narrowing entry (`iommu::narrow_graphics`).
pub fn narrowing_enabled() -> bool {
    GRAPHICS_NARROWING_ENABLED.load(Ordering::Acquire)
}

/// Set the Slice-5 narrowing kill-switch. Enabling requires the graphics arena
/// to be initialized first (the narrowed domain cannot cover the GPU working
/// set otherwise — amendment 5(ii)); enabling with no arena is refused and
/// leaves narrowing OFF (fail-safe). Disabling always succeeds.
#[allow(dead_code)] // gate/boot-consumed (scenario + production narrow enable)
pub fn set_narrowing(enabled: bool) -> bool {
    if enabled && bounds().is_none() {
        return false; // fail-safe: no arena ⇒ cannot narrow (amendment 5(ii))
    }
    if !enabled {
        // Disarming the policy bit must disarm the containment fact with it,
        // or a later `containment_armed()` would report stale hardware state.
        GFX_DOMAIN_ARMED.store(false, Ordering::Release);
    }
    GRAPHICS_NARROWING_ENABLED.store(enabled, Ordering::Release);
    true
}

/// Record that the graphics VT-d domain now actually holds the iGPU requester.
/// Called **only** from `iommu::narrow_graphics` after at least one source-id
/// was successfully repointed — never from policy code.
#[allow(dead_code)] // set by the vtd narrowing path
pub fn mark_domain_armed() {
    GFX_DOMAIN_ARMED.store(true, Ordering::Release);
}

/// Whether the graphics VT-d domain actually contains the iGPU right now.
pub fn domain_armed() -> bool {
    GFX_DOMAIN_ARMED.load(Ordering::Acquire)
}

/// Whether the graphics arena region has been reserved (diagnostic companion
/// to [`containment_armed`], so a refusal breadcrumb can name which of the
/// three preconditions is missing).
pub fn arena_reserved() -> bool {
    bounds().is_some()
}

/// **The I-14(c) gate.** True only when a GPU-initiated DMA is provably
/// contained: the arena is reserved, narrowing policy is on, *and* the VT-d
/// domain actually holds the requester. Any GPU DMA or bus-mastering
/// activation must refuse while this is false — an accelerator that can master
/// the bus with unrestricted physical addresses is exactly what I-14(c)
/// forbids ("a driver's DMA must not reach governance or tensor memory").
pub fn containment_armed() -> bool {
    narrowing_enabled() && bounds().is_some() && domain_armed()
}

/// Record that a real bus-mastering accelerator is bound. Once set, the
/// GPU-reachable allocation chokepoint refuses to hand out unfenced buddy
/// memory (see `DmaBuffer::alloc_gpu`).
#[allow(dead_code)] // set by the hardware probe path
pub fn mark_hardware_accelerator_bound() {
    GFX_HW_ACCEL_BOUND.store(true, Ordering::Release);
}

/// Whether a real bus-mastering accelerator is bound.
pub fn hardware_accelerator_bound() -> bool {
    GFX_HW_ACCEL_BOUND.load(Ordering::Acquire)
}

/// Default graphics working-set arena, in pages, for a boot that carries a
/// real Intel accelerator. Matches the silicon scenario's own reservation so
/// arming at boot and arming in-scenario cover the same working set.
#[cfg(all(feature = "accel-intel", feature = "vtd"))]
const BOOT_GFX_ARENA_PAGES: u64 = 256;

/// Arm iGPU DMA containment during boot, **before** the accel probe runs
/// (Slice-4 AAG bounce, 2026-08-01).
///
/// I-14(c) requires that a bus-mastering accelerator's DMA be contained. That
/// containment has to exist before the device is granted bus-master rights,
/// so this runs from `_kernel_start` between `iommu::init()` and `ai::init()`
/// (which reaches `accel::init()` → `accel_intel::probe_pci`).
///
/// Idempotent and fail-safe: if the arena is already reserved it reuses it,
/// and every failure leaves containment disarmed, which makes the probe refuse
/// bus mastering and leave the iGPU decode-only. It never panics and never
/// blocks boot — I-14(b): a containment failure degrades the GPU to unusable,
/// it does not stall governance.
#[cfg(all(feature = "accel-intel", feature = "vtd"))]
pub fn arm_boot_containment() {
    if !crate::drivers::iommu::available() || !crate::drivers::iommu::enabled() {
        crate::serial_println!(
            "[ kernel ] graphics: VT-d unavailable — iGPU DMA cannot be contained; \
             accelerator will be refused bus mastering (I-14(c) fail-closed)"
        );
        return;
    }
    if bounds().is_none() && init(BOOT_GFX_ARENA_PAGES).is_err() {
        crate::serial_println!(
            "[ kernel ] graphics: arena reservation failed — iGPU stays uncontained and \
             will be refused bus mastering (I-14(c) fail-closed)"
        );
        return;
    }
    if !set_narrowing(true) {
        crate::serial_println!("[ kernel ] graphics: narrowing enable refused — iGPU uncontained");
        return;
    }
    let narrowed = crate::drivers::iommu::narrow_graphics();
    if narrowed == 0 {
        // Leave the policy bit off so `containment_armed()` cannot report a
        // containment we do not actually have.
        set_narrowing(false);
        crate::serial_println!(
            "[ kernel ] graphics: no graphics requester narrowed — iGPU uncontained, \
             bus mastering will be refused (I-14(c) fail-closed)"
        );
        return;
    }
    crate::serial_println!(
        "[ kernel ] graphics: iGPU DMA containment armed (requesters={} arena_pages={}) — I-14(c)",
        narrowed,
        BOOT_GFX_ARENA_PAGES
    );
}

/// Reserve the graphics arena: one contiguous buddy block, registered once.
/// Idempotent-refusing (a second init is an error — the fence is a boot-time
/// fact, not a movable target), matching `dma_arena::init`.
#[allow(dead_code)] // boot/scenario-consumed
pub fn init(pages: u64) -> Result<(), DmaFault> {
    if pages == 0 {
        return Err(DmaFault::ZeroLength);
    }
    if GFX_ARENA_LEN.load(Ordering::Acquire) != 0 {
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
    *GFX_SPANS.lock() = Some(SpanTable::new(covered));
    GFX_ARENA_BASE.store(base, Ordering::Release);
    GFX_ARENA_LEN.store(len, Ordering::Release);
    Ok(())
}

/// The graphics-arena bounds, if initialized — `(base, len)`. This is exactly
/// the physical range the VT-d graphics domain maps.
#[allow(dead_code)] // consumed by iommu (vtd) + set_narrowing + scenario
pub fn bounds() -> Option<(u64, u64)> {
    let len = GFX_ARENA_LEN.load(Ordering::Acquire);
    if len == 0 {
        return None;
    }
    Some((GFX_ARENA_BASE.load(Ordering::Acquire), len))
}

/// THE GATE: validate `[phys, phys+len)` against the graphics fence and mint a
/// witness (the same total arithmetic as `dma_arena::fence`; refusals counted;
/// an uninitialized arena refuses everything — fails closed). Used by the
/// scenario to prove an in-arena address admits and an out-of-arena address
/// refuses at the software level (the domain's page table enforces the same at
/// the hardware level).
#[allow(dead_code)] // scenario-consumed (the software-gate proof)
pub fn fence(phys: u64, len: u64) -> Result<GfxFencedAddr, DmaFault> {
    let base = GFX_ARENA_BASE.load(Ordering::Acquire);
    let arena_len = GFX_ARENA_LEN.load(Ordering::Acquire);
    match validate(phys, len, base, arena_len) {
        Ok(()) => {
            GFX_ADMITS.fetch_add(1, Ordering::Relaxed);
            Ok(GfxFencedAddr { phys, len })
        }
        Err(fault) => {
            GFX_REFUSALS.fetch_add(1, Ordering::Relaxed);
            Err(fault)
        }
    }
}

/// A validated graphics-arena DMA address (witness type, private fields, one
/// constructor — the [`fence`] gate), mirroring `dma_arena::FencedDmaAddr`.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct GfxFencedAddr {
    phys: u64,
    len: u64,
}

#[allow(dead_code)] // witness accessors — gate-consumed (mirrors FencedDmaAddr)
impl GfxFencedAddr {
    pub fn phys(&self) -> u64 {
        self.phys
    }
    pub fn len(&self) -> u64 {
        self.len
    }
}

/// Allocate a page-span from the graphics arena: the physical address of the
/// span's first page, or `None` (unset arena / exhausted). Used by
/// `DmaBuffer::alloc_in_graphics_arena`; the matching [`release_span`] runs on
/// Drop.
pub(crate) fn alloc_span(pages: u64) -> Option<PhysAddr> {
    let (base, _) = bounds()?;
    let mut spans = GFX_SPANS.lock();
    let table = spans.as_mut()?;
    let offset_pages = table.alloc(pages)?;
    let phys = offset_pages
        .checked_mul(PAGE_SIZE)
        .and_then(|off| base.checked_add(off))?;
    Some(PhysAddr::new(phys))
}

/// Return a span to the graphics arena (never to the buddy — the region stays
/// reserved). `false` on a forged/double free (refused, counted by the leaked
/// gauge).
pub(crate) fn release_span(phys: PhysAddr) -> bool {
    let Some((base, len)) = bounds() else {
        return false;
    };
    let raw = phys.as_u64();
    let Some(fence_end) = base.checked_add(len) else {
        GFX_LEAKED_SPANS.fetch_add(1, Ordering::Relaxed);
        return false;
    };
    if raw < base || raw >= fence_end {
        GFX_LEAKED_SPANS.fetch_add(1, Ordering::Relaxed);
        return false;
    }
    let offset_pages = (raw - base) / PAGE_SIZE;
    let mut spans = GFX_SPANS.lock();
    match spans.as_mut() {
        Some(table) => table.free(offset_pages),
        None => false,
    }
}

// ── discriminator accessors (Rule 5: gate-consumed, never dead) ───────────────
#[allow(dead_code)]
pub fn admits() -> u64 {
    GFX_ADMITS.load(Ordering::Relaxed)
}
#[allow(dead_code)]
pub fn refusals() -> u64 {
    GFX_REFUSALS.load(Ordering::Relaxed)
}
#[allow(dead_code)]
pub fn leaked_spans() -> u64 {
    GFX_LEAKED_SPANS.load(Ordering::Relaxed)
}
#[allow(dead_code)]
pub fn used_pages() -> u64 {
    GFX_SPANS
        .lock()
        .as_ref()
        .map(|t| t.used_pages())
        .unwrap_or(0)
}
#[allow(dead_code)]
pub fn reset_counters() {
    GFX_ADMITS.store(0, Ordering::Relaxed);
    GFX_REFUSALS.store(0, Ordering::Relaxed);
}
