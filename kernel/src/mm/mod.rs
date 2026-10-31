// SPDX-License-Identifier: MPL-2.0
//! Memory management — frame allocator, page mapper, kernel heap.
//!
//! Phase 1.1 shipped the minimum viable stack (frame/heap/paging).
//! Phase 1.4 promotes the bump frame allocator to a static-singleton behind
//! a Mutex so it can be re-acquired by Phase 1.4's Pmap construction, ELF
//! loader, and user-stack allocator. The HHDM offset and kernel PML4 base
//! are also captured for later use by `mm::pmap`.
//!
//! **Phase 2 BUILD-2 sub-commit 1:** the frame allocator is now buddy
//! (`frame::BuddyAllocator`), replacing the bump allocator and its
//! intrusive free-list overlay. The `with_frame_alloc` wrapper signature
//! changed to take `&mut frame::BuddyAllocator`, but all callers use the
//! `FrameAllocator<Size4KiB>` trait methods so they didn't need changes.
//! The `frames_in_use()` accessor's semantic is preserved: +1 per
//! `allocate_frame`, -1 per `free`, regardless of buddy order. See
//! `mm/frame.rs` for the buddy details and ADR-012 §Q1 for the design.

//! mm subsystem.
//!
//! **Init order (from `init()`):** paging mapper construction → heap
//! → frame allocator promotion to global. The `pmap` module is
//! Phase 1.4+ runtime use only; it is NOT initialized at `mm::init`.

use core::sync::atomic::{AtomicU64, Ordering};

use limine::memmap::Entry;
use spin::{Mutex, Once};
use x86_64::instructions::tlb;
use x86_64::registers::control::Cr3Flags;
use x86_64::structures::paging::{
    FrameAllocator, Mapper, OffsetPageTable, Page, PageTable, PageTableFlags, PhysFrame, Size4KiB,
    Translate, page_table::PageTableEntry,
};
use x86_64::{PhysAddr, VirtAddr};

use crate::serial_println;

pub mod frame;
pub mod heap;
pub(crate) mod heap_policy;
mod hhdm_requirements;
#[cfg(feature = "inference-stack")]
pub mod inference_stack;
#[cfg(feature = "inference-stack")]
pub(crate) mod inference_stack_logic;
/// OT-61 C2.2 (A2, I-18): memory accounts, the per-content-process quota.
#[cfg(feature = "mem-quota")]
pub mod mem_account_logic;
#[cfg(feature = "model-boot-module")]
pub(crate) mod module_reclaim_logic;
pub mod paging;
pub mod pmap;
pub mod slab;
pub mod tlb_shootdown;

/// Debug-only per-CPU reentrancy guard for `with_frame_alloc` (OT-24). Not
/// compiled in release builds, where the `FRAME_ALLOC` mutex alone enforces
/// correctness — so release stays bit-for-bit unaffected. Kept in its own
/// file (self-contained, only `core::sync::atomic`) so the host regression
/// suite `tools/frame-guard-logic-tests` can `#[path]`-mount and exercise the
/// real guard logic with simulated CPU ids.
#[cfg(debug_assertions)]
mod frame_reentry_guard;

#[cfg(feature = "model-boot-module")]
pub use frame::AdmitError;
pub use hhdm_requirements::{
    BootHhdmRequirements, BootModuleExtent, HhdmBootReport, PhysExtent, required_hhdm_phys_end,
};

/// Global frame allocator, accessible after `mm::init`. Single-CPU; lock
/// contention is impossible during boot but Mutex is correct for the API.
/// Phase 2 BUILD-2 sub-commit 1: was `BumpFrameAllocator`; now `BuddyAllocator`.
static FRAME_ALLOC: Mutex<Option<frame::BuddyAllocator>> = Mutex::new(None);

/// Test witness (CP-60, Jarvis F1): which CPUs are inside `with_frame_alloc`
/// right now, so a check can prove it does not run under the allocator lock
/// and the panic path can report whether it was entered with the lock held.
#[cfg(feature = "neo-test")]
static FRAME_ALLOC_HELD: [core::sync::atomic::AtomicBool; crate::arch::x86_64::per_cpu::MAX_CPUS] =
    [const { core::sync::atomic::AtomicBool::new(false) }; crate::arch::x86_64::per_cpu::MAX_CPUS];

