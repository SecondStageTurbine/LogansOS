// SPDX-License-Identifier: MPL-2.0
//! Intel Gen9 execlist / Logical-Ring-Context (LRC) byte formats — Phase 7
//! BUILD-4X-1 (GuC-free execlist submission runs a context).
//!
//! BUILD-4-1 proved the legacy ring **cannot** engage PPGTT on Gen9: the engine
//! binds the per-process address space to the LRC that execlist submission loads,
//! and a pure ring has no context. 4X-1's single job is the prerequisite — prove a
//! context submitted via the **Execlist Submit Port (ELSP)** actually executes on
//! this silicon, GuC-free. It carries a (minimal, valid) PPGTT root in the LRC but
//! **nothing dereferences it** (the no-op touches no data); the PPGTT-data positive
//! test is 4X-2.
//!
//! This file is **kernel-dependency-free** so the host suite (`tools/ai-ops-tests`)
//! can path-`mod` it and assert — byte-for-byte, independent of the construction
//! code (the CLAUDE.md byte-oracle rule) — the two layouts whose wrongness silently
//! breaks the run: the **64-bit context descriptor** and the **LRC register-state
//! image** (the ring-pointer block + the PML4/PPGTT-root slot). The kernel glue —
//! allocating the LRC/HWSP frames, GGTT-mapping them, enabling execlists, writing
//! ELSP, and polling the Context Status Buffer (CSB) — lives in `accel_intel.rs`.
//!
//! **Source authority — `Plans/RESEARCH-2026-06-08-gen9-execlist-lrc-bringup.md`.**
//! Every offset/bit/sequence below is transcribed from that cited, cross-validated
//! spec (5 research lanes against i915 v4.4–v6.1 + the Intel SKL/KBL PRM), NOT from
//! memory. Primary: i915 `gt/intel_lrc.c` (`execlists_init_reg_state`, `write_desc`,
//! `enable_execlists`), `gt/intel_lrc_reg.h` (the `CTX_*` map, `ASSIGN_CTX_PML4`),
//! `gt/intel_engine_regs.h` (`RING_ELSP`+0x230, `RING_MODE_GEN7`+0x29c, ...).
//!
//! **Non-i915 cross-check (ISC-6 byte-oracle deliverable — closes the "two i915
//! readers agree even if both wrong" loophole).** The `CTX_*` dword map was
//! independently confirmed against Fuchsia Magma `msd-intel-gen`
//! `src/graphics/drivers/msd-intel-gen/src/register_state_helper.h` (a clean-room
//! driver, not derived from i915): MI_LRI headers at dwords **0x1 / 0x21 / 0x41**,
//! ring head/tail/start/control values at **0x05 / 0x07 / 0x09 / 0x0b**, the single
//! PML4 at CTX_PDP0 **0x31 (upper) / 0x33 (lower)**. The host oracle hand-lays the
//! image against those Fuchsia-confirmed indices.
//!
//! **The off-by-one trap (caught by the spec + Fuchsia):** the register-state image
//! does **not** start with the LRI header at dword 0 — dword 0 is an `MI_NOOP` and
//! `CTX_LRI_HEADER_0` is at dword **0x01** (i915's "+1 value-slot" convention: each
//! `CTX_*` constant is the dword of a register *address*; its *value* is at +1).

// ── MI command primitives (PRM Vol. 2a) ─────────────────────────────────────────
// Local copies (this module is path-`mod`'d standalone in the host suite, so it must
// not depend on sibling modules); identical opcode to `intel_gen9_ppgtt`.

/// MI_LOAD_REGISTER_IMM header for `n` register writes: opcode 0x22 in bits [28:23],
/// DWord length = 2*n - 1. Total DWords = 1 + 2*n.
pub const fn mi_load_register_imm(n: u32) -> u32 {
    (0x22 << 23) | (2 * n - 1)
}
/// MI_LRI_FORCE_POSTED (bit 12) — the LRC LRI headers set this so the register writes
/// are posted before the context goes active (PRM Vol. 2a; i915 `MI_LRI_FORCE_POSTED`).
pub const MI_LRI_FORCE_POSTED: u32 = 1 << 12;
/// MI_NOOP — dword 0 of the image, the gaps between the three LRI blocks, and the
/// WaIdleLiteRestore tail pad.
pub const MI_NOOP: u32 = 0;

