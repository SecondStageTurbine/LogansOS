// SPDX-License-Identifier: MPL-2.0
//! Page mapping bring-up using Limine's HHDM.
//!
//! The kernel never builds its own page tables — Limine already set up a
//! sane four-level map (kernel at higher-half, all physical RAM mirrored
//! at HHDM). We just wrap CR3's level-4 table in an `OffsetPageTable` so
//! the rest of the kernel can `map_to(virt, frame, flags)`.

use x86_64::registers::control::Cr3;
use x86_64::structures::paging::{OffsetPageTable, PageTable, PageTableFlags, Translate};
use x86_64::{PhysAddr, VirtAddr};

use crate::serial_println;

/// Read CR3 and return a `&'static mut` to the active level-4 page table
/// via the HHDM. Internal helper for `create_mapper`.
unsafe fn active_level_4_table(hhdm_offset: u64) -> &'static mut PageTable {
    let (level_4_frame, _) = Cr3::read();
    let phys = level_4_frame.start_address().as_u64();
    let virt = VirtAddr::new(phys + hhdm_offset);
    let table_ptr: *mut PageTable = virt.as_mut_ptr();
    // SAFETY: caller guarantees this is the active PML4 reached through HHDM.
    unsafe { &mut *table_ptr }
}

/// Build the kernel's page mapper.
///
/// This is the single-init boundary for kernel paging — calling it twice
/// would produce two `&'static mut` aliases to the same level-4 page
/// table, which is UB. The kernel's single-threaded boot satisfies this
/// (only `mm::init` calls here, exactly once). When SMP lands, this
/// becomes a `spin::Once<Mutex<OffsetPageTable>>` accessor.
pub fn create_mapper(hhdm_offset: u64) -> OffsetPageTable<'static> {
    // SAFETY: called exactly once, during single-threaded boot. See doc.
    unsafe {
        let level_4 = active_level_4_table(hhdm_offset);
        OffsetPageTable::new(level_4, VirtAddr::new(hhdm_offset))
    }
}

/// Set the GLOBAL bit on all kernel-half PML4 leaves (entries 256-511).
/// Limine does not set GLOBAL on its mappings; without this fixup, every
/// cross-process CR3 reload flushes kernel TLB entries unnecessarily.
///
/// SAFETY: This must run in single-threaded boot context before any user
/// Pmaps are created. All page tables are reachable via HHDM and were
/// set up by Limine, so the walk is safe.
pub unsafe fn set_global_bit_on_kernel_mappings(hhdm_offset: u64) {
    use x86_64::registers::control::Cr4Flags;
    use x86_64::registers::control::{Cr3, Cr4};
    use x86_64::structures::paging::PageTableFlags;

    // Verify CR4.PGE is set; if not, set it and log.
    let cr4 = Cr4::read();
    if !cr4.contains(Cr4Flags::PAGE_GLOBAL) {
        let new_cr4 = cr4 | Cr4Flags::PAGE_GLOBAL;
        // SAFETY: Single-threaded boot context. Setting CR4.PGE enables the
        // GLOBAL bit semantic for subsequent leaf-entry GLOBAL writes. No
        // other CPU exists yet, so no concurrent CR4 read can observe a
        // torn write.
        unsafe {
            Cr4::write(new_cr4);
        }
        serial_println!("[ kernel ] paging: CR4.PGE was not set; enabled it.");
    }

    let (cr3_frame, _) = Cr3::read();
    let phys_to_virt = |p: x86_64::PhysAddr| VirtAddr::new(p.as_u64() + hhdm_offset);

    // Read the PML4 table via HHDM.
    let pml4: *mut PageTable = phys_to_virt(cr3_frame.start_address()).as_mut_ptr();
    // SAFETY: pml4 was obtained via HHDM phys_to_virt of the active
    // CR3 frame. Limine mapped it; no aliasing because we hold the
    // only reference at this point in single-threaded boot.
    let pml4_table = unsafe { &mut *pml4 };

    // Walk entries 256-511 (kernel half).
    let mut count_4k: u64 = 0;
    let mut count_2m: u64 = 0;
    let mut count_1g: u64 = 0;

    for i in 256..512 {
        let entry = &mut pml4_table[i];
        if entry.is_unused() || !entry.flags().contains(PageTableFlags::PRESENT) {
            continue;
        }

        // Get the PDPT address and walk it.
        let pdpt_virt = phys_to_virt(entry.addr());
        let pdpt: *mut PageTable = pdpt_virt.as_mut_ptr();
        // SAFETY: pdpt was obtained via HHDM phys_to_virt of a PML4 entry's
        // addr(). The PML4 is Limine-mapped; the PDPT is reachable through
        // HHDM. Single-threaded boot context; no concurrent access.
        let pdpt_table = unsafe { &mut *pdpt };

        for j in 0..512 {
            let pd_entry = &mut pdpt_table[j];
            if pd_entry.is_unused() || !pd_entry.flags().contains(PageTableFlags::PRESENT) {
                continue;
            }

            // Check if this is a 1 GiB huge page.
            if pd_entry.flags().contains(PageTableFlags::HUGE_PAGE) {
                // 1 GiB leaf at PDPT level.
                let addr = pd_entry.addr();
                let flags = pd_entry.flags();
                pd_entry.set_addr(addr, flags | PageTableFlags::GLOBAL);
                count_1g += 1;
                continue;
            }

            // Recurse into the PD.
            let pd_virt = phys_to_virt(pd_entry.addr());
            let pd: *mut PageTable = pd_virt.as_mut_ptr();
            // SAFETY: pd was obtained via HHDM phys_to_virt of a PDPT entry's
            // addr(). Reachable through HHDM; single-threaded boot context.
            let pd_table = unsafe { &mut *pd };

            for k in 0..512 {
                let pt_entry = &mut pd_table[k];
                if pt_entry.is_unused() || !pt_entry.flags().contains(PageTableFlags::PRESENT) {
                    continue;
                }

                // Check if this is a 2 MiB huge page.
                if pt_entry.flags().contains(PageTableFlags::HUGE_PAGE) {
                    // 2 MiB leaf at PD level.
                    let addr = pt_entry.addr();
                    let flags = pt_entry.flags();
                    pt_entry.set_addr(addr, flags | PageTableFlags::GLOBAL);
                    count_2m += 1;
                    continue;
                }

                // Recurse into the PT.
                let pt_virt = phys_to_virt(pt_entry.addr());
                let pt: *mut PageTable = pt_virt.as_mut_ptr();
                // SAFETY: pt was obtained via HHDM phys_to_virt of a PD entry's
                // addr(). Reachable through HHDM; single-threaded boot context.
                let pt_table = unsafe { &mut *pt };

                for l in 0..512 {
                    let leaf = &mut pt_table[l];
                    if leaf.is_unused() || !leaf.flags().contains(PageTableFlags::PRESENT) {
                        continue;
                    }

                    // 4 KiB leaf at PT level.
                    let addr = leaf.addr();
                    let flags = leaf.flags();
                    leaf.set_addr(addr, flags | PageTableFlags::GLOBAL);
                    count_4k += 1;
                }
            }
        }
    }

    serial_println!(
        "[ kernel ] paging: GLOBAL fixup complete - 4K leaves: {}, 2M leaves: {}, 1G leaves: {}",
        count_4k,
        count_2m,
        count_1g
    );
}

