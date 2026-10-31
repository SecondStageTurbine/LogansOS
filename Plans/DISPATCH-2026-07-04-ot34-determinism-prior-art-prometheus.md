<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: OT-34 determinism prior-art mine — how to thread a matmul byte-deterministically (research) → Prometheus

**Date:** 2026-07-04
**From:** PM (Claude)
**To:** Prometheus (research / trap-mining / host-only lane; just back). New branch off `origin/main` HEAD (`67c4fa6`): `prometheus/ot34-determinism-prior-art`.
**Status:** DISPATCHED — **runner-free research that de-risks Oracle's active OT-34 build** (threaded forward matmul, `codex/ot34-threaded-matmul-oracle`). Oracle's load-bearing constraint is ISC-2: the threaded matmul output must be **byte-identical regardless of thread count**, or it breaks the OT-30 llama.cpp top-1 parity oracle (parallel float reduction reorders accumulation → different bytes → the near-tie fragility we hit at 1.5B). **Mine how the field already solves this** so Oracle builds to a known-good pattern instead of rediscovering the float-reassociation trap. **Scope: research/docs, host-only — NO kernel code, no QEMU, no behavior change. Output is a FINDINGS doc.** (Fits your scoped lane: research/prior-art, push-CI-only for the doc.)

## 0. The decision this prep informs
Oracle's ISC-2 needs a **deterministic reduction** (fixed work partition + ordered combine, thread-count-independent). This mine answers: *what is the standard, proven way to thread a CPU matmul so the result is bit-for-bit identical across 1/2/4/N threads?* — with the concrete gotchas, so it's a known pattern, not a from-scratch gamble against float non-associativity.

## 1. What to mine (from real source — never a summarizing fetch, per the byte-oracle-acquisition rule)
- **llama.cpp / GGML CPU threading:** how does `ggml`'s CPU backend partition a matmul across threads, and is the result deterministic across thread counts? Read the actual thread/work-split + reduction code (`ggml.c` / `ggml-cpu`), and whether they guarantee (or explicitly do NOT guarantee) bit-identical output vs. thread count. Note the exact partitioning scheme (row-block / tile) and the accumulation order.
- **The float-non-associativity trap:** document *why* naive parallel reduction is nondeterministic ((a+b)+c ≠ a+(b+c) in IEEE-754), and the standard fixes — **fixed row/tile partition with per-partition sequential accumulation + an ordered (not first-come) join**, vs. a **fixed-shape reduction tree**. Which does GGML use? Which is simplest for a per-CPU work-split + join?
- **PyTorch / NumPy "deterministic" modes + BLAS:** how do they characterize threaded-matmul determinism (e.g. `torch.use_deterministic_algorithms`, OpenBLAS thread-count effects)? Extract the *rule* (same partition + same reduction order ⇒ same bytes) and the known failure mode (thread-count-dependent tiling changing the order).
- **The specific gotcha for us:** partition by **output rows** (each thread owns whole rows → no cross-thread reduction of the same output element → trivially deterministic) vs. **splitting the K/contraction dimension** (threads co-reduce one output element → needs an ordered combine → the risky path). Recommend which Oracle should use for a byte-identical guarantee at minimal complexity.

## 2. ISCs (all produce ANALYSIS, not code)
- **ISC-1 — The determinism recipe.** From the sources above: the *recommended* partition + reduction scheme that gives **bit-identical output independent of thread count**, with the rationale. Lead with the row-partition-avoids-cross-thread-reduction insight if the sources bear it out.
- **ISC-2 — The trap bank.** Each documented gotcha (float reassociation; thread-count-dependent tiling; SIMD horizontal-sum order; `-ffast-math`/FMA-contraction differences; reduction-tree shape) → a one-line "what breaks it + how to avoid," so Oracle's ISC-2 determinism KAT is pre-seeded against the real failure modes.
- **ISC-3 — Applicability note.** Map to our situation: single `.neom` model, F4 native quant kernels (frozen op math), Oracle's per-CPU SMP infra. Which recipe fits our matmul shape, and does it need any change to how the F4 kernels accumulate (ideally none — partition around them)?

## 3. Scope guard
**Host-only + a FINDINGS doc.** Do NOT write kernel code, modify a matmul, add a QEMU scenario, or touch a Cargo feature. If a finding implies a code decision, state it as an OT-34 line-item for Oracle. **Exact-behavior claims (which reduction order GGML uses) come from reading the real source, not a summary.** Output: `Plans/RESEARCH-2026-07-04-deterministic-parallel-matmul-prior-art-FINDINGS.md`.

## 4. Gate + close
- **Host only:** no code ships. `cargo fmt`/SPDX clean on the added doc. **No QEMU gate** (research/docs; paths-ignored CI).
- **Close-gate `scripts/worker-close-gate.sh`** (host set; SPDX; self-stat). RETURN the FINDINGS path + the recommended determinism recipe + the trap bank. PM folds the reference into OT-34; Oracle builds ISC-2 to it.
- **Disk reclaim** after merge.

---
## Relay — what to tell Prometheus
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-07-04-ot34-determinism-prior-art-prometheus.md` (+ Oracle's OT-34 dispatch `Plans/DISPATCH-2026-07-04-ot34-threaded-matmul-oracle.md` for the constraint). **Branch:** `prometheus/ot34-determinism-prior-art` off `origin/main` HEAD (`67c4fa6`).
- **One-line:** **Runner-free prior-art mine to de-risk Oracle's OT-34** — how do GGML/llama.cpp, PyTorch/BLAS thread a matmul so the result is **byte-identical regardless of thread count** (the thing that keeps our OT-30 parity oracle green)? Produce `Plans/RESEARCH-2026-07-04-deterministic-parallel-matmul-prior-art-FINDINGS.md` with: the recommended partition+reduction recipe (lead: partition by output rows to avoid cross-thread reduction of one element) (ISC-1), a trap bank of what breaks determinism — float reassociation, thread-count-dependent tiling, SIMD horizontal-sum order, FMA/fast-math (ISC-2), and an applicability note for our F4-kernel matmul (ISC-3). **Read the REAL source (ggml.c etc.), never a summarizing fetch. Host-only, no code.** Close-gate; RETURN the FINDINGS path + the recipe + trap bank — I fold it into OT-34 for Oracle.