// ── Per-engine RCS MMIO register offsets (engine base 0x2000) ────────────────────
// Stored as DATA inside the LRC register-state image (the engine loads each value
// into the named register on context switch-in) and used directly by the driver for
// ELSP/CSB/HWSP programming. RESEARCH §1/§2/§9 (i915 `intel_engine_regs.h`).

/// Render command-streamer MMIO base (RCS).
pub const RENDER_RING_MMIO_BASE: u32 = 0x0000_2000;

pub const RING_CONTEXT_CONTROL: u32 = RENDER_RING_MMIO_BASE + 0x244;
pub const RING_HEAD: u32 = RENDER_RING_MMIO_BASE + 0x034;
pub const RING_TAIL: u32 = RENDER_RING_MMIO_BASE + 0x030;
pub const RING_START: u32 = RENDER_RING_MMIO_BASE + 0x038;
pub const RING_CTL: u32 = RENDER_RING_MMIO_BASE + 0x03c;
pub const RING_BBADDR_UDW: u32 = RENDER_RING_MMIO_BASE + 0x168;
pub const RING_BBADDR: u32 = RENDER_RING_MMIO_BASE + 0x140;
pub const RING_BBSTATE: u32 = RENDER_RING_MMIO_BASE + 0x110;
pub const RING_SBBADDR_UDW: u32 = RENDER_RING_MMIO_BASE + 0x11c;
pub const RING_SBBADDR: u32 = RENDER_RING_MMIO_BASE + 0x114;
pub const RING_SBBSTATE: u32 = RENDER_RING_MMIO_BASE + 0x118;
pub const RING_BB_PER_CTX_PTR: u32 = RENDER_RING_MMIO_BASE + 0x1c0;
pub const RING_INDIRECT_CTX: u32 = RENDER_RING_MMIO_BASE + 0x1c4;
pub const RING_INDIRECT_CTX_OFFSET: u32 = RENDER_RING_MMIO_BASE + 0x1c8;
pub const RING_CTX_TIMESTAMP: u32 = RENDER_RING_MMIO_BASE + 0x3a8;
pub const RING_R_PWR_CLK_STATE: u32 = RENDER_RING_MMIO_BASE + 0x0c8;

/// CTX_BB_STATE value — `RING_BB_PPGTT` (bit 5). Non-critical for the no-op (no
/// second-level batch buffer), set to match the canonical i915/Fuchsia image.
pub const RING_BB_PPGTT: u32 = 1 << 5;

/// Per-engine PDP{0..3} low/high (the LRC's PPGTT root slots; i915 `GEN8_RING_PDP_LDW/
/// UDW`, base + 0x270 + n*8). Gen9 uses **48-bit** PPGTT: only PDP0 holds the single
/// PML4 root (the 4×PDP form is legacy 32-bit). Loaded *from the LRC* on switch-in —
/// the mechanism BUILD-4-1 proved the legacy ring lacks.
pub const fn ring_pdp_ldw(n: u32) -> u32 {
    RENDER_RING_MMIO_BASE + 0x270 + n * 8
}
pub const fn ring_pdp_udw(n: u32) -> u32 {
    RENDER_RING_MMIO_BASE + 0x274 + n * 8
}

// ── 48-bit / 4-level virtual-address decomposition (PML4 → PDP → PD → PT) ─────────
// iter1 silicon (a context switched in, ran the ring, then faulted in PPGTT space at
// the STATE VA 0x110000) proved that under an execlist context the engine resolves the
// dispatch pipeline's STATE_BASE / IDD / kernel-fetch accesses through the **context
// PPGTT**, NOT GGTT — falsifying the dispatch's "no-op = zero PPGTT dependency" premise.
// So the LRC's PML4 must map the no-op's own pipeline pages. The walk is 9 bits/level.
pub const fn pml4_index(va: u64) -> usize {
    ((va >> 39) & 0x1ff) as usize
}
pub const fn pdp_index_48(va: u64) -> usize {
    ((va >> 30) & 0x1ff) as usize
}
pub const fn pd_index_48(va: u64) -> usize {
    ((va >> 21) & 0x1ff) as usize
}
pub const fn pt_index_48(va: u64) -> usize {
    ((va >> 12) & 0x1ff) as usize
}

