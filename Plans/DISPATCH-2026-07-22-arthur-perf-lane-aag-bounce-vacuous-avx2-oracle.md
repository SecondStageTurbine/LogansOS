<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH (BOUNCE): Arthur perf lane — the in-QEMU AVX2 parity assertion is VACUOUS → Oracle

**Date:** 2026-07-22
**From:** PM (Claude)
**To:** Oracle (main-tier; the perf-lane author)
**Status:** BOUNCE — **NOT merged.** The perf lane (`2a6e0a4`) is *almost* clean: CI green (run `29884626129`, independently re-verified), own-stat clean (6 files, 354/1, no phantom deletion), and **2 of 3 PM-AAG lenses NO SURVIVING FINDING** — correctness-neutrality (measurement provably non-perturbing to logits/KV/seqno; silicon-only labeling honest; formula units correct) and concurrency/mem-safety (TSC calibration div-by-zero/overflow-guarded; `read_tsc` asm correct; fork-join takes no lock so no new lock-order edge; I-14 clean). **The 3rd lens (byte-parity/kill-switch) found ONE CONFIRMED finding that bounces it.** The fix is small; re-gate + re-merge.

## The finding (CONFIRMED — PM-re-verified against the tree)
**The in-QEMU AVX2≡Scalar assertion this commit adds is VACUOUS (scalar==scalar), and the AVX2 half of the lane's headline safety property is green-but-unproven.**

- `arthur_perf_and_parity()` (`kernel/src/test_harness.rs`) computes `out_avx2_k` via `matmul_q4_k_q8_k_with_kernel(..., QKMatmulKernel::AutoAvx2)` and asserts `avx2_bit_identical`.
- But `AutoAvx2` only takes the real AVX2 branch when `avx2::runtime_available()` is true (`kernel/src/ai/ops/qk_matmul.rs:128,573`) — which needs CPUID AVX+AVX2 + OSXSAVE + XCR0 YMM.
- **`arthur_compositor` runs under `-cpu qemu64,+rdrand`, NOT `-cpu max`** — and it is **absent from `scenario_needs_qemu_max()`** (`tools/xtask/src/main.rs:5395-5410`; PM-confirmed `grep -c arthur_compositor` = 0). The `qemu64` base model does not advertise AVX/AVX2 under pure TCG, so `runtime_available()` returns **false** → `AutoAvx2` silently falls back to scalar → the in-QEMU assertion is **scalar==scalar, proving nothing about AVX2**. (The breadcrumb self-reports it: `avx2_available=false` on the gate.)
- The **only** non-vacuous AVX2 proof left is the pre-existing host KATs (`tools/ai-ops-tests`), but they have **no non-vacuity guard** — they silently degrade to scalar==scalar when AVX2 is absent (their own comments say "falls back to scalar (trivially equal)"), and the random sweeps `SKIP`. No test asserts `q4_k_q8_k_avx2_available() == true`, so the AVX2 safety net is green-but-unproven on any non-AVX2 runner.

**Impact:** NOT a logit divergence — the AVX2 integer dot is exact by construction and the host KATs almost certainly DO exercise AVX2 on the self-hosted x86_64 runners. This is a **gate-integrity / honesty defect**: the lane's headline deliverable #3 is "byte-parity is the safety property — Arthur's answer must not change when fast," and the **in-QEMU AVX2 proof of exactly that is vacuous** while advertised as real. A vacuous parity assertion that passes green is precisely the "closed loop passes green while the property is unverified" failure the byte-oracle/gate-honesty discipline exists to prevent. It merges only once the proof is genuine.

