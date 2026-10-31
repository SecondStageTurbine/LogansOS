// SPDX-License-Identifier: MPL-2.0
//! Pure Intel VT-d (DMA-remapping) byte/arithmetic logic — Phase 7.5 BUILD-V,
//! the I-14(c2) hardware-fence arithmetic.
//!
//! This file is **kernel-dependency-free** so the host suite
//! (`tools/ai-ops-tests`) can path-`mod` it and assert every on-hardware table
//! layout — the Root entry, the Context entry, the second-level page-table
//! entry, the fault-recording-register decode — **byte-for-byte, independent
//! of the construction code** (the CLAUDE.md byte-oracle standing rule / I-12:
//! a VT-d table is an on-hardware byte format, so a hand-laid example must be
//! asserted against the builder, not merely round-tripped against a reader the
//! same author wrote). The kernel glue — MMIO register programming, frame
//! allocation for the table pages, the enable sequence, fault-ISR wiring —
//! lives in `mod.rs` and `vtd_regs.rs`.
//!
//! Mirrors the design of the GPU's PPGTT byte-format file
//! (`accel/intel_gen9_ppgtt.rs`): VT-d second-level paging is the **same class
//! of object** as the GPU's EPT-style PPGTT — a multi-level 512-entry page
//! table — so the structure here is deliberately parallel, with the VT-d
//! second-level entry's *different* bit layout (no x86 present/user bits;
//! validity **is** R|W) made explicit.
//!
//! **Spec authority.** Intel® Virtualization Technology for Directed I/O
//! Architecture Specification (VT-d spec), legacy (non-scalable) mode: §9.1
//! Root Entry; §9.3 Context Entry; §9.7 Second-Level Paging Entries; §10.4.2
//! Capability Register (CAP — MGAW [21:16], SAGAW [12:8], FRO [33:24], NFR
//! [47:40]); §11.4.7.1 Fault Recording Register layout. Cross-checked against
//! the Linux `drivers/iommu/intel/` driver (`iommu.c` context/root setup,
//! `dmar.c` CAP decode, the `context_set_address_width` / `agaw_to_level`
//! helpers).

// ── Root Entry (VT-d spec §9.1, legacy mode; 128-bit = [low, high]) ──────────────
// low : P(bit0) | reserved | CTP(bits 63:12, the 4 KiB-aligned context-table phys)
// high: reserved (0)

/// Root-entry Present bit (bit 0). A cleared Present root entry means the bus
/// has no devices behind translation; the hardware blocks/passes per GCMD.
pub const ROOT_PRESENT: u64 = 1 << 0;
/// Context-table-pointer / context-entry / root pointer address field: bits
/// [63:12], 4 KiB-aligned. (Page-aligned physical addresses only.)
pub const ADDR_4K_MASK: u64 = 0xffff_ffff_ffff_f000;

/// A Root entry pointing at the context table for one PCI bus. `[low, high]`,
/// stored little-endian (low qword first) — the byte oracle pins the layout.
pub const fn root_entry(context_table_phys: u64, present: bool) -> [u64; 2] {
    let p = if present { ROOT_PRESENT } else { 0 };
    [(context_table_phys & ADDR_4K_MASK) | p, 0]
}

// ── Context Entry (VT-d spec §9.3, legacy mode; 128-bit = [low, high]) ───────────
// low : P(bit0) | FPD(bit1) | T(bits 3:2) | reserved | SLPTPTR(bits 63:12)
// high: AW(bits 2:0) | ignored | DID(bits 23:8) | reserved
//
// T (Translation Type) = 00b for DMA remapping: untranslated requests are
// translated through second-level paging (SLPTPTR); translation/translated
// requests are blocked. FPD (Fault Processing Disable) = 0: faults ARE
// recorded — that is the whole point of the (c2) guard.

/// Context-entry Present bit (low bit 0).
pub const CTX_PRESENT: u64 = 1 << 0;
/// Context-entry Fault-Processing-Disable bit (low bit 1). We keep it CLEAR:
/// out-of-domain DMA must fault and be recorded (the (c2) discriminator).
pub const CTX_FPD: u64 = 1 << 1;