// ── Engine bring-up registers (RESEARCH §1, enable_execlists) ────────────────────

/// RING_HWSTAM (base + 0x98) ← ~0 masks all HWSP-write IRQs (we poll, no interrupt).
pub const RING_HWSTAM: u32 = RENDER_RING_MMIO_BASE + 0x098;
/// RING_MI_MODE (base + 0x9c) — clear STOP_RING to un-stop the command streamer.
pub const RING_MI_MODE: u32 = RENDER_RING_MMIO_BASE + 0x09c;
/// STOP_RING (bit 8) in MI_MODE — a masked DISABLE un-stops the ring.
pub const STOP_RING: u32 = 1 << 8;
/// Masked write that clears STOP_RING (un-stops the ring): mask set, value clear.
pub const fn mi_mode_unstop_ring() -> u32 {
    STOP_RING << 16
}
/// Per-engine Hardware Status Page address register (GGTT addr; base + 0x80).
pub const RING_HWS_PGA: u32 = RENDER_RING_MMIO_BASE + 0x080;
/// RING_MODE_GEN7 / GFX_MODE (base + 0x29c) and the Gen9 execlist-enable bit (bit 15 —
/// NOT the Gen11 `DISABLE_LEGACY_MODE`). Enabling is a masked write `(bit<<16)|bit`.
pub const RING_MODE: u32 = RENDER_RING_MMIO_BASE + 0x29c;
pub const GFX_RUN_LIST_ENABLE: u32 = 1 << 15;
/// Masked write that ENABLES execlist submission → 0x8000_8000 (RESEARCH §1 step 3).
pub const fn ring_mode_enable_execlists() -> u32 {
    (GFX_RUN_LIST_ENABLE << 16) | GFX_RUN_LIST_ENABLE
}

// ── Execlist submit / status / CSB registers (RESEARCH §4/§5) ────────────────────

/// Execlist Submit Port — the classic Gen9 ELSP register (base + 0x230), write-only,
/// written four times per submission. **Gen9 is NOT ELSQ** (Gen11+ only).
pub const RING_ELSP: u32 = RENDER_RING_MMIO_BASE + 0x230;
/// EXECLIST_STATUS low/high (base + 0x234/0x238) — read to post the write-only ELSP
/// and to observe the current/last context id + engine execlist state.
pub const RING_EXECLIST_STATUS_LO: u32 = RENDER_RING_MMIO_BASE + 0x234;
pub const RING_EXECLIST_STATUS_HI: u32 = RENDER_RING_MMIO_BASE + 0x238;
/// Context Status Buffer — 6 entries (base + 0x370 + i*8): LO = status dword, HI =
/// context-id dword. The engine posts switch-in/idle events here.
pub const CSB_ENTRIES: u32 = 6;
pub const fn ring_context_status_buf_status(i: u32) -> u32 {
    RENDER_RING_MMIO_BASE + 0x370 + i * 8
}
pub const fn ring_context_status_buf_ctxid(i: u32) -> u32 {
    RENDER_RING_MMIO_BASE + 0x374 + i * 8
}
/// Context Status write/read pointer (base + 0x3a0): HW write ptr = bits [2:0], SW read
/// ptr = bits [10:8], mask 0x7. The 0x7-reset quirk: a freshly powered engine reads the
/// write ptr as all-ones (0x7, invalid) — treat it as 5 so the first processed entry is 0.
pub const RING_CONTEXT_STATUS_PTR: u32 = RENDER_RING_MMIO_BASE + 0x3a0;
pub const CSB_PTR_MASK: u32 = 0x7;
pub const CSB_PTR_RESET: u32 = 0x7;

/// CSB status-dword bits (RESEARCH §5). `IDLE_ACTIVE` (switch-IN) and `ACTIVE_IDLE`
/// (went idle) are opposite events at opposite bits — the documented trap.
pub const CSB_IDLE_ACTIVE: u32 = 1 << 0; // engine switched a context IN
pub const CSB_PREEMPTED: u32 = 1 << 1;
pub const CSB_ELEMENT_SWITCH: u32 = 1 << 2;
pub const CSB_ACTIVE_IDLE: u32 = 1 << 3; // context went idle (completed)
pub const CSB_COMPLETE: u32 = 1 << 4;
pub const CSB_LITE_RESTORE: u32 = 1 << 15;

