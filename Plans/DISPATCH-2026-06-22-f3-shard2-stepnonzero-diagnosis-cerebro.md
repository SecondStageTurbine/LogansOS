<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: F3 gate — shard 2/4 step exits non-zero with all-scenarios-PASS (diagnose + fix) → Cerebro

**Date:** 2026-06-22
**From:** PM (Claude)
**To:** Cerebro (the OT-29 discriminator-driven-diagnosis fit — reproducible, all-pass-but-failing, no output). Work on the F3 branch `prometheus/forward-path-f3-real-vocab` (rebased on current `origin/main`), or a fresh branch cherry-picking F3 — your call. **Reassigned from Prometheus** (kernel-touching QEMU debugging → main-tier per the 2026-06-22 routing policy).
**Status:** DISPATCHED. **F3's QEMU gate reproducibly reds shard 2/4 — and it's real, not infra.** The F3 vocab code is sound (PM-verified: the parser matches neomc's real `.vocab` emitter byte-for-byte; kill-switch default-off ≡ fixture; silent fallback). The blocker is a **test-harness/shard integration regression**: adding the `arthur_real_vocab` `TestScenario` shifts the shard partition, and shard 2/4's `Run QEMU integration tests` step now **exits non-zero even though every scenario on it emits `event=PASS`** — with **no panic / no `event=FAIL` / no error line** in the captured log. This needs instrumentation to localize (the OT-29 pattern), not a guess.

## 0. The evidence (already gathered — start here)
- **Reproducible:** attempt 1 (job `82798985886`, runner gaming-pc-2) and attempt 2 (job `82814817637`, runner gaming-pc-3) **both failed only on shard 2/4** — different runners ⇒ not a bad-runner flake.
- **Only shard 2/4.** Shards 1/3/4 + Host + SPDX all GREEN both attempts.
- **All shard-2/4 scenarios PASS:** `kaslr_smoke`, `q7c_missing_shootdown_detection`, `q7c_shootdown_timeout_panic`, `q8b_grant_vs_exit_race`, `q8c_close_peer_my_side_extract`, `sched_ready_queue_capacity_pressure`, `smep_catch`, `sys_wait_multiple_waiters` — every one BEGIN→PASS (each runs twice; `sched_ready…` emits PASS without BEGIN). **No `event=FAIL`, no kernel panic beyond the *expected* `q7c_shootdown_timeout_panic`.**
- **The failing step is #13 "Run QEMU integration tests"** (per-step conclusions confirm it; steps 14/15 dump+upload are `if: failure()`/`continue-on-error` and are *symptoms*, not the cause — the artifact-quota error there is non-fatal and already pruned).
- **F3-attributable:** recent main (OT-30 gate `27975223225` @ `e517a52`) was **all-4-shards green**; the consolidation added no QEMU scenario; F3 adds `arthur_real_vocab` to the `TestScenario` list, shifting the partition. `arthur_real_vocab` itself is **not** on shard 2/4 (it's on a green shard).
- **Timing clue:** shard 2/4 finishes fast (~5–7 min vs ~18 min for the heavy shards) and *then* the step is non-zero — consistent with an exit-code/aggregation issue at the end of the run, not a per-scenario hang/timeout.

## 1. The hypothesis to test first
The `xtask neo-test` shard runner exits non-zero **after all scenarios pass** — so the bug is in the runner's **exit-code aggregation**, exposed on shard 2/4 by F3's partition shift. Likely candidates:
- A scenario whose QEMU process returns a **non-zero exit despite emitting PASS** (the panic-expected `q7c_shootdown_timeout_panic` is the prime suspect — a deliberate panic exits QEMU via debug-exit/triple-fault; if the runner tolerates that per-scenario but leaks the non-zero into the shard's aggregate exit, the step reds while the scenario shows PASS), now landing on shard 2/4 due to the re-partition.
- An off-by-one / count-vs-expected check in the runner that the new partition boundary trips.
- The doubled run (each scenario twice — SMP variants?) interacting with the exit-code capture.

## 2. ISCs
- **ISC-1 — Instrument the shard runner's exit path (capture-before-you-fix).** Make `xtask neo-test`'s shard run print **exactly which sub-step set the non-zero exit** — per-scenario QEMU exit code, the aggregate the runner returns, and where (the OT-29 discriminator discipline: a reproducible failure with no output is a *capture* gap). Re-run shard 2/4 (locally or via the gate) and read the captured cause. **Do not fix blind.**
- **ISC-2 — Root-cause + fix.** From ISC-1, fix the actual defect — most likely the runner's exit-code aggregation (a passed scenario's QEMU exit leaking non-zero), or the partition/registration interaction F3 introduced. If it's a latent runner bug F3 merely *exposed*, fix the runner (and note it). If it's F3's test_harness/xtask integration (a mis-routed/duplicated scenario from the rebase conflict resolution), fix that.
- **ISC-3 — Keep F3's vocab code intact.** The `vocab_sidecar.rs` parser + the `arthur-real-vocab` kill-switch + chat template are PM-verified correct — don't churn them. This is about the test-harness/shard integration, not the feature.
- **ISC-4 — Standards + gate.** SPDX; fmt/clippy via Docker; **the full 4-shard QEMU `workflow_dispatch` must go green** (shard 2/4 included) + `arthur_real_vocab` passes on its shard + `arthur_boots_thinking` green (kill-switch off ≡ fixture). If the fix touches the shard runner, confirm the "4-way partition complete + disjoint" host-check still passes.

## 3. Scope guard
Diagnose + fix the shard-2/4 step-nonzero **only**. **Do NOT** rewrite F3's vocab feature, change the `.neom`/sidecar format, or touch unrelated scenarios. If the root cause is the shard runner's exit-code logic, that fix is in scope (it's the blocker). No new invariant unless the runner fix genuinely needs one (flag it).

## 4. Merge (PM)
**Close-gate: `scripts/worker-close-gate.sh`** (rebase LAST; full set; SPDX; self-stat). RETURN the green push-CI + the **4-shard QEMU `workflow_dispatch`** URL (all shards green, head_sha-verified). PM merges F3 (the whole feature) once the gate is clean. **The PM independently re-verified F3's vocab correctness already** — the remaining gate is this shard-2/4 fix.

**Disk reclaim (after PM confirms merge):** delete the build tree once on `origin/main`. See CLAUDE.md "Disk hygiene."

---
## Relay — what to tell Cerebro
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-06-22-f3-shard2-stepnonzero-diagnosis-cerebro.md`. **Branch:** `prometheus/forward-path-f3-real-vocab` (rebased on `origin/main`) or cherry-pick F3 to your own.
- **One-line:** **F3's gate reproducibly reds shard 2/4 — real, not infra.** Shard 2/4's `Run QEMU integration tests` step exits **non-zero while every scenario emits PASS** (no panic/`event=FAIL`/error), on two different runners (jobs `82798985886`, `82814817637`); shards 1/3/4+host+SPDX green; recent main (OT-30 `e517a52`) was all-4-green, so F3 adding the `arthur_real_vocab` `TestScenario` (which **shifts the shard partition**) is the delta — though `arthur_real_vocab` itself isn't on shard 2/4. **It's the OT-29 shape: reproducible, all-pass-but-failing, no output → instrument the `xtask neo-test` shard-runner exit path FIRST** (per-scenario QEMU exit code + the aggregate it returns + where the non-zero is set), then fix. **Prime suspect:** the panic-expected `q7c_shootdown_timeout_panic` (on shard 2/4) leaks a non-zero QEMU exit into the shard aggregate, exposed by the re-partition — or an off-by-one in the runner's exit-code/count logic. **F3's vocab code is PM-verified correct — don't churn it; this is the test-harness/shard integration.** Fix the runner/integration, get the **full 4-shard QEMU green**. **Close-gate: `scripts/worker-close-gate.sh`; RETURN the green push-CI + 4-shard QEMU URLs; PM merges F3 on green.**
