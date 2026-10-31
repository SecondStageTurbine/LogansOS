// SPDX-License-Identifier: MPL-2.0
//! Intel VT-d (DMA-remapping) driver — Phase 7.5 BUILD-V. Arms I-14(c2): the
//! arena and the IOMMU become one fence (software then hardware). The proven
//! substrate (GPU/NVMe/AHCI/virtio) goes into **identity domains** (GPA==HPA,
//! zero driver change, no regression); connectivity devices go into **fence
//! domains** mapping only the DMA arena's physical range, so a misbehaving
//! device physically cannot reach memory outside the arena — the IOMMU faults.
//!
//! **DMA-remapping only.** No interrupt remapping (out of scope). Legacy
//! (non-scalable) root/context mode, second-level translation, requests
//! without PASID.
//!
//! **I-14 fault-containment posture.** The fault path (`process_faults`)
//! acquires NO AI-governance lock (a) — it touches only this unit's MMIO and
//! local atomics — and is bounded per call (b) — at most `MAX_FAULTS_PER_DRAIN`
//! records processed, so a fault storm degrades VT-d handling, never the
//! governance ledger. The VT-d state lock is a leaf `SwitchMutex` (never nested
//! under the ordered AI band, whose ceiling is `USB_CORE=31`).
//!
//! **Kill-switch (ISC-6).** Compiled only under `vtd`; `vtd`-off ⇒ none of this
//! exists ⇒ bit-for-bit pre-VT-d. Within a `vtd` build, `disable()` clears
//! Translation-Enable on every unit, returning to physical DMA (the runtime
//! kill-switch form the dispatch names).
//!
//! Spec authority: VT-d spec §9 (translation structures), §10.4 (registers),
//! §6 (the enable + invalidation sequence). Cross-checked against Linux
//! `drivers/iommu/intel/iommu.c` (`iommu_enable_translation`,
//! `domain_context_mapping`, `iommu_flush_*`).

use alloc::vec::Vec;

use x86_64::PhysAddr;

use crate::drivers::pci::{self, IommuClass};
use crate::mm;
use crate::sync::SwitchMutex;

pub mod vtd_logic;
mod vtd_regs;

use vtd_logic as logic;
use vtd_regs as regs;

/// Domain identifiers. The substrate shares one identity domain; connectivity
/// shares one fence domain; the iGPU has its own graphics domain (OT-63 Slice
/// 5). (Per-device domains are otherwise unnecessary.)
const DID_IDENTITY: u16 = 1;
const DID_FENCE: u16 = 2;
/// OT-63 GPU P1 Slice 5 (ADR-077 §Slice-5 + amendment 5): the iGPU's own
/// translated domain, mapping the named graphics arena (all GPU-reachable
/// allocations) and nothing else — I-14(c2) for the graphics device.
const DID_GRAPHICS: u16 = 3;

/// Bound on the busy-poll loops waiting for a register status bit, so broken or
/// absent hardware fails safe (we abort enabling, leaving DMA physical) instead
/// of hanging the boot.
const SPIN_MAX: u32 = 50_000_000;

/// Bound on fault records drained per call — the I-14(b) liveness cap.
const MAX_FAULTS_PER_DRAIN: u16 = 64;

const ONE_GIB: u64 = 1 << 30;

// ── discriminator surface (atomics, like the arena's) ───────────────────────────
use core::sync::atomic::{AtomicU64, Ordering};
static FAULTS_RECORDED: AtomicU64 = AtomicU64::new(0);
static LAST_FAULT_ADDR: AtomicU64 = AtomicU64::new(0);
static LAST_FAULT_SID: AtomicU64 = AtomicU64::new(u64::MAX);

/// A second-level translation domain: the physical address of its page-table
/// root (PML4 for 4-level, PDPT for 3-level) and its domain id.
#[derive(Copy, Clone)]
struct Domain {
    root_phys: u64,
    id: u16,
}

/// One DRHD remapping unit: its register accessor, capability snapshots, and the
/// GCMD shadow (the persistent command bits we must preserve across writes).
struct Unit {
    regs: regs::RemapRegs,
    cap: u64,
    ecap: u64,
    gcmd: u32,
}

