// SPDX-License-Identifier: MPL-2.0
//! Intel Gen8+ PPGTT (Per-Process Graphics Translation Table) byte format —
//! Phase 7 BUILD-4-1.
//!
//! This file is **kernel-dependency-free** so the host suite (`tools/ai-ops-tests`)
//! can path-`mod` it and assert the page-table-entry layout and the PPGTT-root load
//! command **byte-for-byte, independent of the construction code** (the CLAUDE.md
//! byte-oracle rule — the page table is an in-memory byte format). The kernel glue —
//! allocating the table frames, walking the buffers into them, and emitting the load
//! on the legacy ring — lives in `accel_intel.rs`.
//!
//! **iter3 — 32-bit legacy PPGTT (the pivot).** iter1/iter2 on real UHD630 silicon
//! proved that **48-bit (4-level, PML4) PPGTT does not engage the data port on the
//! legacy ring**: with `GFX_MODE` bit 7 set and the PML4 root loaded into PDP0, the
//! EU's A64 access still resolved through GGTT (`select=GGTT`, fault at the input VA
//! once GGTT was cleared). Full 48-bit PPGTT is bound to the LRC/execlist context,
//! which BUILD-4-1 excludes. So we use the **legacy 32-bit PPGTT** that the ring
//! actually supports: a **3-level** walk (4 PDP registers → PD → PT) covering 4 GiB.
//! The four PDP registers are the top level — loaded directly via LRI on the ring
//! (this is what the THINK's "load the PDPs" denotes). The tenant's data VAs
//! (`0x13_0000…`, all < 1 GiB) live under PDP0.
//!
//! Because the GPU's GFX_MODE persists across a warm reboot, we **clear** the stale
//! 48-bit bit (it may be left set by a prior iter1/iter2 boot) rather than relying on
//! its default.
//!
//! Map shape: ring/command/state/EU-kernel/seqno/CURBE stay in GGTT (proven by 4a);
//! the data buffers map ONLY in this PPGTT and are cleared from GGTT, so a correct
//! result proves the data port went through PPGTT (else it faults — the self-proving
//! gate). (4-2 will use the R/W bit below for read-only enforcement; here every leaf
//! is present + R/W.)
//!
//! **PRM authority.** PTE/page-table format: Intel Kaby Lake PRM Vol 5 "Memory
//! Views". PPGTT-root (PDP) load + the legacy 32-bit walk: Vol 2c "Command Reference:
//! Registers" (the per-engine PDP0..3 registers, GFX_MODE) and Vol 2a (MI_LOAD_
//! REGISTER_IMM). Register offsets and the 3-level-vs-4-level distinction cross-checked
//! against the Linux i915 driver (`gen8_ppgtt.c`: `gen8_mm_switch_3lvl` /
//! `gen8_emit_pdps`, `i915_reg.h`: `GEN8_RING_PDP_LDW/UDW`).

// ── Gen8+ page-table-entry bits (PRM Vol 5 "Memory Views") ──────────────────────

/// Bit 0 — the entry is valid/present. Set on every level (PDPE/PDE/PTE) we map.
pub const GEN8_PAGE_PRESENT: u64 = 1 << 0;
/// Bit 1 — read/write. Set = writable; clear = read-only (a write through a
/// read-only leaf faults in hardware — the 4-2 enforcement bit). BUILD-4-1 maps every
/// leaf R/W.
pub const GEN8_PAGE_RW: u64 = 1 << 1;
/// Physical-address field of a Gen8+ page-table entry: bits [47:12] (4 KiB-aligned).
/// The DmaBuffer frames are far below 2^39, so the upper bits are always zero.
pub const GEN8_PTE_ADDR_MASK: u64 = 0x0000_ffff_ffff_f000;

/// A **leaf** page-table entry (PTE) mapping `phys` (a 4 KiB-aligned physical frame),
/// present, writable per `writable`. The independent byte oracle pins this layout.
pub const fn leaf_pte(phys: u64, writable: bool) -> u64 {
    let rw = if writable { GEN8_PAGE_RW } else { 0 };
    (phys & GEN8_PTE_ADDR_MASK) | GEN8_PAGE_PRESENT | rw
}

