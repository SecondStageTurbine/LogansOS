<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RESEARCH: Gen9 GuC-free execlist/LRC bring-up — cited engineering spec (for BUILD-4X-1)

**Date:** 2026-06-08
**Author:** PM/Cerebro (deep-research harness — 5 parallel lanes, cross-verified)
**For:** Phase 7 BUILD-4X-1 (`Plans/DISPATCH-2026-06-08-phase-7-build-4X-1-execlist-bringup-jarvis.md`)
**Method:** 5 independent research lanes (LRC layout / descriptor+ELSP / CSB / GuC-free+PPGTT / failure-modes+prior-art), each fanning out its own web searches against Linux i915 source (multiple kernel tags v4.4–v6.1) + the Intel SKL/KBL PRM. Claims below cross-validated across ≥2 independent lanes where marked ✔✔; single-lane high-confidence marked ✔.

## 0. Verification summary
- **Cross-validated (✔✔):** ELSP offset, CSB offsets + status bits, descriptor bit layout, `CTX_PDP0`/PML4 offset, 48-bit mode, the enable-execlists register sequence. Multiple lanes landed on identical values from independent searches → high confidence.
- **One refuted claim:** the research brief's "WaIdleLiteRestore advances 8 *dwords*" is WRONG — it is **2 dwords (8 bytes)**, `WA_TAIL_DWORDS = 2`. Corrected here.
- **One OPEN ACTION (blocks freezing the byte layout, per CLAUDE.md byte-oracle rule):** all evidence is i915-sourced (one source family). The Fuchsia Magma `msd-intel-gen` mirror 404'd and the PRM PDFs 403'd, so **no non-i915 independent oracle was obtained.** Before the LRC/descriptor byte layout is frozen, obtain ONE of: Fuchsia `src/graphics/drivers/msd-intel-gen/src/{engine_command_streamer.cc,register_state_helper.*}`, or Mesa `src/intel/common` context-init, or the SKL PRM Vol 2d "Context Descriptor Format" table — and hand-verify the descriptor bit ranges (ctx-id 32–52, LRCA 12–31) and the `CTX_*` dword map against a hand-laid example. (This is BUILD-4X-1's byte-oracle deliverable.)

---

## 1. Engine bring-up sequence (i915 `enable_execlists`, Gen9 RCS; base = 0x2000) ✔✔
All `_FW` writes require forcewake held. Masked registers: value in low 16 bits, mask in high 16 bits.
1. Hold **forcewake** (RENDER/GT) across the sequence (`assert_forcewakes_active(FORCEWAKE_ALL)`).
2. `RING_HWSTAM` (base **+0x98**) ← `~0` (mask all HWSP-write IRQs).
3. `RING_MODE_GEN7` (base **+0x29c**) ← `_MASKED_BIT_ENABLE(GFX_RUN_LIST_ENABLE)` where `GFX_RUN_LIST_ENABLE = 1<<15` → write **0x80008000**. *(Gen9 uses this bit; the `GEN11_GFX_DISABLE_LEGACY_MODE` path is Gen11+ only.)*
4. `RING_MI_MODE` (base **+0x9c**) ← `_MASKED_BIT_DISABLE(STOP_RING)` (un-stop the ring).
5. `RING_HWS_PGA` (base **+0x80**) ← **GGTT offset** of the per-engine HW status page; then POSTING_READ it.
6. MOCS: program `GEN9_GFX_MOCS` (+ L3CC/LNCF) via MI_LRI — **required for dataport caching correctness**, not to submit. (Moot for the no-op; needed for 4X-2 compute.)
7. LRC + ring must be **GGTT-pinned** (the command streamer fetches them via global addresses; only workload *data* is in PPGTT).

## 2. LRC context image (Gen9 RCS) ✔✔
**Page layout:** page 0 = PPHWSP; register-state context starts at page 1 (PRM: "Ring Context starts at 4K offset from LRCA"). `engine->context_size` for Gen9 RCS = **22 × 4KB**; allocate that + 1 PPHWSP page. (A GuC-shared page exists only if using GuC — we are not.)

**The +1 value-slot rule (highest-risk trap):** each `CTX_*` constant is the dword holding the register's MMIO *address*; the *value* is at `offset + 1`. `regs[CTX_X] = mmio_offset(X); regs[CTX_X + 1] = value;`

**Register-state dword map** (dword indices into the register-state page; values at +1) ✔✔:
| Field | CTX dword | Value/notes |
|---|---|---|
| `CTX_LRI_HEADER_0` | 0x01 | `MI_LOAD_REGISTER_IMM(14) \| MI_LRI_FORCE_POSTED` **(14 regs for RCS; 11 for other engines)** |
| `CTX_CONTEXT_CONTROL` | 0x02 | mask-enable `INHIBIT_SYN_CTX_SWITCH` + `ENGINE_CTX_RESTORE_INHIBIT` |
| `CTX_RING_HEAD` | 0x04 | 0 |
| `CTX_RING_TAIL` | 0x06 | 0 (then bumped per submission — see §6 WaIdleLiteRestore) |
| `CTX_RING_BUFFER_START` | 0x08 | ring **GGTT** base |
| `CTX_RING_BUFFER_CONTROL` | 0x0a | `RING_CTL_SIZE(size) \| RING_VALID` |
| `CTX_BB_STATE` | 0x10 | `RING_BB_PPGTT` |
| `CTX_LRI_HEADER_1` | 0x21 | `MI_LOAD_REGISTER_IMM(9) \| MI_LRI_FORCE_POSTED` |
| `CTX_PDP3_UDW/LDW` | 0x24 / 0x26 | (legacy 32-bit only) |
| `CTX_PDP2_UDW/LDW` | 0x28 / 0x2a | (legacy 32-bit only) |
| `CTX_PDP1_UDW/LDW` | 0x2c / 0x2e | (legacy 32-bit only) |
| **`CTX_PDP0_UDW/LDW`** | **0x30 / 0x32** | **48-bit PML4 base goes here (value at 0x31/0x33)** |
| `CTX_LRI_HEADER_2` (RCS) | 0x41 | `MI_LOAD_REGISTER_IMM(1)` |
| `CTX_R_PWR_CLK_STATE` (RCS) | 0x42 | **RPCS — the ONE SKU-dependent value** (slice/subslice/EU mask; read from fuse or hardcode for UHD 630) |

**PDPs are in DESCENDING order** (PDP3 lowest at 0x24, PDP0 highest at 0x30) — easy to invert.
**Gen9 = 48-bit / 4-level PML4:** write the *single* PML4 page physical address into `CTX_PDP0_UDW+1`(0x31)=upper32, `CTX_PDP0_LDW+1`(0x33)=lower32 (`ASSIGN_CTX_PML4`). The 4×PDP form is legacy 32-bit (≤4GB) — not Gen9's default.
**No hardware golden register-state is needed** — i915 hand-builds the image in software (in-source comment confirms). *Caveat (§6): a golden render-state batch may be needed for first real compute; moot for the no-op.*
**`GEN9_CTX_RCS_INDIRECT_CTX_OFFSET_DEFAULT = 0x26`** (differs from Gen8 0x17) — only if you populate the indirect-ctx field; leave zeroed for minimal.

## 3. 64-bit execlist context descriptor (Gen9) ✔✔
```
desc  = (addr_mode << 3)                  // GEN8_CTX_ADDRESSING_MODE_SHIFT = 3
      | GEN8_CTX_VALID                    // bit 0
      | GEN8_CTX_PRIVILEGE                 // bit 8
      | (LRCA & 0xFFFFF000)               // bits 12-31 = GGTT offset of the PPHWSP page (= ggtt_offset(lrc) + LRC_HEADER_PAGES*4K)
      | ((u64)ctx_id << 32);              // bits 32-52  (GEN8_CTX_ID_SHIFT = 32)
```
Flag bits: `VALID`=1<<0, `FORCE_PD_RESTORE`=1<<1, `FORCE_RESTORE`=1<<2, `L3LLC_COHERENT`=1<<5, `PRIVILEGE`=1<<8.
**Addressing mode (bits 3–4): Gen9 48-bit = `LEGACY_64B_CONTEXT` = 3** (32-bit/3-level = `LEGACY_32B_CONTEXT` = 0).
**Gen9 ctx-id at bit 32** — do NOT use the Gen11 split (bit 37 + engine class/instance).
LRCA = GGTT offset of the **PPHWSP page** (context base + header pages), not the raw object base.

## 4. ELSP submission (Gen9 RCS — classic ELSP-register model) ✔✔
- `RING_ELSP(RCS)` = **0x2230** (base 0x2000 + 0x230), **write-only**. `RING_EXECLIST_STATUS_LO/HI` = 0x2234/0x2238.
- **Gen9 is NOT ELSQ.** `HAS_LOGICAL_RING_ELSQ` is false on Gen9 (Gen11+ only). Do **not** use `RING_EXECLIST_SQ_CONTENTS`(0x510)/`RING_EXECLIST_CONTROL`(0x550)/`EL_CTRL_LOAD`.
- **The 4-dword write sequence** (all to the same ELSP reg 0x2230, forcewake-held):
  1. `upper_32_bits(desc[1])`  — element 1 (second/optional context), HIGH
  2. `lower_32_bits(desc[1])`  — element 1, LOW
  3. `upper_32_bits(desc[0])`  — element 0 (the context to run), HIGH
  4. `lower_32_bits(desc[0])`  — element 0, LOW ← **this last write triggers the load (implicit, no doorbell)**
  - **`desc[1] = 0`** for single-context submission.
  - ELSP is write-only → **post via a read of `EXECLIST_STATUS_LO` (0x2234)**.

## 5. CSB completion (poll via MMIO — simplest for bare-metal) ✔✔
- MMIO CSB: `RING_CONTEXT_STATUS_BUF_LO/HI(i)` = base **+0x370** + i·8 (status dword) / +4 (ctx-ID dword), **6 entries** (i=0..5). Pointer: `RING_CONTEXT_STATUS_PTR` = base **+0x3a0**. (RCS: CSB≈0x2370, PTR≈0x23a0.) *(HWSP-shadow alternative: CSB at HWSP dword 0x10, write-ptr at 0x1f — needs the status-page shadow + an `rmb`/lfence; MMIO is simpler.)*
- **PTR register:** HW write-ptr = bits [2:0]; SW read-ptr = bits [10:8]; `GEN8_CSB_PTR_MASK = 0x7`.
- **The 0x7 reset quirk:** after reset/power-up the write-ptr reads all-ones (`0x7`, invalid) — treat it as **5** so the first entry processed is CSB[0].
- **Status-dword bits:** `IDLE_ACTIVE`=1<<0 (**switch-IN**), `PREEMPTED`=1<<1, `ELEMENT_SWITCH`=1<<2, `ACTIVE_IDLE`=1<<3 (**went idle**), `COMPLETE`=1<<4, `LITE_RESTORE`=1<<15. *(Trap: `IDLE_ACTIVE`=switch-in vs `ACTIVE_IDLE`=idle are opposite events at opposite bits.)*
- **Poll loop:** read PTR; `write = PTR & 0x7`; process entries `(read, write]` (unwrap if read>write by +6); cross-check each entry's **ctx-ID dword** against the descriptor you submitted; publish new read-ptr into bits [10:8] via masked write.
- **Our no-op's "ran to completion" signature:** an entry with `IDLE_ACTIVE` (switched in) → later an entry with `ACTIVE_IDLE` (completed, engine idle), ctx-ID matching ours. The no-op's PIPE_CONTROL seqno (GGTT) landing is the corroborating proof.

## 6. PPGTT binding via the LRC (why this whole arc exists) ✔✔
i915 doc comment, verbatim: *"PPGTTs are actually per-context; having the PDPs contained [in the context] means you don't need to do a switch_mm yourself — the GPU will do it for you on the context switch."* The PML4/PDP root in the LRC register-state image is loaded by the engine when it switches the context in. A pure legacy ring has no per-context PDP state → the engine resolves data through **GGTT** (exactly our BUILD-4-1 finding). The EU's **A64 stateless** dataport accesses then translate through that active per-context PPGTT — so binding the PML4 via the LRC is what isolates EU data accesses. *(Architecturally certain; the verbatim PRM "A64 uses PPGTT" sentence wasn't fetchable — flag for direct PRM read, low risk.)*