/// Quick sanity check: a known-mapped relocated kernel `.text` address
/// translates to *something*. If this returns None,
/// either CR3 is wrong, the HHDM offset is wrong, or our page table walk
/// is broken — and the next `map_to` would silently corrupt random RAM.
///
/// Also probes the GLOBAL bit on kernel `.text`. Without GLOBAL, every
/// CR3 reload (cross-process switch) flushes kernel-half TLB entries —
/// "syscalls feel slow" symptom. We don't fail boot on a missing GLOBAL
/// bit (Limine may legitimately not set it depending on revision); we
/// log a warning instead so the symptom is named.
pub fn sanity_check(mapper: &OffsetPageTable<'static>, hhdm_offset: u64) {
    let kernel_text_sample = VirtAddr::new(crate::_kernel_start as *const () as u64);
    match mapper.translate_addr(kernel_text_sample) {
        Some(phys) => {
            let flags = leaf_flags_for(mapper, kernel_text_sample, hhdm_offset);
            let global = flags
                .map(|f| f.contains(PageTableFlags::GLOBAL))
                .unwrap_or(false);
            serial_println!(
                "[ kernel ] paging: sanity OK - kernel .text sample virt {:?} -> phys {:?} (global={})",
                kernel_text_sample,
                phys,
                global
            );
            if !global {
                serial_println!(
                    "[ kernel ] paging: BUG - GLOBAL bit not set on kernel .text after fixup. \
                     The set_global_bit_on_kernel_mappings call did not reach this address. \
                     Cross-process CR3 reload will incorrectly flush this TLB entry. \
                     This is a sub-commit 0 regression; investigate immediately."
                );
            }
        }
        None => {
            panic!(
                "paging sanity check failed: kernel .text sample at {:?} doesn't translate \
                 - CR3 / HHDM offset is wrong",
                kernel_text_sample
            );
        }
    }
}

/// Walk the page tables manually to read the leaf flags for `va`.
/// Returns None if any level is unmapped or the leaf is a huge-page
/// (we don't care about huge-page flags for the sanity check).
fn leaf_flags_for(
    mapper: &OffsetPageTable<'static>,
    va: VirtAddr,
    hhdm_offset: u64,
) -> Option<PageTableFlags> {
    let _ = mapper; // mapper crate doesn't expose flag-walk directly; use HHDM walk
    let (cr3_frame, _) = Cr3::read();
    let indices = [
        ((va.as_u64() >> 39) & 0x1FF) as usize,
        ((va.as_u64() >> 30) & 0x1FF) as usize,
        ((va.as_u64() >> 21) & 0x1FF) as usize,
        ((va.as_u64() >> 12) & 0x1FF) as usize,
    ];
    let phys_to_virt = |p: PhysAddr| VirtAddr::new(p.as_u64() + hhdm_offset);
    // SAFETY: HHDM-mapped, read-only walk through kernel page tables.
    unsafe {
        let mut current = &*(phys_to_virt(cr3_frame.start_address()).as_ptr::<PageTable>());
        for level in 0..3 {
            let entry = &current[indices[level]];
            if entry.is_unused() {
                return None;
            }
            if entry.flags().contains(PageTableFlags::HUGE_PAGE) {
                return Some(entry.flags()); // huge-page leaf at this level
            }
            current = &*(phys_to_virt(entry.addr()).as_ptr::<PageTable>());
        }
        let leaf = &current[indices[3]];
        if leaf.is_unused() {
            None
        } else {
            Some(leaf.flags())
        }
    }
}