// ── WaIdleLiteRestore (RESEARCH §8.1, corrected) ─────────────────────────────────
// Never resubmit a context with the SAME RING_TAIL — HW reads HEAD==TAIL as an empty
// ring and wedges. Pad each submission with 2 MI_NOOPs (8 bytes) so the tail advances.
// (The original brief said "8 dwords" — wrong; the spec corrects it to 2 dwords.)
pub const WA_TAIL_DWORDS: usize = 2;
pub const WA_TAIL_BYTES: usize = WA_TAIL_DWORDS * 4;

// ── Context descriptor (RESEARCH §3; PRM Vol. 2d "Context Descriptor Format") ────
// 64-bit value submitted via ELSP. Low 12 bits = flags, bits [31:12] = the LRCA (the
// GGTT page address of the PPHWSP page), bits [32:52] = the SW context ID (Gen9 layout
// — NOT the Gen11 bit-37 split). Cross-checked against the Fuchsia descriptor builder.

/// Bit 0 — the descriptor/context is valid. If clear the engine silently rejects the
/// submission (decision tree: "no CSB entry + STATUS shows nothing accepted").
pub const GEN8_CTX_VALID: u64 = 1 << 0;
/// Bit 8 — privileged context (may execute privileged MI / STATE commands; our no-op
/// batch issues STATE_BASE_ADDRESS + PIPE_CONTROL, so the context is privileged).
pub const GEN8_CTX_PRIVILEGE: u64 = 1 << 8;
/// Addressing-mode field shift (bits [4:3]) (i915 `GEN8_CTX_ADDRESSING_MODE_SHIFT`).
pub const GEN8_CTX_ADDRESSING_MODE_SHIFT: u64 = 3;
/// Gen9 48-bit / 4-level PML4 addressing = `LEGACY_64B_CONTEXT` = **3** (RESEARCH §3/§8;
/// 32-bit/3-level = 0). Cross-validated across multiple i915 tags + the spec.
pub const GEN8_CTX_ADDR_MODE_48B: u64 = 0b11;
/// SW context-ID shift — bits [32:52] of the descriptor (Gen9; i915 `GEN8_CTX_ID_SHIFT`).
pub const GEN8_CTX_ID_SHIFT: u64 = 32;
/// LRCA mask — the LRC (PPHWSP page) GGTT address occupies descriptor bits [31:12].
pub const GEN8_CTX_LRCA_MASK: u64 = 0xffff_f000;

/// The descriptor flag bits (bits [11:0]) for a valid, privileged, 48-bit-PPGTT Gen9
/// RCS context. Pinned by the descriptor byte oracle.
pub const fn context_descriptor_flags() -> u64 {
    GEN8_CTX_VALID | GEN8_CTX_PRIVILEGE | (GEN8_CTX_ADDR_MODE_48B << GEN8_CTX_ADDRESSING_MODE_SHIFT)
}

/// Build the 64-bit execlist context descriptor for an LRC whose **PPHWSP page** is at
/// GGTT address `lrca` (page-aligned) with software context id `ctx_id` (ISC-2). The
/// byte oracle pins this for a known (lrca, ctx_id).
pub const fn context_descriptor(lrca: u64, ctx_id: u32) -> u64 {
    context_descriptor_flags()
        | (lrca & GEN8_CTX_LRCA_MASK)
        | ((ctx_id as u64) << GEN8_CTX_ID_SHIFT)
}

/// The four-DWord ELSP write sequence that submits `desc` in element 0 with element 1
/// empty (ISC-3, RESEARCH §4): el1.hi, el1.lo, el0.hi, el0.lo — the engine latches on
/// the final (el0.lo) write. `desc[1] = 0` for single-context submission.
pub const fn elsp_submit_dwords(desc: u64) -> [u32; 4] {
    [
        0,                           // element 1 (empty) — high dword
        0,                           // element 1 (empty) — low dword
        (desc >> 32) as u32,         // element 0 — high dword (SW ctx id, bits 32-52)
        (desc & 0xffff_ffff) as u32, // element 0 — low dword (LRCA | flags) — latches here
    ]
}

