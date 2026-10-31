// SPDX-License-Identifier: MPL-2.0
//! Intel VT-d remapping-unit MMIO register block — Phase 7.5 BUILD-V. Register
//! offsets, command/status bit definitions, and a thin volatile accessor over a
//! DRHD unit's HHDM/uncacheable-mapped register base. The *sequencing* (set
//! root pointer, enable translation, invalidate caches, drain faults) lives in
//! `mod.rs` where the per-unit state (the GCMD shadow) is owned.
//!
//! **Spec authority.** VT-d spec §10.4 (Register Descriptions): GCMD/GSTS
//! §10.4.4–5, RTADDR §10.4.6, CCMD §10.4.7, FSTS/FECTL §10.4.9–10, the fault
//! recording registers §10.4.14, the IOTLB invalidate registers §10.4.8.2.

use core::ptr::{read_volatile, write_volatile};

use x86_64::VirtAddr;

// ── Register offsets from the unit's base (VT-d spec §10.4) ──────────────────────
pub const REG_VER: usize = 0x000; // Version (32)
pub const REG_CAP: usize = 0x008; // Capability (64)
pub const REG_ECAP: usize = 0x010; // Extended Capability (64)
pub const REG_GCMD: usize = 0x018; // Global Command (32, write-only-ish)
pub const REG_GSTS: usize = 0x01c; // Global Status (32, read-only)
pub const REG_RTADDR: usize = 0x020; // Root Table Address (64)
pub const REG_CCMD: usize = 0x028; // Context Command (64)
pub const REG_FSTS: usize = 0x034; // Fault Status (32)
pub const REG_FECTL: usize = 0x038; // Fault Event Control (32)
pub const REG_FEDATA: usize = 0x03c; // Fault Event Data (32)
pub const REG_FEADDR: usize = 0x040; // Fault Event Address (32)
pub const REG_FEUADDR: usize = 0x044; // Fault Event Upper Address (32)

// ── Global Command bits (§10.4.4) — write the desired persistent bits + a
// one-shot trigger; the unit acks via the matching Global Status bit. ───────────
pub const GCMD_TE: u32 = 1 << 31; // Translation Enable (persistent)
pub const GCMD_SRTP: u32 = 1 << 30; // Set Root Table Pointer (one-shot)
pub const GCMD_WBF: u32 = 1 << 27; // Write Buffer Flush (one-shot)

// ── Global Status bits (§10.4.5) ─────────────────────────────────────────────────
pub const GSTS_TES: u32 = 1 << 31; // Translation Enable Status (V3 discriminator)
pub const GSTS_RTPS: u32 = 1 << 30; // Root Table Pointer Status
pub const GSTS_WBFS: u32 = 1 << 27; // Write Buffer Flush Status

/// The persistent (non-one-shot) GCMD bits the shadow must preserve across
/// command writes. GSTS reflects status, not the command shadow, so software
/// keeps its own copy of the enable bits (TE here; QIE/IRE/etc. are out of
/// scope — DMA remapping only, no queued-invalidation/interrupt-remapping).
pub const GCMD_PERSISTENT_MASK: u32 = GCMD_TE;

// ── Root Table Address (§10.4.6): bits[63:12]=root phys, bits[11:10]=RTT ─────────
pub const RTADDR_RTT_LEGACY: u64 = 0b00 << 10; // legacy (non-scalable) root table

// ── Context Command (§10.4.7): global/domain/device context-cache invalidation ──
pub const CCMD_ICC: u64 = 1 << 63; // Invalidate Context-Cache (one-shot)
pub const CCMD_CIRG_GLOBAL: u64 = 0b01 << 61; // Global invalidation granularity

// ── Fault Status (§10.4.9) ───────────────────────────────────────────────────────
pub const FSTS_PFO: u32 = 1 << 0; // Primary Fault Overflow
pub const FSTS_PPF: u32 = 1 << 1; // Primary Pending Fault
/// FSTS bits [15:8] = FRI (Fault Record Index) of the most recent fault.
pub const fn fsts_fault_record_index(fsts: u32) -> u32 {
    (fsts >> 8) & 0xff
}

// ── Fault Event Control (§10.4.10) ───────────────────────────────────────────────
pub const FECTL_IM: u32 = 1 << 31; // Interrupt Mask (1 = fault MSI masked)

// ── IOTLB invalidate (§10.4.8.2): at ECAP.IRO offset; IOTLB reg = IRO*16 + 8 ─────
pub const IOTLB_IVT: u64 = 1 << 63; // Invalidate IOTLB (one-shot)
pub const IOTLB_IIRG_GLOBAL: u64 = 0b01 << 60; // Global invalidation granularity
pub const IOTLB_REG_OFFSET: u64 = 0x08; // IOTLB register is 8 bytes past the IVA register

/// A volatile accessor over one remapping unit's register block. The base is an
/// uncacheable kernel mapping of the DRHD physical register base.
#[derive(Copy, Clone)]
pub struct RemapRegs {
    base: VirtAddr,
}

impl RemapRegs {
    /// # Safety
    /// `base` must be a valid, uncacheable kernel mapping of a DRHD register
    /// block (at least one page), as produced by `mm::map_kernel_mmio_page`.
    pub const unsafe fn new(base: VirtAddr) -> Self {
        Self { base }
    }

    #[inline]
    fn ptr(self, off: usize) -> *mut u8 {
        (self.base.as_u64() as usize + off) as *mut u8
    }

    #[inline]
    pub fn read32(self, off: usize) -> u32 {
        unsafe { read_volatile(self.ptr(off) as *const u32) }
    }
    #[inline]
    pub fn write32(self, off: usize, val: u32) {
        unsafe { write_volatile(self.ptr(off) as *mut u32, val) }
    }
    #[inline]
    pub fn read64(self, off: usize) -> u64 {
        unsafe { read_volatile(self.ptr(off) as *const u64) }
    }
    #[inline]
    pub fn write64(self, off: usize, val: u64) {
        unsafe { write_volatile(self.ptr(off) as *mut u64, val) }
    }
}