/// Test witness: is this CPU inside `with_frame_alloc`?
#[cfg(feature = "neo-test")]
pub fn frame_alloc_held_here() -> bool {
    let cpu = crate::arch::x86_64::per_cpu::current_cpu_id() as usize;
    FRAME_ALLOC_HELD
        .get(cpu)
        .is_some_and(|held| held.load(core::sync::atomic::Ordering::Relaxed))
}

/// Run `f` on the locked allocator, marking this CPU as holding it (test
/// builds) for exactly the closure's duration.
fn run_locked<F, R>(alloc: &mut frame::BuddyAllocator, f: F) -> R
where
    F: FnOnce(&mut frame::BuddyAllocator) -> R,
{
    #[cfg(feature = "neo-test")]
    let held = FRAME_ALLOC_HELD.get(crate::arch::x86_64::per_cpu::current_cpu_id() as usize);
    #[cfg(feature = "neo-test")]
    if let Some(held) = held {
        held.store(true, core::sync::atomic::Ordering::Relaxed);
    }
    let result = f(alloc);
    #[cfg(feature = "neo-test")]
    if let Some(held) = held {
        held.store(false, core::sync::atomic::Ordering::Relaxed);
    }
    result
}

/// HHDM (Higher-Half Direct Map) base — virt offset where Limine maps all of
/// physical RAM. Set once at `mm::init`. Pmap uses this to deref freshly
/// allocated frames as `&mut PageTable` without per-frame mapping.
static HHDM_OFFSET: Once<u64> = Once::new();

/// Largest physical address (exclusive end) seen in the Limine memmap.
/// Used by `heap::try_below_kernel` to compute the lower bound of the
/// fallback heap range without colliding with HHDM-mapped physical RAM.
/// Set once at `mm::init` time alongside `HHDM_OFFSET`.
pub static HHDM_PHYS_END: AtomicU64 = AtomicU64::new(0);

/// Kernel's PML4 physical address — captured at `mm::init` so user Pmaps can
/// copy its upper-128 entries (kernel-half mappings are shared per process).
static KERNEL_PML4_PHYS: Once<PhysAddr> = Once::new();
/// The kernel's CR3 value (PML4 frame and flags), captured at the same boot
/// point as [`KERNEL_PML4_PHYS`], before any user `Pmap` exists. Every
/// kernel thread runs on it. It must never be read lazily from the live CR3:
/// the first read can happen while a user process's PML4 is loaded (CP-60).
static KERNEL_CR3: Once<(PhysFrame, Cr3Flags)> = Once::new();

const PAGE_SIZE: u64 = 4096;

static HHDM_REQUIRED_END: AtomicU64 = AtomicU64::new(0);
static HHDM_REPAIR_PAGES: AtomicU64 = AtomicU64::new(0);
static HHDM_MODULE_COUNT: AtomicU64 = AtomicU64::new(0);

