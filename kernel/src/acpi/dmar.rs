// SPDX-License-Identifier: MPL-2.0
//! Kernel-side DMAR table interpretation — Phase 7.5 BUILD-V (ISC-1). Wraps the
//! pure, host-oracle'd `dmar_logic` walk into a `DmarInfo` the VT-d driver
//! consumes: the remapping-unit register bases, the RMRR reserved-memory
//! regions to identity-map, and the host address width.

extern crate alloc;

use alloc::vec::Vec;

use super::dmar_logic::{
    DMAR_TYPE_DRHD, DMAR_TYPE_RMRR, DmarStructs, dmar_flags, dmar_host_addr_width,
    drhd_include_pci_all, drhd_register_base, rmrr_base_limit,
};

/// The parsed DMAR table: everything BUILD-V needs from firmware.
#[derive(Clone, Debug)]
pub struct DmarInfo {
    /// Host address width in bits (DMAR header field + 1).
    pub host_addr_width: u8,
    /// DMAR header flags (bit 0 = INTR_REMAP — logged, never acted on:
    /// BUILD-V is DMA-remapping-only).
    pub flags: u8,
    /// MMIO register base of each DRHD remapping unit (the `iommu` driver maps
    /// these uncacheable and reads CAP/ECAP from them).
    pub drhd_bases: Vec<u64>,
    /// Per-DRHD INCLUDE_PCI_ALL flag (does the unit cover every device in its
    /// segment). Index-parallel with `drhd_bases`.
    pub drhd_include_all: Vec<bool>,
    /// Reserved-memory regions `[base, limit]` (inclusive) that MUST stay
    /// identity-mapped or their scoped device breaks when translation enables
    /// (V2 — the GPU aperture / USB legacy region is the canary).
    pub rmrrs: Vec<(u64, u64)>,
}

impl DmarInfo {
    /// Parse a checksum-validated DMAR table. Returns `None` if the table
    /// declares no DRHD remapping unit (nothing for the driver to drive).
    pub fn parse(table: &[u8]) -> Option<Self> {
        let host_addr_width = dmar_host_addr_width(table)?;
        let flags = dmar_flags(table)?;

        let mut drhd_bases = Vec::new();
        let mut drhd_include_all = Vec::new();
        let mut rmrrs = Vec::new();

        for s in DmarStructs::new(table) {
            match s.kind {
                DMAR_TYPE_DRHD => {
                    if let Some(base) = drhd_register_base(&s) {
                        drhd_bases.push(base);
                        drhd_include_all.push(drhd_include_pci_all(&s).unwrap_or(false));
                    }
                }
                DMAR_TYPE_RMRR => {
                    if let Some(region) = rmrr_base_limit(&s) {
                        rmrrs.push(region);
                    }
                }
                _ => {}
            }
        }

        if drhd_bases.is_empty() {
            return None;
        }

        Some(DmarInfo {
            host_addr_width,
            flags,
            drhd_bases,
            drhd_include_all,
            rmrrs,
        })
    }
}