/// A **directory** entry (PDE — and, conceptually, the value loaded into a PDP
/// register) pointing at the 4 KiB physical frame of the next-level table. Present +
/// R/W (directory entries are always writable; per-page R/W enforcement is in the leaf).
pub const fn dir_entry(next_table_phys: u64) -> u64 {
    (next_table_phys & GEN8_PTE_ADDR_MASK) | GEN8_PAGE_PRESENT | GEN8_PAGE_RW
}

/// Number of 64-bit entries in one 4 KiB page-table page (512 = 4096 / 8).
pub const ENTRIES_PER_TABLE: usize = 512;

// ── 32-bit-legacy virtual-address decomposition (PRM Vol 5; 3-level walk) ────────
// va[31:30] = PDP index (one of 4 PDP registers, 1 GiB each),
// va[29:21] = PD index, va[20:12] = PT index, va[11:0] = page offset.

pub const fn pdp_index(va: u64) -> usize {
    ((va >> 30) & 0x3) as usize
}
pub const fn pd_index(va: u64) -> usize {
    ((va >> 21) & 0x1ff) as usize
}
pub const fn pt_index(va: u64) -> usize {
    ((va >> 12) & 0x1ff) as usize
}

// ── Data-buffer PPGTT virtual addresses (numerically == 4a's GGTT VAs) ───────────
// Reusing the same numbers makes the self-proving gate sharp: these VAs are mapped
// ONLY in PPGTT now (cleared from GGTT), so the EU's A64 access to them resolves only
// if PPGTT engaged — otherwise it falls to GGTT, finds nothing, and faults.
pub const PPGTT_LHS_VA: u64 = 0x0013_0000;
pub const PPGTT_RHS_VA: u64 = 0x0013_1000;
pub const PPGTT_OUTPUT_VA: u64 = 0x0013_2000;

// ── PPGTT-root (PDP) load on the legacy render ring (PRM Vol 2a/2c; i915) ─────────

/// MI_LOAD_REGISTER_IMM header for `n` register writes (PRM Vol 2a): opcode 0x22 in
/// bits [28:23], DWord length = 2*n - 1. Total DWords = 1 + 2*n.
pub const fn mi_load_register_imm(n: u32) -> u32 {
    (0x22 << 23) | (2 * n - 1)
}

/// Render engine MMIO base (RCS) — the per-engine register block.
pub const RENDER_RING_MMIO_BASE: u32 = 0x0000_2000;
/// Per-engine PDP{0..3} low/high dwords (PRM Vol 2c; i915 `GEN8_RING_PDP_LDW/UDW`,
/// base + 0x270 + n*8). In 32-bit legacy mode each PDP register holds a **Page
/// Directory** pointer directly (the top level — no PML4). The tenant's data VAs live
/// under PDP0; PDP1..3 cover the upper 3 GiB (unused here, loaded 0).
pub const RENDER_RING_PDP0_LDW: u32 = RENDER_RING_MMIO_BASE + 0x270;
pub const RENDER_RING_PDP0_UDW: u32 = RENDER_RING_MMIO_BASE + 0x274;
pub const RENDER_RING_PDP1_LDW: u32 = RENDER_RING_MMIO_BASE + 0x278;
pub const RENDER_RING_PDP1_UDW: u32 = RENDER_RING_MMIO_BASE + 0x27c;
pub const RENDER_RING_PDP2_LDW: u32 = RENDER_RING_MMIO_BASE + 0x280;
pub const RENDER_RING_PDP2_UDW: u32 = RENDER_RING_MMIO_BASE + 0x284;
pub const RENDER_RING_PDP3_LDW: u32 = RENDER_RING_MMIO_BASE + 0x288;
pub const RENDER_RING_PDP3_UDW: u32 = RENDER_RING_MMIO_BASE + 0x28c;
/// Per-engine GFX_MODE register (PRM Vol 2c; i915 `RING_MODE_GEN7`, base + 0x29c).
pub const RENDER_RING_GFX_MODE: u32 = RENDER_RING_MMIO_BASE + 0x29c;