/// Bring up the memory subsystem and stash the long-lived state Phase 1.4
/// needs (FRAME_ALLOC, HHDM_OFFSET, KERNEL_PML4_PHYS).
pub fn init(
    hhdm_offset: u64,
    memmap: &'static [&'static Entry],
    requirements: &BootHhdmRequirements<'_>,
) {
    serial_println!(
        "[ kernel ] mm: hhdm_offset=0x{:016x}, memmap_entries={}",
        hhdm_offset,
        memmap.len()
    );

    // BuddyAllocator::new reads HHDM_OFFSET via phys_to_virt to install
    // FreeBlock headers in each free frame's first 16 bytes. Set HHDM_OFFSET
    // BEFORE constructing the buddy. Phase 1.5 BumpFrameAllocator didn't
    // need this ordering because it held no per-frame metadata at boot;
    // Phase 2 BUILD-2 sub-commit 1's buddy intrusive free-list does.
    HHDM_OFFSET.call_once(|| hhdm_offset);

    // Compute the largest physical end-address from usable memmap entries.
    // Used by heap::try_below_kernel for HHDM-collision avoidance.
    // Bounded cost — memmap entries are O(tens) on real hardware.
    let max_phys_end: u64 = memmap
        .iter()
        .filter(|e| e.type_ == limine::memmap::MEMMAP_USABLE)
        .map(|e| e.base + e.length)
        .max()
        .unwrap_or(0);
    let mut frame_alloc = frame::BuddyAllocator::new(memmap);
    let usable_bytes: u64 = memmap
        .iter()
        .filter(|e| e.type_ == limine::memmap::MEMMAP_USABLE)
        .map(|e| e.length)
        .sum();
    serial_println!(
        "[ kernel ] mm: usable physical memory = {} MiB",
        usable_bytes / (1024 * 1024)
    );

    let mut mapper = paging::create_mapper(hhdm_offset);
    let required_end =
        required_hhdm_phys_end(max_phys_end, requirements.executable, requirements.modules);

    #[cfg(feature = "neo-test-hhdm-force-unmap")]
    force_unmap_first_module_page(&mut mapper, hhdm_offset, requirements);

    let repair_pages =
        repair_required_hhdm_spans(&mut mapper, &mut frame_alloc, hhdm_offset, requirements);
    HHDM_PHYS_END.store(required_end, Ordering::Release);
    HHDM_REQUIRED_END.store(required_end, Ordering::Release);
    HHDM_REPAIR_PAGES.store(repair_pages, Ordering::Release);
    HHDM_MODULE_COUNT.store(requirements.modules.len() as u64, Ordering::Release);
    serial_println!("[ kernel ] mm: hhdm_phys_end=0x{:016x}", required_end);
    log_hhdm_requirements(requirements, required_end, repair_pages);
    assert_required_hhdm_coverage(&mapper, hhdm_offset, requirements);

    // Sub-commit 0: post-boot fixup sets GLOBAL on kernel-half PML4 leaves.
    // Limine does not set GLOBAL on its mappings; we set it here so that
    // cross-process CR3 reloads do not flush kernel TLB entries. Must run
    // before any Pmap::new() — user Pmaps copy kernel-half PML4 entries,
    // and stale (non-GLOBAL) copies would propagate.
    unsafe {
        paging::set_global_bit_on_kernel_mappings(hhdm_offset);
    }

    // sanity_check runs after the fixup — verifies CR3/HHDM wiring AND that
    // the fixup reached kernel .text. Pre-fixup call was removed in the
    // sub-commit 0 remediation: its only unique signal (pre-fixup
    // global=false) is expected and uninteresting, while the BUG warning
    // path now correctly fires only for genuine regressions.
    paging::sanity_check(&mapper, hhdm_offset);

    heap::init(&mut mapper, &mut frame_alloc);
    heap::smoke_test();
    // B1c (F5): the real-LM inference stack, mapped once here so no page
    // table changes at run time and every later user pmap shares it.
    #[cfg(feature = "inference-stack")]
    inference_stack::init(&mut mapper, &mut frame_alloc);

    // Promote the buddy allocator to a global so Phase 1.4's Pmap, ELF loader,
    // and user-stack allocator can re-acquire it.
    let (kernel_pml4_frame, kernel_cr3_flags) = x86_64::registers::control::Cr3::read();
    KERNEL_PML4_PHYS.call_once(|| kernel_pml4_frame.start_address());
    KERNEL_CR3.call_once(|| (kernel_pml4_frame, kernel_cr3_flags));
    *FRAME_ALLOC.lock() = Some(frame_alloc);

    serial_println!(
        "[ kernel ] mm: kernel PML4 phys = 0x{:016x}",
        KERNEL_PML4_PHYS.get().unwrap().as_u64()
    );

    // Initialize slab allocator per-CPU state.
    // Must be called after buddy allocator is operational (FRAME_ALLOC is set).
    slab::init();
}

/// Return the boot-time HHDM repair summary for test scenarios.
#[allow(dead_code)]
pub fn hhdm_boot_report() -> HhdmBootReport {
    HhdmBootReport {
        required_end: HHDM_REQUIRED_END.load(Ordering::Acquire),
        repair_pages: HHDM_REPAIR_PAGES.load(Ordering::Acquire),
        module_count: HHDM_MODULE_COUNT.load(Ordering::Acquire),
    }
}

fn repair_required_hhdm_spans(
    mapper: &mut OffsetPageTable<'static>,
    frame_alloc: &mut frame::BuddyAllocator,
    hhdm_offset: u64,
    requirements: &BootHhdmRequirements<'_>,
) -> u64 {
    let mut repaired =
        repair_hhdm_extent(mapper, frame_alloc, hhdm_offset, requirements.executable);
    for module in requirements.modules {
        repaired += repair_hhdm_extent(mapper, frame_alloc, hhdm_offset, module.extent);
    }
    repaired
}