/// Address-Width field encodings (context-entry high bits [2:0], VT-d §9.3).
/// The value also selects the second-level page-walk level count.
pub const AW_30BIT_2LVL: u8 = 0b000;
pub const AW_39BIT_3LVL: u8 = 0b001;
pub const AW_48BIT_4LVL: u8 = 0b010;
pub const AW_57BIT_5LVL: u8 = 0b011;

/// A Context entry for one (device, function): present, fault-processing
/// enabled, translation-type 0 (second-level DMA remapping), pointing at the
/// second-level page-table root `slptptr_phys`, in domain `domain_id`, with
/// adjusted address width `aw` (one of the `AW_*` constants). `[low, high]`,
/// little-endian. The byte oracle pins this layout.
pub const fn context_entry(slptptr_phys: u64, domain_id: u16, aw: u8, present: bool) -> [u64; 2] {
    let p = if present { CTX_PRESENT } else { 0 };
    // T = 00 (legacy second-level translation), FPD = 0 (faults recorded).
    let low = (slptptr_phys & ADDR_4K_MASK) | p;
    let high = ((aw as u64) & 0x7) | (((domain_id as u64) & 0xffff) << 8);
    [low, high]
}

// ── Second-Level Paging Entries (VT-d spec §9.7; 64-bit) ─────────────────────────
// R(bit0) | W(bit1) | (X bit2, EMT 3:5, ignored) | PS(bit7 large page) |
// ADDR(bits 51:12). **There is no x86-style Present bit:** an entry is "present"
// iff (R|W) != 0. A zeroed entry is not-present. This is the load-bearing
// difference from the GPU PPGTT (which uses bit0=Present, bit1=R/W). Mapping
// `[iova, ...)` to the arena's physical range with R+W is the fence domain;
// mapping GPA==HPA with R+W is the identity domain.

/// Second-level entry Read access (bit 0).
pub const SL_READ: u64 = 1 << 0;
/// Second-level entry Write access (bit 1).
pub const SL_WRITE: u64 = 1 << 1;
/// Second-level entry Page-Size bit (bit 7) — a leaf large page at the PD
/// (2 MiB) or PDPT (1 GiB) level. BUILD-V maps 4 KiB pages only; provided for
/// the identity-domain large-page path and pinned by the oracle.
pub const SL_PAGE_SIZE: u64 = 1 << 7;
/// Second-level address field: bits [51:12] (52-bit physical, 4 KiB-aligned).
/// Our frames sit far below 2^39; the mask is the contract the oracle pins.
pub const SL_ADDR_MASK: u64 = 0x000f_ffff_ffff_f000;

/// A **leaf** second-level page-table entry mapping `phys` (4 KiB-aligned),
/// readable per `read`, writable per `write`. Validity is carried by R|W —
/// pass `read=false, write=false` for a not-present (blocking) entry.
pub const fn sl_leaf(phys: u64, read: bool, write: bool) -> u64 {
    let r = if read { SL_READ } else { 0 };
    let w = if write { SL_WRITE } else { 0 };
    (phys & SL_ADDR_MASK) | r | w
}

/// A leaf **large-page** second-level entry (PS bit set) mapping a 2 MiB / 1 GiB
/// region whose base is `phys`. Used by the identity domain to map the host
/// address space cheaply. R+W, present.
pub const fn sl_leaf_large(phys: u64, read: bool, write: bool) -> u64 {
    sl_leaf(phys, read, write) | SL_PAGE_SIZE
}

/// A **non-leaf** second-level entry pointing at the next-level table at
/// `next_table_phys`. Non-leaf entries are R+W (per-page access is enforced at
/// the leaf), PS clear. Equivalent shape to the PPGTT `dir_entry`, different
/// bits.
pub const fn sl_dir(next_table_phys: u64) -> u64 {
    (next_table_phys & SL_ADDR_MASK) | SL_READ | SL_WRITE
}

