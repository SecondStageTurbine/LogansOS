<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Forward-path performance — profile then targeted fix (OT-30) → Jarvis

**Date:** 2026-06-22
**From:** PM (Claude)
**To:** Jarvis (wrote F1 + the Stage-A harness; owns the repro). New branch off `origin/main` HEAD (`9b693a6` or later): `codex/forward-path-perf-profile-fix-jarvis`.
**Status:** DISPATCHED — **PL-ratified pivot (2026-06-22): perf is now critical-path, profile-first.** The Stage-A validation found F1's Option-A forward can't emit **token 0** in ~11 min (release; debug >15 min) on the real 485 MB 0.5B `.neom` — so the brain isn't runnable, let alone interactive. **This overturns D1's "perf-deferred-to-F4" sequencing.** But the magnitude is suspicious: **release ≈ debug** (~1.4×) and a **>600×** gap over D5's ~0.5–2 s/token — and the per-matmul materialization churn alone is only ~0.4 s/token of memory traffic. That can't explain 11 minutes, so **there is a pathology to find before committing to a fix.** ISC-1 profiles; ISC-2 fixes what profiling actually shows.

## 0. The decision this build makes
Turn "the real-model forward is unrunnable (~11 min, no token 0)" into "**the real 0.5B forward emits tokens in the D5 band (~0.5–2 s/token), so the Stage-A llama.cpp-parity validation completes**" — by **profiling to localize the >600× slowdown** and applying the **targeted** fix (which may be a cheap build/allocator correction, the elimination of per-token materialization, or — only if the data proves the matmul compute itself is the floor — native in-register quant kernels). **Correctness is unchanged** (the dequant values are identical; this is about *how/when* they're computed) — F1's byte-oracle KAT must stay green.

## 1. References
```
git fetch origin
git show origin/main:kernel/src/ai/forward_logic.rs        # F1: matmul_vec_dequantized (dequant full tensor -> gguf_col_major_to_ops_rhs -> matmul_vec), per matmul, per token
git show origin/main:kernel/src/ai/gguf_dequant_logic.rs   # the dequant primitives
git show origin/main:kernel/src/ai/ops/qk_matmul.rs        # the existing q4_K x q8_K native path (in-register, NO materialization) — the model for Option B if needed
git show origin/main:tools/ai-ops-tests/src/lib.rs         # the MERGED Stage-A harness (091f5bf) — the env-gated real-model validation that must now COMPLETE; the success oracle
git show origin/main:docs/THINK-... # forward-path THINK D5 (latency band) + D1 (Option A vs B); ADR-066 (F1)
# the Cargo profile(s) governing how the kernel `ai` crate compiles in the ai-ops-tests host harness — CHECK THIS FIRST (the release≈debug clue)
```

## 2. ISCs
- **ISC-1 — Profile to localize the >600× (the diagnostic; must be conclusive).** Measure, with numbers, where a single forward pass on the real 0.5B spends its time/allocations. Discriminate these candidates — each has a distinct signature; the RETURN must state which dominates:
  1. **Build profile (check FIRST — cheapest, and the `release ≈ debug` smoking gun).** Confirm the kernel `ai` crate (and `gguf_dequant_logic`/`ops`) actually compile **optimized** inside the `ai-ops-tests` host harness. If workspace/dev-profile inheritance leaves the hot crate at `opt-level=0`, that alone is 10–50× and `release` wouldn't help the test binary — fix the profile and **re-measure before anything else.**
  2. **Materialization churn.** Per-token, F1 does ~169 matmuls × (dequant full tensor [alloc ~MBs + write] + `gguf_col_major_to_ops_rhs` [alloc + transpose copy]). Time the dequant vs transpose vs matmul phases; count allocation bytes/token.
  3. **Allocator pathology.** GB/token of alloc/free — page faults, zeroing, fragmentation. Measure alloc count/bytes and time in allocation vs compute.
  4. **Accidental-quadratic.** Does any per-matmul cost scale super-linearly with tensor dimension (a re-scan in the transpose/dequant)?
  Use a coarse instrumented timing (per-phase counters + alloc tallies) — you do not need a fancy profiler; per-phase `Instant`/cycle counts and an allocation counter suffice. **Capture enough that the fix is targeted, not guessed.**
- **ISC-2 — Apply the targeted fix (scoped by ISC-1).** Fix the dominant cause:
  - *Build profile* ⇒ correct the harness/Cargo profile so the hot crate is optimized; re-measure (Option A may then land in-band — the cheapest possible outcome).
  - *Materialization churn* ⇒ eliminate per-token re-work: dequant-once-at-load cache (note the ~2 GB f32 residency cost for 0.5B — D4 implication, and it won't scale to 1.5B), **or** fuse dequant into the matmul without the intermediate `gguf_col_major_to_ops_rhs` allocation (preferred — memory-lean).
  - *Matmul compute is the floor* (only if the above don't reach the band) ⇒ native in-register quant×q8_K dot-product kernels (Option B; model them on the existing `qk_matmul` q4_K path; each new type-pair kernel needs a byte-oracle). **Do this only if the data proves it — don't pre-commit to 6 kernels.**
  Pick the **minimum** fix that reaches the target; if you reach for native kernels, justify it with the ISC-1 numbers.
- **ISC-3 — Correctness preserved (the equality anchor).** The dequantized **values are unchanged** — only the computation strategy changes. **F1's mixed-quant byte-oracle KAT must stay green** (the per-type one-hot dequant→matmul assertions). The kill-switch `phase9-real-quant` default-OFF path stays **bit-for-bit** (fixture/`arthur_boots_thinking` unchanged). No new invariant. New `unsafe` only if a native kernel needs it — flag + justify.
- **ISC-4 — The success bar = the Stage-A validation COMPLETES.** Re-run Jarvis's merged Stage-A harness (`NEO_REAL_NEOM=…`) on the real 0.5B: it must now **emit K≈16–32 tokens, reach top-1 parity vs llama.cpp, finite logits, deterministic**, at a measured **per-token latency in/near the D5 band (~0.5–2 s/token)** — i.e. the run that STOPPED now passes. Report before/after per-token latency.
- **ISC-5 — standards.** SPDX; `cargo fmt --all --check`; workspace test (exclude kernel); `cargo clippy --workspace --all-targets -- -D warnings`; kernel clippy — via Docker. ADR-066 amendment (the perf root-cause + the fix). Update `docs/complexity-governance.md` only if a lock/invariant is touched (it shouldn't be).

## 3. Gate
- **The reported Stage-A run completes + in-band** (ISC-4 — the headline proof; reported, artifact not in git).
- **F1 byte-oracle KAT green** (correctness preserved) + the **forward_mixed_quant** QEMU scenario green (feature ON) + **arthur_boots_thinking** green (OFF ≡ fixture). **Kernel-touching ⇒ the 4-shard QEMU `workflow_dispatch` gate is mandatory** — serialize behind the in-flight kernel gates (F2/OT-29), rebase LAST.
- Host CI green (fmt/clippy/tests).

## 4. Scope guard
Profile + the targeted forward-path perf fix **only**. **Do NOT**: change the dequant *values*/math (correctness is fixed — F1's KAT is the anchor), change the `.neom` format (L1), touch config (F2) or vocab (F3), or build native kernels unless ISC-1 proves the matmul compute is the floor (don't pre-commit to Option B). No new invariant. Keep the kill-switch default-off ≡ fixture bit-for-bit.

## 5. Merge (PM)
**Close-gate: `scripts/worker-close-gate.sh`** (rebase LAST; full host set incl. `cargo fmt --all --check`; SPDX; self-stat). Two hard rules: green CI URL (push-CI + the 4-shard QEMU `workflow_dispatch` URL) **— paste it**; PM re-inspects `git show --stat`. **The RETURN must also paste the completed Stage-A validation** (the before/after latency + the llama.cpp top-1 parity result — the run that previously STOPPED). PM `pm-merge-check` → `--ff-only`/cherry-pick → ledger (**OT-30 closed — real 0.5B forward runnable + parity-validated; OT-28 Stage-A unblocked → Stage-B**).

**Disk reclaim (after PM confirms merge):** delete the build tree once on `origin/main`; staged GGUF/.neom stays out of git. See CLAUDE.md "Disk hygiene."

---
## Relay — what to tell Jarvis
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-06-22-forward-path-perf-profile-fix-jarvis.md` (+ `forward_logic.rs`, `gguf_dequant_logic.rs`, `ops/qk_matmul.rs`, the merged Stage-A harness in `ai-ops-tests/src/lib.rs`). **Branch:** `codex/forward-path-perf-profile-fix-jarvis` off `origin/main`.
- **One-line:** **PL-ratified profile-first perf pivot (OT-30):** F1's Option-A forward can't emit token 0 in ~11 min on the real 0.5B — perf is now critical-path. **ISC-1: PROFILE to localize the >600×** (don't guess) — **check the build profile FIRST** (the `release ≈ debug` clue: is the kernel `ai` crate actually optimized in the `ai-ops-tests` harness? opt-level=0 inheritance alone is 10–50×), then per-phase timing (dequant vs `gguf_col_major_to_ops_rhs` transpose vs matmul) + allocation bytes/token + a quadratic check. **ISC-2: fix the dominant cause, minimum viable** — build-profile correction (cheapest; Option A may then land in-band), or eliminate per-token materialization (fuse dequant into the matmul / dequant-once cache — note ~2 GB f32 for 0.5B, won't scale to 1.5B), or native in-register quant kernels (Option B) **only if the data proves the matmul compute is the floor — don't pre-commit to 6 kernels.** **ISC-3: correctness unchanged** — F1's mixed-quant byte-oracle KAT stays green, kill-switch OFF ≡ fixture bit-for-bit. **ISC-4 success bar: re-run your merged Stage-A harness (`NEO_REAL_NEOM=…`) and it now COMPLETES** — K tokens, top-1 parity vs llama.cpp, ~0.5–2 s/token (report before/after latency). **Scope: profile + targeted fix ONLY — no dequant-math change, no F2/F3/.neom changes, native kernels only if proven needed.** Kernel-touching ⇒ 4-shard QEMU gate (serialize behind F2/OT-29, rebase LAST). **Close-gate: `scripts/worker-close-gate.sh`; RETURN = green push-CI + QEMU URLs + the completed Stage-A validation (before/after latency + parity).**