## 7. Failure-mode → diagnostic decision tree (the blind-run instrument) ✔
Capture: CSB dump + `EXECLIST_STATUS` + `RING_HEAD` + the fault address (iter7 capture) + the active PML4 root (BUILD-4-1 capture).
- **No CSB entry ever + EXECLIST_STATUS shows nothing accepted** → submission rejected. **Check cheapest first:** (d) `RING_MODE_GEN7` bit-15 readback + forcewake held during ELSP write; **then** (a) descriptor `VALID`/addressing bits. (These three share this signature; execlist-enable/forcewake are the more common mistakes.)
- **CSB: active → COMPLETE immediately, `RING_HEAD` unmoved** → empty ring / bad `CTX_RING_TAIL`, or the WaIdleLiteRestore same-tail trap.
- **CSB: active → fault/hang** → bad LRC image / ring-start / PDPs **(b)** vs bad GGTT mapping **(e)** → **split by fault address: GGTT range ⇒ (e); PPGTT range ⇒ (b).**
- **`RING_HEAD` advances + work ran, but CSB all-zero + no IRQ** → HWSP/CSB not set up (c).

## 8. Gen9 quirks (the iteration-savers) ✔/✔✔
1. **WaIdleLiteRestore (bdw, skl):** never resubmit a context with the **same `RING_TAIL`** (HW reads `HEAD==TAIL` as an empty ring and wedges). Pad each submission with **2 `MI_NOOP`s (`WA_TAIL_DWORDS = 2` = 8 bytes)** so the tail always advances. ✔✔ *(corrects the brief's "8 dwords".)*
2. **`GFX_RUN_LIST_ENABLE` = bit 15** (Gen9) — not the Gen11 `DISABLE_LEGACY_MODE`. ✔✔
3. **Gen9 ctx-id at bit 32; addressing-mode = 3 for 48-bit.** ✔✔
4. **48-bit = single PML4 into `CTX_PDP0` (0x31/0x33)**, not 4 PDPs. ✔✔
5. **Golden render-state batch:** Gen9 needs its own null-render-state init batch before the first *real* workload primes pipeline state — **moot for the no-op (4X-1)**, **revisit for 4X-2 compute**. ✔ (medium)
6. **`has_gt_uc=1` on Gen9 is a red herring** — the GuC die-block exists but execlist submission needs no firmware (i915 v5.4 hard-disabled GuC submission). ✔✔
7. **`GEN9_CTX_RCS_INDIRECT_CTX_OFFSET_DEFAULT = 0x26`** if you use indirect-ctx; else leave zeroed. ✔

## 9. Primary sources
- i915 `gt/intel_lrc.c` (v4.4 / v4.19 / v5.4): `enable_execlists`, `execlists_elsp_write`/`write_desc`, `execlists_init_reg_state`, `process_csb`, descriptor build, the doc comment. raw.githubusercontent.com/torvalds/linux/{v4.4,v5.4}/drivers/gpu/drm/i915/...
- i915 `gt/intel_lrc_reg.h` (v5.4/v6.1): `CTX_*` offsets, `ASSIGN_CTX_PML4/PDP`, `CTX_DESC_FORCE_RESTORE`, CSB defines.
- i915 `gt/intel_engine_regs.h` (v6.1): `RING_ELSP`(+0x230), `RING_MODE_GEN7`(+0x29c), `RING_HWS_PGA`(+0x80), `RING_CONTEXT_STATUS_*`(+0x370/+0x3a0), `GEN8_RING_PDP_*`(+0x270+n·8).
- i915 `gt/intel_engine.h` / `intel_engine_types.h` (v5.4): HWSP CSB indices, MMIO-or-HWSP note.
- i915 `i915_pci.c` (v5.4): `GEN9_FEATURES` (ppgtt_size=48, full PPGTT, logical-ring-contexts).
- i915 `gt/uc/intel_uc.c` (v5.4): GuC-submission-disabled proof.
- Intel SKL PRM Vol 06 (Command Stream Programming / logical context) — structural corroboration.
- **Open (third oracle — not yet obtained):** Fuchsia `msd-intel-gen` (`engine_command_streamer.cc`, `register_state_helper`), Mesa `src/intel/common`, or PRM Vol 2d Context Descriptor table.

## 10. Bottom line for BUILD-4X-1
The LRC context image, the descriptor, the ELSP write, and the CSB poll are now **fully specified with cross-validated offsets/bits** — the hardest ISC (the context image) is transcription, not archaeology. The two things that still require eyes-on: (1) obtain a **non-i915 oracle** and hand-lay the descriptor + `CTX_*` byte fixtures (the 4X-1 byte-oracle deliverable — closes the "two i915 readers agree even if both wrong" loophole); (2) the **RPCS** value for the UHD 630 (read from fuse or hardcode). Everything else is build-from-spec.