struct VtdState {
    units: Vec<Unit>,
    root_table_phys: u64,
    agaw: logic::AgawChoice,
    coherent: bool,
    identity: Domain,
    fence: Option<Domain>,
    /// OT-63 Slice 5: the iGPU's translated domain over the graphics arena,
    /// built on first use by `fence_graphics_source_id`.
    graphics: Option<Domain>,
    /// Source-ids classified connectivity (USB controllers, NICs) — the set
    /// `fence_connectivity()` narrows to the fence domain (ISC-4).
    connectivity_sids: Vec<u16>,
    /// Source-ids classified display/graphics (the iGPU) — the set
    /// `narrow_graphics()` moves to the graphics domain (Slice 5).
    graphics_sids: Vec<u16>,
}

static STATE: SwitchMutex<Option<VtdState>> = SwitchMutex::new(None);

// ── frame + table helpers ────────────────────────────────────────────────────────

/// Allocate one zeroed 4 KiB frame; returns its physical address.
fn alloc_zeroed_frame() -> Option<u64> {
    // order 0 = a single 4 KiB block (inherent buddy method, like dma_arena).
    let frame = mm::with_frame_alloc(|fa| fa.allocate_blocks(0))?;
    let phys = frame.start_address().as_u64();
    unsafe {
        let virt = mm::phys_to_virt(PhysAddr::new(phys));
        core::ptr::write_bytes(virt.as_mut_ptr::<u8>(), 0, 4096);
    }
    Some(phys)
}

/// Write a 64-bit table slot (`index` in u64 units) and, on a non-coherent
/// unit, flush the line so the hardware table-walk sees it (ECAP.C handling).
fn write_slot(table_phys: u64, index: usize, value: u64, coherent: bool) {
    let virt = mm::phys_to_virt(PhysAddr::new(table_phys));
    let ptr = (virt.as_u64() as usize + index * 8) as *mut u64;
    unsafe {
        core::ptr::write_volatile(ptr, value);
        if !coherent {
            core::arch::x86_64::_mm_clflush(ptr as *const u8);
        }
    }
}

fn read_slot(table_phys: u64, index: usize) -> u64 {
    let virt = mm::phys_to_virt(PhysAddr::new(table_phys));
    let ptr = (virt.as_u64() as usize + index * 8) as *const u64;
    unsafe { core::ptr::read_volatile(ptr) }
}

// ── domain construction ──────────────────────────────────────────────────────────

/// Build an identity domain (GPA==HPA) covering the low 512 GiB with 1 GiB
/// second-level super-pages. 3-level: the root IS the PDPT (512 × 1 GiB =
/// 512 GiB). 4-level: PML4[0] → one PDPT (512 × 1 GiB). Requires 1 GiB
/// super-page support (checked by the caller); covers any realistic machine.
fn build_identity_domain(agaw: logic::AgawChoice, coherent: bool) -> Option<Domain> {
    let root = alloc_zeroed_frame()?;
    let pdpt = if agaw.levels == 3 {
        root
    } else {
        let pdpt = alloc_zeroed_frame()?;
        write_slot(root, 0, logic::sl_dir(pdpt), coherent);
        pdpt
    };
    for i in 0..logic::SL_ENTRIES_PER_TABLE {
        let pte = logic::sl_leaf_large((i as u64) * ONE_GIB, true, true);
        write_slot(pdpt, i, pte, coherent);
    }
    Some(Domain {
        root_phys: root,
        id: DID_IDENTITY,
    })
}

/// Build a translated domain mapping ONLY `[base, base+len)`, identity within
/// that range, 4 KiB leaves, tagged `did`. Every address outside the range is
/// not-present ⇒ a device DMA there faults — the I-14(c2) hardware guard. The
/// connectivity fence domain (`DID_FENCE`, over the connectivity DMA arena) and
/// the iGPU graphics domain (`DID_GRAPHICS`, over the graphics arena) are the
/// same construction over disjoint ranges — the amendment-5 isolation is that
/// each domain maps only its own arena.
fn build_translated_domain(
    base: u64,
    len: u64,
    did: u16,
    agaw: logic::AgawChoice,
    coherent: bool,
) -> Option<Domain> {
    let root = alloc_zeroed_frame()?;
    let mut addr = base;
    let end = base.checked_add(len)?;
    while addr < end {
        map_4k(root, agaw.levels, addr, addr, coherent)?;
        addr = addr.checked_add(4096)?;
    }
    Some(Domain {
        root_phys: root,
        id: did,
    })
}

