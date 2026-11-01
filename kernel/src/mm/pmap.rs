// SPDX-License-Identifier: MPL-2.0
// Phase 1.4 BUILD-A: this entire module is built but unused until BUILD-B's
// `process::spawn_user` ELF-loads neo-init. Allowing dead_code blanket so
// the BUILD-A boot is warning-clean for human review.
#![allow(dead_code)]

//! Per-process page table (Pmap).
//!
//! Each user `Process` owns a `Pmap`: a fresh PML4 frame whose UPPER-128
//! entries are copied from the kernel's PML4 (so kernel pages — heap,
//! .text, IST stacks, MMIO — are visible in every process's address space)
//! and whose LOWER-256 entries map the user binary, user stack, and
//! eventually mmap regions.
//!
//! "PML4-entry copy with shared lower tables", NOT a deep snapshot — the
//! PDPT/PD/PT frames the kernel's PML4 entries point at are shared with
//! every user Pmap. A kernel modification to those tables (e.g., heap
//! growth) would be visible in all user Pmaps. Phase 1.4's heap is fixed
//! at 4 MiB so growth doesn't happen; ISC-45a debug-asserts the upper-128
//! is stable across Pmap construction.

use alloc::vec::Vec;
use core::sync::atomic::{Ordering, fence};

#[cfg(feature = "atrium-compositor-g1")]
use spin::Mutex;

use crate::sync::irqsafe_mutex::{IrqSafeGuard, IrqSafeMutex};
use x86_64::registers::control::Cr3Flags;
use x86_64::structures::paging::{
    FrameAllocator, PageTable, PageTableFlags, PhysFrame, Size4KiB, page_table::PageTableEntry,
};
use x86_64::{PhysAddr, VirtAddr};

#[cfg(feature = "mem-quota")]
use super::mem_account_logic::{ChargeLedger, MemAccount};
use super::{kernel_pml4_phys, phys_to_virt, with_frame_alloc};

/// Extract the four 9-bit page-table indices (PML4, PDPT, PD, PT) from a
/// virtual address. Pure function; used by `map_user_locked` (mutating
/// walker) and `translate` (read-only walker — see ADR-006 §Q4 for the
/// translate-must-stay-lock-free invariant). Lifting this avoids
/// duplicating the bit-shift sequence and keeps the two walkers
/// trivially auditable for "do they walk the same thing".
#[inline]
fn page_indices(va: VirtAddr) -> [u64; 4] {
    [
        (va.as_u64() >> 39) & 0x1FF, // PML4
        (va.as_u64() >> 30) & 0x1FF, // PDPT
        (va.as_u64() >> 21) & 0x1FF, // PD
        (va.as_u64() >> 12) & 0x1FF, // PT
    ]
}

/// W^X enforcement at the API boundary, per Bible Article 12.3. Used by
/// every public Pmap function that installs a leaf mapping (`map_user`,
/// `allocate_user_region`). Refuses leaf flags that are simultaneously
/// WRITABLE and not NO_EXECUTE.
#[inline]
fn check_user_flags(flags: PageTableFlags) -> Result<(), PmapError> {
    if flags.contains(PageTableFlags::WRITABLE) && !flags.contains(PageTableFlags::NO_EXECUTE) {
        return Err(PmapError::WriteAndExecute);
    }
    Ok(())
}

/// OT-61 C1.0 (AAG must-fix 2): page-round `size` with checked arithmetic — an
/// overflowing `size` is a hard error, never a wrapping add. Returns the page
/// count. (The syscall layer also bounds `size ≤ MAX_MAP_REGION_SIZE`; this
/// keeps the `pub` Pmap methods total regardless of caller.)
#[inline]
fn pages_for_size(size: usize) -> Result<u64, PmapError> {
    let bytes = size
        .checked_add(0xFFF)
        .map(|b| b & !0xFFF)
        .ok_or(PmapError::UserHalfOverlapsKernel)?;
    Ok((bytes / 0x1000) as u64)
}

/// OT-61 C1.0 (AAG must-fix 2): the VA of page `i` above `base`, computed with
/// checked-add + `VirtAddr::try_new`. A range that would cross the canonical
/// boundary (e.g. the top user page + 1) is `UserHalfOverlapsKernel`, **never**
/// a `VirtAddr::new` panic (the kernel is `panic = "abort"`).
#[inline]
fn checked_page_va(base: VirtAddr, i: u64) -> Result<VirtAddr, PmapError> {
    i.checked_mul(0x1000)
        .and_then(|off| base.as_u64().checked_add(off))
        .and_then(|a| VirtAddr::try_new(a).ok())
        .ok_or(PmapError::UserHalfOverlapsKernel)
}

/// Base of the per-process anonymous-mapping cursor used by
/// `allocate_user_region` (driven from `sys_map_region`). Sits well clear
/// of binary segments (typically `0x400000`) and the user stack (top at
/// `0x0000_7fff_ffff_f000`, growing down). Per ADR-006 §Q2.
pub const USER_HEAP_BASE: u64 = 0x0000_4000_0000_0000;
/// Hard ceiling for the cursor — `allocate_user_region` returns
/// `OutOfFrames` (the closest existing variant) if a request would push
/// the cursor past this. Leaves ~64 TiB of address-space room above
/// `USER_HEAP_BASE` and a comfortable gap before the user stack.
pub const USER_HEAP_LIMIT: u64 = 0x0000_7000_0000_0000;

/// Convenience flag bundles for user-space mappings. Callers compose these
/// per-segment in the ELF loader (e.g., R+X = FLAGS_USER_CODE; R+W = FLAGS_USER_DATA_RW).
pub mod flags {
    use x86_64::structures::paging::PageTableFlags as F;
    pub const USER_CODE: F = F::from_bits_truncate(F::PRESENT.bits() | F::USER_ACCESSIBLE.bits());
    pub const USER_DATA_RW: F = F::from_bits_truncate(
        F::PRESENT.bits() | F::WRITABLE.bits() | F::USER_ACCESSIBLE.bits() | F::NO_EXECUTE.bits(),
    );
    pub const USER_DATA_RO: F =
        F::from_bits_truncate(F::PRESENT.bits() | F::USER_ACCESSIBLE.bits() | F::NO_EXECUTE.bits());
}

/// Number of PML4 entries reserved for the kernel half (indexes 256..512).
const KERNEL_PML4_ENTRIES: usize = 256;
const PAGE_SIZE: u64 = 4096;

/// OS-available PTE bit marking a leaf whose frame is BORROWED — owned by a
/// kernel object (an atrium shared pixel buffer), not by this Pmap. The
/// `Drop` walker removes the translation but must NOT free the frame; the
/// owner frees it when its own `Arc` (pinned in `borrowed_pins`) drops.
#[cfg(feature = "atrium-compositor-g1")]
pub const BORROWED_LEAF: PageTableFlags = PageTableFlags::BIT_9;

/// OT-61 C2.2b: the most borrowed mappings one Pmap holds (each keeps a pin
/// until the Pmap drops). A desktop session maps a few dozen buffers.
#[cfg(all(
    feature = "atrium-compositor-g1",
    not(feature = "neo-test-desktop-home-cycle")
))]
pub const MAX_BORROWED_PINS: usize = 1024;
/// `desktop_home_cycle` (Chad #177 item 4): a cap its repeated Arthur rounds
/// pass within a few launches, unless each round's pin is released.
#[cfg(all(
    feature = "atrium-compositor-g1",
    feature = "neo-test-desktop-home-cycle"
))]
pub const MAX_BORROWED_PINS: usize = 8;

/// One borrowed mapping's pin: the owning object's `Arc`.
#[cfg(all(feature = "atrium-compositor-g1", not(feature = "obj-quota")))]
pub type BorrowedPin = alloc::sync::Arc<dyn core::any::Any + Send + Sync>;
/// One borrowed mapping's pin: the owning object's `Arc`, and the object
/// charge for the entry (`obj_weights::PIN`), released when it drops.
#[cfg(all(feature = "atrium-compositor-g1", feature = "obj-quota"))]
pub type BorrowedPin = (
    alloc::sync::Arc<dyn core::any::Any + Send + Sync>,
    super::mem_account_logic::ObjCharge,
);

/// One borrowed mapping: where it starts, how many pages, and its pin. The
/// base keys `Pmap::unmap_borrowed_region` (Chad #177 item 4: a long-lived
/// compositor maps a new client buffer per session and must be able to let
/// the old one go, or it reaches [`MAX_BORROWED_PINS`]).
#[cfg(feature = "atrium-compositor-g1")]
struct BorrowedMapping {
    base: u64,
    pages: u64,
    _pin: BorrowedPin,
}

/// Snapshot of the kernel's upper-128 PML4 entries, captured at first Pmap
/// construction. Subsequent Pmaps debug-assert the kernel half hasn't
/// drifted (per ISC-45a). Wrapped in `Once` so we have no `static mut`;
/// `Once::call_once` provides the single-write discipline AND is SMP-safe
/// (Phase 2 doesn't need to migrate this).
static KERNEL_PML4_SNAPSHOT: spin::Once<[u64; KERNEL_PML4_ENTRIES]> = spin::Once::new();

