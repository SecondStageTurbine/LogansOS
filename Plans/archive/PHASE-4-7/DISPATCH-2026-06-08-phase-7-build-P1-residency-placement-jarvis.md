<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Phase 7 BUILD-P1 — residency-aware op placement (the kernel decides) → Jarvis

**Date:** 2026-06-08
**From:** PM/Cerebro
**To:** Jarvis (new branch off `origin/main` `6632717`: `codex/phase-7-build-P1-residency-placement-jarvis`)
**Status:** DISPATCHED. **QEMU — pure policy logic over proven mechanisms; no silicon, low risk.**
**Builds on:** the governance mechanisms (§9.5 accounting, §9.6 scheduler, §9.7 sharing/residency, §9.8 isolation) + the THINK `Plans/THINK-2026-06-08-phase-7-policy-driven-placement.md` (PL accepted: residency-aware first; policy in-kernel; op-placement unit). ADR-030.

## 0. The leap this build makes
Every placement today is **explicit** — a caller/executor names the device. BUILD-P1 is the first build where the **kernel decides**: before dispatching an op, the executor consults an in-kernel **placement policy** that, from inputs only the kernel sees globally (operand residency + §9.5 accounting + the AccelDriver registry), chooses *where the op runs* — and **logs the decision**. The policy's first rule attacks **migration thrash**: prefer the device where the operands already reside, so a mixed forward pass doesn't bounce tensors host↔device on every op. This is the mechanism→policy leap — the trace starts to read like resource management, not dispatch.

## 1. The policy (minimal, first build)
Given an op + its operands' residency + the registry + §9.5:
1. If all operands are resident on one accelerator `D` that covers this op (and the principal is under quota on `D`) → **place on `D`** (no migration). Reason: `operands-resident`.
2. If operands are split across devices → place on the device holding the **most operand bytes** (minimize migration); deterministic tiebreak (lowest DeviceId). Reason: `min-migration`.
3. If no accelerator covers the op, or operands are CPU-resident with no accelerator benefit, or the chosen device is **over the principal's quota** (§9.5/§9.6) → **place on `accel_cpu`**. Reason: `cpu-fallback` / `over-quota`.
**Keep it minimal** — operand-residency preference + quota-awareness + CPU fallback. **No** cost model, eviction, or migration *initiation* (P2/P3); the policy chooses *where to run*, it does not yet *move* tensors proactively.

## 2. ISCs
- **ISC-1 — The placement policy module (pure, host-testable).** A new `placement` module: `decide(op, operand_residency, registry_state, accounting) → (DeviceId, reason)`. Pure/deterministic — no side effects, reads the registry/§9.5 as inputs. Host-unit-tested across the §1 cases (resident / split / cpu-fallback / over-quota).
- **ISC-2 — The executor consults placement before dispatch.** `ai::exec` (currently dispatching to a fixed/default driver) calls `placement::decide` per op and dispatches to the chosen device. Uncovered/over-quota → `accel_cpu` (the existing fallback path).
- **ISC-3 — The decision trace (the gate line).** Emit per placement decision: `placement: op=<kind> principal=PID<n> → device=Accel(<n>)|cpu reason=<operands-resident|min-migration|cpu-fallback|over-quota> migrated=<bytes>`.
- **ISC-4 — The correctness invariant (REQUIRED — placement must not change *what* computes).** Placement changes *where* an op runs, never its result. **The forward-pass output must stay byte-identical / within-tolerance regardless of placement** — assert the §9.5/Phase-5 `infer_*` oracles are unchanged under the placement policy (the BUILD-1 transparency guarantee extended: routing-by-policy produces the same bytes as routing-by-default). A placement that changes output is a bug, not an optimization.
- **ISC-5 — The gate scenario (QEMU): thrash avoided.** A forward pass where operands reside on an accelerator and consecutive ops use them: assert (a) the `placement:` trace fires with the right reasons, (b) the **migration count is strictly lower** than a fixed-non-resident-device baseline that would thrash (the policy *measurably* avoids the round-trips), and (c) the **output is unchanged** (ISC-4). Drive residency deterministically (explicit initial placement), not by race.
- **ISC-6 — Standards.** Default behavior (placement is always-on substrate — but it must be a *strict improvement*: when there's no accelerator/benefit it reproduces today's behavior exactly); SPDX; no new `unsafe`; `cargo fmt`; host suite (placement unit tests + the gate scenario + the `infer_*` transparency regressions + accounting/admission/sharing) green; `shard-coverage`; `clippy -D warnings` (CI/Linux); builds on+off. Open **ANSPEC-001 §9.9** (the placement-policy model — inputs, the residency-preference rule, quota-awareness, the *result-invariance* guarantee, the decision-trace semantics) + a short ADR.

## 3. Gate (two-stage — QEMU, no OptiPlex)
Pure policy logic; no hardware dependency (no blind-hardware run → no failure-mode/discriminator section).
- **Stage A (you):** builds (on+off); placement unit tests + the gate scenario + the `infer_*` transparency regressions green; clippy; fmt; shard-coverage.
- **Stage B (QEMU):** the forward pass shows the `placement:` decisions, a strictly-lower migration count than the thrashing baseline, and **byte-identical/within-tolerance output**. No silicon.

## 4. Scope guard
Residency-aware op-placement *decision* only. **Do NOT** add eviction (BUILD-P2), proactive/cost-aware migration (BUILD-P3), concurrency, or new ops; **do NOT** change *what* any op computes (ISC-4 — placement is result-invariant); **do NOT** duplicate accounting (read §9.5) or re-implement the scheduler (read §9.6 quota). The kernel decides *where* an op runs; it does not yet *move* tensors proactively or change *what* runs.

## 5. Merge (PM, on Stage-B PASS)
Close-gate: `git fetch origin && git rebase origin/main`, re-run Stage-A, push. PM `pm-merge-check` → `--ff-only` → `main`. Then **BUILD-P2** (eviction under device-memory pressure — LRU from §9.5, the `eviction:` decision) and **BUILD-P3** (cost-aware proactive migration). With P1, the kernel makes its first placement *decision* — the trace begins to read like an OS managing a resource.