/// Map one 4 KiB page `iova → phys` (R+W) in a second-level table, creating
/// intermediate tables as needed. `levels` is 3 or 4 (the AGAW choice).
fn map_4k(root: u64, levels: u8, iova: u64, phys: u64, coherent: bool) -> Option<()> {
    let indices = [
        logic::sl_pml4_index(iova),
        logic::sl_pdpt_index(iova),
        logic::sl_pd_index(iova),
        logic::sl_pt_index(iova),
    ];
    // 3-level walks PDPT→PD→PT (skip PML4); 4-level walks PML4→PDPT→PD→PT.
    let start = if levels == 4 { 0 } else { 1 };
    let walk = &indices[start..];
    let mut table = root;
    for &idx in &walk[..walk.len() - 1] {
        let entry = read_slot(table, idx);
        let next = if entry & (logic::SL_READ | logic::SL_WRITE) != 0 {
            entry & logic::SL_ADDR_MASK
        } else {
            let frame = alloc_zeroed_frame()?;
            write_slot(table, idx, logic::sl_dir(frame), coherent);
            frame
        };
        table = next;
    }
    let leaf_idx = walk[walk.len() - 1];
    write_slot(table, leaf_idx, logic::sl_leaf(phys, true, true), coherent);
    Some(())
}

// ── root / context tables ────────────────────────────────────────────────────────

/// Set the context entry for `sid` to point at `domain`. Allocates the bus's
/// context table (and its root entry) on first use. `bus_ctx` caches
/// bus → context-table-phys so each bus's context table is shared.
fn set_context(
    root_table_phys: u64,
    bus_ctx: &mut Vec<(u8, u64)>,
    sid: u16,
    domain: &Domain,
    agaw: logic::AgawChoice,
    coherent: bool,
) -> Option<()> {
    let bus = logic::source_id_bus(sid);
    let devfn = logic::source_id_devfn(sid) as usize;

    let ctx_phys = match bus_ctx.iter().find(|(b, _)| *b == bus) {
        Some((_, p)) => *p,
        None => {
            let ctx = alloc_zeroed_frame()?;
            // Point the root entry for this bus at the new context table.
            let re = logic::root_entry(ctx, true);
            write_slot(root_table_phys, (bus as usize) * 2, re[0], coherent);
            write_slot(root_table_phys, (bus as usize) * 2 + 1, re[1], coherent);
            bus_ctx.push((bus, ctx));
            ctx
        }
    };

    let ce = logic::context_entry(domain.root_phys, domain.id, agaw.aw_field, true);
    write_slot(ctx_phys, devfn * 2, ce[0], coherent);
    write_slot(ctx_phys, devfn * 2 + 1, ce[1], coherent);
    Some(())
}

// ── register sequences ───────────────────────────────────────────────────────────

/// Spin until `(read32(reg) & mask) == want_set`, bounded. Returns false on
/// timeout (fail-safe).
fn wait_status(unit: &Unit, reg: usize, mask: u32, set: bool) -> bool {
    for _ in 0..SPIN_MAX {
        let v = unit.regs.read32(reg);
        if ((v & mask) != 0) == set {
            return true;
        }
        core::hint::spin_loop();
    }
    false
}

/// Global context-cache invalidation (V6: required after every context change,
/// especially under CAP.CM).
fn invalidate_context_global(unit: &Unit) {
    unit.regs
        .write64(regs::REG_CCMD, regs::CCMD_ICC | regs::CCMD_CIRG_GLOBAL);
    for _ in 0..SPIN_MAX {
        if unit.regs.read64(regs::REG_CCMD) & regs::CCMD_ICC == 0 {
            break;
        }
        core::hint::spin_loop();
    }
}

/// Global IOTLB invalidation (V6).
fn invalidate_iotlb_global(unit: &Unit) {
    let off = (logic::ecap_iotlb_offset(unit.ecap) + regs::IOTLB_REG_OFFSET) as usize;
    unit.regs
        .write64(off, regs::IOTLB_IVT | regs::IOTLB_IIRG_GLOBAL);
    for _ in 0..SPIN_MAX {
        if unit.regs.read64(off) & regs::IOTLB_IVT == 0 {
            break;
        }
        core::hint::spin_loop();
    }
}

