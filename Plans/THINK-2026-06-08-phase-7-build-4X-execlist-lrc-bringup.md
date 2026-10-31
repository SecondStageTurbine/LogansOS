<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: Phase 7 BUILD-4X — minimal GuC-free execlist/LRC bring-up (the only path to a hardware-enforced device boundary)

**Date:** 2026-06-08
**Seat:** PM/Cerebro (design pass for the second bring-up arc; not a dispatch)
**Context:** BUILD-4-1 conclusively falsified PPGTT-on-the-legacy-ring on Gen9 — all three root mechanisms (32-bit PDP, 48-bit PML4, Gen7 PP_DIR_BASE) read `select=GGTT` over 4 OptiPlex iterations (evidence in `Plans/evidence/phase7-build41-*` + `Plans/BLOCKER-2026-06-08-...`). The Gen9 render engine binds PPGTT to the **Logical Ring Context**; without an execlist/LRC context, the engine has no per-process address space. So the device-boundary governance line (§9.8) requires the execlist/LRC mechanism — the deferred "(B)" lift. PL chose to drive through it (not defer).
**Related:** ADR-032 (4a legacy-ring driver; reused command stream), BUILD-4-1 (the PPGTT page tables + PTE oracle + PDP capture, all reused), THINK-2026-06-07-phase-7-build-4 (the falsified (A) reframe), ANSPEC-001 §9.7/§9.8. **Blind-hardware rule governs this — failure-mode/discriminator mandatory per build.**

## 1. Goal
Engage **per-context PPGTT** by submitting work through the **execlist** path with a **Logical Ring Context (LRC)** that carries the PPGTT root — the *only* way to give the Gen9 render engine a per-process address space. Once a context carries its own PPGTT, the EU's A64 data accesses resolve through *that context's* page tables (`select=PPGTT`), and a cross-tenant/RW-on-RO access faults at the device boundary. This reaches the fourth governance line (§9.8: spatial isolation, the BUILD-4-2 negative test).

## 2. What changes — and what's reused (this is an addition, not a from-scratch rewrite)
**Reused wholesale (proven):**
- The **4a command stream** — PIPELINE_SELECT → STATE_BASE_ADDRESS → MEDIA_VFE_STATE → IDD → GPGPU_WALKER → PIPE_CONTROL(seqno). The compute itself is unchanged; only *how the batch is submitted* changes.
- **BUILD-4-1's PPGTT page tables** (PML4→PT construction), the **RW/RO PTE byte-oracle**, the **PDP-root capture**, the GGTT-clear self-proving gate, and the block-sink/fault-decode instrumentation. The LRC will simply *point at* the page tables 4-1 already builds.

**New (the execlist/LRC mechanism):**
- An **LRC context image** — the per-engine register-state context in memory (ring buffer pointers + the full RCS register state + **the PPGTT root stored at its context-image offset**). This is the artifact that binds PPGTT to the context, and it is the hard part.
- A **context descriptor** (64-bit: LRC GGTT address + context ID + flags).
- **ELSP submission** — write the descriptor to the Execlist Submit Port (the 4-DWord ELSP write sequence) to submit the context.
- **Context Status Buffer (CSB)** handling — read the engine's context-switch events (from the HWSP/MMIO) to know the context ran/idled (poll first; interrupts later if needed).

## 3. Slicing — isolate the unknowns (each a bare-metal run, like 4a)
The unknowns are independent; slice so each run answers exactly one:

