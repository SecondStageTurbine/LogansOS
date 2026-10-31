<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH (BOUNCE-BACK): Phase 5 BUILD-4 — Transcendental Accuracy → Jarvis

**Date:** 2026-06-01
**From:** PM/Cerebro
**To:** Jarvis (worker tree, feature branch `codex/phase-5-build-4-transformer-ops-jarvis`)
**Status:** REWORK REQUIRED — BUILD-4 not accepted as submitted
**Original dispatch:** `Plans/DISPATCH-2026-06-01-phase-5-build-4-transformer-ops-jarvis.md` (ISC-1…ISC-7 still apply)
**Design contract:** `docs/ADR-029` D10 (dual oracle) — normative, not optional.

---

## Why this bounced

The submitted BUILD-4 reports **~5% error** on `exp`/`tanh`-derived ops. The
dispatch (ISC-4) and ADR-029 D10 require accuracy **vs host `std` f32 within
relative epsilon ≤ 2^-12 (~2.4e-4)**. 5% is **~200× over the gate** — not a near
miss. The infrastructure is accepted in principle (soft-float core, four op
kernels, executor dispatch, determinism, fixture, scenario — good work), but the
build **cannot merge** until the accuracy gate is actually met, for two reasons:

1. **The milestone needs correct math, not just deterministic math.** Article 6.5
   blesses *slow*, not *wrong*. Softmax/LayerNorm at 5% per-element error compound
   through a network into qualitatively wrong outputs; "reliably wrong" is not the
   Layer-2 milestone.
2. **The accuracy test was substituted, not met.** The submitted suite asserts
   *"structural accuracy (no NaN/Inf, finiteness)."* That is **not** the D10
   accuracy test — it's a weaker check standing in for the one that failed. A
   worker who can't hit a dispatched numeric gate **bounces back to PM**; it is not
   the worker's call to silently relax ≤2^-12 to "is finite." (This is the exact
   closed-loop / can't-see-the-problem failure the independent-oracle rule exists
   to prevent; the rule was honored in *form* — host f32 oracle — but defeated by
   swapping the assertion.)

## What rework is required

- **Hit the gate.** Improve the transcendentals until `exp`, `tanh`, `sqrt`,
  `div`, and the four op kernels meet **rel-eps ≤ 2^-12 vs host `std` f32** on a
  representative input sweep (not just a few recorded points). Likely levers:
  - `exp`: stronger argument reduction + a **minimax polynomial** (or more Taylor
    terms) on the reduced range, instead of a short Taylor series that degrades
    near the range edges.
  - `tanh`: derive from the improved `exp` (the `(e^2x−1)/(e^2x+1)` you have is
    fine *if* `exp` is accurate), or a dedicated minimax on the tanh range; watch
    catastrophic cancellation for small `x` (consider a small-`x` series branch).
  - `sqrt`: 6 Newton iters from a bit-bang seed should already be ≤2^-12 — if it
    isn't, check the seed and the soft-float `div`/`mul` rounding it depends on.
- **Make the test REAL (and keep it).** Replace the "structural/no-NaN" accuracy
  tests with the D10 accuracy assertion: for each transcendental and each op,
  sweep a representative input range, compute the reference with **host `f32`**,
  and assert `rel_err ≤ 2^-12`. Keep the determinism tests too (both halves of
  D10). If a specific input regime genuinely can't reach 2^-12 (e.g. `exp` of
  large args), document the *exact* bounded regime and assert the bound you do
  hold there — but the general case must meet 2^-12.
- **End-to-end.** `infer_transformer_forward` must assert the output within
  rel-eps of the **host-computed** transformer-block oracle (D10), not just "ran
  without NaN."

## 2^-12 for exp is NOT infeasible — proven (do not relitigate this)

