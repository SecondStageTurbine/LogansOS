<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Phase 7 BUILD-4-1 — PPGTT engaged (compute through per-tenant page tables) → Jarvis

**Date:** 2026-06-07
**From:** PM/Cerebro
**To:** Jarvis (new branch off `origin/main` `dfc2da3`: `codex/phase-7-build-4-1-ppgtt-positive-jarvis`)
**Status:** DISPATCHED. **Blind-hardware (OptiPlex) — likely multi-iteration; the run is designed to be conclusive on failure.**
**Builds on:** BUILD-4a (`ab8fa0d`, the proven legacy-ring/GGTT substrate) + the THINK `Plans/THINK-2026-06-07-phase-7-build-4-ppgtt-spatial-isolation.md` (PL chose enforcement-first; execlists deferred). ADR-032 (driver; fault-addr + GTT/PPGTT-select capture already present).

## 0. Scope — the *positive* slice only
This is **4-1**: prove **PPGTT is engaged** — a single tenant's existing compute (the `accel_intel_add_bringup` Add, and matmul/GELU) runs correctly with its **data buffers resolved through a per-tenant PPGTT instead of GGTT-global**. **No cross-tenant negative test yet** (that's 4-2). Enforcement-first per the THINK: PPGTT on the **proven legacy ring**; **no execlists, no concurrency** (still temporal). This is an *incremental add to 4a*, not a rewrite.

## 1. The self-proving gate (why the positive path proves engagement)
Map the tenant's **data buffers (lhs/rhs/output, and the matmul/GELU operands) ONLY in the PPGTT — not in GGTT.** Keep everything 4a proved in GGTT (ring, command stream, STATE_BASE_ADDRESS/Instruction Base, state heap, EU kernel blob, seqno). The cross-thread A64 payload carries the **PPGTT virtual addresses**. Therefore: **compute succeeds ⟺ PPGTT resolved the access.** If PPGTT isn't engaged, the A64 access falls to GGTT, the address isn't mapped there, and it **faults** — so a correct `[4,6]` is itself proof the data port went through PPGTT. The fault-decode `select` bit confirms it independently.

## 2. ISCs
- **ISC-1 — Construct a per-tenant PPGTT.** Build the Gen8+ 48-bit PPGTT (PML4 → PDP → PD → PT) mapping the tenant's data buffers to PPGTT virtual addresses, **present + R/W** bits set, backed by the same physical frames the buffers already use. SAFETY/PRM citations for the PTE format. (4-2 will use the R/W bit for RO enforcement; here all present+RW.)
- **ISC-2 — Load the PPGTT root before the batch.** Program the PPGTT root on the legacy ring (MI_LOAD_REGISTER_IMM to the PDP/PML4 registers per the Gen9 PRM) so the EU's A64 data accesses for this batch resolve through this PPGTT. Hand-verify the LRI command bytes. Keep STATE_BASE_ADDRESS / Instruction Base on GGTT (proven — command/state fetch is *not* PPGTT).
- **ISC-3 — Move data buffers GGTT→PPGTT (only).** The lhs/rhs/output (+ matmul/GELU operands) map in PPGTT, **unmapped in GGTT**; the cross-thread payload carries their PPGTT VAs. Ring/command/state/kernel/seqno stay GGTT. (BUILD-1 §9.5 residency now reflects the PPGTT mapping; no double-count regressions.)
- **ISC-4 — Capture-before-you-need-it: log the active PDP root.** Add per-batch logging of the **PPGTT root that was loaded** (alongside the existing fault-addr + GTT/PPGTT-select capture). 4-2's cross-tenant discriminator needs "which PPGTT was active when a fault fired" — capture it now.
- **ISC-5 — Independent byte oracle for the PTE (REQUIRED, CLAUDE.md).** A **hand-laid PPGTT page-table-entry fixture**: a PTE for a known physical address with present/R-W bits, asserted byte-for-byte independent of the construction code. The page-table layout is an in-memory byte format — exactly the rule's target. Also keep the 4a command oracles green; re-lay if the batch adds the PDP-load LRI.
- **ISC-6 — Standards.** Default-off (`accel-intel`); SPDX; SAFETY+PRM citations; `cargo fmt`; host suite (the PTE oracle + Add/matmul/GELU + accounting/sharing regressions) green; `shard-coverage`; `clippy -D warnings` (CI/Linux); builds on+off. ADR-032 gets a BUILD-4-1 section (the PPGTT model + PDP-load + the self-proving gate); note the 4-2 negative-test follow-on.

## 3. Failure-mode + discriminator section (REQUIRED — blind-hardware)
The fault-address + GTT/PPGTT-select capture is in place; ISC-4 adds the PDP root. Expected modes:

| # | Mode | Likely cause | Discriminator |
|---|---|---|---|
| 1 | **Compute faults** (own Add/matmul faults) | PPGTT page tables don't map the tenant's own buffers, or PTE bits wrong | fault addr = the tenant's buffer VA + `select=PPGTT` → page-table construction bug (ISC-1); `select=GGTT` → PPGTT not loaded, fell to GGTT (ISC-2 PDP load) |
| 2 | **Compute hangs** (no retire, no fault) | PDP load wedged the ring / malformed LRI | last breadcrumb + RING_FAULT/INSTDONE (4a EU-idle-vs-stuck); the PDP-root log shows whether the load even happened |
| 3 | **Compute *succeeds* but `select=GGTT`** | a stale GGTT mapping of the buffers still exists → access didn't actually use PPGTT (the silent false-pass) | the select bit on the data access must read **PPGTT**; if it reads GGTT, ISC-3's "unmapped in GGTT" wasn't honored — this is why we unmap from GGTT and assert the select |
| 4 | **`select=PPGTT`, correct result** | — | ✅ PPGTT engaged; 4-1 done |

The gate is **not** "compute works" — it is "compute works **and** the data access resolved through PPGTT (`select=PPGTT`), with the buffers absent from GGTT." That rules out the silent-fallthrough false-pass.

## 4. Gate (two-stage)
- **Stage A (you):** builds (on+off); PTE byte oracle + Add/matmul/GELU + accounting/sharing regressions green; clippy; fmt; shard-coverage.
- **Stage B (PL, OptiPlex):** the Add (and matmul/GELU) retire with correct results, **data buffers absent from GGTT**, the data access decode showing **`select=PPGTT`**, the PDP-root logged. Report result + the select bit + (on any miss) fault addr + PDP root + discriminator branch.

## 5. Scope guard
PPGTT-engaged positive path only. **Do NOT** do the cross-tenant negative test (4-2), execlists/LRC, concurrency, or RO-permission enforcement (4-2 uses the R/W bit). **Do NOT** move ring/command/state off GGTT (proven). **Do NOT** add CPU fallback or silently switch mechanisms (ADR-032 stance). One tenant, one PPGTT, compute proven to flow through it.

## 6. Merge (PM, on Stage-B PASS)
Close-gate: `git fetch origin && git rebase origin/main`, re-run Stage-A, push. PM `pm-merge-check` → `--ff-only` → `main`. Then **4-2** (the negative test — cross-tenant OOB access + RW-on-RO write must fault at the device boundary): the fourth governance line, the page-table-enforced boundary made real.
