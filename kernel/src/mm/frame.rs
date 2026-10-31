// SPDX-License-Identifier: MPL-2.0
//! Buddy frame allocator. Phase 2 BUILD-2 sub-commit 1.
//!
//! Per ADR-012 §Q1: classic power-of-2 buddy, orders 0..MAX_ORDER. The
//! default build keeps orders 0..14 (4 KiB to 64 MiB). Supersedes Phase 1.5's
//! `BumpFrameAllocator` and its intrusive
//! free-list overlay (the bump's `FREE_HEAD` pattern was a stepping
//! stone; buddy's per-order free lists subsume it).
//!
//! **Free-list shape.** Per-order intrusive linked lists. The first 16
//! bytes of each free frame hold a `FreeBlock { next, debug_marker }`.
//! No heap allocation for bookkeeping (avoids the chicken-and-egg with
//! the kernel heap that depends on buddy frames).
//!
//! **Bootstrap.** Walks Limine's memmap, picks the largest USABLE
//! region, trims its base UP to MAX_ORDER alignment so buddy
//! XOR arithmetic works in [0, region_size) space, decomposes the
//! trimmed range into power-of-2 blocks aligned to natural boundaries,
//! pushes each to its order's free list. Smaller USABLE regions are
//! ignored; Phase 2 BUILD-4 SMP hardening can add multi-zone support
//! if measurement shows the wasted memory matters.
//!
//! **Counter invariant (ADR-012 §implementation note 5 + checkpoint Q2.b).**
//! `FRAMES_IN_USE` counts allocated PAGES (4 KiB units). +1 per
//! `allocate_frame`; -1 per `free`. Split and coalesce internal
//! operations DO NOT touch the counter — they rearrange free blocks
//! but don't change the in-use count. Splitting an order-N free into
//! two order-(N-1) frees doesn't allocate anything; coalescing two
//! order-N frees into one order-(N+1) free doesn't free anything new.
//!
//! **Locking.** Per ADR-012 §implementation note 1 + checkpoint Q1.c:
//! `BuddyAllocator` has NO internal lock for sub-commit 1. Exclusivity
//! comes from `mm::with_frame_alloc`'s outer `Mutex<Option<BuddyAllocator>>`
//! wrapper plus `without_interrupts` discipline. Sub-commit 2's slab
//! integration may surface a need for separable buddy-lock acquisition
//! (slab calling buddy without reacquiring the outer wrapper); inner
//! `Mutex<()>` lands then if needed.
//!
//! **Double-free detection.** Debug builds use `FREE_POISON`-encoded
//! `debug_marker` (per-order tagged) — coalesce / free verifies the
//! marker matches the order being freed. The bump's `FREE_POISON`
//! moves here; the overlay retires fully.

use core::ptr::null_mut;
use core::sync::atomic::{AtomicU64, Ordering};

use limine::memmap::{Entry, MEMMAP_USABLE};
use x86_64::PhysAddr;
use x86_64::structures::paging::{FrameAllocator, PhysFrame, Size4KiB};

use super::phys_to_virt;

const FRAME_SIZE: u64 = 4096;
const FRAME_SHIFT: u32 = 12;

/// Max buddy order. Order N covers 2^N pages = 2^(N+12) bytes.
/// MAX_ORDER = 14 yields a 64 MiB largest contiguous block in normal builds.
/// The full 0.5B Qwen image has two larger immutable tensors, so a kernel that
/// runs a real LM (`real-lm`, B1c; implied by the real-brain test features)
/// lifts this to order 16 without changing the default allocator contract.
/// Per ADR-012 §Q1: chosen so Phase 5's tensor allocator can source
/// bulk frames from buddy without forcing a Phase 5 retrofit.
#[cfg(feature = "real-lm")]
pub const MAX_ORDER: usize = 16;
#[cfg(not(feature = "real-lm"))]
pub const MAX_ORDER: usize = 14;
const NUM_ORDERS: usize = MAX_ORDER + 1;

/// Live frame count. +1 per `allocate_frame`; -1 per `free`. Read by
/// `mm::frames_in_use()` for the BUILD-6 leak audit.
static FRAMES_IN_USE: AtomicU64 = AtomicU64::new(0);

