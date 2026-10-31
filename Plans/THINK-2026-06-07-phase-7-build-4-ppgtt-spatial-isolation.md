<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: Phase 7 BUILD-4 — per-context spatial isolation (PPGTT): the device-boundary governance line

**Date:** 2026-06-07
**Seat:** PM/Cerebro (design-questions pass for the hard build; not a dispatch)
**Context:** Phase 7 BUILD-1/2/2L/3 made three governance lines real on the **temporal** substrate (measure §9.5, arbitrate-enforced §9.6, own §9.7) — the governance thesis is *demonstrated*. BUILD-4 is the **fourth and last line**, and the hard one: `RW denied at the device boundary, concurrent`. It is **hardening, not demonstration** — but it's where "the kernel governs AI" stops being a page-table we *discuss* and becomes one we *build*.
**Related:** ADR-032 (BUILD-4a Intel driver; C1 names execlist/ELSP as the next mechanism; the fault decode already captures GTT-vs-PPGTT select + fault address), ADR-035 (BUILD-3 sharing — leaves a device-resident-shared-tensor enforcement seam), ANSPEC-001 §9.7. The blind-hardware rule (CLAUDE.md) governs this build.

## 1. The line, and what "enforced" means at the metal
BUILD-3 enforces RO sharing at the **op boundary** (`sys_tensor_write` → EPERM). BUILD-4 enforces it at the **page-table boundary**: a tenant's tensors live in *its own* GPU address space, and a cross-tenant or write-violating access **faults in hardware** because the page tables don't permit it. That closes the BUILD-3 device-resident seam and makes the fourth governance line real:
```
tensor X: PID7 RO mapping granted; PID12 RW request denied at device boundary
```
— denied not by a kernel check the tenant could be racing, but by the **PPGTT entry itself**.

## 2. The de-risking reframe (the most important call in this THINK)
"Per-context spatial isolation" conflates two things that have very different costs. **Separate them:**

- **(A) Spatial isolation / enforcement** — each tenant's memory in its own address space; cross-tenant access page-table-denied. **This is the governance line.** Achievable with **PPGTT on the *legacy ring*** (the proven BUILD-4a path): construct per-tenant PPGTT, load the page-directory-pointers (PDPs) before each tenant's batch, run the batch — the EU's A64 data accesses resolve through *that tenant's* PPGTT and physically cannot reach another's memory. Execution stays **temporal** (one tenant's batch at a time, PPGTT swapped between), but the *boundary is real and hardware-enforced*.
- **(B) Concurrent execution** — multiple tenants resident/scheduled without serialization. Requires the full **execlist/LRC** rewrite (hardware context save/restore, ELSP submission, context-status interrupts). Big lift, essentially a second bring-up.

**The thesis needs (A), not (B).** "RW denied at the device boundary" is an *enforcement* property, satisfied by per-tenant PPGTT — concurrency is a *performance* property. So **BUILD-4 should do (A) on the legacy ring first** — a far smaller, lower-risk lift that reuses the entire proven 4a substrate — and **defer (B) execlist concurrency** to a later hardening (BUILD-4b / Phase-7+). This is the single decision that most de-risks the hardest remaining work.

## 3. What changes from BUILD-4a (incremental, not a rewrite — under reframe A)
Keep, unchanged (proven on silicon): the legacy render ring, GGTT for the ring/command-stream/state-heap/kernel/seqno, STATE_BASE_ADDRESS (Instruction Base via GGTT), the NEO indirect-data payload, forcewake, the fault-address + GTT/PPGTT-select capture. **Add:**
1. **Per-tenant PPGTT construction** — 48-bit Gen8+ page tables mapping *that tenant's* data buffers (its tensors) into its own address space, with per-page **R/W permission bits** (RO-shared tensors mapped read-only).
2. **PDP load before each tenant's batch** — program the PPGTT root (via MI_LOAD_REGISTER_IMM in the ring, or the context's PP_DIR) so the EU's A64 data accesses for that batch resolve through that PPGTT. Data buffers move from GGTT-global to per-tenant PPGTT; ring/command/state stay in GGTT.
3. **Wire residency (BUILD-1 §9.5) and sharing (BUILD-3 §9.7) to PPGTT** — a tensor's device residency is now its mapping in a tenant's PPGTT; a shared RO tensor appears in the grantee's PPGTT as an RO page pointing at the same physical frame.