/// Set the root table pointer and enable translation on one unit. Returns
/// whether GSTS.TES (the V3 discriminator) came up.
fn enable_unit(unit: &mut Unit, root_table_phys: u64) -> bool {
    // Mask the fault-event interrupt: BUILD-V drains faults by polling
    // (`poll_faults`); the no-MSI path keeps the unit self-contained.
    unit.regs.write32(regs::REG_FECTL, regs::FECTL_IM);

    // Program RTADDR (legacy root table type) and pulse Set-Root-Table-Pointer.
    unit.regs.write64(
        regs::REG_RTADDR,
        (root_table_phys & logic::ADDR_4K_MASK) | regs::RTADDR_RTT_LEGACY,
    );
    unit.regs
        .write32(regs::REG_GCMD, unit.gcmd | regs::GCMD_SRTP);
    if !wait_status(unit, regs::REG_GSTS, regs::GSTS_RTPS, true) {
        return false;
    }

    // Flush stale context/IOTLB before enabling (V6).
    invalidate_context_global(unit);
    invalidate_iotlb_global(unit);

    // Enable translation (persistent TE bit kept in the shadow).
    unit.gcmd |= regs::GCMD_TE;
    unit.regs.write32(regs::REG_GCMD, unit.gcmd);
    wait_status(unit, regs::REG_GSTS, regs::GSTS_TES, true)
}

// ── fault processing (I-14 (a) no AI lock, (b) bounded) ──────────────────────────

/// Drain recorded faults on one unit, recording the most recent
/// (source-id, faulting address) into the discriminator atomics and clearing
/// each record (RW1C). Bounded by `MAX_FAULTS_PER_DRAIN`. Acquires no
/// AI-governance lock. Returns the number of faults handled.
fn process_faults(unit: &Unit) -> u32 {
    let fro = logic::cap_fault_recording_offset(unit.cap);
    let nfr = logic::cap_num_fault_recording(unit.cap).min(MAX_FAULTS_PER_DRAIN);
    let mut handled = 0u32;
    for i in 0..nfr {
        let off = (fro + (i as u64) * 16) as usize;
        let low = unit.regs.read64(off);
        let high = unit.regs.read64(off + 8);
        let rec = logic::decode_fault_record(low, high);
        if !rec.fault {
            continue;
        }
        FAULTS_RECORDED.fetch_add(1, Ordering::Relaxed);
        LAST_FAULT_ADDR.store(rec.fault_addr, Ordering::Relaxed);
        LAST_FAULT_SID.store(rec.source_id as u64, Ordering::Relaxed);
        // RW1C the fault bit to clear the record.
        unit.regs.write64(off + 8, logic::FAULT_RECORD_CLEAR_F);
        handled += 1;
    }
    // Clear the primary fault status bits (RW1C).
    let fsts = unit.regs.read32(regs::REG_FSTS);
    if fsts & (regs::FSTS_PPF | regs::FSTS_PFO) != 0 {
        unit.regs
            .write32(regs::REG_FSTS, fsts & (regs::FSTS_PPF | regs::FSTS_PFO));
    }
    handled
}

// ── public API ───────────────────────────────────────────────────────────────────

