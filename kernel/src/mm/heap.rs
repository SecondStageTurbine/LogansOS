// SPDX-License-Identifier: MPL-2.0
//! Kernel heap setup.
//!
//! Maps a 4 MiB virtual range frame-by-frame, then hands the range to
//! `linked_list_allocator` which serves all `alloc::*` traffic via the
//! `#[global_allocator]` static.
//!
//! BUILD-3 derives the heap virtual base from the relocated kernel image end.
//! This keeps KASLR from inheriting the old fixed `0xffff_ffff_8800_0000`
//! kernel address.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use core::alloc::{GlobalAlloc, Layout};
use core::ptr::NonNull;
#[cfg(feature = "neo-test-instrumentation")]
use core::sync::atomic::{AtomicU64, Ordering};

use linked_list_allocator::Heap;
use spin::Mutex;
use x86_64::VirtAddr;
use x86_64::structures::paging::{
    FrameAllocator, Mapper, Page, PageTableFlags, Size4KiB, Translate,
};

use crate::arch::x86_64::without_interrupts;
use crate::serial_println;

/// Kernel heap size, selected by the real-brain-boot feature flags via the pure
/// `heap_policy::heap_size_bytes` (host-tested in `tools/heap-policy-logic-tests`).
/// Kill-switch (Rule 1): with all model-specific flags OFF this is bit-for-bit
/// the previous layer's budget — 944 MiB for the 0.5B boot (Q8_0; 768 MiB under
/// Q4_K_M), 4 MiB otherwise.
/// The largest selected model tier wins; see `heap_policy` for the pure oracle.
/// B1b: with `model-boot-module` the 0.5B tier drops its staging buffer.
/// B1c: the 0.5B tier is selected by `real-lm` (which every real-brain test
/// feature implies), so a product real-LM kernel gets it too.
pub const HEAP_SIZE: usize = if POLICY_HEAP_SIZE > HEAP_FLOOR {
    POLICY_HEAP_SIZE
} else {
    HEAP_FLOOR
};
/// The least a build's features need, above the model policy. NFS-W1: the
/// large-write scenario holds its 9 MiB file in RAM to write it. OT-87 R4.3:
/// the installer holds the claim's 2 MiB of expected windows, its write
/// chunks and a NeoFS mount of the whole target partition (whose live set is
/// one byte per 4 KiB block) at once. Neither feature: no floor, the policy's
/// size bit for bit.
const HEAP_FLOOR: usize = if cfg!(feature = "installer") {
    64 * 1024 * 1024
} else if cfg!(feature = "neo-test-neofs-large-write") {
    32 * 1024 * 1024
} else {
    0
};
const POLICY_HEAP_SIZE: usize = crate::mm::heap_policy::heap_size_bytes_for_source(
    cfg!(feature = "model-boot-module"),
    cfg!(feature = "real-lm"),
    cfg!(feature = "neo-test-arthur-real-brain-boot-15b"),
    cfg!(feature = "neo-test-arthur-real-brain-boot-3b"),
    cfg!(feature = "neo-test-arthur-real-brain-boot-qwen35"),
);
const HEAP_GUARD_SIZE: u64 = 2 * 1024 * 1024; // 2 MiB guard after kernel image
const HEAP_ALIGN: u64 = 2 * 1024 * 1024; // P2-aligned for future huge pages

unsafe extern "C" {
    static __kernel_image_start: u8;
    static __kernel_image_end: u8;
}

#[global_allocator]
static ALLOCATOR: IrqSafeHeap = IrqSafeHeap::empty();

#[cfg(feature = "neo-test-instrumentation")]
static HEAP_ALLOC_CALLS: AtomicU64 = AtomicU64::new(0);
#[cfg(feature = "neo-test-instrumentation")]
static HEAP_DEALLOC_CALLS: AtomicU64 = AtomicU64::new(0);

struct IrqSafeHeap {
    inner: Mutex<Heap>,
}