fn repair_hhdm_extent(
    mapper: &mut OffsetPageTable<'static>,
    frame_alloc: &mut frame::BuddyAllocator,
    hhdm_offset: u64,
    extent: PhysExtent,
) -> u64 {
    if extent.is_empty() {
        return 0;
    }

    let mut repaired = 0;
    let mut phys = align_down_4k(extent.start);
    let end = align_up_4k(extent.end);
    while phys < end {
        let virt = hhdm_virt(hhdm_offset, phys);
        match mapper.translate_addr(virt) {
            Some(mapped) if mapped.as_u64() == phys => {}
            Some(mapped) => {
                panic!(
                    "hhdm repair: virt {:?} maps to wrong phys {:#x}, expected {:#x}",
                    virt,
                    mapped.as_u64(),
                    phys
                );
            }
            None => {
                map_hhdm_4k(mapper, frame_alloc, virt, phys);
                repaired += 1;
            }
        }
        phys = phys
            .checked_add(PAGE_SIZE)
            .expect("hhdm repair: extent page walk overflow");
    }
    repaired
}

fn map_hhdm_4k(
    mapper: &mut OffsetPageTable<'static>,
    frame_alloc: &mut frame::BuddyAllocator,
    virt: VirtAddr,
    phys: u64,
) {
    let page = Page::<Size4KiB>::containing_address(virt);
    let frame = PhysFrame::<Size4KiB>::containing_address(PhysAddr::new(phys));
    // SAFETY: This runs once in single-threaded boot before AP or scheduler
    // bring-up. The target is an upper-half HHDM page for a Limine-owned
    // physical span that boot code must read; intermediate page-table frames
    // come from the local boot allocator, not the global FRAME_ALLOC.
    unsafe {
        mapper
            .map_to(page, frame, hhdm_ram_flags(), frame_alloc)
            .expect("hhdm repair: map_to failed")
            .flush();
    }
}

fn hhdm_ram_flags() -> PageTableFlags {
    PageTableFlags::PRESENT
        | PageTableFlags::WRITABLE
        | PageTableFlags::GLOBAL
        | PageTableFlags::NO_EXECUTE
}

fn assert_required_hhdm_coverage(
    mapper: &OffsetPageTable<'static>,
    hhdm_offset: u64,
    requirements: &BootHhdmRequirements<'_>,
) {
    assert_hhdm_extent_covered(mapper, hhdm_offset, "kernel", requirements.executable);
    for module in requirements.modules {
        assert_hhdm_extent_covered(mapper, hhdm_offset, "module", module.extent);
    }
}

fn assert_hhdm_extent_covered(
    mapper: &OffsetPageTable<'static>,
    hhdm_offset: u64,
    label: &str,
    extent: PhysExtent,
) {
    if extent.is_empty() {
        return;
    }
    assert_hhdm_addr_maps_to_phys(mapper, hhdm_offset, label, extent.start);
    assert_hhdm_addr_maps_to_phys(mapper, hhdm_offset, label, extent.end - 1);
}

fn assert_hhdm_addr_maps_to_phys(
    mapper: &OffsetPageTable<'static>,
    hhdm_offset: u64,
    label: &str,
    phys: u64,
) {
    let virt = hhdm_virt(hhdm_offset, phys);
    match mapper.translate_addr(virt) {
        Some(mapped) if mapped.as_u64() == phys => {}
        Some(mapped) => {
            panic!(
                "hhdm coverage: {} virt {:?} maps to wrong phys {:#x}, expected {:#x}",
                label,
                virt,
                mapped.as_u64(),
                phys
            );
        }
        None => {
            panic!(
                "hhdm coverage: {} phys {:#x} virt {:?} is unmapped",
                label, phys, virt
            );
        }
    }
}

#[cfg(feature = "neo-test-hhdm-force-unmap")]
fn force_unmap_first_module_page(
    mapper: &mut OffsetPageTable<'static>,
    hhdm_offset: u64,
    requirements: &BootHhdmRequirements<'_>,
) {
    let extent = requirements
        .modules
        .first()
        .map(|module| module.extent)
        .unwrap_or(requirements.executable);
    if extent.is_empty() {
        panic!("hhdm force-unmap: no non-empty extent available");
    }
    let virt = hhdm_virt(hhdm_offset, extent.start);
    let page = Page::<Size4KiB>::containing_address(virt);
    match mapper.unmap(page) {
        Ok((_frame, flush)) => {
            flush.flush();
            serial_println!(
                "[hhdm] forced_unmap_page virt=0x{:016x}",
                page.start_address().as_u64()
            );
        }
        Err(err) => panic!("hhdm force-unmap: unmap failed: {:?}", err),
    }
}