/// Public read accessor. Used by `mm::frame_audit` for the leak detector.
pub fn frames_in_use() -> u64 {
    FRAMES_IN_USE.load(Ordering::Acquire)
}

#[cfg(feature = "kernel-observability")]
pub struct BuddyFreeSummary {
    pub free_pages: u64,
    pub largest_order: u32,
}

/// Free block header. Stored intrusively in the first 16 bytes of each
/// free frame; the frame is FREE so nobody else owns those bytes.
/// Layout is `repr(C)` so the offsets are stable.
#[repr(C)]
struct FreeBlock {
    /// Pointer to the next free block at this order, or null.
    /// Stored as HHDM-mapped kernel virtual address (matches the bump
    /// allocator's prior intrusive-list convention).
    next: *mut FreeBlock,
    /// Debug-only marker. Encoding (debug builds):
    ///   `(FREE_POISON | (order as u64))`
    /// Release builds leave this zero; only the `next` pointer is used
    /// on the hot path.
    debug_marker: u64,
}

#[cfg(debug_assertions)]
const FREE_POISON_MASK: u64 = 0xDEAD_BEEF_DEAD_BE00;

struct Zone {
    /// Per-order free list heads (intrusive linked list).
    /// `free_heads[N]` is the head of the list of free blocks at order N.
    /// A block at order N spans 2^N pages = 2^(N+12) bytes.
    free_heads: [*mut FreeBlock; NUM_ORDERS],
    /// Base physical address of the buddy's managed range.
    /// Trimmed UP to MAX_ORDER alignment so buddy XOR arithmetic
    /// `(addr - base) ^ (1 << (N + FRAME_SHIFT))` works in
    /// `[0, region_size)` space without crossing into adjacent zones.
    base_phys: u64,
    /// First page this zone owns: `base_phys` for the primary zone; for an
    /// admitted zone, the planned start (its `base_phys` is aligned below it
    /// for the buddy arithmetic, but it owns nothing there).
    start_phys: u64,
    /// End physical address (exclusive) of the buddy's managed range.
    /// Aligned DOWN to page boundary.
    end_phys: u64,
}

// SAFETY: BuddyAllocator is accessed under `mm::with_frame_alloc`'s
// `Mutex<Option<BuddyAllocator>>` wrapper, itself wrapped in
// `without_interrupts`. Single-CPU for Phase 2 BUILD-2; SMP retrofit
// (BUILD-3+) revisits the lock model. The raw `*mut FreeBlock` pointers
// in `free_heads` are not Send/Sync by default; we assert they're safe
// to send across threads because access is fully serialized.
unsafe impl Send for Zone {}
unsafe impl Sync for Zone {}