pub struct Pmap {
    /// Physical frame holding this process's top-level PML4 page table.
    /// Read-only after `Pmap::new` returns; the page table contents the
    /// frame points to ARE mutated, but only via `&self` methods that
    /// take `inner` for serialization.
    pml4_frame: PhysFrame<Size4KiB>,
    /// Interior-mutable state. Per ADR-006 §Q4: the IPC `translate(&self)`
    /// hot path MUST NOT acquire this lock — translate is a pure
    /// HHDM-mapped read of page-table entries, indistinguishable from
    /// any other shared read. Mutation paths (`map_user`,
    /// `allocate_user_region`) take the lock and serialize page-table
    /// writes against each other.
    inner: IrqSafeMutex<PmapState>,
    /// Lifetime pins for borrowed-leaf mappings (`map_borrowed_region`,
    /// atrium G1): each mapping holds the owning object's `Arc` here so
    /// the object cannot free frames this Pmap still maps. Declared as a
    /// struct field so Rust's drop order gives the required sequencing
    /// for free: `Drop::drop` (removes PTEs + TLB shootdown) runs BEFORE
    /// fields drop (releasing the pins) — a pinned frame is never
    /// released back to its owner while a PTE can still reach it.
    ///
    /// OT-61 C2.2b: at most [`MAX_BORROWED_PINS`]; under `obj-quota` each
    /// entry also holds its object charge, released when the entry drops.
    #[cfg(feature = "atrium-compositor-g1")]
    borrowed_pins: Mutex<Vec<BorrowedMapping>>,
    /// OT-61 C2.2 (I-18): the frames this Pmap owns, charged to its process's
    /// memory account: the PML4, every intermediate table, and every leaf it
    /// allocated or was handed by `alloc_user_frame`. Borrowed leaves are not
    /// its frames and are never charged. `None` for a kernel-built Pmap (test
    /// fixtures), which is not accounted.
    #[cfg(feature = "mem-quota")]
    ledger: Option<ChargeLedger>,
}

struct PmapState {
    /// Cursor for `allocate_user_region` (mmap-style anonymous regions).
    /// Monotonic; never resets within a process's lifetime. Page-aligned.
    next_mmap_va: u64,
}

/// OT-61 C1.1 hang discriminator: identify who holds `Pmap::inner` when a
/// waiter cannot get it, and turn the resulting silent hang into a loud,
/// localised panic.
///
/// # Why this exists
///
/// The `threads_futex` gate hang was captured (4/4 locally at `NEO_SMP=4`) via
/// the QEMU monitor: the BSP sits in `_mm_pause` forever, called from
/// `spin::Mutex<PmapState>::lock` under
/// `Pmap::allocate_user_region_guarded` ← `thread_create_for_current`, **with
/// `IF=0`** (`RFL=0x02`). Two consequences shaped this design:
///
///  1. No kernel-side breadcrumb can report it. The BSP takes no timer tick
///     (this kernel's timer is PIC IRQ0, BSP-only, and IF is clear), so the
///     `wake_trace` watchdog — and anything else on the tick — can never run.
///     Only a diagnostic *at the spin site itself* can speak.
///  2. The monitor proves no *live* CPU holds the lock (every AP is idling in
///     `run_ap_bootstrap`), so the holder is a thread that is not running: one
///     that parked while holding the guard, or one whose guard was leaked when
///     its stack was freed raw (the CP-30 mechanism — a force-zombified thread
///     never drops a guard living on that stack, and this lock is then
///     held forever by nobody).
///
/// # Why recording only the *acquire* is sufficient
///
/// There is no release hook: the walkers (`map_user_locked` and friends) take
/// `&mut IrqSafeGuard<'_, PmapState>` **by name** as a type-level proof that the
/// caller holds the lock, so wrapping the guard in a `Drop` newtype would break
/// that contract. It does not matter. While the lock is held, no further
/// acquire can succeed — therefore **the last successful acquire IS the current
/// holder**, exactly when a waiter is stuck. The record is only consulted from
/// the stuck path.
/// Which `Pmap::inner` acquire site — reported by name so a deadlock panic
/// points at a function rather than a line number that drifts. Always
/// compiled (it is only a label) so the call sites need no `cfg`.
#[derive(Copy, Clone)]
pub(crate) enum PmapLockSite {
    MapUser,
    UnmapUser,
    AllocateUserRegion,
    AllocateUserRegionGuarded,
    UnmapUserRange,
    ProtectUser,
    MapBorrowedRegion,
    UnmapBorrowedRegion,
    #[cfg(feature = "boot-time")]
    WriteUserBytes,
    WithUserFrame,
}

impl PmapLockSite {
    /// Only the deadlock reporter reads this back.
    #[cfg_attr(not(feature = "neo-test"), allow(dead_code))]
    fn name(self) -> &'static str {
        match self {
            PmapLockSite::MapUser => "map_user",
            PmapLockSite::UnmapUser => "unmap_user",
            PmapLockSite::AllocateUserRegion => "allocate_user_region",
            PmapLockSite::AllocateUserRegionGuarded => "allocate_user_region_guarded",
            PmapLockSite::UnmapUserRange => "unmap_user_range",
            PmapLockSite::ProtectUser => "protect_user",
            PmapLockSite::MapBorrowedRegion => "map_borrowed_region",
            PmapLockSite::UnmapBorrowedRegion => "unmap_borrowed_region",
            #[cfg(feature = "boot-time")]
            PmapLockSite::WriteUserBytes => "write_user_bytes",
            PmapLockSite::WithUserFrame => "with_user_frame",
        }
    }
}

#[cfg(feature = "neo-test")]
mod lock_watch {
    use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

    use super::PmapLockSite as Site;

    /// Last successful acquirer of *some* `Pmap::inner`. Global rather than
    /// per-Pmap so the record costs nothing per address space; the deadlock
    /// under investigation involves a single live user Pmap, and the panic
    /// prints the `Pmap` pointer on both sides so a cross-Pmap case is still
    /// distinguishable.
    static OWNER_CPU: AtomicU32 = AtomicU32::new(u32::MAX);
    static OWNER_TID: AtomicU64 = AtomicU64::new(u64::MAX);
    static OWNER_SITE: AtomicU32 = AtomicU32::new(u32::MAX);
    static OWNER_PMAP: AtomicU64 = AtomicU64::new(0);
    static ACQUIRES: AtomicU64 = AtomicU64::new(0);

    pub fn record_acquire(site: Site, pmap: *const super::Pmap) {
        OWNER_CPU.store(
            crate::arch::x86_64::per_cpu::current_cpu_id(),
            Ordering::Relaxed,
        );
        OWNER_TID.store(
            crate::arch::x86_64::per_cpu::current_thread_id(),
            Ordering::Relaxed,
        );
        OWNER_SITE.store(site as u32, Ordering::Relaxed);
        OWNER_PMAP.store(pmap as u64, Ordering::Relaxed);
        ACQUIRES.fetch_add(1, Ordering::Relaxed);
    }

    fn site_name(raw: u32) -> &'static str {
        match raw {
            0 => "map_user",
            1 => "unmap_user",
            2 => "allocate_user_region",
            3 => "allocate_user_region_guarded",
            4 => "unmap_user_range",
            5 => "protect_user",
            6 => "map_borrowed_region",
            _ => "<none>",
        }
    }

    /// Report the stuck acquire and panic. Split out (and `#[inline(never)]`)
    /// so the hot acquire path stays a plain `try_lock` loop.
    ///
    /// Panicking is deliberate: it converts a 180 s silent timeout — which
    /// destroys its own evidence on a retry — into an immediate, attributable
    /// failure naming both sides of the deadlock. Safe from `IF=0`: the panic
    /// path needs only the serial spin lock, never the timer.
    #[inline(never)]
    pub fn report_stuck_and_panic(waiter_site: Site, pmap: *const super::Pmap, spins: u64) -> ! {
        let owner_cpu = OWNER_CPU.load(Ordering::Relaxed);
        let owner_tid = OWNER_TID.load(Ordering::Relaxed);
        let owner_site = site_name(OWNER_SITE.load(Ordering::Relaxed));
        let owner_pmap = OWNER_PMAP.load(Ordering::Relaxed);
        let waiter_cpu = crate::arch::x86_64::per_cpu::current_cpu_id();
        let waiter_tid = crate::arch::x86_64::per_cpu::current_thread_id();
        // The discriminator, stated in the log so the verdict is not left to
        // the reader: same thread ⇒ re-entrant acquire; different thread that
        // is not running ⇒ a parked or dead holder (leaked guard).
        let verdict = if owner_cpu == waiter_cpu && owner_tid == waiter_tid {
            "SAME THREAD => re-entrant acquire of a non-reentrant spin mutex"
        } else if owner_cpu == waiter_cpu {
            "SAME CPU, different thread => holder parked/was killed while holding the guard"
        } else {
            "DIFFERENT CPU => holder is off-CPU (parked or force-zombified) or a leaked guard"
        };
        panic!(
            "pmap lock deadlock: waiter cpu={} tid={} site={} pmap={:#x} spun={} | \
             holder cpu={} tid={} site={} pmap={:#x} acquires={} | {}",
            waiter_cpu,
            waiter_tid,
            waiter_site.name(),
            pmap as u64,
            spins,
            owner_cpu,
            owner_tid,
            owner_site,
            owner_pmap,
            ACQUIRES.load(Ordering::Relaxed),
            verdict,
        );
    }
}

#[derive(Debug)]
pub enum PmapError {
    OutOfFrames,
    UserHalfOverlapsKernel,
    /// W^X violation: caller passed PageTableFlags with both WRITABLE and
    /// (no NO_EXECUTE) set. Bible Article 12.3 mandates W^X from Phase 1;
    /// this is the mechanical enforcement at the API boundary.
    WriteAndExecute,
    /// OT-61 C1.0: `protect`/`unmap` targeted a page with no leaf mapping
    /// (e.g. a guard page, or an out-of-range VA) — the operation is refused
    /// rather than silently creating a mapping.
    NotMapped,
    /// OT-61 C2.2: the frame would take the process's memory account (or one
    /// above it) over its limit. Nothing was allocated.
    #[cfg(feature = "mem-quota")]
    QuotaExceeded,
    /// OT-61 C2.2b: the Pmap already holds [`MAX_BORROWED_PINS`] borrowed
    /// mappings. Nothing was mapped.
    #[cfg(feature = "atrium-compositor-g1")]
    TooManyPins,
}