fn log_hhdm_requirements(
    requirements: &BootHhdmRequirements<'_>,
    required_end: u64,
    repair_pages: u64,
) {
    crate::serial_print!(
        "[hhdm] required_end=0x{:x} exec=[0x{:x},0x{:x}) modules=[",
        required_end,
        requirements.executable.start,
        requirements.executable.end
    );
    for (idx, module) in requirements.modules.iter().enumerate() {
        if idx != 0 {
            crate::serial_print!(",");
        }
        crate::serial_print!(
            "module{}=[0x{:x},0x{:x})",
            idx,
            module.extent.start,
            module.extent.end
        );
    }
    crate::serial_println!("] repair_pages={}", repair_pages);
}

fn hhdm_virt(hhdm_offset: u64, phys: u64) -> VirtAddr {
    VirtAddr::new(
        hhdm_offset
            .checked_add(phys)
            .expect("hhdm repair: hhdm_offset + phys overflow"),
    )
}

const fn align_down_4k(value: u64) -> u64 {
    value & !(PAGE_SIZE - 1)
}

fn align_up_4k(value: u64) -> u64 {
    value
        .checked_add(PAGE_SIZE - 1)
        .expect("hhdm repair: align_up overflow")
        & !(PAGE_SIZE - 1)
}

/// Acquire the global frame allocator under a closure. Wrapped in
/// `without_interrupts` (Phase 1.5) to make the free-list pop+push
/// atomic against IRQ-driven preemption. Per Plan-agent CRITICAL #2.
///
/// Reentry on the same CPU is forbidden — closing-over `with_frame_alloc`
/// from within the closure deadlocks (and would also break the free-list
/// invariant). Debug builds panic via a per-CPU reentrancy guard; release
/// builds rely on the Mutex. Cross-CPU contention is legitimate: the
/// `spin::Mutex` serializes access and each CPU spins only briefly.
///
/// The reentrancy guard is per-CPU (indexed by `current_cpu_id()`) so that
/// two different CPUs can independently enter `with_frame_alloc` without
/// tripping each other's guard. Same-CPU reentrancy — the actual bug —
/// still panics deterministically. See `frame_reentry_guard` (OT-24).
pub fn with_frame_alloc<F, R>(f: F) -> R
where
    F: FnOnce(&mut frame::BuddyAllocator) -> R,
{
    use crate::arch::x86_64::without_interrupts;
    without_interrupts(|| {
        #[cfg(debug_assertions)]
        {
            use crate::arch::x86_64::per_cpu;
            use crate::mm::frame_reentry_guard::ReentryGuard;

            // Per-CPU guard (OT-24): one flag per CPU rather than a single
            // global `AtomicBool`, so two APs legitimately entering
            // `with_frame_alloc` at the same time during SMP bring-up
            // (e.g. `slab::init_per_cpu`, `apic::init_local_apic`) no longer
            // false-trip the same-CPU reentry check. `current_cpu_id()` reads
            // `gs:[24]`, valid here because every call site runs after the
            // CPU's `per_cpu` init (BSP in `init_traps`, each AP in `ap_entry`).
            static IN_FRAME_ALLOC: ReentryGuard<{ per_cpu::MAX_CPUS }> = ReentryGuard::new();

            let cpu_id = per_cpu::current_cpu_id() as usize;
            IN_FRAME_ALLOC.enter(cpu_id);
            let result = {
                let mut guard = FRAME_ALLOC.lock();
                let alloc = guard
                    .as_mut()
                    .expect("mm::with_frame_alloc called before mm::init");
                run_locked(alloc, f)
            };
            IN_FRAME_ALLOC.exit(cpu_id);
            result
        }
        #[cfg(not(debug_assertions))]
        {
            let mut guard = FRAME_ALLOC.lock();
            let alloc = guard
                .as_mut()
                .expect("mm::with_frame_alloc called before mm::init");
            run_locked(alloc, f)
        }
    })
}

/// HHDM offset. Available after `mm::init`. Maps a phys addr to its
/// kernel-readable virtual addr: `phys + HHDM_OFFSET = virt`.
pub fn hhdm_offset() -> u64 {
    *HHDM_OFFSET
        .get()
        .expect("mm::hhdm_offset called before mm::init")
}

