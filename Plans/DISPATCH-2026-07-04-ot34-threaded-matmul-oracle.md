<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: OT-34 — multi-threaded forward matmul across cores (make Arthur faster) → Oracle

**Date:** 2026-07-04
**From:** PM (Claude)
**To:** Oracle (main-tier — SMP barrier + 1.5B + CB-5a author). New branch off `origin/main` HEAD (`78c5455`): `codex/ot34-threaded-matmul-oracle`.
**Status:** DISPATCHED — the **banked perf lever (OT-34), now unblocked** (F4 native quant kernels landed `546ef3d`; the latency oracle harness is on `main` `305b04c`). The forward matmul is single-threaded; threading it across the OptiPlex's cores is a modeled **~4–6× speedup** — a direct usability win for a real-time assistant (1.5B is ~1.86 s/token single-thread). **Non-conflicting** with Janus (Arthur memory) and Jarvis (observability). **Kernel-touching ⇒ 4-shard QEMU + AAG.**

## 0. The decision this build makes
Thread the forward-pass matmul across CPUs (per-CPU work split + join) — **without breaking correctness.** The hard, Oracle-shaped constraint: the result must be **byte-identical regardless of thread count**, or it breaks the OT-30 llama.cpp top-1 parity oracle (parallel float reduction changes accumulation order → different bytes → the exact near-tie fragility we saw at 1.5B). So a **deterministic reduction** (fixed work partition + ordered combine) is the deliverable, not just "spread the loop."

## 1. References
```
git fetch origin
git show origin/main:kernel/src/ai/forward_logic.rs   # the Qwen2 forward + the matmul to thread (OT-16: model-graph is here)
git show origin/main:kernel/src/ai/ops/               # the F4 native quant kernels the threads call (546ef3d)
git show origin/main:kernel/src/arch/x86_64/smp.rs    # the AP-ready barrier + per-CPU infra Oracle built — the threading rides this
git show origin/main:docs/OPEN-THREADS.md             # OT-34 (this), OT-7 (latency oracle — the measurement), OT-30 (parity oracle — must stay green)
# the OT-7 latency-oracle harness (305b04c) — benchmark-or-it-didn't-happen
git show origin/main:Plans/RESEARCH-2026-07-04-deterministic-parallel-matmul-prior-art-FINDINGS.md   # Prometheus's determinism prior-art mine — READ FIRST; ISC-2 builds to this
```

> **Determinism prior-art FOLDED IN (Prometheus, `eae1de8`, 2026-07-04) — build ISC-1/ISC-2 to it:** the mine confirms with GGML source evidence that **partition by output rows** is the correct recipe: each thread owns a disjoint row-range, so **no output element is ever co-reduced across threads → the result is byte-identical regardless of thread count, trivially** (GGML does exactly this: row-tile chunks + sequential `vec_dot`). **Our F4 kernels need ZERO changes** — the row loop in `matmul_q4_k_q8_k` just wraps the existing sequential dot products; static partition `rows*T/N .. rows*(T+1)/N`, barrier join via `smp.rs`, **no atomics on output data.** *Avoid* the K/contraction-dimension split (co-reduces one element → needs an ordered combine → the risky path; OpenBLAS fails the cross-thread-count test precisely because its K-block accumulation order changes with tile size). The 6-trap bank (float reassociation=CRITICAL; thread-count-dependent tiling=HIGH; FMA/fast-math=HIGH; SIMD horizontal-sum order + reduction-tree shape=MEDIUM; work-stealing=LOW-with-row-partition) pre-seeds your ISC-2 determinism KAT.