impl IrqSafeHeap {
    /// Bytes allocated on the heap now (B1a census).
    #[cfg(any(
        feature = "model-boot-module",
        feature = "neo-test-admission-park-kill",
        feature = "neo-test-obj-quota"
    ))]
    fn used(&self) -> usize {
        without_interrupts(|| self.inner.lock().used())
    }

    const fn empty() -> Self {
        Self {
            inner: Mutex::new(Heap::empty()),
        }
    }

    /// SAFETY: caller guarantees `[start, start + size)` is exclusive, valid
    /// writable memory that remains reserved for the kernel heap lifetime.
    unsafe fn init(&self, start: *mut u8, size: usize) {
        without_interrupts(|| {
            let mut heap = self.inner.lock();
            // SAFETY: the caller's contract supplies linked_list_allocator's
            // range validity and exclusivity requirements; the lock keeps init
            // mutually exclusive and interrupts are masked for the whole
            // critical section.
            unsafe { heap.init(start, size) };
        });
    }
}

unsafe impl GlobalAlloc for IrqSafeHeap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        #[cfg(feature = "neo-test-instrumentation")]
        HEAP_ALLOC_CALLS.fetch_add(1, Ordering::Relaxed);

        let ptr = without_interrupts(|| {
            self.inner
                .lock()
                .allocate_first_fit(layout)
                .ok()
                .map_or(core::ptr::null_mut(), |ptr| ptr.as_ptr())
        });
        if ptr.is_null() {
            crate::observability::record_heap_alloc_failure();
        } else {
            crate::observability::record_heap_alloc(layout.size());
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        #[cfg(feature = "neo-test-instrumentation")]
        HEAP_DEALLOC_CALLS.fetch_add(1, Ordering::Relaxed);
        crate::observability::record_heap_dealloc(layout.size());

        without_interrupts(|| {
            let mut heap = self.inner.lock();
            // SAFETY: GlobalAlloc's caller guarantees `ptr` came from this
            // allocator with `layout`, and dealloc receives a non-null pointer.
            unsafe { heap.deallocate(NonNull::new_unchecked(ptr), layout) };
        });
    }
}

/// `ot90_quiesce_heap_held`: take the allocator's lock and never release it
/// (a CPU parked mid-allocation). `try_lock`, so a CPU that already held it
/// when its NMI came doesn't spin on itself.
#[cfg(feature = "neo-test-ot90-quiesce-heap-held")]
pub fn hold_lock_forever_for_test() {
    if let Some(guard) = ALLOCATOR.inner.try_lock() {
        core::mem::forget(guard);
    }
}

pub fn init<M, A>(mapper: &mut M, frame_alloc: &mut A)
where
    M: Mapper<Size4KiB> + Translate,
    A: FrameAllocator<Size4KiB>,
{
    let heap_start = heap_start();

    // ISC-24b sanity: the very first heap page must NOT already be mapped,
    // otherwise we'd silently write over something the bootloader put there.
    if mapper.translate_addr(heap_start).is_some() {
        panic!(
            "heap: start {:?} is already mapped - pick a different virtual range",
            heap_start
        );
    }

    let heap_start_page = Page::<Size4KiB>::containing_address(heap_start);
    let heap_end_page = Page::<Size4KiB>::containing_address(heap_start + (HEAP_SIZE as u64 - 1));
    let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_EXECUTE;

    for page in Page::range_inclusive(heap_start_page, heap_end_page) {
        let frame = frame_alloc
            .allocate_frame()
            .expect("heap: out of physical frames during heap mapping");

        // SAFETY: we hold the unique `&mut OffsetPageTable` (single-init
        // boundary in paging::create_mapper); boot is single-threaded so
        // no other CPU is mapping concurrently; `heap_start..heap_end` was
        // verified unmapped above and the loop maps disjoint pages; the
        // bump allocator is monotonic, so `frame` (heap content) and any
        // intermediate page-table frames the walker pulls from the same
        // `frame_alloc` are guaranteed disjoint; `map_to` zeros each new
        // PML4/PDPT/PD/PT before installing it. flush() updates the TLB
        // for this leaf page.
        unsafe {
            mapper
                .map_to(page, frame, flags, frame_alloc)
                .expect("heap: map_to failed")
                .flush();
        }
    }

    // Smoke check: write-then-read at the first byte before handing the
    // region to the allocator. If this faults, the mapping math is wrong.
    let v = unsafe {
        let p = heap_start.as_mut_ptr::<u8>();
        p.write_volatile(0xa5);
        p.read_volatile()
    };
    assert!(v == 0xa5, "heap: write-then-read mismatch (got 0x{:x})", v);

    // SAFETY: heap_start..heap_start+HEAP_SIZE is now backed by writable
    // physical frames and isn't aliased anywhere else.
    unsafe {
        ALLOCATOR.init(heap_start.as_mut_ptr(), HEAP_SIZE);
    }

    serial_println!(
        "[ kernel ] heap: ready ({} MiB at {:?}, {} pages mapped)",
        HEAP_SIZE / (1024 * 1024),
        heap_start,
        HEAP_SIZE / 4096
    );
}