impl Pmap {
    /// Allocate a fresh PML4 frame, zero its lower 256 entries, copy the
    /// kernel's upper 256 entries. Returns the new Pmap ready for
    /// `map_user`.
    pub fn new() -> Result<Self, PmapError> {
        let pml4_frame =
            with_frame_alloc(|fa| fa.allocate_frame()).ok_or(PmapError::OutOfFrames)?;

        // Snapshot the kernel half on first call so subsequent Pmaps can
        // assert it's stable. (Kernel-half mutation would invalidate every
        // existing user Pmap silently.)
        let kernel_pml4_virt = phys_to_virt(kernel_pml4_phys());
        // SAFETY: HHDM-mapped, kernel-owned, read-only access here.
        let kernel_pml4 = unsafe { &*(kernel_pml4_virt.as_ptr::<PageTable>()) };

        let snapshot = KERNEL_PML4_SNAPSHOT.call_once(|| {
            let mut arr = [0u64; KERNEL_PML4_ENTRIES];
            for i in 0..KERNEL_PML4_ENTRIES {
                arr[i] = kernel_pml4[i + KERNEL_PML4_ENTRIES].flags().bits()
                    | kernel_pml4[i + KERNEL_PML4_ENTRIES].addr().as_u64();
            }
            arr
        });

        // Debug-assert: kernel half hasn't drifted since the snapshot.
        #[cfg(debug_assertions)]
        for i in 0..KERNEL_PML4_ENTRIES {
            let live = kernel_pml4[i + KERNEL_PML4_ENTRIES].flags().bits()
                | kernel_pml4[i + KERNEL_PML4_ENTRIES].addr().as_u64();
            debug_assert!(
                live == snapshot[i],
                "kernel PML4[{}] drifted since first Pmap snapshot — Phase 1.4 assumed fixed kernel half",
                i + KERNEL_PML4_ENTRIES
            );
        }

        // Initialize the new PML4 in HHDM. Lower half zeroed (no user
        // mappings yet); upper half = copy of kernel's PML4 entries.
        let new_pml4_virt = phys_to_virt(pml4_frame.start_address());
        // SAFETY: pml4_frame was just allocated; we own exclusive access until
        // the first CR3 load. HHDM mapping makes the physical frame readable
        // and writable as a `PageTable`.
        let new_pml4 = unsafe { &mut *(new_pml4_virt.as_mut_ptr::<PageTable>()) };
        new_pml4.zero();
        for i in 0..KERNEL_PML4_ENTRIES {
            // Copy entry i+256 (kernel half) by value. The PDPT it points at
            // is SHARED with kernel and other Pmaps.
            new_pml4[i + KERNEL_PML4_ENTRIES] = kernel_pml4[i + KERNEL_PML4_ENTRIES].clone();
        }

        Ok(Self {
            pml4_frame,
            inner: IrqSafeMutex::new(PmapState {
                next_mmap_va: USER_HEAP_BASE,
            }),
            #[cfg(feature = "atrium-compositor-g1")]
            borrowed_pins: Mutex::new(Vec::new()),
            #[cfg(feature = "mem-quota")]
            ledger: None,
        })
    }

    /// OT-61 C2.2: a Pmap whose frames are charged to `account`. The PML4 is
    /// charged before it is allocated, like every later frame.
    #[cfg(feature = "mem-quota")]
    pub fn new_charged(account: alloc::sync::Arc<MemAccount>) -> Result<Self, PmapError> {
        let ledger = ChargeLedger::new(account);
        ledger.charge(1).map_err(|_| PmapError::QuotaExceeded)?;
        match Self::new() {
            Ok(mut pmap) => {
                pmap.ledger = Some(ledger);
                Ok(pmap)
            }
            Err(e) => {
                let _ = ledger.release(1);
                Err(e)
            }
        }
    }

    /// OT-61 C2.2: the memory account this Pmap's frames are charged to.
    #[cfg(feature = "mem-quota")]
    pub fn account(&self) -> Option<&alloc::sync::Arc<MemAccount>> {
        self.ledger.as_ref().map(ChargeLedger::account)
    }

    /// OT-61 C2.2: this Pmap's account, or one above it, has a limit.
    #[cfg(feature = "mem-quota")]
    pub fn mem_budgeted(&self) -> bool {
        self.account().is_some_and(|a| a.is_budgeted())
    }