## 4. The validation is a NEGATIVE test (a new shape — call it out)
Every prior build proved *authorized* access *succeeds*. Isolation can only be proven by showing *un*authorized access **fails**. So BUILD-4's gate is a **negative test**: a batch deliberately issues an A64 access to an address that is *another tenant's* tensor (or a write to an RO-mapped page), and it **must fault**. Crucially — and this is why the build is subtle — *a fault alone is not success*; the fault must be the *right* fault. The driver **already captures** the fault address and the GTT-vs-PPGTT select (iter7/ADR-032), which is exactly the discriminator:
- **fault in PPGTT at the OOB address** → isolation works ✅
- **no fault (access succeeds)** → PPGTT not engaged; the access fell through to GGTT-global ❌ (the silent-hole failure)
- **fault at the tenant's OWN buffer** → page-table construction is broken ❌
- **the grantee reads the other tenant's data** → PDPs not swapped between tenants ❌ (the worst case — a silent isolation hole that looks like success)

## 5. Design questions
- **Q1 — (A) PPGTT-on-legacy-ring first, (B) execlists deferred.** Recommended (§2). The PL call: accept enforcement-first (temporal + spatial-isolated) as the BUILD-4 scope, with execlist concurrency as a named later hardening?
- **Q2 — Isolation unit = per-principal PPGTT.** One address space per principal (ProcessId), aligning with the §9.5/§9.6/§9.7 principal. A tenant's tensors map into its PPGTT; nothing else does.
- **Q3 — RO enforcement via PPGTT permission bits (closes the BUILD-3 seam).** A shared RO tensor is mapped **read-only** in the grantee's PPGTT (same physical frame as the owner's RW mapping). A write faults in hardware. This is the device-side completion of BUILD-3's op-boundary RO.
- **Q4 — Slicing.** Likely: **4-1** PPGTT construct + PDP load + the *positive* path (a single tenant's Add/matmul/GELU runs correctly through its own PPGTT instead of GGTT — proves PPGTT is engaged without breaking compute); **4-2** the *negative* enforcement test (cross-tenant OOB access faults; RW-on-RO faults) — the governance line. Each is a bare-metal Stage-B run.

## 6. Failure-mode + discriminator section (REQUIRED — blind-hardware rule)
This is a bare-metal driver lift; expect multiple iterations (4a took eight). The fault-address + GTT/PPGTT-select capture is already in place — use it. Modes:

| # | Mode | Likely cause | Discriminator |
|---|---|---|---|
| 1 | **Positive path faults** (own Add/matmul faults under PPGTT) | PPGTT page tables don't map the tenant's own buffers; or PDP load wrong | fault addr = tenant's own buffer + select=PPGTT → page-table construction bug; select=GGTT → PDPs not loaded (still on GGTT) |
| 2 | **Negative test does NOT fault** (OOB access succeeds) | PPGTT not engaged — access fell through to GGTT-global (the silent hole) | select=GGTT on the access → PPGTT never took effect; this is the most dangerous false-pass, the negative test exists to catch it |
| 3 | **Grantee reads another tenant's data** | PDPs not swapped between tenants (context confusion) | a positive cross-read test: B reads its OWN tensor (must succeed) + B reads A's address (must fault); if B gets A's data, PDPs aren't swapping |
| 4 | **Hang instead of fault** | fault not surfaced; or PPGTT setup wedges the ring | last breadcrumb + RING_FAULT/INSTDONE (the 4a EU-idle-vs-stuck signature) |
| 5 | **RW-on-RO doesn't fault** | PPGTT PTE written R/W instead of RO for a shared tensor | the PTE permission bits + the write-fault test |

**Capture-before-you-need-it:** the fault address + GTT/PPGTT select already exist (good). What may be missing: logging the *active PDP root* per batch (to prove which PPGTT was loaded when a fault fires — discriminates mode 3). Add that capture in 4-1.

## 7. Risks
- **Silicon-grade hardness.** PPGTT setup + PDP load on Gen9 is real driver work (a chunk of what i915 does); blind, multi-iteration. Budget for it like 4a.
- **The worst failure is a silent isolation hole** (modes 2/3): isolation *appears* to work because compute succeeds, but a tenant *can* reach another's memory. The negative + cross-read tests are non-optional precisely because the dangerous failure looks like success. **The gate is the negative test passing, not the positive path.**
- **Don't scope-creep into execlists.** (B) is a separate, later lift; pulling it in turns a tractable enforcement build into a second full bring-up.

## 8. Standing-rule requirements
- **Blind-hardware:** the failure-mode/discriminator section (above) is mandatory before DISPATCHED; the run must be conclusive on failure (the negative test + fault-address/PDP/select capture make it so).
- **Byte-oracle:** the PPGTT page-table entry layout is an on-wire/in-memory byte format → a **hand-laid PTE byte oracle** (a PPGTT entry with known physical address + R/W/present bits, asserted independent of the construction code). This is exactly the format-with-a-byte-layout the rule targets.
- **No CPU fallback / no silent mechanism switch** (ADR-032 stance) — PPGTT is engaged or the build reports why.

## 9. Open decisions for the PL
1. **Q1 (the call):** accept **enforcement-first** — PPGTT-on-legacy-ring (spatial isolation, temporal execution), execlist concurrency deferred — as BUILD-4's scope?
2. **Q4:** the 4-1 (positive: compute through PPGTT) / 4-2 (negative: cross-tenant fault) slice, each a bare-metal run?
3. Anything else gating: is the device-boundary line worth the bare-metal driver lift *now*, given the thesis is already demonstrated on the temporal substrate — or is BUILD-4 a deliberately-scheduled hardening we open when the OptiPlex loop is available?

## 10. Recommendation in one line
Do BUILD-4 as **PPGTT-on-the-legacy-ring for spatial isolation/enforcement** (reusing the entire proven 4a substrate, adding per-tenant page tables + PDP load + RO permission bits), sliced into 4-1 (compute correctly through PPGTT) and 4-2 (the negative test: cross-tenant/RW-on-RO access faults at the device boundary), with the fault-address/PDP/select capture making each blind run conclusive — and **defer execlist/LRC concurrency** as a later hardening. That gets the fourth governance line — the page-table-enforced device boundary — at the smallest honest risk.

---

## ADDENDUM 2026-06-07 — §10's recommendation is FALSIFIED; the path is execlist/LRC

BUILD-4-1's OptiPlex run (3 iterations) **conclusively falsified the enforcement-first / PPGTT-on-the-legacy-ring premise** (reframe (A) in §2). The discriminators worked exactly as designed: both root-register modes were correctly programmed and both read `select=GGTT`.

| iter | mode | GFX_MODE | result |
|---|---|---|---|
| 2 | 48-bit (PML4→PDP0) | bit7 set (0x80) | fault @0x130000, `select=GGTT` |
| 3 | 32-bit (4× PDP→PD) | bit7 clear | fault @0x130000, `select=GGTT` |

**Finding:** on Gen9 the render engine binds PPGTT to the **Logical Ring Context** — without an execlist/LRC context holding the PPGTT descriptor, the engine has no per-process address space and resolves all EU A64 data accesses through GGTT, regardless of how the PDP/PML4 registers and GFX_MODE are programmed. The "smaller lift" of §2 reframe (A) **does not exist on this silicon.**

**Ruled out — the PP_DIR_BASE aliasing-PPGTT experiment** (a proposed cheap pre-check): same root cause (a context-less engine ignores any PPGTT root register, PP_DIR_BASE included), and even if it engaged, aliasing PPGTT is a *single shared* address space — **not per-tenant**, so it cannot deliver the isolation goal even in the best case. Not worth the iteration.

**Deferring the line is not on the table** (PL standing preference — drive through hard hardware bring-up loops). So the path is reframe **(B): a minimal execlist/LRC bring-up** — the only way to engage per-context PPGTT on Gen9. This is a second bring-up arc (execlist submission via ELSP, an LRC context image holding the PPGTT root, context-status interrupts) — treat it like the 4a bring-up (multi-iteration, blind, failure-mode/discriminator section mandatory).

**Not wasted:** the BUILD-4-1 PPGTT machinery is fully reused — the PML4→PT page-table construction, the RW/RO PTE byte-oracle, and the PDP-root capture all stand; the LRC simply points the engine at the page tables 4-1 already builds. The execlist build adds the *context mechanism* around the *page tables that already work*.

**Revised recommendation:** reopen BUILD-4 as **BUILD-4X — minimal GuC-free execlist/LRC bring-up** (engage the engine via ELSP with an LRC carrying the per-context PPGTT root), reusing 4-1's page-table machinery; then the original 4-1 positive gate (`select=PPGTT`) and 4-2 negative gate (cross-tenant fault) run *on the execlist context*. A focused execlist THINK/dispatch follows.