/// The kernel image's virtual range `[start, end)` (B1c: the inference stack
/// placement must not overlap it).
#[cfg(feature = "inference-stack")]
pub(crate) fn kernel_image_range() -> (u64, u64) {
    // SAFETY: linker-defined symbols; only their addresses are taken.
    unsafe {
        (
            &__kernel_image_start as *const u8 as u64,
            &__kernel_image_end as *const u8 as u64,
        )
    }
}

/// Bytes allocated on the kernel heap now (B1a census; CP-66's baselines).
#[cfg(any(
    feature = "model-boot-module",
    feature = "neo-test-admission-park-kill",
    feature = "neo-test-obj-quota"
))]
pub fn used_bytes() -> usize {
    ALLOCATOR.used()
}

pub fn heap_start() -> VirtAddr {
    // Try preferred placement: above kernel image with guard.
    if let Some(addr) = try_above_kernel() {
        return addr;
    }

    // Fallback: between HHDM end and kernel image start.
    if let Some(addr) = try_below_kernel() {
        serial_println!(
            "[ kernel ] heap: KASLR placed kernel near canonical ceiling; \
             heap fell back to below-kernel placement at {:?}",
            addr,
        );
        return addr;
    }

    // Both placements failed — Limine produced an impossible layout.
    // This should never happen on real hardware; if it does, the bug
    // is in Limine's KASLR or in this kernel's mm::init wiring.
    panic!(
        "heap: no viable placement (kernel near ceiling AND HHDM/kernel \
         layout leaves no room below); Limine placement is degenerate"
    );
}

/// Try to place the heap above the kernel image with a 2 MiB guard.
/// Returns `None` if the arithmetic overflows (KASLR placed kernel near
/// canonical-space ceiling) — caller falls back to `try_below_kernel`.
///
/// Exposed `pub(crate)` for `test_harness::run_kaslr_heap_synthetic`.
pub(crate) fn try_above_kernel() -> Option<VirtAddr> {
    let kernel_end = unsafe { &__kernel_image_end as *const u8 as u64 };
    let unaligned = kernel_end.checked_add(HEAP_GUARD_SIZE)?;
    let aligned = align_up_checked(unaligned, HEAP_ALIGN)?;
    // Verify the exclusive end (aligned + HEAP_SIZE) is representable in u64.
    // The allocator computes heap_end = heap_start + heap_size internally; if
    // that overflows, allocation requests fail despite the heap region being
    // notionally valid. Checking the inclusive end (HEAP_SIZE - 1) is one
    // byte too lenient: it allows aligned == 0xffff_ffff_ffc0_0000 with a
    // 4 MiB heap, where exclusive end = 0x1_0000_0000_0000_0000 overflows.
    let _heap_end_exclusive = aligned.checked_add(HEAP_SIZE as u64)?;
    Some(VirtAddr::new(aligned))
}

/// Try to place the heap below the kernel image, above HHDM end.
/// Returns `None` if there's no room between HHDM end (plus guard) and
/// kernel image start (minus guard) for a `HEAP_SIZE` allocation.
///
/// Exposed `pub(crate)` for `test_harness::run_kaslr_heap_synthetic`.
pub(crate) fn try_below_kernel() -> Option<VirtAddr> {
    let kernel_start = unsafe { &__kernel_image_start as *const u8 as u64 };

    // Upper bound: kernel_start - guard - heap_size, aligned down.
    let upper_bound = kernel_start.checked_sub(HEAP_GUARD_SIZE)?;
    let candidate = upper_bound.checked_sub(HEAP_SIZE as u64)?;
    let aligned = candidate & !(HEAP_ALIGN - 1);

    // Lower bound: must not collide with HHDM region.
    // HHDM_OFFSET + HHDM_PHYS_END + guard < aligned, otherwise return None.
    let hhdm_offset = crate::mm::hhdm_offset();
    let hhdm_phys_end = crate::mm::HHDM_PHYS_END.load(core::sync::atomic::Ordering::Acquire);
    let hhdm_end = hhdm_offset
        .checked_add(hhdm_phys_end)?
        .checked_add(HEAP_GUARD_SIZE)?;
    if aligned <= hhdm_end {
        return None;
    }

    Some(VirtAddr::new(aligned))
}

