// SPDX-License-Identifier: MPL-2.0
//! Pure DMAR (DMA Remapping Reporting) ACPI-table parsing logic — Phase 7.5
//! BUILD-V (ISC-1). Kernel-dependency-free so the host suite
//! (`tools/ai-ops-tests`) can path-`mod` it and assert the remapping-structure
//! walk and the DRHD/RMRR field extraction against a **hand-laid DMAR byte
//! example** (I-12 / the format-ABI byte-oracle standing rule), independent of
//! the kernel's `phys_to_virt` plumbing in `acpi/dmar.rs`.
//!
//! Like `dma_arena_logic.rs`, the parse is **total**: every truncated field or
//! out-of-bounds structure is an explicit `None`/iteration-stop, never a panic
//! and never a wrap. A malformed firmware table degrades VT-d, it must never
//! fault the kernel (I-14(b) liveness posture at the parse layer too).
//!
//! **Spec authority.** Intel VT-d spec §8 "BIOS Considerations" / the ACPI
//! DMAR table: §8.1 (DMAR header: Host Address Width + Flags), §8.3 (DRHD:
//! flags, segment, register base), §8.4 (RMRR: segment, base, limit). Layout
//! cross-checked against Linux `drivers/iommu/intel/dmar.c`
//! (`dmar_walk_remapping_entries`, `struct acpi_dmar_hardware_unit`,
//! `struct acpi_dmar_reserved_memory`).

/// Remapping-structure type codes (DMAR §8.2, the `type` u16 of each entry).
pub const DMAR_TYPE_DRHD: u16 = 0;
pub const DMAR_TYPE_RMRR: u16 = 1;
pub const DMAR_TYPE_ATSR: u16 = 2;
pub const DMAR_TYPE_RHSA: u16 = 3;
pub const DMAR_TYPE_ANDD: u16 = 4;

/// Bytes from the start of the DMAR table to the first remapping structure:
/// the 36-byte standard ACPI SDT header + 1 byte Host Address Width + 1 byte
/// Flags + 10 reserved bytes = 48.
pub const DMAR_HEADER_LEN: usize = 48;

/// The 4-byte common header on every remapping structure: type(u16) + length(u16).
pub const REMAP_STRUCT_HEADER_LEN: usize = 4;

/// DRHD flags bit 0 — INCLUDE_PCI_ALL: this remapping unit covers every PCIe
/// device in its segment not claimed by an explicit device-scope of another
/// unit. (The common single-unit desktop case — the OptiPlex.)
pub const DRHD_FLAG_INCLUDE_PCI_ALL: u8 = 1 << 0;

/// DMAR-header Flags bit 0 — INTR_REMAP (interrupt remapping supported by the
/// platform). BUILD-V is DMA-remapping-only and never sets interrupt remapping,
/// but the bit is surfaced for the V1/log breadcrumb.
pub const DMAR_FLAG_INTR_REMAP: u8 = 1 << 0;

// ── total little-endian readers (None on truncation; never panic) ───────────────

fn rd_u8(b: &[u8], off: usize) -> Option<u8> {
    b.get(off).copied()
}
fn rd_u16(b: &[u8], off: usize) -> Option<u16> {
    let lo = *b.get(off)? as u16;
    let hi = *b.get(off + 1)? as u16;
    Some(lo | (hi << 8))
}
fn rd_u32(b: &[u8], off: usize) -> Option<u32> {
    let lo = rd_u16(b, off)? as u32;
    let hi = rd_u16(b, off + 2)? as u32;
    Some(lo | (hi << 16))
}
fn rd_u64(b: &[u8], off: usize) -> Option<u64> {
    let lo = rd_u32(b, off)? as u64;
    let hi = rd_u32(b, off + 4)? as u64;
    Some(lo | (hi << 32))
}

/// The total length of the DMAR table, from the SDT header's `length` field
/// (offset 4). The parse must never read past this even if the buffer mapped is
/// larger.
pub fn sdt_length(table: &[u8]) -> Option<u32> {
    rd_u32(table, 4)
}

/// The DMAR Host Address Width field (table offset 36) = width in bits − 1
/// (so the reported host address width is `haw_field + 1`).
pub fn dmar_host_addr_width(table: &[u8]) -> Option<u8> {
    rd_u8(table, 36).map(|v| v + 1)
}

/// The DMAR Flags byte (table offset 37).
pub fn dmar_flags(table: &[u8]) -> Option<u8> {
    rd_u8(table, 37)
}