    /// OT-61 C2.2: one frame for this Pmap, charged to its account before
    /// it is allocated (released again if the allocator is empty). The only
    /// way a frame later handed to `map_user` enters a charged Pmap.
    ///
    /// #143: the frame comes back unmapped, owned by the guard until
    /// [`UnmappedFrame::mapped`]: a map that fails (a page table the budget
    /// refuses) or any return before it frees the frame, then releases its
    /// charge. Only the page tables' walk (`Drop`) frees a mapped one.
    #[cfg(feature = "mem-quota")]
    pub fn alloc_user_frame(&self) -> Result<UnmappedFrame<'_>, PmapError> {
        if let Some(l) = &self.ledger {
            l.charge(1).map_err(|_| PmapError::QuotaExceeded)?;
        }
        // mem-quota: charged above.
        match with_frame_alloc(|fa| fa.allocate_frame()) {
            Some(frame) => Ok(UnmappedFrame {
                pmap: self,
                frame,
                armed: true,
            }),
            None => {
                self.release_frames(1);
                Err(PmapError::OutOfFrames)
            }
        }
    }

    /// OT-61 C2.2: release `n` frames this Pmap charged (freed or handed
    /// back). A release beyond what it holds is a bookkeeping bug: refused
    /// by the ledger, and loud under `neo-test`.
    #[cfg(feature = "mem-quota")]
    fn release_frames(&self, n: u64) {
        if let Some(l) = &self.ledger {
            let r = l.release(n);
            #[cfg(feature = "neo-test")]
            assert!(
                r.is_ok(),
                "mem-quota: Pmap released {n} frames it did not hold"
            );
            let _ = r;
        }
    }

    /// Install a leaf 4 KiB mapping in the user half. Caller composes
    /// `flags` exactly — no auto-USER_ACCESSIBLE or auto-NX. Use the
    /// `flags::USER_*` constants in this module for the common cases.
    ///
    /// **W^X is enforced mechanically** per Bible Article 12.3: a leaf
    /// flag set with both WRITABLE and (no NO_EXECUTE) returns
    /// `PmapError::WriteAndExecute`. The ELF loader already rejects
    /// PT_LOAD segments with `PF_W | PF_X`; this is the API-boundary
    /// backstop for any other caller (`allocate_user_region`, future shm).
    ///
    /// Returns Err if `va` is in the kernel half (>= 0xffff_8000_0000_0000).
    pub fn map_user(
        &self,
        va: VirtAddr,
        frame: PhysFrame<Size4KiB>,
        flags: PageTableFlags,
    ) -> Result<(), PmapError> {
        if va.as_u64() >= 0xffff_8000_0000_0000 {
            return Err(PmapError::UserHalfOverlapsKernel);
        }
        check_user_flags(flags)?;
        let mut g = self.lock_inner(PmapLockSite::MapUser);
        // OT-61 C2.2 (#143): in a charged Pmap every owned leaf is a frame
        // `alloc_user_frame` charged, so one this map replaces is freed (after
        // the shootdown) and only then its charge released.
        #[cfg(feature = "mem-quota")]
        let old = match self.ledger {
            Some(_) => self.owned_leaf_locked(&g, va).filter(|&old| old != frame),
            None => None,
        };
        let replaced = self.map_user_locked(&mut g, va, frame, flags)?;
        // Guard released BEFORE the shootdown (CP-37 — see
        // `pte_write_fence_and_shootdown`). Only a replaced leaf can leave a
        // stale translation on a peer.
        drop(g);
        if replaced {
            pte_write_fence_and_shootdown(Some(page_range(va)));
        }
        #[cfg(feature = "mem-quota")]
        if let Some(old) = old {
            with_frame_alloc(|fa| fa.free(old));
            self.release_frames(1);
        }
        Ok(())
    }

    /// OT-61 C2.2 (#143): the frame of the owned (not borrowed) leaf at `va`,
    /// if one is mapped.
    #[cfg(feature = "mem-quota")]
    fn owned_leaf_locked(
        &self,
        _g: &IrqSafeGuard<'_, PmapState>,
        va: VirtAddr,
    ) -> Option<PhysFrame<Size4KiB>> {
        let indices = page_indices(va);
        let mut current =
            unsafe { &*(phys_to_virt(self.pml4_frame.start_address()).as_ptr::<PageTable>()) };
        for &idx in indices.iter().take(3) {
            let entry = &current[idx as usize];
            if entry.is_unused() || entry.flags().contains(PageTableFlags::HUGE_PAGE) {
                return None;
            }
            current = unsafe { &*(phys_to_virt(entry.addr()).as_ptr::<PageTable>()) };
        }
        let leaf = &current[indices[3] as usize];
        #[cfg(feature = "atrium-compositor-g1")]
        if leaf.flags().contains(BORROWED_LEAF) {
            return None;
        }
        (!leaf.is_unused()).then(|| PhysFrame::containing_address(leaf.addr()))
    }

    /// Acquire `inner`, with deadlock attribution under `neo-test`.
    ///
    /// Behaviour-identical to `self.inner.lock()` on the happy path (a
    /// `try_lock` that succeeds first time), and byte-identical in a production
    /// build — the `neo-test`-off arm IS `self.inner.lock()`. The only
    /// difference is that a *stuck* acquire names its holder and panics instead
    /// of spinning silently until the harness times out. See [`lock_watch`].
    ///
    /// `SPIN_LIMIT` is far above any legitimate hold (a real critical section
    /// here is a bounded page-mapping loop — microseconds, thousands of spins at
    /// worst under TCG) and low enough that the panic lands well inside the
    /// scenario's 180 s budget rather than after it.
    #[cfg(feature = "neo-test")]
    fn lock_inner(&self, site: PmapLockSite) -> IrqSafeGuard<'_, PmapState> {
        const SPIN_LIMIT: u64 = 200_000_000;
        let mut spins: u64 = 0;
        loop {
            if let Some(g) = self.inner.try_lock() {
                lock_watch::record_acquire(site, self as *const Pmap);
                return g;
            }
            spins += 1;
            if spins >= SPIN_LIMIT {
                lock_watch::report_stuck_and_panic(site, self as *const Pmap, spins);
            }
            core::hint::spin_loop();
        }
    }

    #[cfg(not(feature = "neo-test"))]
    #[inline(always)]
    fn lock_inner(&self, _site: PmapLockSite) -> IrqSafeGuard<'_, PmapState> {
        self.inner.lock()
    }

    /// Page-table walker that installs a leaf mapping. The `&mut IrqSafeGuard`
    /// parameter type-encodes the "caller holds `self.inner.lock()`" contract
    /// without `unsafe`: an `IrqSafeGuard<PmapState>` cannot be constructed
    /// without taking the lock, so the type system proves the precondition.
    /// Factored out so `allocate_user_region` can call it in a loop under
    /// one lock acquisition (re-acquiring per page would self-deadlock the
    /// spin mutex).
    fn map_user_locked(
        &self,
        _g: &mut IrqSafeGuard<'_, PmapState>,
        va: VirtAddr,
        frame: PhysFrame<Size4KiB>,
        flags: PageTableFlags,
    ) -> Result<bool, PmapError> {
        // Walk the four levels, allocating intermediate page-table frames
        // as needed. Intermediate flags are PRESENT|WRITABLE|USER; leaf
        // flags determine the actual permission via flag-intersection
        // during page walks (Intel SDM Vol 3 §4.6.1). NX on intermediate
        // tables is redundant — the leaf's NX bit alone is honored when
        // any walker-level entry has it set.
        let indices = page_indices(va);

        let mut current = unsafe {
            &mut *(phys_to_virt(self.pml4_frame.start_address()).as_mut_ptr::<PageTable>())
        };

        // Descend three intermediate levels (PML4 → PDPT → PD); the fourth
        // (PT leaf) is set after the loop.
        for &idx in indices.iter().take(3) {
            let idx = idx as usize;
            if current[idx].is_unused() {
                #[cfg(not(feature = "mem-quota"))]
                let new_frame =
                    with_frame_alloc(|fa| fa.allocate_frame()).ok_or(PmapError::OutOfFrames)?;
                #[cfg(feature = "mem-quota")]
                let table = self.alloc_user_frame()?;
                #[cfg(feature = "mem-quota")]
                let new_frame = table.frame();
                unsafe {
                    let virt = phys_to_virt(new_frame.start_address());
                    (&mut *virt.as_mut_ptr::<PageTable>()).zero();
                }
                let mut entry = PageTableEntry::new();
                entry.set_addr(
                    new_frame.start_address(),
                    PageTableFlags::PRESENT
                        | PageTableFlags::WRITABLE
                        | PageTableFlags::USER_ACCESSIBLE,
                );
                current[idx] = entry;
                // Reachable from the PML4 now: `Drop`'s walk frees it.
                #[cfg(feature = "mem-quota")]
                table.mapped();
            }
            let next_phys = current[idx].addr();
            current = unsafe { &mut *(phys_to_virt(next_phys).as_mut_ptr::<PageTable>()) };
        }

        let leaf_idx = indices[3] as usize;
        let replacing_existing_leaf = !current[leaf_idx].is_unused();
        let mut entry = PageTableEntry::new();
        entry.set_addr(frame.start_address(), flags);
        current[leaf_idx] = entry;
        // The shootdown is the CALLER's job, AFTER it drops the pmap guard —
        // see `pte_write_fence_and_shootdown`. Returning "did this replace a
        // live leaf?" lets the caller shoot down only when a stale translation
        // can actually exist.
        Ok(replacing_existing_leaf)
    }

    /// Remove a user-half 4 KiB leaf mapping, returning the frame that was
    /// mapped there. Missing mappings are not an error.
    pub fn unmap_user(&self, va: VirtAddr) -> Result<Option<PhysFrame<Size4KiB>>, PmapError> {
        if va.as_u64() >= 0xffff_8000_0000_0000 {
            return Err(PmapError::UserHalfOverlapsKernel);
        }
        let mut g = self.lock_inner(PmapLockSite::UnmapUser);
        let (cleared, frame) = self.unmap_user_locked(&mut g, va)?;
        // Guard released BEFORE the shootdown (CP-37), but the shootdown still
        // happens BEFORE we hand `frame` back — the caller frees it, and a stale
        // translation must never survive into a reallocation of that frame.
        drop(g);
        if cleared {
            pte_write_fence_and_shootdown(Some(page_range(va)));
        }
        // OT-61 C2.2: the frame leaves this Pmap (its caller frees it), so
        // its charge goes with it. Only uncharged Pmaps (the test fixtures)
        // call this: a charged one's frames are freed before their release
        // (`unmap_user_range`, `Drop`), never handed out still allocated.
        #[cfg(feature = "mem-quota")]
        if frame.is_some() {
            #[cfg(feature = "neo-test")]
            assert!(
                self.ledger.is_none(),
                "mem-quota: unmap_user hands out a charged Pmap's frame"
            );
            self.release_frames(1);
        }
        Ok(frame)
    }

    /// Clear one leaf. Returns `(cleared, frame_to_free)`: `cleared` says a live
    /// PTE was actually removed, so the caller owes a shootdown; `frame_to_free`
    /// is `None` for an absent mapping *and* for a BORROWED leaf (whose frame
    /// belongs to a kernel object). The two are distinct — a borrowed leaf is
    /// cleared and needs the flush, but must not be freed.
    fn unmap_user_locked(
        &self,
        _g: &mut IrqSafeGuard<'_, PmapState>,
        va: VirtAddr,
    ) -> Result<(bool, Option<PhysFrame<Size4KiB>>), PmapError> {
        let indices = page_indices(va);
        let mut current = unsafe {
            &mut *(phys_to_virt(self.pml4_frame.start_address()).as_mut_ptr::<PageTable>())
        };

        for &idx in indices.iter().take(3) {
            let idx = idx as usize;
            if current[idx].is_unused() {
                return Ok((false, None));
            }
            if current[idx].flags().contains(PageTableFlags::HUGE_PAGE) {
                return Ok((false, None));
            }
            let next_phys = current[idx].addr();
            current = unsafe { &mut *(phys_to_virt(next_phys).as_mut_ptr::<PageTable>()) };
        }

        let leaf = &mut current[indices[3] as usize];
        if leaf.is_unused() {
            return Ok((false, None));
        }

        // OT-61 C1.0 (AAG proactive guard): a BORROWED leaf's frame is owned by a
        // kernel object (an atrium shared pixel buffer), not this Pmap — remove
        // the PTE but return `None` so the caller does NOT free it, exactly as the
        // `Drop` walker skips borrowed frames. Freeing it would double-free the
        // still-pinned frame. (Latent cross-feature UAF if `browser-runtime` and
        // `atrium-compositor-g1` ever co-enable; guarded now.)
        // `BORROWED_LEAF` (the bit) only exists — and is only ever *set* — when
        // `atrium-compositor-g1` is compiled; without that feature no leaf can
        // carry it, so the guard is a compile-time `false`.
        #[cfg(feature = "atrium-compositor-g1")]
        let borrowed = leaf.flags().contains(BORROWED_LEAF);
        #[cfg(not(feature = "atrium-compositor-g1"))]
        let borrowed = false;
        let frame = PhysFrame::containing_address(leaf.addr());
        leaf.set_unused();
        // The shootdown is the CALLER's job, after it drops the pmap guard —
        // and, critically, BEFORE it frees the returned frame (a stale
        // translation must not survive into a reallocation of that frame).
        // See `pte_write_fence_and_shootdown`.
        if borrowed {
            Ok((true, None))
        } else {
            Ok((true, Some(frame)))
        }
    }

    /// Allocate `size` bytes of fresh user-half address space at the
    /// per-Pmap mmap cursor, page-allocate frames for each page, install
    /// mappings with `flags`, advance the cursor. Returns the base VA.
    ///
    /// Per ADR-006 (Q1+Q2+Q4): the kernel-side primitive `sys_map_region`
    /// dispatches to this. Each call returns a fresh region that does not
    /// overlap any prior allocation in this Pmap. Cursor is monotonic; no
    /// `unmap` in Phase 1.5b — process exit's `Pmap::Drop` walker frees
    /// everything via the user-half walk.
    pub fn allocate_user_region(
        &self,
        size: usize,
        flags: PageTableFlags,
    ) -> Result<u64, PmapError> {
        debug_assert!(
            size > 0,
            "allocate_user_region: size must be > 0; sys_map_region pre-checks at the syscall layer"
        );
        check_user_flags(flags)?;

        let bytes = (size + 0xFFF) & !0xFFF;
        let pages = bytes / 0x1000;

        let mut g = self.lock_inner(PmapLockSite::AllocateUserRegion);
        let base = g.next_mmap_va;
        let new_cursor = base
            .checked_add(bytes as u64)
            .ok_or(PmapError::UserHalfOverlapsKernel)?;
        if new_cursor > USER_HEAP_LIMIT {
            return Err(PmapError::UserHalfOverlapsKernel);
        }
        g.next_mmap_va = new_cursor;
        // Lock held across the whole map loop — prevents another caller
        // from racing into the same cursor range on Phase 2 SMP. The
        // intermediate page-table allocations go through `with_frame_alloc`
        // which has its own lock; ordering is fine (pmap.inner before
        // frame_alloc).

        for i in 0..pages {
            let page_va = base + (i as u64) * 0x1000;
            #[cfg(not(feature = "mem-quota"))]
            let frame = with_frame_alloc(|fa| fa.allocate_frame()).ok_or(PmapError::OutOfFrames)?;
            #[cfg(feature = "mem-quota")]
            let page = self.alloc_user_frame()?;
            #[cfg(feature = "mem-quota")]
            let frame = page.frame();
            // Zero the frame so the user sees fresh memory.
            // SAFETY: HHDM-mapped, freshly-allocated, owned exclusively.
            unsafe {
                let dst = phys_to_virt(frame.start_address()).as_mut_ptr::<u8>();
                core::ptr::write_bytes(dst, 0, 0x1000);
            }
            // #143: a refused page table drops `page` unmapped (freed, then
            // released) on the way out.
            let replaced = self.map_user_locked(&mut g, VirtAddr::new(page_va), frame, flags)?;
            #[cfg(feature = "mem-quota")]
            page.mapped();
            // No shootdown is owed here, and that is a property rather than an
            // omission: `next_mmap_va` is monotonic and never revisits a VA, so
            // this region's pages cannot already be mapped and no peer can hold
            // a stale translation for them. (Pre-CP-37 the shootdown was inline
            // and conditional on exactly this flag, so it never fired here
            // either — behaviour is unchanged.)
            debug_assert!(
                !replaced,
                "allocate_user_region: monotonic cursor mapped an already-mapped VA"
            );
        }
        Ok(base)
    }

    /// OT-61 C1.0 (P5): allocate a region like [`allocate_user_region`], but
    /// leave one **unmapped guard page** immediately below the returned base
    /// when `guard` is true. A downward access past `base` (e.g. a thread
    /// stack overflow) faults into the guard — contained to the process. The
    /// monotonic cursor grows up, so the guard is reserved (cursor advanced
    /// without a mapping) *before* the region.
    pub fn allocate_user_region_guarded(
        &self,
        size: usize,
        flags: PageTableFlags,
        guard: bool,
    ) -> Result<u64, PmapError> {
        check_user_flags(flags)?;
        let bytes = (size + 0xFFF) & !0xFFF;
        let pages = bytes / 0x1000;

        let mut g = self.lock_inner(PmapLockSite::AllocateUserRegionGuarded);
        // Reserve the guard page below the region (unmapped: the cursor skips
        // it, so it stays not-present → a fault on access).
        let guard_pages: u64 = if guard { 1 } else { 0 };
        let base = g
            .next_mmap_va
            .checked_add(guard_pages * 0x1000)
            .ok_or(PmapError::UserHalfOverlapsKernel)?;
        let new_cursor = base
            .checked_add(bytes as u64)
            .ok_or(PmapError::UserHalfOverlapsKernel)?;
        if new_cursor > USER_HEAP_LIMIT {
            return Err(PmapError::UserHalfOverlapsKernel);
        }
        g.next_mmap_va = new_cursor;
        for i in 0..pages {
            let page_va = base + (i as u64) * 0x1000;
            #[cfg(not(feature = "mem-quota"))]
            let frame = with_frame_alloc(|fa| fa.allocate_frame()).ok_or(PmapError::OutOfFrames)?;
            #[cfg(feature = "mem-quota")]
            let page = self.alloc_user_frame()?;
            #[cfg(feature = "mem-quota")]
            let frame = page.frame();
            // SAFETY: HHDM-mapped, freshly-allocated, owned exclusively.
            unsafe {
                let dst = phys_to_virt(frame.start_address()).as_mut_ptr::<u8>();
                core::ptr::write_bytes(dst, 0, 0x1000);
            }
            // #143: a refused page table drops `page` unmapped (freed, then
            // released) on the way out.
            let replaced = self.map_user_locked(&mut g, VirtAddr::new(page_va), frame, flags)?;
            #[cfg(feature = "mem-quota")]
            page.mapped();
            // Same monotonic-cursor property as `allocate_user_region`: no
            // shootdown is owed because no page in a fresh region can already
            // be mapped. This is the thread-spawn path, so it is also the one
            // that must not pay for an unnecessary cross-CPU IPI round.
            debug_assert!(
                !replaced,
                "allocate_user_region_guarded: monotonic cursor mapped an already-mapped VA"
            );
        }
        Ok(base)
    }

    /// OT-61 C1.0 (P5): unmap `size` bytes starting at `va` (page-granular),
    /// returning each freed frame to the buddy allocator. Refuses a range that
    /// contains an unmapped page (`NotMapped`) so a partial/garbage range is a
    /// hard error, not a silent partial unmap. W^X is not relevant (unmapping
    /// removes access entirely).
    pub fn unmap_user_range(&self, va: VirtAddr, size: usize) -> Result<(), PmapError> {
        if va.as_u64() >= 0xffff_8000_0000_0000 {
            return Err(PmapError::UserHalfOverlapsKernel);
        }
        let pages = pages_for_size(size)?;
        // `user_copy_fault` test only: tell an IPC copy that is holding a
        // frame of this range open that the unmap is now waiting for it.
        #[cfg(feature = "neo-test-user-copy-fault")]
        crate::ipc::copy_race_test::note_unmap_waiting();
        let mut g = self.lock_inner(PmapLockSite::UnmapUserRange);
        // First pass: verify every page is mapped (fail-closed, no partial).
        // `checked_page_va` uses checked-add + try_new so a range crossing the
        // canonical boundary is `UserHalfOverlapsKernel`, never a VirtAddr panic
        // (AAG must-fix 2 — the kernel is panic=abort).
        for i in 0..pages {
            let page = checked_page_va(va, i)?;
            if self.translate_locked(&g, page).is_none() {
                return Err(PmapError::NotMapped);
            }
        }
        // CP-37: clear every PTE under the guard, but do NOT shoot down or free
        // anything while holding it. Frames are collected and freed only after
        // the flush, so a stale translation can never survive into a
        // reallocation of a freed frame — the ordering the old per-page inline
        // shootdown provided, preserved without the guard-held IPI wait.
        let mut to_free: Vec<PhysFrame<Size4KiB>> = Vec::new();
        let mut cleared_any = false;
        for i in 0..pages {
            let page = checked_page_va(va, i)?;
            // Skip freeing a BORROWED leaf (an atrium shared-buffer frame owned by
            // a kernel object, not this Pmap) — remove the PTE only, matching the
            // Drop walker; freeing it would double-free (AAG proactive guard).
            let (cleared, frame) = self.unmap_user_locked(&mut g, page)?;
            cleared_any |= cleared;
            if let Some(frame) = frame {
                to_free.push(frame);
            }
        }
        drop(g);

        // One shootdown for the whole range instead of one per page — the same
        // invalidation with a single cross-CPU round.
        if cleared_any {
            let first = checked_page_va(va, 0)?;
            let last = checked_page_va(va, pages.saturating_sub(1))?;
            pte_write_fence_and_shootdown(Some((page_range(first).0, page_range(last).1)));
        }

        #[cfg(feature = "mem-quota")]
        let freed = to_free.len() as u64;
        for frame in to_free {
            with_frame_alloc(|fa| fa.free(frame));
        }
        // OT-61 C2.2: released after the frees (never before a frame is back).
        #[cfg(feature = "mem-quota")]
        self.release_frames(freed);
        Ok(())
    }

    /// OT-61 C1.0 (P5): change the permissions of an existing mapping in place
    /// (keeping the frame), page-granular, over `[va, va+size)`. **W^X-checked**
    /// (`check_user_flags`) — a transition to WRITABLE+executable is refused
    /// (`WriteAndExecute`), preserving the A1 invariant. An unmapped page in the
    /// range is `NotMapped` (no new mapping is created). Each changed leaf gets
    /// a TLB fence + shootdown.
    pub fn protect_user(
        &self,
        va: VirtAddr,
        size: usize,
        flags: PageTableFlags,
    ) -> Result<(), PmapError> {
        if va.as_u64() >= 0xffff_8000_0000_0000 {
            return Err(PmapError::UserHalfOverlapsKernel);
        }
        check_user_flags(flags)?; // A1: no W+X, ever.
        let pages = pages_for_size(size)?;
        let mut g = self.lock_inner(PmapLockSite::ProtectUser);
        // Fail-closed: verify the whole range is mapped before changing any leaf.
        // `checked_page_va` avoids a canonical-boundary VirtAddr panic (must-fix 2).
        for i in 0..pages {
            let page = checked_page_va(va, i)?;
            if self.translate_locked(&g, page).is_none() {
                return Err(PmapError::NotMapped);
            }
        }
        for i in 0..pages {
            let page = checked_page_va(va, i)?;
            self.reflag_leaf_locked(&mut g, page, flags)?;
        }
        // CP-37: release the guard, THEN flush — but still before reporting
        // success, because a permission *downgrade* is only actually enforced
        // once peers have dropped their cached translations. One shootdown for
        // the whole range replaces the old per-leaf inline one.
        drop(g);
        if pages > 0 {
            let first = checked_page_va(va, 0)?;
            let last = checked_page_va(va, pages - 1)?;
            pte_write_fence_and_shootdown(Some((page_range(first).0, page_range(last).1)));
        }
        Ok(())
    }

    /// Rewrite an existing leaf's flags in place (same frame). Caller holds the
    /// lock and has pre-verified the leaf is present.
    fn reflag_leaf_locked(
        &self,
        _g: &mut IrqSafeGuard<'_, PmapState>,
        va: VirtAddr,
        flags: PageTableFlags,
    ) -> Result<(), PmapError> {
        let indices = page_indices(va);
        let mut current = unsafe {
            &mut *(phys_to_virt(self.pml4_frame.start_address()).as_mut_ptr::<PageTable>())
        };
        for &idx in indices.iter().take(3) {
            let idx = idx as usize;
            if current[idx].is_unused() {
                return Err(PmapError::NotMapped);
            }
            let next_phys = current[idx].addr();
            current = unsafe { &mut *(phys_to_virt(next_phys).as_mut_ptr::<PageTable>()) };
        }
        let leaf = &mut current[indices[3] as usize];
        if leaf.is_unused() {
            return Err(PmapError::NotMapped);
        }
        let frame_addr = leaf.addr();
        let mut entry = PageTableEntry::new();
        entry.set_addr(frame_addr, flags);
        *leaf = entry;
        // Shootdown deferred to the caller, outside the pmap guard — see
        // `pte_write_fence_and_shootdown`. A permission *downgrade* is only
        // effective once peers have flushed, so the caller must flush before it
        // reports success.
        Ok(())
    }

    /// Lock-held translate: is `va` mapped to a present leaf? (Used by
    /// `protect_user`/`unmap_user_range` to fail-closed on an unmapped page.)
    fn translate_locked(
        &self,
        _g: &IrqSafeGuard<'_, PmapState>,
        va: VirtAddr,
    ) -> Option<PhysFrame<Size4KiB>> {
        let indices = page_indices(va);
        let mut current =
            unsafe { &*(phys_to_virt(self.pml4_frame.start_address()).as_ptr::<PageTable>()) };
        for &idx in indices.iter().take(3) {
            let idx = idx as usize;
            if current[idx].is_unused() {
                return None;
            }
            let next_phys = current[idx].addr();
            current = unsafe { &*(phys_to_virt(next_phys).as_ptr::<PageTable>()) };
        }
        let leaf = &current[indices[3] as usize];
        if leaf.is_unused() {
            None
        } else {
            Some(PhysFrame::containing_address(leaf.addr()))
        }
    }

    /// L7.1: copy `src` into this address space's user memory at `va`
    /// without touching the user mapping from ring 0, so the copy cannot
    /// fault. Under the pmap lock (which every unmap/protect takes), each
    /// page of `[va, va + len)` must be a present 4 KiB leaf that is USER and
    /// WRITABLE at every level; otherwise nothing is written and the call
    /// returns `NotMapped`. The bytes then go through the HHDM to the backing
    /// frames while the lock is still held, so no other thread can unmap or
    /// downgrade a page between the check and the write. The caller has
    /// checked `[va, va + len)` lies in the user half.
    #[cfg(feature = "boot-time")]
    pub fn write_user_bytes(&self, va: u64, src: &[u8]) -> Result<(), PmapError> {
        let len = src.len() as u64;
        if len == 0 {
            return Ok(());
        }
        let end = va
            .checked_add(len)
            .ok_or(PmapError::UserHalfOverlapsKernel)?;
        let first = va & !0xFFF;
        let last = (end - 1) & !0xFFF;
        let g = self.lock_inner(PmapLockSite::WriteUserBytes);
        // Pass 1: every page writable by the user, or nothing is written.
        let mut page = first;
        loop {
            self.user_writable_frame_locked(&g, page)
                .ok_or(PmapError::NotMapped)?;
            if page == last {
                break;
            }
            page += 0x1000;
        }
        // Pass 2: the write, still under the lock.
        let mut done = 0usize;
        while done < src.len() {
            let at = va + done as u64;
            let frame = self
                .user_writable_frame_locked(&g, at & !0xFFF)
                .ok_or(PmapError::NotMapped)?;
            let off = (at & 0xFFF) as usize;
            let n = core::cmp::min(0x1000 - off, src.len() - done);
            // SAFETY: `frame` backs a present user leaf of this Pmap, which
            // can't be unmapped while `g` is held; the HHDM maps all of RAM
            // writable to the kernel, and `off + n <= 4096`.
            unsafe {
                let dst = phys_to_virt(frame.start_address())
                    .as_mut_ptr::<u8>()
                    .add(off);
                core::ptr::copy_nonoverlapping(src.as_ptr().add(done), dst, n);
            }
            done += n;
        }
        drop(g);
        Ok(())
    }

    /// Lock-held walk for [`Pmap::write_user_bytes`]: the 4 KiB frame behind
    /// `va` when every level is PRESENT + USER_ACCESSIBLE and the leaf is
    /// also WRITABLE; `None` for any unmapped level or a huge page.
    #[cfg(feature = "boot-time")]
    fn user_writable_frame_locked(
        &self,
        _g: &IrqSafeGuard<'_, PmapState>,
        va: u64,
    ) -> Option<PhysFrame<Size4KiB>> {
        if va >= 0x0000_8000_0000_0000 {
            return None;
        }
        let need = PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE;
        let indices = page_indices(VirtAddr::new(va));
        // SAFETY: read-only walk through HHDM-mapped tables, under the lock.
        let mut current =
            unsafe { &*(phys_to_virt(self.pml4_frame.start_address()).as_ptr::<PageTable>()) };
        for &idx in indices.iter().take(3) {
            let e = &current[idx as usize];
            let f = e.flags();
            if e.is_unused() || !f.contains(need) || f.contains(PageTableFlags::HUGE_PAGE) {
                return None;
            }
            current = unsafe { &*(phys_to_virt(e.addr()).as_ptr::<PageTable>()) };
        }
        let leaf = &current[indices[3] as usize];
        let f = leaf.flags();
        if leaf.is_unused() || !f.contains(need | PageTableFlags::WRITABLE) {
            return None;
        }
        Some(PhysFrame::containing_address(leaf.addr()))
    }

    /// Map caller-provided frames as **borrowed** leaves at the mmap
    /// cursor (atrium G1 shared pixel buffers). Differences from
    /// `allocate_user_region`:
    ///
    /// - the frames are OWNED BY `pin`'s object, not this Pmap: every leaf
    ///   PTE carries [`BORROWED_LEAF`] so `Pmap::Drop`'s walker removes the
    ///   translation but never frees the frame (the double-free guard the
    ///   G1 synthesis §3 mandates);
    /// - `pin` (the owning object's `Arc`) is held until this Pmap is
    ///   dropped, so the owner cannot free frames still reachable here;
    /// - no zeroing — the owner allocated and zeroed the frames.
    ///
    /// W^X is enforced exactly as for owned mappings; the two atrium
    /// callers pass `USER_DATA_RW` (client role) or `USER_DATA_RO`
    /// (compositor role — hardware read-only + NX).
    #[cfg(feature = "atrium-compositor-g1")]
    pub fn map_borrowed_region(
        &self,
        frames: &[PhysFrame<Size4KiB>],
        flags: PageTableFlags,
        pin: alloc::sync::Arc<dyn core::any::Any + Send + Sync>,
    ) -> Result<u64, PmapError> {
        debug_assert!(!frames.is_empty(), "map_borrowed_region: empty frame list");
        check_user_flags(flags)?;

        let bytes = (frames.len() as u64) * PAGE_SIZE;
        let mut g = self.lock_inner(PmapLockSite::MapBorrowedRegion);
        // Taken under `inner`, as the push below always was.
        let mut pins = self.borrowed_pins.lock();
        if pins.len() >= MAX_BORROWED_PINS {
            return Err(PmapError::TooManyPins);
        }
        // OT-61 C2.2b: the entry is charged to this Pmap's account here, with
        // `inner` held (interrupts off): no preemption, so no force-kill
        // (CP-30/CP-65) can find the charge on this thread's stack before
        // the Pmap owns it. A refusal maps nothing and moves no cursor.
        #[cfg(feature = "obj-quota")]
        let pin = {
            let account = self
                .account()
                .cloned()
                .unwrap_or_else(crate::sched::process::root_mem_account);
            match super::mem_account_logic::ObjCharge::new(
                &account,
                crate::sched::process::obj_weights::PIN,
            ) {
                Ok(charge) => (pin, charge),
                Err(_) => return Err(PmapError::QuotaExceeded),
            }
        };
        let base = g.next_mmap_va;
        let new_cursor = base
            .checked_add(bytes)
            .ok_or(PmapError::UserHalfOverlapsKernel)?;
        if new_cursor > USER_HEAP_LIMIT {
            return Err(PmapError::UserHalfOverlapsKernel);
        }
        g.next_mmap_va = new_cursor;

        // Record the pin BEFORE installing any borrowed PTE: if the map loop
        // fails midway (intermediate page-table OutOfFrames), pages [0..N)
        // are already reachable through BORROWED_LEAF PTEs — the pin must
        // already exist or the owner could free frames those PTEs still map
        // (the partial-failure use-after-free the cross-family review
        // caught). On failure the pin + the cursor range stay burned until
        // this Pmap drops — a bounded leak, never a UAF (fail-safe; the
        // same partial-failure posture as `allocate_user_region`).
        pins.push(BorrowedMapping {
            base,
            pages: frames.len() as u64,
            _pin: pin,
        });
        drop(pins);

        for (i, frame) in frames.iter().enumerate() {
            let page_va = base + (i as u64) * PAGE_SIZE;
            let replaced = self.map_user_locked(
                &mut g,
                VirtAddr::new(page_va),
                *frame,
                flags | BORROWED_LEAF,
            )?;
            // Same monotonic-cursor property as `allocate_user_region`: these
            // are fresh VAs, so no peer can hold a stale translation and no
            // shootdown is owed (this function already released the guard before
            // returning, and nothing shot down under it).
            debug_assert!(
                !replaced,
                "map_borrowed_region: monotonic cursor mapped an already-mapped VA"
            );
        }
        drop(g);

        Ok(base)
    }

    /// Chad #177 item 4: remove the borrowed mapping that `map_borrowed_region`
    /// put at `base`, and release its pin and (under `obj-quota`) its charge.
    /// The PTEs are cleared under `inner`, the TLB is shot down after the
    /// guard drops (CP-37), and only then is the pin dropped: the owner can
    /// free its frames only once no translation here can reach them, the order
    /// `Pmap::Drop` keeps. The frames are never freed by this Pmap (borrowed
    /// leaves). `NotMapped` if no borrowed mapping starts at `base`. The VA
    /// range is not reused (the mmap cursor only grows).
    #[cfg(feature = "atrium-compositor-g1")]
    pub fn unmap_borrowed_region(&self, base: u64) -> Result<(), PmapError> {
        let mut g = self.lock_inner(PmapLockSite::UnmapBorrowedRegion);
        let mapping = {
            let mut pins = self.borrowed_pins.lock();
            let at = pins
                .iter()
                .position(|m| m.base == base)
                .ok_or(PmapError::NotMapped)?;
            pins.swap_remove(at)
        };
        let first = VirtAddr::new(base);
        let mut cleared_any = false;
        for i in 0..mapping.pages {
            let page = checked_page_va(first, i)?;
            let (cleared, frame) = self.unmap_user_locked(&mut g, page)?;
            debug_assert!(frame.is_none(), "unmap_borrowed_region: an owned leaf");
            cleared_any |= cleared;
        }
        drop(g);
        if cleared_any {
            let last = checked_page_va(first, mapping.pages.saturating_sub(1))?;
            pte_write_fence_and_shootdown(Some((page_range(first).0, page_range(last).1)));
        }
        // The pin (and charge) go only after no translation can reach the
        // owner's frames.
        drop(mapping);
        Ok(())
    }

    /// How many borrowed mappings this Pmap holds (the #177 leg's check).
    #[cfg(all(feature = "atrium-compositor-g1", feature = "neo-test"))]
    #[allow(dead_code)]
    pub fn borrowed_mappings_for_test(&self) -> usize {
        self.borrowed_pins.lock().len()
    }

    /// Lock-free read of a leaf PTE's flags (same walk discipline as
    /// [`Pmap::translate`]). Scenario introspection for the atrium RO/NX
    /// mapping asserts (ISC-5/ISC-6).
    #[cfg(feature = "atrium-compositor-g1")]
    pub fn translate_flags(&self, va: VirtAddr) -> Option<PageTableFlags> {
        let indices = page_indices(va);
        // SAFETY: read-only walk through HHDM-mapped tables.
        let mut current =
            unsafe { &*(phys_to_virt(self.pml4_frame.start_address()).as_ptr::<PageTable>()) };
        for level in 0..3 {
            let entry = &current[indices[level] as usize];
            if entry.is_unused() {
                return None;
            }
            current = unsafe { &*(phys_to_virt(entry.addr()).as_ptr::<PageTable>()) };
        }
        let leaf = &current[indices[3] as usize];
        if leaf.is_unused() {
            None
        } else {
            Some(leaf.flags())
        }
    }

    /// CR3 value to load when switching INTO this Pmap. The flags field is
    /// 0 (no PCID for Phase 1.4 — full TLB flush on every CR3 write).
    pub fn cr3(&self) -> (PhysFrame<Size4KiB>, Cr3Flags) {
        (self.pml4_frame, Cr3Flags::empty())
    }

    /// Run `f` on the HHDM address of the user byte at `va`, with this
    /// Pmap's `inner` lock held from the translation until `f` returns, so
    /// the frame can't be unmapped and freed under `f` (every path that frees
    /// a mapped frame clears its PTE under `inner` first, CP-37). `None`
    /// (and `f` not run) unless ring 3 could make the access
    /// (`translate_user`). `f` must stay within `va`'s page, take no lock
    /// that is taken under `inner`, and not block: `inner` is an
    /// `IrqSafeMutex`. The IPC copy reaches its peer's buffer through this
    /// (CP-99).
    pub fn with_user_frame<R>(
        &self,
        va: VirtAddr,
        write: bool,
        f: impl FnOnce(*mut u8) -> R,
    ) -> Option<R> {
        let _g = self.lock_inner(PmapLockSite::WithUserFrame);
        let phys = self.translate_user(va, write)?;
        Some(f(phys_to_virt(phys).as_mut_ptr::<u8>()))
    }

    /// `translate`, for a user access: `None` unless every level of the walk
    /// is present and user-accessible, the walk ends at a 4 KiB leaf, and,
    /// when `write`, every level is writable, i.e. exactly when ring 3 could
    /// make that access. The IPC copy resolves its peer's buffer through
    /// this (CP-99): a peer's read-only page is `EFAULT`, not written through
    /// the HHDM. Lock-free, like `translate` (ADR-006 §Q4).
    pub fn translate_user(&self, va: VirtAddr, write: bool) -> Option<PhysAddr> {
        let mut need = PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE;
        if write {
            need |= PageTableFlags::WRITABLE;
        }
        let indices = page_indices(va);
        // SAFETY: read-only walk through HHDM-mapped tables.
        let mut current =
            unsafe { &*(phys_to_virt(self.pml4_frame.start_address()).as_ptr::<PageTable>()) };
        for level in 0..3 {
            let entry = &current[indices[level] as usize];
            let flags = entry.flags();
            if !flags.contains(need) || flags.contains(PageTableFlags::HUGE_PAGE) {
                return None;
            }
            current = unsafe { &*(phys_to_virt(entry.addr()).as_ptr::<PageTable>()) };
        }
        let leaf = &current[indices[3] as usize];
        if leaf.flags().contains(need) {
            Some(leaf.addr() + (va.as_u64() & 0xFFF))
        } else {
            None
        }
    }

    /// Walk the page tables and return the physical address that backs `va`,
    /// or `None` if any level along the way is unmapped. Phase 1.5b's IPC
    /// path uses this to resolve a peer process's user buffer (without
    /// switching CR3) for the cross-process rendezvous copy.
    ///
    /// Per ADR-006 §Q4: lock-free. Body MUST NOT touch `self.inner` —
    /// growing a lock through this borrow chain would defeat the
    /// interior-mutability split's whole purpose.
    pub fn translate(&self, va: VirtAddr) -> Option<PhysAddr> {
        let indices = page_indices(va);
        // SAFETY: read-only walk through HHDM-mapped tables.
        let mut current =
            unsafe { &*(phys_to_virt(self.pml4_frame.start_address()).as_ptr::<PageTable>()) };
        for level in 0..3 {
            let entry = &current[indices[level] as usize];
            if entry.is_unused() {
                return None;
            }
            current = unsafe { &*(phys_to_virt(entry.addr()).as_ptr::<PageTable>()) };
        }
        let leaf = &current[indices[3] as usize];
        if leaf.is_unused() {
            None
        } else {
            Some(leaf.addr() + (va.as_u64() & 0xFFF))
        }
    }
}

