<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Phase 7 BUILD-4X-2 — tensor data through per-context PPGTT (`select=PPGTT`, the positive boundary) → Jarvis

**Date:** 2026-06-08
**From:** PM/Cerebro
**To:** Jarvis (new branch off `origin/main` `a1dbc52`: `codex/phase-7-build-4X-2-ppgtt-data-jarvis`)
**Status:** DISPATCHED. **Blind-hardware (OptiPlex).** Far lower risk than 4X-1 — the execlist context + per-context PPGTT are now silicon-proven; this maps *data* into them.
**Builds on:** BUILD-4X-1 (`7626f62`, GuC-free execlist runs a context with the context PPGTT active) + BUILD-4-1 PPGTT machinery + the cited spec `Plans/RESEARCH-2026-06-08-gen9-execlist-lrc-bringup.md`. ADR-032, ANSPEC §9.8.

## 0. Scope — the positive isolation test (one tenant)
4X-1 proved a context runs via ELSP with its PPGTT as the active address space (the no-op's *pipeline* pages resolved through PPGTT). **4X-2's job: prove the actual *tensor data* resolves through the per-context PPGTT** — run the Add (then matmul/GELU) via the execlist context with its **data buffers mapped PPGTT-only**, and confirm `select=PPGTT` on the data access + the correct result. This is BUILD-4-1's original `select=PPGTT` gate, now reachable. **One tenant only — no cross-tenant test (4X-3), no RO-enforcement (4X-3), no concurrency.**

## 1. The self-proving gate (same logic as 4-1, now on the execlist context)
Extend the context's PML4 (4X-1 already maps the pipeline/state pages it needs) to also map the compute **data buffers** (lhs/rhs/output; matmul/GELU operands) at their A64 VAs, **present+RW, and UNMAPPED in GGTT.** Then: **correct result ⟺ the data resolved through PPGTT** (a GGTT fallthrough faults — the buffers aren't there), and the fault-decode `select` bit confirms it independently. This is the inverse-proof of 4-1's `select=GGTT` dead end.

## 2. ISCs
- **ISC-1 — Map the data buffers into the context PML4.** Extend 4X-1's per-context 48-bit PML4 to map lhs/rhs/output (and the matmul/GELU operands) at their A64 VAs, present+RW, backed by their physical frames. Reuse 4-1's `intel_gen9_ppgtt.rs` PML4→PT construction. **Note the 4X-1 iter1 lesson:** in the execlist context, *more* resolves through PPGTT than on the legacy ring (4X-1 faulted on a STATE page until it was PPGTT-mapped) — so map **every page the Add's dispatch dereferences as data/state through the EU/dataport** (the A64 buffers + any state heap the context routes through PPGTT), not just the three tensors. Instruction Base / command stream stay GGTT (as 4a, proven).
- **ISC-2 — Data buffers PPGTT-only (unmapped in GGTT).** The self-proving gate: the A64 data VAs must NOT be GGTT-mapped, so a non-PPGTT access faults instead of silently succeeding.
- **ISC-3 — Run the Add via the execlist context, read back.** Use 4X-1's ELSP submission + CSB completion for the proven 4a/4b Add batch (start with the Add; matmul/GELU once Add passes). Read back `output_base`, compare to `add_oracle_output()` = `[4.0f, 6.0f]`.
- **ISC-4 — The gate captures.** On the data access: the **`select` bit must read PPGTT** (the iter7 fault-decode), the PML4 root logged (4-1 capture), CSB switch-in→idle, ctx-id matched. Correct result + `select=PPGTT` + buffers-absent-from-GGTT = pass.
- **ISC-5 — Byte oracle.** The PML4/PT entries mapping the data buffers (reuse the RW/RO PTE oracle, asserting the data-buffer VAs map to their frames present+RW). Re-lay the command oracle only if the batch changes.
- **ISC-6 — Standards.** Default-off; SPDX; SAFETY/PRM+RESEARCH citations; `cargo fmt`; host suite (the data-mapping PTE oracle + 4X-1/4-1/4a/4b + accounting/admission/sharing regressions) green; `shard-coverage`; `clippy -D warnings` (CI/Linux); builds on+off. ADR-032 gets a BUILD-4X-2 section; **§9.8 flips spatial-isolation from "mechanism-proven" to "positive-proven" on a PASS** (full "enforcement-proven" still waits for 4X-3's negative test).

## 3. Failure-mode + discriminator section (REQUIRED — blind-hardware)
Captures: `select` bit + fault address + PML4 root + CSB + RING_HEAD.
| # | Mode | Likely cause | Discriminator |
|---|---|---|---|
| 1 | **Add faults, `select=GGTT`** | a stale GGTT mapping of the data buffers survived (ISC-2 not honored) — silent-fallthrough | the `select` on the data access must be PPGTT; if GGTT, the buffers weren't actually unmapped from GGTT |
| 2 | **Add faults, `select=PPGTT`, addr = a data-buffer VA** | the PML4 doesn't map that buffer (page-table construction) | fault addr = which buffer; check the PML4→PT walk for that VA |
| 3 | **Add faults, `select=PPGTT`, addr = a STATE/pipeline page** | the execlist context routes that page through PPGTT and it's unmapped (the 4X-1 iter1 pattern) | fault addr identifies the page → add it to the context PML4 (ISC-1) |
| 4 | **Add runs, wrong result** | data mapped but wrong frame/stride, or compute regression | read both output elements; compare to the GGTT-path 4b result (which was correct) to isolate mapping vs compute |
| 5 | **Correct `[4,6]` + `select=PPGTT` + buffers absent from GGTT** | — | ✅ tensor data isolated through the per-context PPGTT; 4X-2 done → 4X-3 |

## 4. Gate (two-stage)
- **Stage A (you):** builds (on+off); the data-mapping PTE oracle + regressions green; clippy; fmt; shard-coverage.
- **Stage B (PL, OptiPlex):** the Add retires `[4,6]` via the execlist context, **data buffers absent from GGTT**, **`select=PPGTT`** on the data access, CSB switch-in→idle. Report result + select bit + PML4 root + (on any miss) fault addr + discriminator row.

## 5. Scope guard
Positive single-tenant data-isolation only. **Do NOT** do the cross-tenant negative test or RO-permission enforcement (both 4X-3); **do NOT** add concurrency/multi-context; **do NOT** change 4X-1's execlist submission or the 4a command stream (proven). Map data into PPGTT, prove it resolves there. The negative ("B can't reach A") is next.

## 6. Merge (PM, on Stage-B PASS)
Close-gate: `git fetch origin && git rebase origin/main`, re-run Stage-A, push. PM `pm-merge-check` → `--ff-only` → `main`. Then **4X-3** — the decisive negative test: a second tenant's device-side access to the first tenant's tensor must **page-fault**, and an RO-mapped shared tensor must fault on write — the §9.8 silicon-enforced device boundary, the fourth governance line made real.