/// One remapping structure: its `kind` (type code) and its full bytes
/// (including the 4-byte header), so field extractors can index by spec offset.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct RemapStruct<'a> {
    pub kind: u16,
    pub bytes: &'a [u8],
}

/// Iterator over the DMAR remapping structures. Bounded by the SDT `length`
/// field and the buffer; stops on any truncated header, a zero/under-length
/// `length` (which would otherwise loop forever), or a structure that overruns
/// the table. Never panics.
pub struct DmarStructs<'a> {
    table: &'a [u8],
    cursor: usize,
    end: usize,
}

impl<'a> DmarStructs<'a> {
    /// Build an iterator over `table` (the full DMAR table bytes). The walk
    /// starts after the 48-byte fixed header and ends at `min(SDT.length, len)`.
    pub fn new(table: &'a [u8]) -> Self {
        let end = match sdt_length(table) {
            Some(len) => (len as usize).min(table.len()),
            None => 0,
        };
        DmarStructs {
            table,
            cursor: DMAR_HEADER_LEN,
            end,
        }
    }
}

impl<'a> Iterator for DmarStructs<'a> {
    type Item = RemapStruct<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        // Need at least a 4-byte header within bounds.
        if self.cursor + REMAP_STRUCT_HEADER_LEN > self.end {
            return None;
        }
        let kind = rd_u16(self.table, self.cursor)?;
        let length = rd_u16(self.table, self.cursor + 2)? as usize;
        // A length that can't even hold its header, or that overruns the table,
        // terminates the walk (fail-safe, never an infinite loop or OOB read).
        if length < REMAP_STRUCT_HEADER_LEN || self.cursor + length > self.end {
            return None;
        }
        let bytes = &self.table[self.cursor..self.cursor + length];
        self.cursor += length;
        Some(RemapStruct { kind, bytes })
    }
}

// ── DRHD field extractors (DMAR §8.3; offsets within the structure bytes) ────────
// type(0..2) length(2..4) flags(4) reserved(5) segment(6..8) register_base(8..16)
// [ device scope entries follow at offset 16 ]

/// DRHD flags byte (offset 4).
pub fn drhd_flags(s: &RemapStruct<'_>) -> Option<u8> {
    if s.kind != DMAR_TYPE_DRHD {
        return None;
    }
    rd_u8(s.bytes, 4)
}

/// DRHD PCI segment number (offset 6).
pub fn drhd_segment(s: &RemapStruct<'_>) -> Option<u16> {
    if s.kind != DMAR_TYPE_DRHD {
        return None;
    }
    rd_u16(s.bytes, 6)
}

/// DRHD register base address (offset 8) — the physical base of this remapping
/// unit's MMIO register block. The value `mod.rs` maps uncacheable.
pub fn drhd_register_base(s: &RemapStruct<'_>) -> Option<u64> {
    if s.kind != DMAR_TYPE_DRHD {
        return None;
    }
    rd_u64(s.bytes, 8)
}

/// Whether this DRHD has the INCLUDE_PCI_ALL flag (covers all devices in its
/// segment) — the desktop single-unit case.
pub fn drhd_include_pci_all(s: &RemapStruct<'_>) -> Option<bool> {
    drhd_flags(s).map(|f| f & DRHD_FLAG_INCLUDE_PCI_ALL != 0)
}

// ── RMRR field extractors (DMAR §8.4) ────────────────────────────────────────────
// type(0..2) length(2..4) reserved(4..6) segment(6..8) base(8..16) limit(16..24)
// [ device scope entries follow at offset 24 ]

/// RMRR PCI segment number (offset 6).
pub fn rmrr_segment(s: &RemapStruct<'_>) -> Option<u16> {
    if s.kind != DMAR_TYPE_RMRR {
        return None;
    }
    rd_u16(s.bytes, 6)
}

/// RMRR reserved-memory region base and limit (offsets 8 and 16). The region
/// `[base, limit]` (inclusive) MUST be identity-mapped for any device scoped to
/// it, or that device breaks the instant translation enables (V2 — the GPU
/// aperture / USB legacy region is the canary).
pub fn rmrr_base_limit(s: &RemapStruct<'_>) -> Option<(u64, u64)> {
    if s.kind != DMAR_TYPE_RMRR {
        return None;
    }
    let base = rd_u64(s.bytes, 8)?;
    let limit = rd_u64(s.bytes, 16)?;
    Some((base, limit))
}