- **BUILD-4X-1 — Execlist submission engages (NO PPGTT yet).** Submit the proven 4a Add via an LRC + ELSP, with the context's address space still GGTT (or PPGTT pointing at the same buffers — but the gate here is *not* PPGTT). **Gate:** the Add retires, the CSB shows the context ran and idled, the seqno lands — i.e. **a context submitted via ELSP executes at all.** This proves the hardest unknown (does GuC-free execlist submission work on this silicon?) decoupled from PPGTT. Reuses the entire 4a batch; the only new surface is the LRC + ELSP + CSB.
- **BUILD-4X-2 — PPGTT-in-LRC engages (the 4-1 positive gate, now on the context).** The LRC carries a per-tenant PPGTT root; data buffers PPGTT-only/unmapped-in-GGTT (the 4-1 self-proving gate). **Gate:** the Add retires with **`select=PPGTT`**. This is 4-1's original goal, finally reachable.
- **BUILD-4X-3 — the negative test (4-2's gate).** Cross-tenant OOB access + RW-on-RO write must **fault** at the device boundary; a legitimate in-context access succeeds. **Gate:** the §9.8 line — `tenant B → tenant A's tensor → page fault, denied by PPGTT`. The fourth governance line, silicon-enforced.

Order rationale: 4X-1 de-risks "execlists at all" (the biggest unknown) before coupling to PPGTT; 4X-2 proves PPGTT-in-context; 4X-3 proves the boundary. Don't conflate them on one blind run.

## 4. The hard part, named honestly
**The LRC context image is the risk.** It's a precise, ~register-state-sized memory layout (the RCS logical context — ring head/tail/start/control, the register state context with dozens of MMIO values at fixed dword offsets, and the PPGTT root at its specific offset). Getting it wrong → the context never runs, or runs wrong, or hangs. This is what i915 spent real effort on. Expect 4X-1 to be the multi-iteration one (like 4a's eight): the failure mode "context submitted but never executes" is the default first result, and localizing it (descriptor wrong vs register-state wrong vs ring-pointer wrong) is the work. **Budget for it. Treat 4X-1 as the real bring-up.**

## 5. Failure-mode + discriminator section (REQUIRED — blind-hardware; this is 4X-1's, the riskiest)
The CSB + the existing fault/seqno/select capture are the instruments; ISC adds ELSP-write readback + CSB dump.

| # | Mode | Likely cause | Discriminator |
|---|---|---|---|
| 1 | **Context never runs** (no seqno, no CSB event) | ELSP write malformed, context descriptor wrong, or LRC register state invalid | CSB dump (did the engine acknowledge/switch-in at all?) + ELSP readback + the HWSP head — distinguishes "submission rejected" from "submitted but context-state bad" |
| 2 | **Context runs, wrong/no result** | ring head/tail/start in the LRC point at the wrong batch | seqno absent but CSB shows switch-in→idle → the ring pointers in the context image |
| 3 | **Hang / context-switch storm** | a faulting context immediately resubmits; or register state wedges the engine | CSB flood vs silence; RING_FAULT/INSTDONE (4a signature); the fault address if it faults |
| 4 | **Runs correctly but that's all 4X-1 needs** | — | ✅ execlist submission works; proceed to 4X-2 (PPGTT) |

**Capture-before-you-need-it:** add the **CSB ring dump** and the **ELSP-write readback** now (4X-1) — without them, "context never runs" is a blind mystery, which is the exact failure the blind-hardware rule forbids shipping into.

## 6. Risks / honest scope
- **This is a second bring-up.** I'm not going to soften that — the enforcement-first reframe was meant to avoid it and couldn't. 4X-1 is a genuine multi-iteration blind-hardware arc.
- **GuC-free execlists** are the target (no GuC/HuC firmware dependency, per ADR-032 D10). Gen9 supports direct ELSP submission without GuC — confirm in 4X-1.
- **The payoff is bounded and known:** even when this lands, §9.8's *temporal-fairness* gap (no preemption) remains — BUILD-4X buys spatial isolation, not preemptive fairness.

## 7. Standing-rule requirements
- **Blind-hardware:** the failure-mode/discriminator section (above) is mandatory per slice; the run must be conclusive on failure (CSB + select + fault-addr capture make it so).
- **Byte-oracle:** the **LRC context-image layout** and the **context descriptor** are on-wire/in-memory byte formats → hand-laid byte oracles (the descriptor, and the key register-state offsets incl. the PPGTT-root slot), asserted independent of the construction code. (The PTE oracle from 4-1 already stands.)
- **No GuC dependency; no silent mechanism switch.** Execlist is engaged or the build reports why.

## 8. Open decisions for the PL
1. Accept **BUILD-4X = minimal GuC-free execlist/LRC bring-up**, sliced 4X-1 (execlist submits + runs) → 4X-2 (PPGTT-in-LRC, `select=PPGTT`) → 4X-3 (the negative boundary test)?
2. Branch handling: **4-1 is not merged** (gate red by design); **4X branches from the 4-1 branch** to reuse the page-table machinery + oracles, replacing legacy-ring submission with ELSP/LRC. Confirm.
3. Firmware stance: GuC-free direct ELSP (recommended, no firmware dep) — confirmed as the first cut, with GuC as an explicit later fallback only if direct ELSP proves unviable.

## 9. Recommendation in one line
Reopen BUILD-4 as **BUILD-4X — minimal GuC-free execlist/LRC bring-up** — branch from 4-1, reuse its PPGTT page tables + PTE oracle + the 4a command stream, and add an LRC context image (carrying the PPGTT root) submitted via ELSP with CSB completion; slice **4X-1** (prove execlist submission executes a context at all — the real bring-up, multi-iteration, with CSB+ELSP capture) → **4X-2** (PPGTT-in-LRC: `select=PPGTT`) → **4X-3** (the cross-tenant negative test: the silicon-enforced device boundary, §9.8). It's a second bring-up, named honestly — and the only path to the fourth governance line on this hardware.