// ── LRC register-state image (RESEARCH §2; Fuchsia-confirmed dword map) ──────────
// The register state lives in the **second** page of the LRC (page 0 = PPHWSP). Dword
// 0x00 is MI_NOOP; the first LRI header is at dword 0x01. Three MI_LOAD_REGISTER_IMM
// blocks, gaps padded MI_NOOP. Each `CTX_*` index is the dword of a register *address*;
// its *value* is at index + 1 (the "+1 value-slot" rule).

pub const CTX_LRI_HEADER_0: usize = 0x01;
pub const CTX_RING_HEAD_VALUE: usize = 0x05;
pub const CTX_RING_TAIL_VALUE: usize = 0x07;
pub const CTX_RING_START_VALUE: usize = 0x09;
pub const CTX_RING_CTL_VALUE: usize = 0x0b;
pub const CTX_LRI_HEADER_1: usize = 0x21;
pub const CTX_PDP0_UDW_VALUE: usize = 0x31;
pub const CTX_PDP0_LDW_VALUE: usize = 0x33;
pub const CTX_LRI_HEADER_2: usize = 0x41;
pub const CTX_R_PWR_CLK_STATE_VALUE: usize = 0x43;

/// Number of DWords in the populated register-state image (index 0x00..=0x43). The
/// LRC page that holds it is 4 KiB; the remainder past this extent stays zero.
pub const LRC_REG_STATE_DWORDS: usize = 0x44;
pub const LRC_REG_STATE_BYTES: usize = LRC_REG_STATE_DWORDS * 4;

/// Context-control value: a masked-ENABLE of INHIBIT_SYN_CTX_SWITCH (bit 3) +
/// ENGINE_CTX_RESTORE_INHIBIT (bit 0). On a fresh context the engine initializes from
/// these LRIs rather than restoring a golden image we never built (RESEARCH §2).
pub const CTX_CTRL_INHIBIT_SYN_CTX_SWITCH: u32 = 1 << 3;
pub const CTX_CTRL_ENGINE_CTX_RESTORE_INHIBIT: u32 = 1 << 0;
pub const fn context_control_value() -> u32 {
    let bits = CTX_CTRL_INHIBIT_SYN_CTX_SWITCH | CTX_CTRL_ENGINE_CTX_RESTORE_INHIBIT;
    (bits << 16) | bits
}

/// Parameters that vary per submission; everything else in the image is fixed.
#[derive(Copy, Clone)]
pub struct LrcParams {
    /// GGTT address of the ring buffer holding the batch (loaded into RING_START).
    pub ring_start: u32,
    /// RING_CTL value (buffer length field | VALID), same encoding as the legacy ring.
    pub ring_ctl: u32,
    /// RING_HEAD (0 for a fresh ring).
    pub ring_head: u32,
    /// RING_TAIL — the batch length **plus the WaIdleLiteRestore pad** in bytes (the
    /// engine runs HEAD..TAIL; the tail must clear the batch so HEAD != TAIL).
    pub ring_tail: u32,
    /// The four PDP slots. Gen9 48-bit: `pdp[0]` = the single PML4 page physical address
    /// (written to CTX_PDP0 at 0x31/0x33); `pdp[1..3]` = 0 (legacy-32b only).
    pub pdp: [u64; 4],
}