/// Bring up VT-d: parse cached DMAR, map each unit, identity-domain the whole
/// PCI device set, enable translation. Returns whether translation engaged on
/// at least one unit. Fails SAFE: any missing capability (no DMAR, no 1 GiB
/// super-pages, unsupported AGAW) logs a conclusive verdict and leaves DMA
/// physical (the substrate is unaffected — never a regression).
pub fn init() -> bool {
    if STATE.lock().is_some() {
        return enabled();
    }

    // V1: the DMAR must exist (else "enable VT-d in BIOS").
    let Some((drhd_bases, host_aw)) =
        crate::acpi::with_dmar(|info| (info.drhd_bases.clone(), info.host_addr_width))
    else {
        crate::serial_println!("[ kernel ] vtd: no DMAR — VT-d not available, DMA stays physical");
        return false;
    };
    if drhd_bases.is_empty() {
        crate::serial_println!("[ kernel ] vtd: DMAR has no DRHD units");
        return false;
    }

    // Map each unit's register block (uncacheable) and snapshot CAP/ECAP.
    let mut units: Vec<Unit> = Vec::new();
    for base in &drhd_bases {
        let virt = mm::map_kernel_mmio_page(PhysAddr::new(*base));
        let unit = unsafe { regs::RemapRegs::new(virt) };
        let cap = unit.read64(regs::REG_CAP);
        let ecap = unit.read64(regs::REG_ECAP);
        crate::serial_println!(
            "[ kernel ] vtd: unit base={:#x} cap={:#x} ecap={:#x} sagaw={:#x} mgaw={} sllps={:#x} cm={} coherent={}",
            base,
            cap,
            ecap,
            logic::cap_sagaw(cap),
            logic::cap_mgaw_bits(cap),
            logic::cap_sllps(cap),
            logic::cap_caching_mode(cap),
            logic::ecap_coherent(ecap),
        );
        units.push(Unit {
            regs: unit,
            cap,
            ecap,
            gcmd: 0,
        });
    }

    let cap0 = units[0].cap;
    let coherent = logic::ecap_coherent(units[0].ecap);

    // V7: pick the adjusted address width from CAP.SAGAW.
    let Some(agaw) = logic::select_agaw(logic::cap_sagaw(cap0)) else {
        crate::serial_println!(
            "[ kernel ] vtd: no supported AGAW (sagaw={:#x}) — not enabling (V7)",
            logic::cap_sagaw(cap0)
        );
        return false;
    };
    // The identity domain uses 1 GiB super-pages; require them.
    if !logic::cap_supports_1g_page(cap0) {
        crate::serial_println!(
            "[ kernel ] vtd: 1 GiB super-pages unsupported (sllps={:#x}) — not enabling (substrate unaffected)",
            logic::cap_sllps(cap0)
        );
        return false;
    }
    crate::serial_println!(
        "[ kernel ] vtd: agaw levels={} aw_field={:#x} addr_width={} host_aw={}",
        agaw.levels,
        agaw.aw_field,
        agaw.addr_width,
        host_aw
    );

    // Build the identity domain and the root table; place every enumerated
    // device into identity (ISC-3 no-regression order: substrate first).
    let Some(identity) = build_identity_domain(agaw, coherent) else {
        crate::serial_println!("[ kernel ] vtd: identity-domain alloc failed");
        return false;
    };
    let Some(root_table_phys) = alloc_zeroed_frame() else {
        crate::serial_println!("[ kernel ] vtd: root-table alloc failed");
        return false;
    };

    let devices = pci::enumerate_all();
    let mut bus_ctx: Vec<(u8, u64)> = Vec::new();
    let mut connectivity_sids: Vec<u16> = Vec::new();
    let mut graphics_sids: Vec<u16> = Vec::new();
    let mut placed = 0u32;
    for dev in &devices {
        let sid = logic::source_id(dev.bus, dev.device, dev.function);
        if set_context(
            root_table_phys,
            &mut bus_ctx,
            sid,
            &identity,
            agaw,
            coherent,
        )
        .is_some()
        {
            placed += 1;
            match pci::iommu_class(*dev) {
                IommuClass::Connectivity => connectivity_sids.push(sid),
                IommuClass::Display => graphics_sids.push(sid),
                IommuClass::Substrate => {}
            }
        }
    }
    crate::serial_println!(
        "[ kernel ] vtd: identity-domained {} devices across {} buses ({} connectivity, {} graphics)",
        placed,
        bus_ctx.len(),
        connectivity_sids.len(),
        graphics_sids.len()
    );

    // Order writes before enabling (the table edits must be globally visible).
    core::sync::atomic::fence(Ordering::SeqCst);

    let mut any_enabled = false;
    let mut units_mut = units;
    for unit in units_mut.iter_mut() {
        let ok = enable_unit(unit, root_table_phys);
        any_enabled |= ok;
        crate::serial_println!("[ kernel ] vtd: unit base set, TES={} (V3)", ok);
    }

    *STATE.lock() = Some(VtdState {
        units: units_mut,
        root_table_phys,
        agaw,
        coherent,
        identity,
        fence: None,
        graphics: None,
        connectivity_sids,
        graphics_sids,
    });

    if any_enabled {
        crate::serial_println!("[ kernel ] vtd: translation ENABLED (identity, no regression)");
    } else {
        crate::serial_println!("[ kernel ] vtd: translation did NOT enable (TES never set)");
    }
    any_enabled
}