impl Zone {
    /// Bootstrap from Limine's memmap. Picks the largest USABLE region,
    /// trims to MAX_ORDER alignment, decomposes into power-of-2 blocks,
    /// pushes each to its order's free list. See file-doc for rationale.
    fn from_memmap(memmap: &'static [&'static Entry]) -> Self {
        let largest = memmap
            .iter()
            .filter(|e| e.type_ == MEMMAP_USABLE)
            .max_by_key(|e| e.length)
            .expect("buddy: no USABLE memory regions in memmap");

        let max_order_size: u64 = 1u64 << (MAX_ORDER as u32 + FRAME_SHIFT);
        let region_base = largest.base;
        let region_end = largest.base + largest.length;
        // Trim base UP to MAX_ORDER alignment so buddy XOR works in the
        // [0, region_size) space without crossing zone boundaries.
        let trimmed_base = (region_base + max_order_size - 1) & !(max_order_size - 1);
        // Trim end DOWN to page alignment.
        let trimmed_end = region_end & !(FRAME_SIZE - 1);

        assert!(
            trimmed_base < trimmed_end,
            "buddy: largest USABLE region too small after MAX_ORDER trim (base={:#x} end={:#x})",
            region_base,
            region_end
        );
        // B1c (F4): the alignment trim costs up to 2^MAX_ORDER - 1 pages
        // (256 MiB - 4 KiB at order 16, vs 64 MiB - 4 KiB at 14); log what it
        // actually cost on this machine.
        crate::serial_println!(
            "[ kernel ] mm: primary zone region={:#x}..{:#x} trimmed_base={:#x} trim_pages={} max_order={}",
            region_base,
            region_end,
            trimmed_base,
            (trimmed_base - region_base) / FRAME_SIZE,
            MAX_ORDER
        );

        let mut buddy = Self {
            free_heads: [null_mut(); NUM_ORDERS],
            base_phys: trimmed_base,
            start_phys: trimmed_base,
            end_phys: trimmed_end,
        };

        // Decompose [trimmed_base, trimmed_end) into power-of-2 blocks.
        // Greedy: at each cursor position, pick the LARGEST order whose
        // block size both (a) fits within the remaining range and (b) is
        // naturally aligned at the cursor. Push to that order's free list.
        let mut cursor = trimmed_base;
        while cursor + FRAME_SIZE <= trimmed_end {
            let mut order = MAX_ORDER;
            loop {
                let block_size: u64 = 1u64 << (order as u32 + FRAME_SHIFT);
                let aligned = (cursor & (block_size - 1)) == 0;
                let fits = cursor + block_size <= trimmed_end;
                if aligned && fits {
                    break;
                }
                // Order-0 blocks are page-aligned by construction (cursor
                // is page-aligned by trim) and FRAME_SIZE always fits if
                // the loop entry condition held; the panic is defensive.
                assert!(
                    order > 0,
                    "buddy: order-0 block doesn't fit at cursor={:#x}; cursor must be page-aligned",
                    cursor
                );
                order -= 1;
            }
            let block_size: u64 = 1u64 << (order as u32 + FRAME_SHIFT);
            // SAFETY: `cursor` is in the trimmed managed range; HHDM
            // maps it to kernel-virtual space; the block is freshly
            // initialized (no prior owner).
            unsafe {
                buddy.push_block_to_free_list(cursor, order);
            }
            cursor += block_size;
        }

        buddy
    }

    /// Push a block at physical address `addr` to the free list at `order`.
    /// SAFETY: caller must ensure the block is exclusively owned and that
    /// `addr` is naturally aligned for `order`.
    unsafe fn push_block_to_free_list(&mut self, addr: u64, order: usize) {
        let virt = phys_to_virt(PhysAddr::new(addr));
        let block_ptr = virt.as_mut_ptr::<FreeBlock>();
        unsafe {
            (*block_ptr).next = self.free_heads[order];
            #[cfg(debug_assertions)]
            {
                (*block_ptr).debug_marker = FREE_POISON_MASK | (order as u64);
            }
        }
        self.free_heads[order] = block_ptr;
    }

    /// Pop a block from the free list at `order`, or None if empty.
    /// Returns the physical address of the block.
    /// SAFETY: the popped block's first 16 bytes get overwritten by
    /// the caller (or zeroed if the caller doesn't write them).
    unsafe fn pop_block_from_free_list(&mut self, order: usize) -> Option<u64> {
        let head = self.free_heads[order];
        if head.is_null() {
            return None;
        }
        unsafe {
            #[cfg(debug_assertions)]
            {
                let expected = FREE_POISON_MASK | (order as u64);
                let marker = (*head).debug_marker;
                assert!(
                    marker == expected,
                    "buddy: free-list corruption at order {} — marker {:#x} != expected {:#x}",
                    order,
                    marker,
                    expected
                );
                (*head).debug_marker = 0;
            }
            self.free_heads[order] = (*head).next;
        }
        let virt = x86_64::VirtAddr::from_ptr(head);
        let phys = virt.as_u64() - super::hhdm_offset();
        Some(phys)
    }

    /// Remove a SPECIFIC block from the free list at `order`. Used by
    /// the coalesce path when the buddy is found on a free list and
    /// must be unlinked before merging. O(N) walk; N is small at
    /// Phase 2 scale.
    /// Returns true if the block was found and removed.
    unsafe fn remove_specific_block_from_free_list(
        &mut self,
        order: usize,
        target_addr: u64,
    ) -> bool {
        let target_virt = phys_to_virt(PhysAddr::new(target_addr));
        let target_ptr = target_virt.as_mut_ptr::<FreeBlock>();

        let mut prev_link: *mut *mut FreeBlock = &mut self.free_heads[order];
        unsafe {
            while !(*prev_link).is_null() {
                if *prev_link == target_ptr {
                    // Unlink.
                    *prev_link = (**prev_link).next;
                    #[cfg(debug_assertions)]
                    {
                        (*target_ptr).debug_marker = 0;
                    }
                    return true;
                }
                prev_link = &mut (**prev_link).next;
            }
        }
        false
    }

    /// Allocate a block at `order`. Returns the physical address or None
    /// on OOM. Splits higher-order blocks as needed.
    fn allocate_block(&mut self, order: usize) -> Option<u64> {
        if order > MAX_ORDER {
            return None;
        }

        // Try this order first.
        // SAFETY: pop returns a block we now exclusively own.
        if let Some(addr) = unsafe { self.pop_block_from_free_list(order) } {
            return Some(addr);
        }

        // Recursively split a higher-order block.
        let higher_addr = self.allocate_block(order + 1)?;
        // The higher block is at order (order + 1). Split it into two
        // halves at `order`. Take the first half as the allocated block;
        // push the second half to the free list at `order`.
        let half_size: u64 = 1u64 << (order as u32 + FRAME_SHIFT);
        let buddy_addr = higher_addr + half_size;
        // SAFETY: buddy_addr is owned (just split off from higher block).
        unsafe {
            self.push_block_to_free_list(buddy_addr, order);
        }
        Some(higher_addr)
    }

    /// Free a block at `order`. Coalesces with its buddy if possible,
    /// recursively up to MAX_ORDER.
    /// SAFETY: caller asserts the block was previously allocated at
    /// this exact order (or split-tracked equivalently) and is no
    /// longer owned.
    unsafe fn free_block(&mut self, addr: u64, order: usize) {
        if order >= MAX_ORDER {
            // No buddy at MAX_ORDER; just push to free list.
            unsafe { self.push_block_to_free_list(addr, order) };
            return;
        }

        // Compute buddy address. Buddy of block at addr (offset from
        // base_phys) at order N is `addr ^ (1 << (N + FRAME_SHIFT))`
        // computed in the offset space.
        let block_size: u64 = 1u64 << (order as u32 + FRAME_SHIFT);
        let offset = addr - self.base_phys;
        let buddy_offset = offset ^ block_size;
        let buddy_addr = self.base_phys + buddy_offset;

        // Verify buddy is in the managed range; if not, no coalesce.
        if buddy_addr + block_size > self.end_phys {
            unsafe { self.push_block_to_free_list(addr, order) };
            return;
        }

        // If buddy is on the free list at this order, remove it and
        // recursively free the merged block at the next-higher order.
        // SAFETY: remove walks the free list; if found, the buddy is
        // ours to merge.
        let coalesced = unsafe { self.remove_specific_block_from_free_list(order, buddy_addr) };
        if coalesced {
            let merged_addr = addr.min(buddy_addr);
            unsafe { self.free_block(merged_addr, order + 1) };
        } else {
            unsafe { self.push_block_to_free_list(addr, order) };
        }
    }

    /// Whether this zone owns `addr`.
    fn contains(&self, addr: u64) -> bool {
        self.start_phys <= addr && addr < self.end_phys
    }

    /// Whether this zone owns any page of `[start, end)`.
    #[cfg_attr(not(feature = "model-boot-module"), allow(dead_code))]
    fn intersects(&self, start: u64, end: u64) -> bool {
        start < self.end_phys && self.start_phys < end
    }

    /// Free pages on this zone's lists (census / observability).
    #[cfg_attr(not(feature = "model-boot-module"), allow(dead_code))]
    fn free_pages(&self) -> u64 {
        let mut free_pages = 0u64;
        for (order, &head) in self.free_heads.iter().enumerate() {
            let mut ptr = head;
            while !ptr.is_null() {
                free_pages = free_pages.saturating_add(1u64 << order);
                // SAFETY: free-list nodes are owned free frames under the
                // FRAME_ALLOC lock.
                unsafe {
                    ptr = (*ptr).next;
                }
            }
        }
        free_pages
    }

    #[cfg(feature = "kernel-observability")]
    fn observability_free_summary(&self) -> BuddyFreeSummary {
        let mut free_pages = 0u64;
        let mut largest_order = 0u32;
        for (order, &head) in self.free_heads.iter().enumerate() {
            let mut ptr = head;
            let mut blocks = 0u64;
            while !ptr.is_null() {
                blocks += 1;
                unsafe {
                    ptr = (*ptr).next;
                }
            }
            if blocks != 0 {
                largest_order = order as u32;
                free_pages = free_pages.saturating_add(blocks << order);
            }
        }
        BuddyFreeSummary {
            free_pages,
            largest_order,
        }
    }
}