// ── Second-level virtual (IOVA) decomposition — 4-level / 48-bit ─────────────────
// iova[47:39] = PML4 index, [38:30] = PDPT, [29:21] = PD, [20:12] = PT,
// [11:0] = page offset. (3-level/39-bit drops the PML4 level; the PDPT becomes
// the root, exactly as in `select_agaw`.)

pub const fn sl_pml4_index(iova: u64) -> usize {
    ((iova >> 39) & 0x1ff) as usize
}
pub const fn sl_pdpt_index(iova: u64) -> usize {
    ((iova >> 30) & 0x1ff) as usize
}
pub const fn sl_pd_index(iova: u64) -> usize {
    ((iova >> 21) & 0x1ff) as usize
}
pub const fn sl_pt_index(iova: u64) -> usize {
    ((iova >> 12) & 0x1ff) as usize
}

/// Entries per 4 KiB second-level paging page (512 = 4096 / 8).
pub const SL_ENTRIES_PER_TABLE: usize = 512;

// ── Capability-register decode (VT-d spec §10.4.2) + AGAW selection (V7) ─────────

/// CAP.MGAW (bits [21:16]) → Maximum Guest Address Width in bits = field + 1.
/// The host address width the identity domain must cover end-to-end.
pub const fn cap_mgaw_bits(cap: u64) -> u8 {
    (((cap >> 16) & 0x3f) as u8) + 1
}

/// CAP.SAGAW (bits [12:8]) — the bitmask of Supported Adjusted Guest Address
/// Widths. bit1=39-bit(3-level), bit2=48-bit(4-level), bit3=57-bit(5-level).
pub const fn cap_sagaw(cap: u64) -> u8 {
    ((cap >> 8) & 0x1f) as u8
}

/// CAP.FRO (bits [33:24]) → byte offset of the first Fault-Recording Register
/// from the unit's register base (the field is in 16-byte units).
pub const fn cap_fault_recording_offset(cap: u64) -> u64 {
    ((cap >> 24) & 0x3ff) * 16
}

/// CAP.NFR (bits [47:40]) → Number of Fault-recording Registers = field + 1.
pub const fn cap_num_fault_recording(cap: u64) -> u16 {
    (((cap >> 40) & 0xff) as u16) + 1
}

/// CAP.CM (bit 7) — Caching Mode. When set (QEMU `intel-iommu,caching-mode=on`,
/// and some real units), the driver MUST explicitly invalidate context/IOTLB
/// after every mapping change — the V6 discriminator's reason to exist.
pub const fn cap_caching_mode(cap: u64) -> bool {
    (cap >> 7) & 1 != 0
}

/// CAP.SLLPS (bits [37:34]) — Second-Level Large Page Sizes bitmask:
/// bit0=21-bit(2 MiB), bit1=30-bit(1 GiB), bit2=39-bit(512 GiB), bit3=48-bit.
pub const fn cap_sllps(cap: u64) -> u8 {
    ((cap >> 34) & 0xf) as u8
}
/// Whether the unit supports 1 GiB second-level super-pages (CAP.SLLPS bit 1 /
/// CAP bit 35). The identity domain maps the host space with 1 GiB pages.
pub const fn cap_supports_1g_page(cap: u64) -> bool {
    (cap >> 35) & 1 != 0
}
/// Whether the unit supports 2 MiB second-level super-pages (CAP.SLLPS bit 0).
pub const fn cap_supports_2m_page(cap: u64) -> bool {
    (cap >> 34) & 1 != 0
}

/// ECAP.IRO (bits [17:8]) → byte offset of the IOTLB invalidate registers from
/// the unit's register base (16-byte units). The IOTLB register sits at
/// `IRO*16 + 8`, the Invalidate-Address register at `IRO*16`.
pub const fn ecap_iotlb_offset(ecap: u64) -> u64 {
    ((ecap >> 8) & 0x3ff) * 16
}