/// Synthetic-input variant of `try_above_kernel` for test purposes.
/// Takes a synthetic `kernel_end` instead of reading the linker symbol.
///
/// Exposed `pub(crate)` for `test_harness::run_kaslr_heap_synthetic` only.
/// Production code uses `try_above_kernel`.
#[cfg(any(test, feature = "neo-test-kaslr-heap-synthetic"))]
pub(crate) fn try_above_kernel_synthetic(synthetic_kernel_end: u64) -> Option<VirtAddr> {
    let unaligned = synthetic_kernel_end.checked_add(HEAP_GUARD_SIZE)?;
    let aligned = align_up_checked(unaligned, HEAP_ALIGN)?;
    // Mirror try_above_kernel's exclusive-end check exactly.
    let _heap_end_exclusive = aligned.checked_add(HEAP_SIZE as u64)?;
    Some(VirtAddr::new(aligned))
}

/// Synthetic-input variant of `try_below_kernel` for test purposes.
/// Takes synthetic `kernel_start` and `hhdm_end` instead of reading
/// linker symbol and `HHDM_PHYS_END`.
///
/// Exposed `pub(crate)` for `test_harness::run_kaslr_heap_synthetic` only.
#[cfg(any(test, feature = "neo-test-kaslr-heap-synthetic"))]
pub(crate) fn try_below_kernel_synthetic(
    synthetic_kernel_start: u64,
    synthetic_hhdm_end: u64,
) -> Option<VirtAddr> {
    let upper_bound = synthetic_kernel_start.checked_sub(HEAP_GUARD_SIZE)?;
    let candidate = upper_bound.checked_sub(HEAP_SIZE as u64)?;
    let aligned = candidate & !(HEAP_ALIGN - 1);
    let hhdm_end = synthetic_hhdm_end.checked_add(HEAP_GUARD_SIZE)?;
    if aligned <= hhdm_end {
        return None;
    }
    Some(VirtAddr::new(aligned))
}

const fn align_up_checked(value: u64, align: u64) -> Option<u64> {
    match value.checked_add(align - 1) {
        Some(v) => Some(v & !(align - 1)),
        None => None,
    }
}

// PHASE-2-MIGRATE: when integration test infrastructure lands, this
// smoke test moves to a proper test kernel rather than running on the
// production boot path.
pub fn smoke_test() {
    let boxed = Box::new(0xdead_beef_u64);
    assert!(*boxed == 0xdead_beef, "Box deref mismatch: 0x{:x}", *boxed);
    drop(boxed);

    let mut v: Vec<u32> = Vec::with_capacity(100);
    for i in 0..100 {
        v.push(i);
    }
    let sum: u32 = v.iter().sum();
    assert!(sum == 4950, "Vec sum mismatch: {sum}");

    let mut s = String::from("hello");
    s.push_str(" world");
    assert!(s == "hello world", "String concat mismatch: {s}");

    serial_println!("[ kernel ] heap: smoke test passed (Box, Vec, String)");
}

#[cfg(feature = "neo-test-instrumentation")]
pub struct HeapInstrumentationCounters {
    pub alloc_calls: u64,
    pub dealloc_calls: u64,
}

#[cfg(feature = "neo-test-instrumentation")]
pub fn reset_instrumentation_counters() {
    HEAP_ALLOC_CALLS.store(0, Ordering::Relaxed);
    HEAP_DEALLOC_CALLS.store(0, Ordering::Relaxed);
}

#[cfg(feature = "neo-test-instrumentation")]
pub fn instrumentation_counters() -> HeapInstrumentationCounters {
    HeapInstrumentationCounters {
        alloc_calls: HEAP_ALLOC_CALLS.load(Ordering::Relaxed),
        dealloc_calls: HEAP_DEALLOC_CALLS.load(Ordering::Relaxed),
    }
}