/// B1a (Amendment 1 (b)): zones admitted after boot from a consumed Limine
/// module's planned pages. Two is enough for the model and its vocab.
pub const MAX_ADMITTED_ZONES: usize = 2;

/// Pages admitted from consumed boot modules (capacity, not use). Kept apart
/// from `FRAMES_IN_USE`: admission adds free pages and never touches it, so
/// it cannot underflow (Chad C2).
static ADMITTED_PAGES: AtomicU64 = AtomicU64::new(0);

/// Pages ever admitted from consumed boot modules.
pub fn admitted_pages() -> u64 {
    ADMITTED_PAGES.load(Ordering::Acquire)
}

/// Frees of an address no zone owns: refused, never coalesced (C2).
static REFUSED_FREES: AtomicU64 = AtomicU64::new(0);

/// Refused frees so far (a non-zero value is a bug the census reports).
pub fn refused_frees() -> u64 {
    REFUSED_FREES.load(Ordering::Acquire)
}

/// Why a region could not be admitted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdmitError {
    /// No whole page.
    Empty,
    /// The pages are not page-aligned.
    Unaligned,
    /// Every admitted-zone slot is in use.
    NoSlot,
    /// The pages intersect a zone the allocator already manages.
    Overlap,
}

/// The frame allocator: the boot-time primary zone (today's single zone) plus
/// up to [`MAX_ADMITTED_ZONES`] zones admitted from consumed boot modules.
/// With no admitted zone every path is the primary zone's own code, as before
/// B1a (the kill-switch).
pub struct BuddyAllocator {
    primary: Zone,
    admitted: [Option<Zone>; MAX_ADMITTED_ZONES],
}