/// CP-60 test witnesses: how often `assert_pml4_not_live` ran, and how often
/// it ran with this CPU inside `with_frame_alloc` (must stay 0).
#[cfg(feature = "neo-test")]
pub static LIVE_CHECKS: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);
#[cfg(feature = "neo-test")]
pub static LIVE_CHECKS_UNDER_FRAME_ALLOC: core::sync::atomic::AtomicU64 =
    core::sync::atomic::AtomicU64::new(0);

/// CP-60: no CPU may still have `pml4` loaded when its `Pmap` is dropped.
/// Every switch away from a Pmap's threads reloads CR3 (cross-process prep),
/// and the last reference outlives its last thread, so a CPU still on it
/// means a kernel thread is running on a dead process's page table: freeing
/// it would let the next `Pmap::new` zero a live PML4. Fail loudly instead.
/// Runs before any frame is freed and never under `FRAME_ALLOC` (so the
/// panic path can still allocate). `drop` holds `&mut self` and every
/// schedulable thread owns an `Arc<Pmap>`, so nothing can load this PML4
/// between the check and the free.
fn assert_pml4_not_live(pml4: PhysFrame<Size4KiB>) {
    #[cfg(feature = "neo-test")]
    {
        LIVE_CHECKS.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
        if crate::mm::frame_alloc_held_here() {
            LIVE_CHECKS_UNDER_FRAME_ALLOC.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
        }
    }
    let freed = pml4.start_address().as_u64();
    assert!(
        freed != crate::mm::kernel_cr3().0.start_address().as_u64(),
        "Pmap::drop: freeing the kernel's own PML4"
    );
    for (cpu, pc) in crate::arch::x86_64::per_cpu::PER_CPU.iter().enumerate() {
        let active = pc
            .active_pmap_cr3
            .load(core::sync::atomic::Ordering::Relaxed);
        assert!(
            active & !0xfff != freed,
            "Pmap::drop: PML4 {freed:#x} is still the active CR3 on cpu {cpu}"
        );
    }
}