/// ECAP.C (bit 0) — Coherency. When set, the unit's table walks are coherent
/// with the CPU cache and software need NOT flush after editing the
/// root/context/second-level tables. When CLEAR (some real client parts),
/// software MUST `clflush` every modified table cache line or the unit reads
/// stale entries — a documented field trap pre-seeded here for the OptiPlex.
pub const fn ecap_coherent(ecap: u64) -> bool {
    ecap & 1 != 0
}

/// The address-width choice for a domain: the page-walk `levels`, the
/// context-entry `aw_field`, and the covered `addr_width` in bits.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct AgawChoice {
    pub levels: u8,
    pub aw_field: u8,
    pub addr_width: u8,
}

/// Pick the adjusted guest address width from CAP.SAGAW, preferring 48-bit
/// (4-level) when supported, else 39-bit (3-level). Returns `None` if the unit
/// supports neither (a V7 "width-mismatch" conclusive failure — the dispatch's
/// width discriminator). 57-bit/5-level is deliberately not selected: our
/// physical memory is far below 2^48, so 4-level always suffices and keeps the
/// walk shorter.
pub const fn select_agaw(sagaw: u8) -> Option<AgawChoice> {
    if sagaw & (1 << 2) != 0 {
        Some(AgawChoice {
            levels: 4,
            aw_field: AW_48BIT_4LVL,
            addr_width: 48,
        })
    } else if sagaw & (1 << 1) != 0 {
        Some(AgawChoice {
            levels: 3,
            aw_field: AW_39BIT_3LVL,
            addr_width: 39,
        })
    } else {
        None
    }
}

// ── Source-ID (VT-d spec §3.4; the requester id used to index root/context) ──────

/// The VT-d **source-id** = (bus << 8) | (device << 3) | function. This is the
/// requester-id the hardware uses to walk Root[bus] → Context[dev:func].
/// (Distinct from the kernel's internal `pack_bdf` in `ahci.rs`, which uses a
/// (bus<<16)|(dev<<8)|func packing for bookkeeping — VT-d needs THIS encoding.)
pub const fn source_id(bus: u8, device: u8, function: u8) -> u16 {
    ((bus as u16) << 8) | (((device as u16) & 0x1f) << 3) | ((function as u16) & 0x7)
}

/// Bus index out of a source-id (the Root-table index).
pub const fn source_id_bus(sid: u16) -> u8 {
    (sid >> 8) as u8
}
/// Device:function index out of a source-id (the Context-table index, 0..=255).
pub const fn source_id_devfn(sid: u16) -> u8 {
    (sid & 0xff) as u8
}

// ── Fault Recording Register decode (VT-d spec §11.4.7.1; the (c2) evidence) ─────
// 128-bit register: low qword = FI (faulting address, bits 63:12); high qword =
// F(bit63) | T(bit62, 0=write/1=read) | AT(61:60) | ... | FR(bits 39:32, fault
// reason) | SID(bits 15:0, source-id). Decoding it tells the ISR exactly which
// device DMA'd which out-of-arena address — the conclusive (c2) discriminator
// and the V4/V5 evidence.

/// A decoded fault-recording register: did a fault occur, was it a read,
/// the fault reason code, the offending device's source-id, and the faulting
/// (4 KiB-aligned) address.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct FaultRecord {
    pub fault: bool,
    pub is_read: bool,
    pub reason: u8,
    pub source_id: u16,
    pub fault_addr: u64,
}

/// Decode a fault-recording register from its `(low, high)` 64-bit halves.
pub const fn decode_fault_record(low: u64, high: u64) -> FaultRecord {
    FaultRecord {
        fault: (high >> 63) & 1 != 0,
        is_read: (high >> 62) & 1 != 0,
        reason: ((high >> 32) & 0xff) as u8,
        source_id: (high & 0xffff) as u16,
        fault_addr: low & ADDR_4K_MASK,
    }
}

/// The high-qword write that **clears** a recorded fault: writing 1 to the
/// F bit (bit 63) clears it (RW1C). The ISR writes this back after recording.
pub const FAULT_RECORD_CLEAR_F: u64 = 1 << 63;