/// Convert a physical address to its HHDM-mapped virtual address. Used to
/// dereference freshly-allocated page-table frames as `&mut PageTable`.
pub fn phys_to_virt(phys: PhysAddr) -> VirtAddr {
    VirtAddr::new(phys.as_u64() + hhdm_offset())
}

/// Map one kernel MMIO page at its HHDM virtual address.
///
/// Limine's HHDM covers usable RAM, but device MMIO holes such as the local
/// APIC page are not guaranteed present. This installs the missing leaf in
/// the active kernel PML4 so xAPIC MMIO can use the same phys+HHDM address
/// convention as normal physical-memory access.
pub fn map_kernel_mmio_page(phys: PhysAddr) -> VirtAddr {
    let virt = phys_to_virt(phys);
    let indices = page_indices(virt);
    let flags = PageTableFlags::PRESENT
        | PageTableFlags::WRITABLE
        | PageTableFlags::NO_CACHE
        | PageTableFlags::WRITE_THROUGH
        | PageTableFlags::GLOBAL
        | PageTableFlags::NO_EXECUTE;

    // SAFETY: KERNEL_PML4_PHYS is the active kernel root captured at mm::init,
    // reachable through HHDM. We mutate only the leaf for the APIC MMIO page,
    // which is not RAM-owned by Rust objects.
    let mut current = unsafe { &mut *(phys_to_virt(kernel_pml4_phys()).as_mut_ptr::<PageTable>()) };

    for &idx in &indices[..3] {
        let entry = &mut current[idx as usize];
        assert!(
            !entry.flags().contains(PageTableFlags::HUGE_PAGE),
            "mmio map crossed an existing huge-page kernel mapping"
        );
        if entry.is_unused() {
            let new_frame =
                with_frame_alloc(|fa| fa.allocate_frame()).expect("mmio map: out of frames");
            // SAFETY: `new_frame` was just allocated for a page table and is
            // reachable through HHDM; zeroing prepares it before publication.
            unsafe {
                (&mut *phys_to_virt(new_frame.start_address()).as_mut_ptr::<PageTable>()).zero();
            }
            let mut new_entry = PageTableEntry::new();
            new_entry.set_addr(
                new_frame.start_address(),
                PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
            );
            *entry = new_entry;
        }

        // SAFETY: the present non-huge entry points at the next page-table
        // level, which is kernel-owned and HHDM-reachable.
        current = unsafe { &mut *(phys_to_virt(entry.addr()).as_mut_ptr::<PageTable>()) };
    }

    let leaf = &mut current[indices[3] as usize];
    if !leaf.is_unused() {
        assert_eq!(
            leaf.addr(),
            PhysFrame::<Size4KiB>::containing_address(phys).start_address(),
            "mmio map collided with an existing non-APIC mapping"
        );
        leaf.set_flags(leaf.flags() | flags);
    } else {
        leaf.set_addr(
            PhysFrame::<Size4KiB>::containing_address(phys).start_address(),
            flags,
        );
    }
    tlb::flush(virt);
    virt
}

/// Whether `virt` is currently mapped in the active kernel page tables
/// (handling huge pages at any level). Lets a caller decide whether a physical
/// page needs an explicit mapping before access — the ACPI parser uses this
/// because the RSDP/ACPI tables can live in reserved/BIOS memory (QEMU's RSDP
/// is in 0xE0000-0xFFFFF) that the kernel's direct map does not cover, while
/// other tables sit in huge-page-mapped RAM that `map_kernel_mmio_page` refuses.
pub fn kernel_virt_mapped(virt: VirtAddr) -> bool {
    let indices = page_indices(virt);
    // SAFETY: the kernel PML4 is the active root, reachable through HHDM; we
    // only read entries here.
    let mut current = unsafe { &*(phys_to_virt(kernel_pml4_phys()).as_ptr::<PageTable>()) };
    for &idx in &indices[..3] {
        let entry = &current[idx as usize];
        if entry.is_unused() {
            return false;
        }
        if entry.flags().contains(PageTableFlags::HUGE_PAGE) {
            return true; // covered by a huge page at this level
        }
        current = unsafe { &*(phys_to_virt(entry.addr()).as_ptr::<PageTable>()) };
    }
    !current[indices[3] as usize].is_unused()
}