impl BuddyAllocator {
    /// Bootstrap from Limine's memmap (the primary zone; see file doc).
    pub fn new(memmap: &'static [&'static Entry]) -> Self {
        Self {
            primary: Zone::from_memmap(memmap),
            admitted: [const { None }; MAX_ADMITTED_ZONES],
        }
    }

    fn has_admitted(&self) -> bool {
        self.admitted.iter().any(Option::is_some)
    }

    /// Allocate from the primary zone, then from admitted zones.
    fn allocate_block(&mut self, order: usize) -> Option<u64> {
        if let Some(addr) = self.primary.allocate_block(order) {
            return Some(addr);
        }
        self.admitted
            .iter_mut()
            .flatten()
            .find_map(|zone| zone.allocate_block(order))
    }

    /// Free into the zone that owns `addr`. With no admitted zone this is
    /// exactly the primary zone's free (as before B1a); otherwise an address
    /// no zone owns is refused and counted, never coalesced.
    /// SAFETY: as `Zone::free_block`.
    unsafe fn free_block(&mut self, addr: u64, order: usize) -> bool {
        if !self.has_admitted() || self.primary.contains(addr) {
            unsafe { self.primary.free_block(addr, order) };
            return true;
        }
        match self
            .admitted
            .iter_mut()
            .flatten()
            .find(|z| z.contains(addr))
        {
            Some(zone) => {
                unsafe { zone.free_block(addr, order) };
                true
            }
            None => {
                REFUSED_FREES.fetch_add(1, Ordering::AcqRel);
                false
            }
        }
    }

    /// Public free. Decrements FRAMES_IN_USE. SAFETY: caller asserts the
    /// frame was previously allocated by this allocator and is not aliased.
    /// Phase 2 BUILD-2 sub-commit 1: only order-0 frees are exposed via
    /// this entry point (mirrors the bump's API). Higher-order frees
    /// happen internally during coalesce.
    pub fn free(&mut self, frame: PhysFrame<Size4KiB>) {
        // SAFETY: caller asserts the frame was allocated by us at order 0.
        if unsafe { self.free_block(frame.start_address().as_u64(), 0) } {
            FRAMES_IN_USE.fetch_sub(1, Ordering::AcqRel);
        }
    }

    /// Free a contiguous block previously returned by `allocate_blocks`
    /// at the same order.
    pub fn deallocate_blocks(&mut self, frame: PhysFrame<Size4KiB>, order: usize) {
        if order > MAX_ORDER {
            debug_assert!(order <= MAX_ORDER, "invalid buddy block order");
            return;
        }
        // SAFETY: TensorStorage and max-order tests retain the exact order
        // returned by allocate_blocks, so this block is no longer owned and
        // can be returned to the matching free list/coalescer.
        if unsafe { self.free_block(frame.start_address().as_u64(), order) } {
            let pages = 1u64 << (order as u32);
            FRAMES_IN_USE.fetch_sub(pages, Ordering::AcqRel);
        }
    }

    /// Allocate a single contiguous block of 2^order pages and return
    /// its physical base address. None on OOM. Increments FRAMES_IN_USE
    /// by 2^order. Used by future tensor allocator (Phase 5+) and by
    /// the BUILD-2 max-order test scenario; sub-commit 1's other
    /// callers all go through `allocate_frame` (order 0) via the
    /// `FrameAllocator` trait.
    #[allow(dead_code)]
    pub fn allocate_blocks(&mut self, order: usize) -> Option<PhysFrame<Size4KiB>> {
        let addr = self.allocate_block(order)?;
        let pages = 1u64 << (order as u32);
        FRAMES_IN_USE.fetch_add(pages, Ordering::AcqRel);
        Some(PhysFrame::containing_address(PhysAddr::new(addr)))
    }

    /// B1a: admit the whole pages `[start, end)` of a consumed boot module as
    /// a new zone (Amendment 1 (b)). The caller has planned them with
    /// `module_reclaim_logic::plan_module_reclaim` and holds the module's
    /// registry tombstone, so nothing else can reach these frames. Never goes
    /// through `free()`: the pages enter the new zone's lists directly and
    /// `FRAMES_IN_USE` is untouched. Returns the pages admitted.
    #[cfg(feature = "model-boot-module")]
    pub fn admit_external_region(&mut self, start: u64, end: u64) -> Result<u64, AdmitError> {
        use super::module_reclaim_logic::{Span, blocks, zone_base};
        if !start.is_multiple_of(FRAME_SIZE) || !end.is_multiple_of(FRAME_SIZE) {
            return Err(AdmitError::Unaligned);
        }
        if start >= end {
            return Err(AdmitError::Empty);
        }
        let span = Span::new(start, end);
        if self.primary.intersects(start, end)
            || self
                .admitted
                .iter()
                .flatten()
                .any(|z| z.intersects(start, end))
        {
            return Err(AdmitError::Overlap);
        }
        let slot = self
            .admitted
            .iter_mut()
            .find(|z| z.is_none())
            .ok_or(AdmitError::NoSlot)?;
        let mut zone = Zone {
            free_heads: [null_mut(); NUM_ORDERS],
            base_phys: zone_base(start, MAX_ORDER),
            start_phys: start,
            end_phys: end,
        };
        for (addr, order) in blocks(span, MAX_ORDER) {
            // SAFETY: the planned pages are whole, page-aligned, owned by no
            // other zone (checked above) or object (the registry tombstone),
            // HHDM-mapped (boot repair), and `addr` is naturally aligned for
            // `order` (`blocks`).
            unsafe { zone.push_block_to_free_list(addr, order) };
        }
        *slot = Some(zone);
        let pages = span.pages();
        ADMITTED_PAGES.fetch_add(pages, Ordering::AcqRel);
        Ok(pages)
    }

    /// Free pages per zone: the primary first, then each admitted slot
    /// (`None` when empty). For the boot census.
    #[cfg_attr(not(feature = "model-boot-module"), allow(dead_code))]
    pub fn free_pages_by_zone(&self) -> (u64, [Option<u64>; MAX_ADMITTED_ZONES]) {
        let mut admitted = [None; MAX_ADMITTED_ZONES];
        for (out, zone) in admitted.iter_mut().zip(self.admitted.iter()) {
            *out = zone.as_ref().map(Zone::free_pages);
        }
        (self.primary.free_pages(), admitted)
    }

    /// Allocate a block from admitted zones only (B1a discriminator: proves
    /// admitted frames are real, writable and reusable without first
    /// exhausting the primary zone).
    #[cfg(feature = "neo-test-boot-module-reclaim")]
    pub fn allocate_admitted_blocks(&mut self, order: usize) -> Option<PhysFrame<Size4KiB>> {
        let addr = self
            .admitted
            .iter_mut()
            .flatten()
            .find_map(|zone| zone.allocate_block(order))?;
        FRAMES_IN_USE.fetch_add(1u64 << (order as u32), Ordering::AcqRel);
        Some(PhysFrame::containing_address(PhysAddr::new(addr)))
    }

    #[cfg(feature = "kernel-observability")]
    fn observability_free_summary(&self) -> BuddyFreeSummary {
        let mut summary = self.primary.observability_free_summary();
        for zone in self.admitted.iter().flatten() {
            let z = zone.observability_free_summary();
            summary.free_pages = summary.free_pages.saturating_add(z.free_pages);
            summary.largest_order = summary.largest_order.max(z.largest_order);
        }
        summary
    }
}