## 2. ISCs
- **ISC-1 — Deterministic parallel matmul.** Split the matmul's output rows (or a fixed tiling) across N CPUs, each thread computing its partition with the **existing F4 kernels** (engine math unchanged per op), then an **ordered join** so the assembled output is byte-identical to the single-threaded result. The partition + combine order must be **fixed and thread-count-independent** (not first-come). Reuse Oracle's per-CPU/SMP infra.
- **ISC-2 — Determinism proof (the load-bearing artifact).** A host KAT + the QEMU gate asserting **threaded output == single-threaded output, bit-for-bit**, at NEO_SMP=1/2/4 (same bytes at every thread count). AND the **OT-30 Stage-A parity oracle stays green** under threading (`first_divergence=None` / the near-tie teacher-forced check). If threading changes a single logit bit, it fails — that's the guard.
- **ISC-3 — Latency measurement (OT-7).** Measure the speedup via the latency oracle (OT-7) — **benchmark-or-it-didn't-happen**. Report tokens/s single-threaded vs 2/4-threaded (QEMU-TCG numbers are virtualized, so state them as relative/ratio; the absolute silicon number is a later bare-metal OT-7 read). No unmodeled claims.
- **ISC-4 — Kill-switch + SMP safety.** Default single-threaded ≡ today bit-for-bit (a `threaded-matmul` feature/flag off ⇒ the exact current forward). Prove the equality. The threading must not introduce a lock in the per-token hot path that contends a governance lock (I-14) or violate I-5 lock order — the join is a barrier, not a shared-mutable-state race.

## 3. Standing-rule sections
- **Kill-switch (Rule 1):** ISC-4 — off ≡ single-threaded, bit-for-bit.
- **Invariant budget (Rule 2):** preserve the ledger; **no new invariant**. **I-1 (result-invariance / the byte-oracle) is the one that bites** — threaded output MUST be byte-identical (ISC-2); this is I-1 applied to parallelism.
- **I-14 / I-5:** the join/barrier acquires no AI-governance lock in a way that could stall governance; no new ordered lock above the existing band; re-measure the lock census if it changes.
- **Determinism-under-parallelism** is the whole game — a nondeterministic reduction that's "close enough" is a FAIL (it poisons the parity oracle).
- **Scope guard:** thread the matmul + the determinism/latency proof + kill-switch ONLY. **Do NOT:** change the op math (F4 kernels frozen), the `.neom`/vocab, the model graph (OT-16 untouched); touch Arthur memory (Janus) / observability (Jarvis) / WiFi; add an invariant.

## 4. Gate + close
- **Kernel-touching ⇒ mandatory 4-shard QEMU `NEO_SMP=4` + AAG.** The AAG centers on **ISC-2 (byte-identical at every thread count + parity stays green)** — that's the correctness ballgame. Push-CI green. **Green without the CI URLs is not done.**
- **Close-gate `scripts/worker-close-gate.sh`** (rebase; re-run; host set; SPDX; self-stat).
- **RETURN** the QEMU shard URLs + SHA, the **byte-identical-across-thread-counts proof** + the parity-stays-green evidence, the latency ratio (OT-7), and the kill-switch equality. **Disk reclaim** after merge.

---
## Relay — what to tell Oracle
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-07-04-ot34-threaded-matmul-oracle.md` (+ `forward_logic.rs`, `ai/ops`, your `smp.rs`, OT-34/OT-7/OT-30). **Branch:** `codex/ot34-threaded-matmul-oracle` off `origin/main` HEAD (`78c5455`).
- **One-line:** **Thread the forward matmul across cores for a ~4–6× speedup — deterministically.** Split the matmul across CPUs (reuse your SMP infra + the F4 kernels), ordered join, so the output is **byte-identical regardless of thread count** (a nondeterministic reduction poisons the OT-30 parity oracle — that's the trap). **Prove it:** threaded==single-threaded bit-for-bit at NEO_SMP=1/2/4 + OT-30 parity stays green (that's the AAG centerpiece). Measure the speedup via the OT-7 latency oracle (relative ratios; TCG numbers are virtualized). **Kill-switch** off ≡ single-threaded bit-for-bit. **No op-math/model-graph/`.neom` change; no memory/observability/WiFi touch.** 4-shard QEMU + AAG. RETURN shard URLs + SHA + the byte-identical-across-threads proof + parity-green + latency ratio + kill-switch equality.