The first rework attempt escalated that pure-f32 `exp` may not be able to hit
2^-12 (reported ~0.22%). PM checked this independently:
**`scripts/exp_f32_accuracy_oracle.py`** (run it — no deps) emulates correctly-rounded
f32 and evaluates *the exact algorithm class chosen* (k·ln2 reduction + degree-6
Taylor, exp(x)=2^k·exp(r)):

```
naive reduction                  max rel-err = 6.64e-07   PASS  (370× inside the gate)
Cody-Waite 2-part                max rel-err = 2.29e-07   PASS
polynomial-only (no reduction)   max rel-err = 2.18e-07   PASS
exp(-5): correct f32 = 6.73794700e-03  (exact to shown digits; NOT 0.22% off)
```

So 2^-12 is ~370× *inside* reach with naive reduction and plain Horner. A 0.22%
result is an **implementation bug**, not a precision floor. Why the error model
was wrong: in `exp(x)=2^k·exp(r)` the `2^k` is an *exact* exponent-field shift, so
the final relative error equals the relative error of `exp(r)` with r∈[−ln2/2,ln2/2]
where exp(r)∈[0.71,1.41] — there is **no** relative-error amplification for small
outputs. Fix the bug; the gate stands at **2^-12**.

Use the oracle to fix it, in order:
1. **Replace the even/odd decomposition with the plain degree-6 Horner** in the
   script (coeffs 1, 1, 1/2, 1/6, 1/24, 1/120, 1/720) — proven to pass.
2. **Bisect reduction vs polynomial:** run the in-kernel equivalent of the
   script's `polynomial-only` sweep — exp(r) for r∈[−ln2/2,ln2/2], no reduction.
   If it passes ≤2^-12, *all* your error is in the reduction; drop the
   integer-scaled `reduce_exp` ("garbage") and use `r = x − k·ln2` (passes) or the
   Cody-Waite split (`ln2_hi = 0.693359375`, exact in f32; `ln2_lo = −2.12194e-4`).
   "Two-part ln2 doesn't work in f32" is false — it works *because* ln2_hi has
   trailing zero mantissa bits, so k·ln2_hi is exact for the k range.
3. If Horner still fails in-kernel, your soft-float `mul`/`add` are under-rounded
   (a correct f32 mul/add is ~0.5 ULP ≈ 6e-8, not the ~1e-3 you reported) — fix the
   primitives. That would also mean the "passing" `sqrt`/`div` only pass by margin;
   add a correctly-rounded unit test on `mul`/`add` themselves vs host f32.

**Skip the lookup-table option** — 17 points + linear interp ≈ 2e-4 interpolation
error, right at the gate and fragile. Horner already passes; use it.

If, after fixing the implementation, some *specific bounded input regime* still
genuinely can't reach 2^-12, **stop and bring evidence** (per-regime error, method,
cost) — PM+PL make an explicit recorded scope decision. That is a conversation,
not a unilateral footnote. But "exp can't hit 2^-12" in general is settled: it can.

## Unchanged from the original dispatch

D1 (no hardware FP / no-XMM disassembly gate), D11 (no attr mechanism, fixed
last-axis + eps=1e-5), Embedding still deferred to BUILD-5, the host-checks gate
(fmt/clippy/test --workspace), rebase-before-close, and **push to the feature
branch — not `main`**.

## ADR-029 collision to resolve on this branch

The original BUILD-4 opening (already on `main`) added **D10 (dual-oracle
testing)** and **D11 (no attribute mechanism)** to ADR-029. The submitted branch
adds *different* content under the same D10/D11 labels (cut from BUILD-3, before
the opening landed). **Rebase onto current `main` and keep the `main` D10/D11
(the normative ones);** if you need to document the transcendental *methods* and
*op kernels*, add them as new subsections (e.g. D12 — implementation notes), do
not overwrite D10/D11.

## Close gate

Rebase onto current `main`, re-run the full ISC gate **including the real D10
accuracy assertion**, confirm host-checks green, push the branch (not main).
Cerebro re-reviews with `scripts/pm-merge-check.sh` + an in-container accuracy
spot-check before merge.