/// Build the Gen9 RCS LRC register-state image as DWords (ISC-1). Three LRI blocks:
///   header_0 @0x01 (14 regs): CONTEXT_CONTROL, ring HEAD/TAIL/START/CTL, BB/SBB state,
///                             per-ctx-BB + indirect-ctx pointers;
///   header_1 @0x21 (9 regs):  CTX_TIMESTAMP + the eight PDP{3..0} UDW/LDW dwords
///                             (DESCENDING order: PDP3 at 0x24 … PDP0 at 0x30);
///   header_2 @0x41 (1 reg):   R_PWR_CLK_STATE.
/// The byte oracle pins the ring-pointer block and the PML4 (PDP0) slot.
pub fn lrc_reg_state_dwords(p: LrcParams) -> [u32; LRC_REG_STATE_DWORDS] {
    let mut s = [MI_NOOP; LRC_REG_STATE_DWORDS];

    // dword 0x00 stays MI_NOOP (the image does NOT start with the LRI header).
    // ---- LRI header 0 @0x01: 14 register writes (CONTEXT_CONTROL .. INDIRECT_CTX_OFFSET) ----
    s[CTX_LRI_HEADER_0] = mi_load_register_imm(14) | MI_LRI_FORCE_POSTED;
    s[0x02] = RING_CONTEXT_CONTROL;
    s[0x03] = context_control_value();
    s[0x04] = RING_HEAD;
    s[0x05] = p.ring_head;
    s[0x06] = RING_TAIL;
    s[0x07] = p.ring_tail;
    s[0x08] = RING_START;
    s[0x09] = p.ring_start;
    s[0x0a] = RING_CTL;
    s[0x0b] = p.ring_ctl;
    s[0x0c] = RING_BBADDR_UDW;
    s[0x0d] = 0;
    s[0x0e] = RING_BBADDR;
    s[0x0f] = 0;
    s[0x10] = RING_BBSTATE;
    s[0x11] = RING_BB_PPGTT;
    s[0x12] = RING_SBBADDR_UDW;
    s[0x13] = 0;
    s[0x14] = RING_SBBADDR;
    s[0x15] = 0;
    s[0x16] = RING_SBBSTATE;
    s[0x17] = 0;
    s[0x18] = RING_BB_PER_CTX_PTR;
    s[0x19] = 0;
    s[0x1a] = RING_INDIRECT_CTX;
    s[0x1b] = 0;
    s[0x1c] = RING_INDIRECT_CTX_OFFSET;
    s[0x1d] = 0;
    // 0x1e..0x20 : MI_NOOP padding (already zero).

    // ---- LRI header 1 @0x21: 9 register writes (CTX_TIMESTAMP + PDP3..PDP0 UDW/LDW) ----
    s[CTX_LRI_HEADER_1] = mi_load_register_imm(9) | MI_LRI_FORCE_POSTED;
    s[0x22] = RING_CTX_TIMESTAMP;
    s[0x23] = 0;
    // PDPs in DESCENDING order: PDP3 lowest (0x24), PDP0 highest (0x30).
    s[0x24] = ring_pdp_udw(3);
    s[0x25] = (p.pdp[3] >> 32) as u32;
    s[0x26] = ring_pdp_ldw(3);
    s[0x27] = (p.pdp[3] & 0xffff_ffff) as u32;
    s[0x28] = ring_pdp_udw(2);
    s[0x29] = (p.pdp[2] >> 32) as u32;
    s[0x2a] = ring_pdp_ldw(2);
    s[0x2b] = (p.pdp[2] & 0xffff_ffff) as u32;
    s[0x2c] = ring_pdp_udw(1);
    s[0x2d] = (p.pdp[1] >> 32) as u32;
    s[0x2e] = ring_pdp_ldw(1);
    s[0x2f] = (p.pdp[1] & 0xffff_ffff) as u32;
    // CTX_PDP0 holds the single 48-bit PML4 root (Gen9): upper @0x31, lower @0x33.
    s[0x30] = ring_pdp_udw(0);
    s[0x31] = (p.pdp[0] >> 32) as u32;
    s[0x32] = ring_pdp_ldw(0);
    s[0x33] = (p.pdp[0] & 0xffff_ffff) as u32;
    // 0x34..0x40 : MI_NOOP padding (already zero).

    // ---- LRI header 2 @0x41: 1 register write (R_PWR_CLK_STATE) ----
    s[CTX_LRI_HEADER_2] = mi_load_register_imm(1) | MI_LRI_FORCE_POSTED;
    s[0x42] = RING_R_PWR_CLK_STATE;
    // RPCS value 0 = enable bit (31) clear → the hardware keeps its boot-default fused
    // GT2 slice/subslice/EU config (the one SKU value; we do not override it for the
    // no-op). RESEARCH §10 flags an explicit RPCS as a 4X-2 iteration knob.
    s[CTX_R_PWR_CLK_STATE_VALUE] = 0;

    s
}

/// Encode [`lrc_reg_state_dwords`] as little-endian bytes into `dst` (the LRC's second
/// page). Returns the byte count written, or `None` if `dst` is too small.
pub fn write_lrc_reg_state(dst: &mut [u8], p: LrcParams) -> Option<usize> {
    if dst.len() < LRC_REG_STATE_BYTES {
        return None;
    }
    for (idx, dword) in lrc_reg_state_dwords(p).iter().enumerate() {
        let base = idx * 4;
        dst[base..base + 4].copy_from_slice(&dword.to_le_bytes());
    }
    Some(LRC_REG_STATE_BYTES)
}