## The fix (small — the `xsave_ymm_context_switch` precedent)
1. **Make the in-QEMU AVX2 path real:** add `"arthur_compositor"` to `scenario_needs_qemu_max()` (`tools/xtask/src/main.rs`). `-cpu max` exposes a working AVX2 under TCG (the exact precedent already used for `xsave_ymm_context_switch`/`kernel_rng_selftest`), so `runtime_available()` returns true and the assertion **genuinely exercises AVX2**. Confirm the breadcrumb flips to `avx2_available=true` on the gate.
2. **Add a non-vacuity guard so it can never silently degrade again:** on the (now AVX2-capable) gate, **assert `q4_k_q8_k_avx2_available() == true`** before the parity check — so if the AVX2 path is ever NOT taken, the scenario FAILS rather than passing vacuously. (Optionally add the same guard to the host KATs on the known-AVX2 CI runner.)
3. **Correct the false comment:** any doc/comment claiming the AVX2 parity is "proven in-QEMU under `-cpu max`" must be true after fix 1 (it was false before — the scenario ran under `qemu64`). Fix the wording either way.

## What is ALREADY verified real — do NOT rework
- **Threaded 1≡N parity is NON-VACUOUS** (PM-AAG confirmed): the gate runs `NEO_SMP=4`, `online_cpu_count()`=4, `COLS=1024 > PAR_MIN_COLS=512` ⇒ the column-partition fork actually forks; the 1≡N byte-identity genuinely exercises the multi-partition path. Keep as-is.
- **The kill-switch is real:** `forward-perf` is purely additive (cfg-gated; the `cfg(not)` arm is byte-identical); the default build pulls none of the features ⇒ features-off ≡ the prior scalar single-thread path, bit-for-bit. Keep as-is.
- **The measurement + concurrency + mem-safety** all cleared two full AAG lenses. Do not touch `forward_perf.rs` / `exec.rs` timing — the fix is scoped to the AVX2 scenario `-cpu` selection + the non-vacuity guard + the comment.

## Close-gate + merge
- Re-run `scripts/worker-close-gate.sh` (rebase-fresh on current `main` — it advanced past your base with docs-only commits, so a clean rebase; **the code fix is the scenario `-cpu` list + a guard assert + a comment**). Green 4-shard `workflow_dispatch` QEMU URL with **`arthur_compositor` now under `-cpu max`** and the breadcrumb showing `avx2_available=true`. PM re-runs the **byte-parity lens only** (delta re-AAG) to confirm the assertion is now non-vacuous; the other two lenses stand.
- Same branch `codex/arthur-perf-lane-fast-forward-oracle` (re-spin); PM merges after the re-gate.

## Relay — what to tell Oracle
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-07-22-arthur-perf-lane-aag-bounce-vacuous-avx2-oracle.md`. Work in `Workers\oracle`; re-spin `codex/arthur-perf-lane-fast-forward-oracle`.
- **One-line:** Perf lane is 2/3 AAG-clean but **BOUNCED on one CONFIRMED finding: the in-QEMU AVX2≡Scalar assertion is VACUOUS.** `arthur_compositor` runs under `-cpu qemu64` (not `-cpu max`) and isn't in `scenario_needs_qemu_max()`, so `avx2::runtime_available()`→false → `AutoAvx2` falls back to scalar → your AVX2 parity check is scalar==scalar (the breadcrumb even prints `avx2_available=false`). The AVX2 half of the "Arthur's answer doesn't change when fast" safety property is green-but-unproven. **Fix (small):** add `"arthur_compositor"` to `scenario_needs_qemu_max()` (the `xsave_ymm_context_switch` precedent — `-cpu max` gives working AVX2 under TCG), add a **non-vacuity guard** (`assert q4_k_q8_k_avx2_available()` on the gate so it can't silently degrade again), and correct any "proven under `-cpu max`" comment (it was false). **Do NOT touch** the timing/measurement, the threaded parity (non-vacuous, verified), or the kill-switch (real, verified) — the fix is scoped to the AVX2 scenario `-cpu` + guard + comment. Re-gate with the breadcrumb showing `avx2_available=true`; PM re-runs the byte-parity lens only.
- **RAG** (PowerShell): `scenario_needs_qemu_max qemu max avx2`, `q4_k_q8_k_avx2_available runtime_available`, `arthur_perf_and_parity avx2 bit_identical`. Health-check `Invoke-RestMethod http://127.0.0.1:8404/health` first; if down, note it + grep/Read.