/// OT-61 C2.2 (#143): a charged frame [`Pmap::alloc_user_frame`] handed
/// out and not yet in the page tables. Dropped unmapped it is freed, then
/// its charge released (a frame is back before its charge, as everywhere
/// here), so a map that fails leaves neither behind: `Drop`'s walk would
/// never reach it, yet releases the whole ledger.
#[cfg(feature = "mem-quota")]
#[must_use = "an unmapped frame is freed when dropped"]
pub struct UnmappedFrame<'a> {
    pmap: &'a Pmap,
    frame: PhysFrame<Size4KiB>,
    armed: bool,
}

#[cfg(feature = "mem-quota")]
impl UnmappedFrame<'_> {
    pub fn frame(&self) -> PhysFrame<Size4KiB> {
        self.frame
    }

    /// The frame is in the page tables now, which own it.
    pub fn mapped(mut self) {
        self.armed = false;
    }
}

#[cfg(feature = "mem-quota")]
impl Drop for UnmappedFrame<'_> {
    fn drop(&mut self) {
        if self.armed {
            let frame = self.frame;
            with_frame_alloc(|fa| fa.free(frame));
            self.pmap.release_frames(1);
        }
    }
}

impl Drop for Pmap {
    /// Phase 1.5: two-pass walker (Plan-agent CRITICAL #1+#3+#7).
    ///
    /// **Pass 1** is a read-only DFS over user-half PML4 entries
    /// (`0..KERNEL_PML4_ENTRIES`), collecting every reachable frame
    /// (intermediate PT/PD/PDPT + leaf data) into a local Vec.
    /// Upper 128 PML4 entries are NEVER touched — those PDPTs are
    /// kernel-shared and freeing them would corrupt every other Pmap
    /// and the kernel itself.
    ///
    /// **Pass 2** frees every collected frame, then frees the PML4 itself.
    /// Doing collection BEFORE any free eliminates the
    /// "free → realloc → overwrite → re-read" hazard that a one-pass
    /// walker would have.
    ///
    /// HUGE_PAGE entries at PD or PDPT level are treated as leaf data
    /// frames (don't recurse). Phase 1.5 doesn't use huge pages but the
    /// check is defensive against Phase 5+ tensor storage which may.
    fn drop(&mut self) {
        // CP-60, before anything is freed and outside the allocator lock
        // (Jarvis F1: a panic inside `with_frame_alloc` holds `FRAME_ALLOC`
        // and IF=0, and the bare-metal test panic path allocates DMA frames
        // to flush its log, so it would re-enter the lock and deadlock).
        assert_pml4_not_live(self.pml4_frame);
        let mut to_free: Vec<PhysFrame<Size4KiB>> = Vec::new();

        // Pass 1: collect frames, read-only walk.
        // SAFETY: pml4_frame is HHDM-mapped; no concurrent writers (we
        // hold &mut self; Pmap is Drop, not Clone).
        let pml4 =
            unsafe { &*(phys_to_virt(self.pml4_frame.start_address()).as_ptr::<PageTable>()) };
        for i in 0..KERNEL_PML4_ENTRIES {
            debug_assert!(
                i < KERNEL_PML4_ENTRIES,
                "Pmap::Drop walker tried to descend into kernel half — would corrupt kernel"
            );
            let entry = &pml4[i];
            if entry.is_unused() {
                continue;
            }
            // PML4 entries always point at PDPT (no HUGE_PAGE at this level
            // in x86_64 long mode), so always recurse.
            let pdpt_phys = entry.addr();
            collect_pdpt(pdpt_phys, &mut to_free);
            to_free.push(PhysFrame::containing_address(pdpt_phys));
        }

        // K1a-0 F1 witness: this shootdown waits on every ready CPU, and a CPU
        // spinning for CAP_GRAPH (interrupts off) can't answer it, so a Pmap
        // must never drop under CAP_GRAPH.
        #[cfg(feature = "neo-test")]
        assert!(
            !crate::sched::process::cap_graph_held_here()
                && !crate::sched::process::releasing_cap_graph_pins_here(),
            "K1a-0 F1: a Pmap dropped (TLB shootdown) under CAP_GRAPH or its caller's IF-off section"
        );
        // Before freeing page-table frames, invalidate any stale translations
        // that could still reference this address space on another CPU. Q8.c
        // will add the stronger "stop all threads first" process-exit rule;
        // this is the SC-3C page-table-teardown shootdown boundary.
        pte_write_fence_and_shootdown(None);

        // Pass 2: free everything collected.
        let n_frames = to_free.len();
        with_frame_alloc(|fa| {
            for frame in &to_free {
                fa.free(*frame);
            }
            // PML4 itself last (checked not live at the top of `drop`).
            fa.free(self.pml4_frame);
        });
        // OT-61 C2.2: everything this Pmap charged is freed now; release it
        // once, after the frees. `held` is the single source, so a frame
        // released earlier (unmap) is not released twice.
        #[cfg(feature = "mem-quota")]
        if let Some(l) = &self.ledger {
            let r = l.release_all();
            #[cfg(feature = "neo-test")]
            assert!(
                r.is_ok(),
                "mem-quota: Pmap drop released more than its account held"
            );
            let _ = r;
        }

        // Debug-only log: serial_println takes a Mutex, and Pmap::Drop
        // can run from inconvenient contexts (panic unwind, nested Drop).
        // Production builds skip the log to avoid any deadlock surface.
        #[cfg(debug_assertions)]
        crate::serial_println!(
            "[ kernel ] pmap: dropped — freed {} frames + 1 PML4",
            n_frames
        );
        // Suppress unused warning when the println is compiled out.
        #[cfg(not(debug_assertions))]
        let _ = n_frames;
    }
}

