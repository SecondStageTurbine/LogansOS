<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Phase 7 BUILD-4X-1 — GuC-free execlist submission runs a context (the bring-up) → Jarvis

**Date:** 2026-06-08
**From:** PM/Cerebro
**To:** Jarvis (new branch reusing BUILD-4-1's machinery: `codex/phase-7-build-4X-1-execlist-bringup-jarvis`)
**Status:** DISPATCHED. **Blind-hardware (OptiPlex) — this is the real bring-up; expect multiple iterations like 4a. Designed to be conclusive on failure.**
**Builds on:** BUILD-4a (`ab8fa0d`, the proven command stream) + BUILD-4-1 (the PPGTT page tables + PTE oracle + capture, reused) + the THINK `Plans/THINK-2026-06-08-phase-7-build-4X-execlist-lrc-bringup.md` (PL accepted the 4X scope). ADR-032.

## 0. Scope — execlist submission runs a context, nothing more
BUILD-4-1 proved the legacy ring cannot engage PPGTT (the engine binds it to the LRC). **4X-1's only job: prove a context submitted via the execlist path (ELSP) actually executes on this Gen9 silicon, GuC-free.** Decoupled from PPGTT — that's 4X-2. **No data buffers, no PPGTT data test, no isolation.** This isolates the single hardest unknown (does GuC-free execlist submission work at all?) before anything is layered on it.

## 1. Refinement of the THINK's slice — use the NO-OP, not the Add
The THINK said "submit the Add via execlist." **Use the finalizer no-op (`NOOP_KERNEL_GEN9`, EOT-only, no memory access) instead.** Reason: the Add dereferences data buffers — if it faults, you can't tell "execlist didn't run" from "PPGTT data mapping is wrong." The no-op touches **no data** (instruction fetch is Instruction-Base/GGTT, proven; the seqno write is GGTT post-sync, as in 4a), so a no-op that retires proves **the execlist context ran the dispatch pipeline** with *zero* PPGTT-data dependency. Clean isolation. The LRC still carries a (minimal, valid) PPGTT root, but nothing dereferences it this build.

## 2. ISCs
- **ISC-1 — Build the LRC context image.** Construct the Gen9 RCS Logical Ring Context in GGTT-mapped memory per the KBL PRM (Vol. 2d / the per-engine logical context layout): the ring-buffer registers (RING_BUFFER_START/CONTROL/HEAD/TAIL pointing at the context's ring containing the no-op batch), the register-state context (the MMIO state at its fixed dword offsets), and the **PPGTT root field** set to a minimal valid PPGTT. Cite the PRM offsets. **This is the hard part — budget the iterations here.**
- **ISC-2 — Context descriptor.** The 64-bit execlist context descriptor (LRC GGTT base + context ID + the valid/legacy/addressing-mode flags) per the PRM.
- **ISC-3 — ELSP submission.** Submit the descriptor via the Execlist Submit Port (the 4-DWord ELSP write sequence to the RCS ELSP register). Hand-verify the write order against the PRM.
- **ISC-4 — CSB completion.** Read the Context Status Buffer (in the HWSP / the CSB MMIO range) to observe the context **switch-in → idle**. Poll (no interrupt needed this build); confirm the engine processed the submission.
- **ISC-5 — Capture-before-you-need-it (REQUIRED — the run is blind without these).** Add: a **CSB ring dump** (the raw status DWords), an **ELSP-write readback** (and the EXECLIST_STATUS register), and the HWSP head — alongside the existing seqno + RING_FAULT/INSTDONE capture. Without the CSB dump, "context never ran" is an undiagnosable mystery, which the blind-hardware rule forbids.
- **ISC-6 — Byte oracles (REQUIRED).** Hand-laid byte fixtures, asserted independent of the construction code, for: the **context descriptor** (a known LRC base + ID + flags → the 64-bit value), and the **key LRC register-state offsets** (at minimum the ring-pointer block and the PPGTT-root slot — the dwords whose wrongness silently breaks the context). The 4-1 PTE oracle stands.
- **ISC-7 — Standards.** Default-off (`accel-intel`); SPDX; SAFETY+PRM citations on every MMIO/context write; `cargo fmt`; host suite (the new descriptor/LRC oracles + 4-1 PTE oracle + accounting/admission/sharing regressions) green; `shard-coverage`; `clippy -D warnings` (CI/Linux); builds on+off. ADR-032 gets a BUILD-4X-1 section (the execlist/LRC mechanism + the failure-mode table). A new scenario `accel_intel_execlist_bringup` builds.

## 3. Failure-mode + discriminator section (REQUIRED — blind-hardware; this is the riskiest run of Phase 7)
The CSB dump + ELSP readback + seqno/RING_FAULT/INSTDONE capture are the instruments. Expect "never runs" first.

| # | Mode | Likely cause | Discriminator |
|---|---|---|---|
| 1 | **Context never runs** (no seqno, no CSB switch event) | ELSP write malformed / descriptor wrong / LRC register-state invalid | CSB dump: did the engine acknowledge a switch-in at all? + ELSP/EXECLIST_STATUS readback. *No CSB event* → submission rejected (descriptor/ELSP, ISC-2/3); *switch-in then immediate idle with no seqno* → context state ran but the ring/batch is wrong (ISC-1 ring pointers) |
| 2 | **Switch-in → runs → wrong** (CSB shows run, no seqno) | RING_BUFFER_START/HEAD/TAIL in the LRC point at the wrong batch, or the ring isn't the no-op | the CSB confirms execution; seqno absence + the ring-pointer oracle localizes |
| 3 | **Hang / context-switch storm** | register-state context wedges the engine, or a faulting context resubmits | CSB flood vs silence; RING_FAULT/INSTDONE (the 4a stuck-vs-idle signature) |
| 4 | **No-op retires, seqno lands, CSB switch-in→idle** | — | ✅ execlist submission works GuC-free; 4X-1 done → 4X-2 (PPGTT-in-LRC) |

The gate is **the CSB confirming a clean switch-in→idle AND the no-op seqno landing** — not just "no hang." That distinguishes "context actually executed" from "engine did nothing quietly."

## 4. Gate (two-stage)
- **Stage A (you):** builds (on+off); the descriptor + LRC byte oracles + 4-1 PTE oracle + regressions green; clippy; fmt; shard-coverage.
- **Stage B (PL, OptiPlex):** the no-op context submitted via ELSP **runs** — CSB shows switch-in→idle, the no-op seqno lands, no hang/fault. Report the CSB dump + seqno + EXECLIST_STATUS + (on any miss) the discriminator branch.

## 5. Scope guard
Execlist-runs-a-context only. **Do NOT** do the PPGTT-data positive test (`select=PPGTT` — that's 4X-2), the cross-tenant negative test (4X-3), or any data-buffer compute (use the no-op — no data). **Do NOT** take a GuC/HuC firmware dependency (direct ELSP, GuC-free, per ADR-032 D10). **Do NOT** rework 4-1's PPGTT page tables / PTE oracle (reuse as-is; they're the base for 4X-2). One context, submitted via ELSP, proven to run.

## 6. Branch + merge
**Reuse BUILD-4-1's machinery** (the PPGTT page-table module + PTE oracle + capture + block-sink wiring): branch from the 4-1 branch, or cherry-pick those onto main — your call — and **replace the legacy-ring submission with the execlist/LRC path**. 4-1 itself stays unmerged (its gate is red by design; it's the conclusive legacy-ring record).
Close-gate: `git fetch origin && git rebase origin/main` (main has advanced — the 4X THINK + §9.8), re-run Stage-A, push. PM `pm-merge-check` → `--ff-only` → `main`. Then **4X-2** (the LRC carries a per-tenant PPGTT root, data buffers PPGTT-only → `select=PPGTT`: 4-1's gate finally reachable), then **4X-3** (the cross-tenant negative test — the silicon-enforced device boundary, §9.8).

---

## ADDENDUM 2026-06-08 — research landed; build from the cited spec, not blind

The deep-research lanes returned. **The authoritative spec is `Plans/RESEARCH-2026-06-08-gen9-execlist-lrc-bringup.md`** — cross-validated offsets/bits/sequences from i915 (v4.4–v6.1) for every ISC. Build the LRC image, descriptor, ELSP write, and CSB poll **from that spec**; it turns ISC-1 (the context image) from archaeology into transcription. Key bindings (all in the spec with sources):

- **Bring-up (enable_execlists):** forcewake held; `RING_MODE_GEN7`(base+0x29c) ← `0x80008000` (`GFX_RUN_LIST_ENABLE`=bit15, *Gen9 bit, not Gen11's*); `RING_MI_MODE` clear `STOP_RING`; `RING_HWS_PGA`(base+0x80) ← GGTT status-page offset; HWSTAM ← ~0; GGTT-pin LRC+ring.
- **LRC image:** 22-page RCS register-state + 1 PPHWSP page; the **+1 value-slot rule** and **descending PDP order** are the transcription traps; **48-bit → single PML4 into `CTX_PDP0` (dwords 0x31/0x33)**; `R_PWR_CLK_STATE`/**RPCS (0x42) is the one SKU value** — read from fuse or hardcode for the UHD 630.
- **Descriptor:** `VALID`(bit0)|`PRIVILEGE`(bit8)|(addr_mode<<3, **=3 for 48-bit**)|LRCA(bits12-31 = GGTT offset of the **PPHWSP page**)|ctx-id(**bits 32-52, Gen9 layout — NOT Gen11's bit-37 split**).
- **ELSP (RCS 0x2230, classic — NOT Gen11 ELSQ):** 4 writes — `el1.hi, el1.lo, el0.hi, el0.lo` (last triggers load); `desc[1]=0` single; post via `EXECLIST_STATUS_LO` (0x2234).
- **CSB (MMIO, simplest):** entries at base+0x370 (6×, status+ctxid dwords), PTR at base+0x3a0 (write[2:0]/read[10:8], mask 0x7); **0x7-reset quirk → start read index at 5**; watch `IDLE_ACTIVE`(bit0, switch-in) → `ACTIVE_IDLE`(bit3, idle); cross-check the ctx-id dword against our descriptor.

**ISC-3 sharpened — the failure-mode table is REPLACED by the research decision tree** (capture: CSB dump + EXECLIST_STATUS + RING_HEAD + fault-addr + active PML4 root):
- No CSB entry + STATUS shows nothing → **check execlist-enable (RING_MODE bit15 readback) + forcewake FIRST**, then descriptor VALID/addr bits (same signature; the former are the common mistakes).
- CSB active→COMPLETE, RING_HEAD unmoved → empty ring / bad CTX_RING_TAIL / same-tail trap.
- CSB active→fault/hang → bad LRC/ring-start/PDPs **vs** bad GGTT — **split by fault address** (GGTT range = mapping; PPGTT range = LRC/PDP).
- RING_HEAD advances + ran but CSB all-zero → HWSP/CSB not set up.

**New ISC — WaIdleLiteRestore (corrected):** never resubmit a context with the **same `RING_TAIL`** (HW reads `HEAD==TAIL` as empty ring and wedges). Pad each submission with **2 `MI_NOOP`s (`WA_TAIL_DWORDS=2`, 8 bytes)** so the tail always advances. *(The brief said "8 dwords" — wrong; it's 2.)*

**ISC-6 byte-oracle — expanded per the open research action:** all spec evidence is i915-sourced (one family). Before freezing the descriptor/LRC byte layout, **obtain a non-i915 oracle** (Fuchsia `msd-intel-gen` `register_state_helper`/`engine_command_streamer.cc`, or Mesa `src/intel/common`, or the PRM Vol 2d Context Descriptor table) and hand-lay the descriptor (ctx-id 32-52, LRCA 12-31) + the `CTX_*` map against it — closing the "two i915 readers agree even if both wrong" loophole. This is the build's byte-oracle deliverable.

**No-op golden-state note:** Gen9 needs a golden render-state batch before first *compute* — but the **no-op touches nothing, so it's moot for 4X-1**; carry it as a flagged risk for 4X-2.