#[inline]
fn page_indices(va: VirtAddr) -> [u64; 4] {
    [
        (va.as_u64() >> 39) & 0x1ff,
        (va.as_u64() >> 30) & 0x1ff,
        (va.as_u64() >> 21) & 0x1ff,
        (va.as_u64() >> 12) & 0x1ff,
    ]
}

/// Kernel's PML4 physical address. Used by `Pmap::new` to find the
/// upper-128 PML4 entries that user processes need to share.
#[allow(dead_code)] // first caller lands in Phase 1.4 BUILD-B (Pmap::new)
pub fn kernel_pml4_phys() -> PhysAddr {
    *KERNEL_PML4_PHYS
        .get()
        .expect("mm::kernel_pml4_phys called before mm::init")
}

/// The kernel's CR3 (see [`KERNEL_CR3`]). Available after `mm::init`.
pub fn kernel_cr3() -> (PhysFrame, Cr3Flags) {
    *KERNEL_CR3
        .get()
        .expect("mm::kernel_cr3 called before mm::init")
}

/// Re-export of the live frame count. See `mm::frame::frames_in_use`.
pub fn frames_in_use() -> u64 {
    frame::frames_in_use()
}

/// Last frame count emitted by `frame_audit`. Used to compute the
/// "delta since previous audit" annotation. Stays at 0 until the first
/// audit call, which then publishes the current count.
static LAST_AUDIT_FRAMES: AtomicU64 = AtomicU64::new(0);

/// Phase 1.5b BUILD-6 leak detector. Emit a single line of the form
/// `[ frame ] phase=<label> used=N delta=±M` reflecting the live frame
/// count and the delta since the previous `frame_audit` call. Cheap
/// (one atomic load + one atomic swap + one serial println).
///
/// The kernel's `spawn_neo_init` flow calls this at well-defined phase
/// boundaries (baseline pre-spawn; after the wait loop terminates) and
/// the per-process create / terminate / exit paths call it to surface
/// in-flight accounting. Together they give a complete leak audit:
/// `final == baseline` is the assertion the e2e smoke test enforces.
pub fn frame_audit(label: &str) {
    let cur = frames_in_use();
    let prev = LAST_AUDIT_FRAMES.swap(cur, Ordering::AcqRel);
    let delta = cur as i64 - prev as i64;
    serial_println!("[ frame ] phase={} used={} delta={:+}", label, cur, delta);
}

/// B1a boot census (Amendment 1 (d)): one serial line a bare-metal run can be
/// read against. `tensor_capacity` arrives with the model in B1b.
///
/// `CENSUS label=<l> heap_capacity=<B> heap_used=<B> primary_free_pages=<n>
/// admitted_pages=<n> admitted_free_pages=<n> frames_in_use=<n> refused_frees=<n>`
#[cfg(feature = "model-boot-module")]
pub fn boot_census(label: &str) {
    let (primary_free, admitted) = with_frame_alloc(|fa| fa.free_pages_by_zone());
    let admitted_free: u64 = admitted.iter().flatten().sum();
    // Tensor storage draws buddy blocks from every zone (`tensor.rs`), so its
    // block capacity is the free pages of all of them. The heap high-water is
    // only tracked under `kernel-observability`; 0 means "not tracked".
    #[cfg(feature = "kernel-observability")]
    let heap_high_water = crate::observability::snapshot().heap_high_water_bytes;
    #[cfg(not(feature = "kernel-observability"))]
    let heap_high_water = 0u64;
    serial_println!(
        "CENSUS label={} heap_capacity={} heap_used={} primary_free_pages={} admitted_pages={} admitted_free_pages={} frames_in_use={} refused_frees={} tensor_block_capacity_pages={} heap_high_water={}",
        label,
        heap::HEAP_SIZE,
        heap::used_bytes(),
        primary_free,
        frame::admitted_pages(),
        admitted_free,
        frames_in_use(),
        frame::refused_frees(),
        primary_free + admitted_free,
        heap_high_water
    );
    // B1c (F5): the inference stack's deepest use so far and its call count.
    #[cfg(feature = "inference-stack")]
    serial_println!(
        "CENSUS label={} inference_stack_size={} inference_stack_high_water={} inference_stack_calls={}",
        label,
        inference_stack_logic::SIZE,
        inference_stack::high_water(),
        inference_stack::calls()
    );
}