fn page_range(va: VirtAddr) -> (VirtAddr, VirtAddr) {
    let start = va.as_u64() & !(PAGE_SIZE - 1);
    (
        VirtAddr::new(start),
        VirtAddr::new(
            start
                .checked_add(PAGE_SIZE)
                .expect("user page end overflow"),
        ),
    )
}

/// Publish the PTE writes, then invalidate peers' stale translations.
///
/// # MUST NOT be called while `Pmap::inner` is held (OT-61 C1.1, CP-37)
///
/// `Pmap::inner` is an [`IrqSafeMutex`]: it is held with **interrupts masked**,
/// because it is taken from both syscall (`IF=0`) and kernel-thread (`IF=1`)
/// contexts and a preemptible holder deadlocks the CPU (CP-37 — that is exactly
/// what the `threads_futex` hang was). Masking interrupts makes this rule
/// mandatory rather than merely tidy: **an `IF=0` section must never wait on
/// another CPU.**
///
/// A shootdown does exactly that — it IPIs peers and waits for their ACKs, and
/// its own `SHOOTDOWN_LOCK` wait even re-enables interrupts to stay ACK-able
/// (`tlb_shootdown::acquire_shootdown_lock`). Running that under an IF-off guard
/// is the CP-2/CP-3 trap: spin-waiting with interrupts masked for something whose
/// completion needs this CPU to service interrupts.
///
/// So every caller performs its PTE writes under the guard, **drops it**, and
/// only then calls this — the same discipline `process.rs` already documents for
/// the process-registry lock ("never hold the registry spinlock across it").
/// Two ordering obligations survive the move:
///   - flush **before** freeing any frame the unmap returned, so a stale
///     translation cannot outlive the frame into a reallocation, and
///   - flush **before** reporting success for a permission downgrade.
fn pte_write_fence_and_shootdown(range: Option<(VirtAddr, VirtAddr)>) {
    // SC-3C R-C2: make the PTE write globally visible before the IPI can
    // cause a remote CPU to observe and act on the invalidation request.
    fence(Ordering::SeqCst);

    #[cfg(not(feature = "neo-test-missing-shootdown-negative"))]
    super::tlb_shootdown::shootdown(range, super::tlb_shootdown::other_cpus_mask());

    #[cfg(feature = "neo-test-missing-shootdown-negative")]
    let _ = range;
}