/// Whether translation is enabled on at least one unit (GSTS.TES — V3).
pub fn enabled() -> bool {
    let guard = STATE.lock();
    let Some(state) = guard.as_ref() else {
        return false;
    };
    state
        .units
        .iter()
        .any(|u| u.regs.read32(regs::REG_GSTS) & regs::GSTS_TES != 0)
}

/// Move a device (by source-id) into the fence domain (arena-only) and
/// invalidate the context cache (V6). Builds the fence domain on first use from
/// the current DMA-arena bounds; returns false if no arena is registered or
/// VT-d is not up. The (c2) test hook: after this, the device's DMA outside the
/// arena faults.
pub fn fence_source_id(sid: u16) -> bool {
    let mut guard = STATE.lock();
    let Some(state) = guard.as_mut() else {
        return false;
    };
    // Ensure the fence domain exists (needs the arena initialized).
    if state.fence.is_none() {
        let Some((base, len)) = crate::drivers::dma_arena::bounds() else {
            crate::serial_println!("[ kernel ] vtd: fence requested but arena unset");
            return false;
        };
        match build_translated_domain(base, len, DID_FENCE, state.agaw, state.coherent) {
            Some(d) => {
                crate::serial_println!(
                    "[ kernel ] vtd: fence domain built over arena [{:#x},{:#x})",
                    base,
                    base + len
                );
                state.fence = Some(d);
            }
            None => return false,
        }
    }
    let fence = state.fence.unwrap();
    repoint(state, sid, &fence)
}

/// Narrow every connectivity-class device (USB host controllers, NICs) to the
/// fence domain (ISC-4 — the production path; the classifier decides). Requires
/// the DMA arena to be initialized. Returns the count fenced. The scenario also
/// uses `fence_source_id` to fence a specific controllable device for the (c2)
/// proof.
pub fn fence_connectivity() -> usize {
    // Snapshot the sids and drop the lock before re-locking in fence_source_id
    // (SwitchMutex is not reentrant).
    let sids: Vec<u16> = match STATE.lock().as_ref() {
        Some(s) => s.connectivity_sids.clone(),
        None => return 0,
    };
    let mut count = 0;
    for sid in sids {
        if fence_source_id(sid) {
            count += 1;
        }
    }
    count
}

/// OT-63 Slice 5: move a device (the iGPU, by source-id) into the GRAPHICS
/// domain — its own translated domain over the named graphics arena (all
/// GPU-reachable allocations, and nothing else), and invalidate the context
/// cache (V6). Builds the graphics domain on first use from the current
/// graphics-arena bounds; returns false if no graphics arena is registered or
/// VT-d is not up. After this, the device's DMA outside the graphics arena
/// faults — the I-14(c2) hardware guard for the iGPU (the falsifiable negative
/// control). Mirrors `fence_source_id`, over the graphics arena + `DID_GRAPHICS`.
pub fn fence_graphics_source_id(sid: u16) -> bool {
    let mut guard = STATE.lock();
    let Some(state) = guard.as_mut() else {
        return false;
    };
    // Ensure the graphics domain exists (needs the graphics arena initialized).
    if state.graphics.is_none() {
        let Some((base, len)) = crate::drivers::graphics_arena::bounds() else {
            crate::serial_println!(
                "[ kernel ] vtd: graphics fence requested but graphics arena unset"
            );
            return false;
        };
        match build_translated_domain(base, len, DID_GRAPHICS, state.agaw, state.coherent) {
            Some(d) => {
                crate::serial_println!(
                    "[ kernel ] vtd: graphics domain built over graphics arena [{:#x},{:#x})",
                    base,
                    base + len
                );
                state.graphics = Some(d);
            }
            None => return false,
        }
    }
    let graphics = state.graphics.unwrap();
    repoint(state, sid, &graphics)
}