unsafe impl FrameAllocator<Size4KiB> for BuddyAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
        let addr = self.allocate_block(0)?;
        FRAMES_IN_USE.fetch_add(1, Ordering::AcqRel);
        Some(PhysFrame::containing_address(PhysAddr::new(addr)))
    }
}

#[cfg(feature = "kernel-observability")]
pub fn observability_free_summary() -> BuddyFreeSummary {
    crate::mm::with_frame_alloc(|fa| fa.observability_free_summary())
}

// === Test-harness hooks (BUILD-2 sub-commit 1 verification scenarios) ====

/// B1c (F4): free blocks on the primary zone's list at `order` (the
/// order-`MAX_ORDER` allocate/free/coalesce check).
#[cfg(feature = "neo-test-real-lm-generate")]
pub fn debug_primary_free_blocks(order: usize) -> u64 {
    crate::mm::with_frame_alloc(|fa| {
        let mut blocks = 0u64;
        let mut ptr = fa.primary.free_heads[order];
        while !ptr.is_null() {
            blocks += 1;
            // SAFETY: free-list nodes are owned free frames under the
            // FRAME_ALLOC lock, which `with_frame_alloc` holds.
            unsafe {
                ptr = (*ptr).next;
            }
        }
        blocks
    })
}

/// B1c (F4): does `phys` lie in the primary zone?
#[cfg(feature = "neo-test-real-lm-generate")]
pub fn debug_in_primary(phys: u64) -> bool {
    crate::mm::with_frame_alloc(|fa| fa.primary.intersects(phys, phys + 1))
}

/// Snapshot the head pointer at `order` (for test scenarios verifying
/// split/coalesce shape). Returns null if the free list is empty.
#[cfg(any(
    feature = "neo-test-buddy-split-coalesce",
    feature = "neo-test-buddy-max-order",
    feature = "neo-test-expect-pass-but-alloc-fail",
    feature = "neo-test-buddy-alloc-free-alloc-returns-same",
))]
pub fn debug_free_list_head(order: usize) -> *mut () {
    // SAFETY: snapshot read; pointer typed; debug-only.
    crate::mm::with_frame_alloc(|fa| fa.primary.free_heads[order] as *mut ())
}

/// Find the highest order with a non-null free list head. Used by
/// `buddy_split_coalesce` to identify the "seed order" the test draws
/// from. Returns None if all free lists are empty.
#[cfg(any(
    feature = "neo-test-buddy-split-coalesce",
    feature = "neo-test-buddy-max-order",
    feature = "neo-test-expect-pass-but-alloc-fail",
))]
pub fn debug_highest_nonempty_order() -> Option<usize> {
    crate::mm::with_frame_alloc(|fa| {
        (0..NUM_ORDERS)
            .rev()
            .find(|&order| !fa.primary.free_heads[order].is_null())
    })
}