// ── iter4: the Gen7-era aliasing-PPGTT root registers (a long-shot experiment) ──
// Both legacy-ring Gen8 PPGTT modes (48-bit PML4, 32-bit PDPs) read select=GGTT on
// this silicon. Before concluding PPGTT needs the LRC/execlist context, try the older
// PP_DIR_BASE root path (i915 `RING_PP_DIR_BASE`/`RING_PP_DIR_DCLV`, base + 0x228 /
// + 0x220) — the Gen6/7 aliasing PPGTT whose root is a **GGTT offset** of the page
// directory (so the PD is GGTT-mapped), not a physical PDP. Deprecated on Gen8+, but
// the register may still be honored.
pub const RENDER_RING_PP_DIR_BASE: u32 = RENDER_RING_MMIO_BASE + 0x228;
pub const RENDER_RING_PP_DIR_DCLV: u32 = RENDER_RING_MMIO_BASE + 0x220;
/// PP_DIR_DCLV — all 32 page-directory cachelines valid.
pub const PP_DIR_DCLV_ALL_VALID: u32 = 0xffff_ffff;
/// GFX_MODE bit 7 — 48-bit (4-level PML4) PPGTT select (i915 `GEN8_GFX_PPGTT_48B`).
/// 32-bit legacy PPGTT requires this bit **clear**.
pub const GEN8_GFX_PPGTT_48B: u32 = 1 << 7;

/// GFX_MODE value that masked-**clears** the 48-bit-PPGTT bit (mask in [31:16], value
/// bit clear) — forcing 32-bit legacy mode even if a prior 48-bit boot left it set
/// (GPU GFX_MODE persists across a warm reboot).
pub const fn gfx_mode_clear_48b() -> u32 {
    GEN8_GFX_PPGTT_48B << 16
}

/// DWords in the PDP-load command: MI_LOAD_REGISTER_IMM(8) header + 8 (offset,value)
/// pairs (PDP0..3, UDW+LDW each).
pub const PDP_LOAD_DWORDS: usize = 1 + 2 * 8;
pub const PDP_LOAD_BYTES: usize = PDP_LOAD_DWORDS * 4;

/// The legacy-ring command sequence that loads the 32-bit PPGTT root (ISC-2): one
/// MI_LOAD_REGISTER_IMM(8) writing all four PDP register pairs — **PDP0 = the Page
/// Directory** (`pd_phys`, UDW then LDW), PDP1..3 = 0 (unused upper 3 GiB, cleared to
/// invalidate any stale value a prior boot left). The byte oracle pins these bytes for
/// a known `pd_phys`.
pub const fn pdp_load_dwords(pd_phys: u64) -> [u32; PDP_LOAD_DWORDS] {
    let hi = (pd_phys >> 32) as u32;
    let lo = (pd_phys & 0xffff_ffff) as u32;
    [
        mi_load_register_imm(8),
        RENDER_RING_PDP0_UDW,
        hi,
        RENDER_RING_PDP0_LDW,
        lo,
        RENDER_RING_PDP1_UDW,
        0,
        RENDER_RING_PDP1_LDW,
        0,
        RENDER_RING_PDP2_UDW,
        0,
        RENDER_RING_PDP2_LDW,
        0,
        RENDER_RING_PDP3_UDW,
        0,
        RENDER_RING_PDP3_LDW,
        0,
    ]
}

/// Encode [`pdp_load_dwords`] as little-endian bytes into `dst` (the ring head, ahead
/// of the compute batch). Returns the byte count, or `None` if `dst` is too small.
pub fn encode_pdp_load(dst: &mut [u8], pd_phys: u64) -> Option<usize> {
    if dst.len() < PDP_LOAD_BYTES {
        return None;
    }
    for (idx, dword) in pdp_load_dwords(pd_phys).iter().enumerate() {
        let base = idx * 4;
        dst[base..base + 4].copy_from_slice(&dword.to_le_bytes());
    }
    Some(PDP_LOAD_BYTES)
}