/// Pass 1 helper: walk a PDPT, collect leaf data frames + intermediate
/// PT/PD frames into `to_free`. Push intermediate PD frames AFTER their
/// children so the order in `to_free` is leaves-first (matches the natural
/// "free children before parents" expectation; not strictly required since
/// Pass 2 frees them all anyway, but cleaner for diagnostics).
fn collect_pdpt(pdpt_phys: x86_64::PhysAddr, to_free: &mut Vec<PhysFrame<Size4KiB>>) {
    // SAFETY: HHDM-mapped, read-only access.
    let pdpt = unsafe { &*(phys_to_virt(pdpt_phys).as_ptr::<PageTable>()) };
    for j in 0..512 {
        let entry = &pdpt[j];
        if entry.is_unused() {
            continue;
        }
        if entry.flags().contains(PageTableFlags::HUGE_PAGE) {
            // 1 GiB huge page: leaf data frame, no PD below.
            to_free.push(PhysFrame::containing_address(entry.addr()));
            continue;
        }
        let pd_phys = entry.addr();
        collect_pd(pd_phys, to_free);
        to_free.push(PhysFrame::containing_address(pd_phys));
    }
}

fn collect_pd(pd_phys: x86_64::PhysAddr, to_free: &mut Vec<PhysFrame<Size4KiB>>) {
    let pd = unsafe { &*(phys_to_virt(pd_phys).as_ptr::<PageTable>()) };
    for k in 0..512 {
        let entry = &pd[k];
        if entry.is_unused() {
            continue;
        }
        if entry.flags().contains(PageTableFlags::HUGE_PAGE) {
            // 2 MiB huge page: leaf data frame, no PT below.
            to_free.push(PhysFrame::containing_address(entry.addr()));
            continue;
        }
        let pt_phys = entry.addr();
        collect_pt(pt_phys, to_free);
        to_free.push(PhysFrame::containing_address(pt_phys));
    }
}

fn collect_pt(pt_phys: x86_64::PhysAddr, to_free: &mut Vec<PhysFrame<Size4KiB>>) {
    let pt = unsafe { &*(phys_to_virt(pt_phys).as_ptr::<PageTable>()) };
    for l in 0..512 {
        let entry = &pt[l];
        if entry.is_unused() {
            continue;
        }
        // Borrowed leaves (atrium shared pixel buffers) are owned by their
        // kernel object, not this Pmap: remove the translation (the whole
        // table is discarded) but never free the frame — the owner does,
        // once the mapping pin drops (after this walker's shootdown).
        #[cfg(feature = "atrium-compositor-g1")]
        if entry.flags().contains(BORROWED_LEAF) {
            continue;
        }
        // PT entries are always leaf 4 KiB data frames.
        to_free.push(PhysFrame::containing_address(entry.addr()));
    }
}
