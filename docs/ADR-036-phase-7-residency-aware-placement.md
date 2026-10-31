<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# ADR-036 — Phase 7 BUILD-P1: residency-aware op placement (the kernel decides where an op runs)

**Status:** Accepted (Stage-B QEMU PASS, 2026-06-09).
**Date:** 2026-06-09
**Builds on:** BUILD-1 (accounting, ADR-033, §9.5); BUILD-2/2L (admission scheduler, ADR-034, §9.6 — the `DISPATCH_QUOTA` predicate read here); BUILD-3 (tensor residency/`DeviceId`, ADR-035, §9.7); the BUILD-4X arc (spatial isolation, ADR-032, §9.8); the THINK (`Plans/THINK-2026-06-08-phase-7-policy-driven-placement.md`, PL accepted: residency-aware first, policy in-kernel, op-placement unit).
**Related:** ANSPEC-001 §9.9 (the placement-policy model — opened by this build); §9.4 (the result-invariance guarantee, extended); §9.5/§9.6 (read for residency-folding and quota).

## Context

§9.5–§9.8 built the governance **mechanisms** — measure, arbitrate, own, isolate. They are exercised and (for spatial isolation) silicon-enforced. What remained is the **policy** layer: the kernel *reasoning* about placement, not merely *executing* a placement a caller named. Every placement before this build was explicit — the executor dispatched every op to a fixed/default driver. BUILD-P1 is the first build where the **kernel decides**: before dispatching an op, the executor consults an in-kernel placement policy that, from inputs only the kernel sees globally (operand residency + §9.5 accounting + the registry), chooses *where the op runs* and **logs the decision**. The first rule attacks **migration thrash** — the correctness-for-performance concern from the THINK §3: a mixed forward pass that bounces a tensor device→host→device on every op can be net-negative. Lean: the policy *decides where to run*; it does **not** yet move tensors proactively (BUILD-P2/P3).

## Decision

A pure, in-kernel residency-aware placement policy the executor consults per op, result-invariant, with a per-op decision trace.

1. **The pure policy (ISC-1).** `placement_policy.rs` is kernel-free (no `alloc`, `#[path]`-mod'd by the host suite) and host-unit-tested across the §1 cases (resident / split / cpu-fallback / over-quota / uncovered) plus the deterministic lowest-id tiebreak. `decide(candidates, total_operand_bytes) → Placement{device, reason, migrated_bytes}`. Among *selectable* candidates (`covers_op` AND (`!over_quota` OR is the host fallback)) the one holding the **most operand bytes** wins, lowest `DeviceId` breaks ties. The host fallback (accel_cpu) is always selectable — the policy is total. Reasons: `operands-resident` (all bytes on one accelerator, zero migration), `min-migration` (split — most-bytes device), `cpu-fallback` (no accelerator benefit), `over-quota` (residency-preferred accelerator over its §9.5 quota → spill to host).

2. **The kernel glue (`placement.rs`).** Snapshots the live state into the policy's inputs: operand residency from each operand tensor's `DeviceId` (host-class `Device::Cpu` folds into the accel_cpu candidate — same memory, no migration between them), op coverage from each registered driver's `supported_ops`, and the per-principal quota from the **§9.5 accounting table** (`dispatch_count ≥ DISPATCH_QUOTA`, the §9.6 predicate — read, never a second counter). Converts the chosen `device_abi` back to a `Device`, and emits the trace.

3. **The executor consults placement before dispatch (ISC-2).** `exec::run` (the default path) now routes each op through `Target::Placed`: it builds the operand-residency snapshot, calls `placement::decide_placement`, dispatches to the chosen device's driver, and — for a real accelerator — allocates the op's **output on that device** so a chain keeps its intermediates resident (the next op sees its operand in place; thrash avoided). The host fallback still yields `Device::Cpu` outputs, reproducing pre-P1 behavior exactly. The legacy fixed-driver path survives as `run_with_driver` (`Target::Fixed`) for the cross-device equivalence scenarios and the gate's thrashing baseline; its behavior is unchanged (outputs stay `Device::Cpu`).

4. **Result-invariance — the load-bearing invariant (ISC-4).** Placement changes *where* an op runs, **never its result.** This extends §9.4's numerical-equivalence guarantee from "any backend computes the same bytes" to "routing-by-policy computes the same bytes as routing-by-default." The gate asserts byte-identical output against a host reference; a placement that changes output is a bug, not an optimization.

5. **The decision trace (ISC-3 — the governance line).** One line per decision: `placement: op=<kind> principal=PID<n> → device=Accel(<n>)|cpu reason=<…> migrated=<bytes>`. The reasoning the application did not request — the moment the trace starts to read like resource management.

6. **The migration metric.** A residency-folded counter (`placement::migration_to`) tallies operands not resident on a dispatch's run device, fed identically by both the placed and fixed paths so the gate compares like for like. It measures the migration a dispatch *implies* — the policy minimizes it by choosing the resident device; P1 does not yet *perform* proactive moves (P3).

## Seams (named, not built)

- **Eviction (BUILD-P2)** — when a device is at memory quota and a new tensor needs residency, decide which resident tensor to evict (LRU from §9.5), the `eviction:` trace.
- **Proactive / cost-aware migration (BUILD-P3)** — *move* tensors in anticipation on a transfer-cost-vs-compute-benefit model, the `residency:` retain/migrate trace. P1 chooses where to run; it does not yet move tensors proactively.
- **Concurrency** — placement is per op in a synchronous pass; cross-device parallel placement is later policy.

## Gate / Stage result

QEMU, pure policy logic over proven mechanisms — no hardware dependency, no blind-hardware run (so no failure-mode/discriminator section).

- **Stage A (complete):** builds on+off; the pure placement-policy host unit tests (the §1 cases + tiebreak) + the full ai-ops-tests suite (53 oracles) green; `shard-coverage` (199 scenarios, incl. `accel_placement_residency`); fmt; my code clippy-clean. The `accel_placement_residency` ISO builds. New fixture `valid_unary_chain.neom` (a weight-free `input → Gelu → Relu → Gelu → Relu` chain) generated + validated by neomc.
- **Stage B (QEMU, PASS — 2026-06-09):** the `accel_placement_residency` scenario migrates the chain's input onto `Accel(1)` (accel_sim), then runs two measured passes. **Policy** (`exec::run`): all four ops placed on `Accel(1)` with `reason=operands-resident migrated=0` — outputs kept there → **migration count 0**. **Baseline** (`run_with_driver(accel_sim)`): host-resident intermediates force every op after the first to re-stage host→device → **migration count 3**. The `placement:` trace fired with the right reason on every op, `policy_migrations=0 < baseline_migrations=3`, and the output was **byte-identical** to the accel_cpu reference (`output_unchanged=true`). Evidence: `Plans/evidence/phase7-buildP1-qemu-PASS-placement-residency.log`. §9.9 is now the live policy floor; BUILD-P2 (eviction) is unblocked. The headline trace from the THINK is real:
  ```
  placement: op=Gelu principal=PID0 → device=Accel(1) reason=operands-resident migrated=0
  accel-placement: gate policy_migrations=0 baseline_migrations=3 output_unchanged=true
  ```
