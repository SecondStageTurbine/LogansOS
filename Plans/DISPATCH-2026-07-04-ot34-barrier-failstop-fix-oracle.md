<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: OT-34 AAG bounce — the BARRIER_LIMIT timeout is unsound (fail-stop, don't fail-continue) → Oracle

**Date:** 2026-07-04
**From:** PM (Claude)
**To:** Oracle (main-tier coder). Rebase the existing `codex/ot34-threaded-matmul-oracle` onto current `origin/main` and fix; or re-cut it fresh. **Branch stays `codex/ot34-threaded-matmul-oracle`.**
**Status:** DISPATCHED — **AAG bounce, one finding.** The OT-34 threaded-matmul branch (`e242ed3`) passed its individual 4-shard QEMU gate and cleared **four of five** adversarial-audit lenses cleanly. A **two-lens convergence** (memory-safety + concurrency, independently) found **one real soundness gap** in the `BARRIER_LIMIT` fail-closed path of `kernel/src/ai/par_matmul.rs`. It is **LOW/latent** (not reachable by any current execution path, and structurally invisible to the QEMU gate), but it is a genuine UB path in ring-0 `unsafe`, and OT-34's whole thesis is provable rigor — so it does not land as-is. **The fix is small and surgical: make the timeout fail-STOP, not fail-continue-into-free. Scope: `par_matmul.rs` only. Nothing else in OT-34 changes.**

## What the AAG cleared (so you know the blast radius is ONE path)
Both kernel-facing lenses verified clean: memory ordering (release/acquire publish + join edges correct), single-winner claim + steal-back exhaustiveness (incl. the ABA case: an AP that stalls from job N into job N+1's republished slot still computes N+1's freshly-read work, single-winner), join completeness (BSP never proceeds on a partial vector), determinism / no-shared-accumulator (byte-identity across thread counts holds — the ISC-2 centerpiece is sound), I-14 / I-5 (executor takes no governance and no ordered lock; `PAR_BUSY` is a try-flag), `ap_poll` re-entrancy, partition index math, quant-block alignment, and no DMA surface. **The determinism story — the actual feature — is fully verified.** Only the timeout backstop is wrong.

## The finding (F1) — precise repro
`kernel/src/ai/par_matmul.rs:400-405` (the `spins >= BARRIER_LIMIT` arm) + the return at `:412-413`.

Interleaving that breaks it:
1. An AP calls `ap_poll` and **wins** its claim CAS — `pending: true → false` (`:429-431`).
2. Because `pending` is now `false`, the BSP's steal-back branch (`:381-382`, which requires `pending == true`) **can never fire for this slot** — the slot is claimed-but-unfinished, not stealable.
3. The claiming AP stalls for ≥ `BARRIER_LIMIT` (8e9) BSP spins **before** its `done.store(true)` (`:444`).
4. The BSP hits the timeout: `result = Err(AttentionUnsupported); break` (`:403-404`), finishes the slot loop, `PAR_BUSY.store(false)` (`:412`), and **returns** from `run_generic` (`:413`).
5. The `Err` propagates out of `run_matmul_q8k` / the forward pass; the caller may **drop or reuse the `out` `Vec`**. The `&mut [f32]` borrow that made `out_ptr` valid is now dead.
6. The still-live AP finishes `run_disjoint`, **writes `out_ptr`** and stores `done` (`:442-444`) — into **freed / reused memory**. Write-after-return (UAF) + a `done`-store data race.

**Why the code's own justification is false.** The `unsafe impl Sync for ApSlot` (`:171`) and the SAFETY notes (`:436-439`) rest on *"inputs are live because the BSP blocks on the barrier until this partition reports done."* The timeout is an escape hatch **out of exactly that block**, so the premise does not hold on that path. The inline comment *"impossible for lock-free pure compute"* is also wrong on its own terms: the AP idle loop runs with **interrupts enabled** (`kernel/src/sched/scheduler.rs:288`), so the executor is **not** uninterruptible/un-descheduled compute — a long ISR/SMI between claim and `done` is a legal interleaving.

**Cascade (why it's worse than a one-shot).** The abandoned AP's late `slot.done.store(true)` (`:444`) can land **after** a *subsequent* job has republished the same static slot (`done` reset to `false` at `:352`). The stale `true` then trips the next job's barrier prematurely (`:374`), so a **later, otherwise-correct** job's BSP returns while *its* AP is mid-write — silent corruption decoupled from the original stall.

**Why LOW / why it still bounces.** Unreachable in the current AP model (empty AP runqueues ⇒ a timer ISR returns immediately rather than preempting to another thread; a pure-compute partition finishes in far fewer than 8e9 BSP-spins), and TCG can't trigger it (an unscheduled AP stays `pending == true` ⇒ it gets *stolen*, the safe path) — so the hermetic byte-identity gate is green and blind to it. But it is a real UB path in ring-0 `unsafe` whose stated safety invariant is violable, gating a *performance* feature (Correctness > … > Performance, Bible 1.2). Known-unsound `unsafe` does not go on `main` when the fix is a few lines.

## The fix (ISC-F1) — fail-STOP, never fail-continue-into-free
The correct behavior when liveness of a **claimed** partition can't be proven is to **never let the shared `out` / inputs be freed while that partition may still touch them.** Two acceptable shapes (your call, whichever is cleaner):

- **(A) Bounded-wait-then-halt.** On `spins >= BARRIER_LIMIT` for a *claimed-but-unfinished* slot (`pending == false && !done`), do **not** `break`/return. Treat it as an unrecoverable kernel fault: `panic!`/halt (the machine is already in an impossible state — an AP wedged mid-pure-compute). Fail-stop keeps `out` on the (now-halted) stack; nothing is freed underneath the wedged AP. This matches how we treat other "can't happen" ring-0 invariant breaks.
- **(B) Unconditional drain before return.** Keep the fail-closed `Err`, but before returning, **wait unconditionally on `done` for every claimed slot** (no timeout on the *claimed* case — only the *unclaimed* case is stealable/skippable). i.e. the timeout may only fire for a slot the BSP can still steal; a slot already claimed by an AP must be drained to `done`, because only the AP can retire it. Return `Err` after drain if you want to signal the stall, but never return with a claimed slot non-`done`.

Either way, **the steal-back is only valid for the *unclaimed* case; the *claimed-but-stalled* case must be drained or must halt — it must not fall through to a return that frees `out`.** Also fix the cascade: guarantee no slot's `done` from job N can be observed by job N+1 (e.g. the drain in (B), or a per-job epoch/generation on the slot so a stale `done` store is ignored). Update the SAFETY comments at `:171` and `:436-439` so they state a premise that is actually true on **all** paths (including timeout), and adjust the ADR-070 "impossible for lock-free pure compute" line — the executor is interruptible.

## Scope guard
**`kernel/src/ai/par_matmul.rs` only** (plus the ADR-070 wording + SAFETY-comment corrections). **Do NOT** touch the determinism/column-partition math, the F4 kernels, the kill-switch, or the QEMU scenario — all verified clean. Do not change the public `execute` signature. If you add a slot generation/epoch field, keep it minimal.

## Gate + close
- **Kernel-touching ⇒ the mandatory pre-merge 4-shard QEMU `workflow_dispatch` gate is required** (all 4 shards + host + SPDX), on the **rebased** branch.
- **Close-gate `scripts/worker-close-gate.sh`** (rebase-freshness onto current `origin/main` — which now carries the M3 + IPC merges; full CI host set in order; SPDX; self-stat). RETURN the green push-CI URL **and** the green QEMU `workflow_dispatch` URL, plus your own `git diff --stat` vs `main`.
- **Optional but welcome:** if you can add a host-side KAT that *drives* the claimed-but-stalled path deterministically (e.g. a test seam that makes one partition's executor observably not-yet-`done` at the timeout, asserting the BSP halts/drains rather than returns), that converts F1 from "reasoned" to "regression-guarded." Not required if it means faking the AP model; a reasoned fix + corrected SAFETY invariant is sufficient.
- **PM re-runs the memory-safety + concurrency AAG lenses on the fix** before merge (targeted re-audit of the timeout path only).
- **Disk reclaim** after the PM confirms merge to `origin/main`.

---
## Relay — what to tell Oracle
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-07-04-ot34-barrier-failstop-fix-oracle.md`. **Branch:** `codex/ot34-threaded-matmul-oracle` — **rebase it onto current `origin/main`** (which now has the M3 + IPC merges) and apply the fix.
- **One-line:** **AAG bounce, ONE finding — the OT-34 determinism work is fully verified clean; only the `BARRIER_LIMIT` fail-closed path in `par_matmul.rs` is unsound.** A partition that an AP *claimed* (`pending` flipped false at `:431`) but did not finish is neither stealable (steal needs `pending==true`, `:381-382`) nor waited-for: on timeout the BSP returns (`:413`), the `&mut out` borrow dies, the caller frees/reuses `out`, and the still-live AP writes into freed memory (`:442-444`) — UAF + a stale-`done` cascade into the next job. The "impossible for lock-free pure compute" premise is false because the AP loop runs interrupts-enabled (`scheduler.rs:288`). **Fix = fail-STOP, not fail-continue:** on a claimed-but-unfinished slot at the limit, either halt/`panic!` (never freeing `out`) **or** drain `done` unconditionally before returning — the timeout may only skip an *unclaimed* (stealable) slot, never a claimed one; and close the cascade so a stale job-N `done` can't trip job N+1 (drain, or a per-job slot epoch). `par_matmul.rs` + ADR-070 wording + the SAFETY comments only — determinism math / F4 kernels / kill-switch / QEMU scenario are verified, don't touch them. **Kernel-touching: the 4-shard QEMU `workflow_dispatch` gate is mandatory.** Close-gate (rebase-fresh, full host set, SPDX, self-stat); RETURN both green CI URLs (push + QEMU) + your `git diff --stat`. PM re-runs the two kernel AAG lenses on the fix before merge.