/// OT-63 Slice 5 production path: narrow every display/graphics-class device
/// (the iGPU) to the graphics domain — the analogue of `fence_connectivity`.
/// Gated by the Slice-5 narrowing kill-switch (default OFF ⇒ this is a no-op
/// and the iGPU stays identity-domained, bit-for-bit Slice-4). Requires the
/// graphics arena to be initialized (enforced by `set_narrowing`). Returns the
/// count narrowed.
pub fn narrow_graphics() -> usize {
    if !crate::drivers::graphics_arena::narrowing_enabled() {
        return 0; // kill-switch OFF: iGPU stays in the identity domain (≡ Slice-4)
    }
    // Snapshot the sids and drop the lock before re-locking in
    // fence_graphics_source_id (SwitchMutex is not reentrant).
    let sids: Vec<u16> = match STATE.lock().as_ref() {
        Some(s) => s.graphics_sids.clone(),
        None => return 0,
    };
    let mut count = 0;
    for sid in sids {
        if fence_graphics_source_id(sid) {
            count += 1;
        }
    }
    if count > 0 {
        // I-14(c): containment is now a hardware fact, not just policy. This
        // is the ONLY writer of the armed flag the accel probe and the
        // GPU-allocation chokepoint gate on.
        crate::drivers::graphics_arena::mark_domain_armed();
    }
    count
}

/// Restore a device to the identity domain (+ context invalidation).
pub fn identity_source_id(sid: u16) -> bool {
    let mut guard = STATE.lock();
    let Some(state) = guard.as_mut() else {
        return false;
    };
    let identity = state.identity;
    repoint(state, sid, &identity)
}

fn repoint(state: &mut VtdState, sid: u16, domain: &Domain) -> bool {
    let mut bus_ctx: Vec<(u8, u64)> = Vec::new();
    // Re-derive the bus's existing context table by reading the root entry.
    let bus = logic::source_id_bus(sid);
    let re_low = read_slot(state.root_table_phys, (bus as usize) * 2);
    if re_low & logic::ROOT_PRESENT == 0 {
        // Bus had no context table; create one (the device was never placed).
        if set_context(
            state.root_table_phys,
            &mut bus_ctx,
            sid,
            domain,
            state.agaw,
            state.coherent,
        )
        .is_none()
        {
            return false;
        }
    } else {
        let ctx_phys = re_low & logic::ADDR_4K_MASK;
        let devfn = logic::source_id_devfn(sid) as usize;
        let ce = logic::context_entry(domain.root_phys, domain.id, state.agaw.aw_field, true);
        write_slot(ctx_phys, devfn * 2, ce[0], state.coherent);
        write_slot(ctx_phys, devfn * 2 + 1, ce[1], state.coherent);
    }
    core::sync::atomic::fence(Ordering::SeqCst);
    for unit in &state.units {
        invalidate_context_global(unit);
        invalidate_iotlb_global(unit);
    }
    true
}

/// Drain faults across all units (poll-driven; a production fault MSI would
/// call the same bounded path). Returns the total handled this call.
pub fn poll_faults() -> u32 {
    let guard = STATE.lock();
    let Some(state) = guard.as_ref() else {
        return 0;
    };
    let mut total = 0u32;
    for unit in &state.units {
        total += process_faults(unit);
    }
    total
}

/// Runtime kill-switch: clear Translation-Enable on every unit, returning to
/// physical DMA (≡ pre-VT-d). The dispatch's "translation never enabled" form.
pub fn disable() {
    let mut guard = STATE.lock();
    let Some(state) = guard.as_mut() else {
        return;
    };
    for unit in state.units.iter_mut() {
        unit.gcmd &= !regs::GCMD_TE;
        unit.regs.write32(regs::REG_GCMD, unit.gcmd);
        wait_status(unit, regs::REG_GSTS, regs::GSTS_TES, false);
    }
    crate::serial_println!("[ kernel ] vtd: translation DISABLED (kill-switch)");
}

/// The arena range the fence domain covers, if built.
pub fn fence_bounds() -> Option<(u64, u64)> {
    crate::drivers::dma_arena::bounds()
}

// ── discriminator accessors (Rule 5: gate-consumed, never dead) ──────────────────
pub fn faults_recorded() -> u64 {
    FAULTS_RECORDED.load(Ordering::Relaxed)
}
pub fn last_fault_addr() -> u64 {
    LAST_FAULT_ADDR.load(Ordering::Relaxed)
}
pub fn last_fault_source_id() -> u16 {
    LAST_FAULT_SID.load(Ordering::Relaxed) as u16
}
pub fn reset_fault_counters() {
    FAULTS_RECORDED.store(0, Ordering::Relaxed);
    LAST_FAULT_ADDR.store(0, Ordering::Relaxed);
    LAST_FAULT_SID.store(u64::MAX, Ordering::Relaxed);
}

/// Whether a DMAR table was found (VT-d exposed by the platform).
pub fn available() -> bool {
    crate::acpi::dmar_available()
}
