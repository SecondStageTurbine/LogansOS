<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# ADR-038: Phase 7 — Proactive Residency: Cost-Aware Retention + Headroom Banking (BUILD-P3, Path A)

**Status:** Accepted
**Date:** 2026-06-10
**Phase:** 7 (BUILD-P3)
**Builds on:** ADR-037 (eviction, §9.10), ADR-036 (placement, §9.9), §9.5 accounting/recency
**Ratified design:** `Plans/THINK-2026-06-10-phase-7-build-P3-cost-aware-migration.md` (PL ratified Q1–Q4: Path A; structural signal; retain + cold-demote only; bytes-only cost)
**Opens:** ANSPEC-001 §9.11

## Context

P1 places, P2 makes room — both react. The next governance step is anticipation:
refusing an eviction the op graph says will be undone one op later, and banking
headroom before pressure arrives. The THINK pass fixed the architecture (Path A:
synchronous, decision-only, single-mutator preserved; concurrency is P4) and the
signal (structural op-graph lookahead, `W = 1`). This ADR records the build's
concrete decisions.

## Decisions

1. **Predictor is pure and index-based.** `ai/predictor.rs` (host-tested,
   kernel-free) computes over **value indices**, not tensor ids: `hot_values`
   (consumed within `W` remaining ops) and `future_use_values` (consumed by any
   remaining op). The executor resolves indices against its value table, so a
   not-yet-produced operand marks nothing by construction.

2. **The policy extends `evict_policy.rs`; P2's selector is untouched.**
   `decide_eviction(...) → {victims, retained, reason}` adds the predicted-hot
   exclusion in phase 1 and falls through to the §9.10-exact selection (hot
   included) when retention cannot make room; `retained` reports exactly the
   hot tensors plain LRU would have evicted before the warmest chosen victim —
   the `RETAINED` trace lines. `select_demotion` picks at most one
   dead-to-the-pass tensor per seam. `select_victims` and its P2 host tests
   are unchanged, and the host suite asserts `decide_eviction` with no
   prediction flags equals `select_victims` across the P2 case matrix — the
   strict-superset proof at the policy layer.

3. **Headroom target `H` = the largest placement-need seen so far this pass**
   (`output + transient staging`), not the dispatch's suggested "one
   max-operand-tensor worth": on arena devices need ≈ 3× tensor bytes, so one
   tensor-worth provably never crosses the threshold on the gate workloads —
   it under-banks by the staging factor. One placement's worth is the smallest
   `H` that can actually prevent the next eviction round-trip. The **backward
   max** (not the next op's forward need, which `W = 1` could see) is chosen
   for stability: it is monotone within a pass, so the threshold never
   chatters demote decisions the way a per-op forward estimate would.

4. **Demote eligibility = no remaining op use ∧ not a declared model output —
   the thrash guard realized structurally.** A `W`-window demote eligibility is
   provably unsound: on the retain-gate graph it demotes the op-5 operand at
   the op-2 seam (re-migration guaranteed — the exact self-defeating move the
   guard forbids). The asymmetry with retention's narrow `W = 1` window is
   failure-cost-driven: retention failing just falls through to LRU (benign);
   demotion failing is the thrash this build eliminates (fatal). Optimistic
   where wrong is cheap, pessimistic where wrong is fatal.

5. **The demote seam runs at the op boundary in `run_inner`, not inside the
   placement consult.** An op's own operands are pinned by their live
   references for the whole of `allocate_result` — a seam there can never
   demote the tensor that just went dead (it is the op's own operand, still
   referenced). At the boundary, after the node's operand references drop,
   the dead tensor becomes the legal candidate the arithmetic expects. The
   seam never runs after the final op (headroom banks for a next placement;
   none exists) and sweeps every capacity-declaring device.

6. **`exec::run` flips to `W = 1` (the always-on substrate);
   `exec::run_reactive` (`W = 0`) pins the reactive layer.** Caller
   enumeration at build time: the inference syscall (pressure-free at current
   workloads, proactive arms inert by arithmetic), the P1 placement gate
   (32-byte tensors — inert), and the P2 eviction gate. Only the P2 gate has
   pressure-coupled exact counters; it now runs `run_reactive` and keeps
   gating §9.10 verbatim, while `accel_residency_demote` gates the `W = 1`
   default **on the same fixture** — the improvement (0 evictions vs 2) is
   the build's measured claim, not a regression.

7. **Two complementary QEMU gates instead of the dispatch's single scenario.**
   The precise impossibility: a single quota-bounded pass cannot demonstrate
   *eviction-elimination* (0-vs-2 requires demote to fire early and kill the
   pressure) and *retention-under-pressure* (requires the pressure to arrive)
   at maximal strength simultaneously — §9.6 `DISPATCH_QUOTA = 4` caps a pass
   at ~5 ops, leaving no room to stage both. The arms do **compose** (the
   retain gate shows retain + evict + demote co-firing in one pass); what
   cannot co-exist is the two *strong assertions*. Coverage union closes the
   dispatch gate: retain ⇒ (a),(c),(d),(e); demote ⇒ (b),(c),(e) with the
   strictly-lower eviction comparison. Both gates run their `W = 0` baseline
   under a separate principal (`PrincipalGuard`) to isolate the §9.6 quota,
   draining residency to zero between runs.

8. **The retain fixture makes the cold victim a declared model output.**
   `valid_retain_chain` (1×10240 F32, `Gelu→Relu→Gelu→Relu→Add(a,d)`, outputs
   `{b, e}`): `b` has no remaining op use yet is output-protected from
   demotion, which keeps the pre-pressure seams demote-free so the pressure
   can arrive — and `b` is still a legal *reactive* eviction victim (eviction
   is reactive; the thrash guard governs proactive moves only). At op 4 plain
   LRU's coldest is `a` (the op-5 operand); P3 retains `a`, evicts `b`.

## The traces

```
residency: tensor=<id> device=Accel(<n>) RETAINED reason=reuse-soon (next-use<=W, would-evict=<bytes>)
residency: tensor=<id> device=Accel(<n>) → cpu reason=cold-demote (resident=<R>/<C>, headroom-target=<H>)
```

## Rejected alternatives

- **Path B (async mover/daemon) in this build** — rejected by the ratified
  THINK: couples an unproven cost model to an unproven concurrency mechanism;
  P4 owns the migration-reservation primitive and the invariant relaxation.
- **`W`-window demote eligibility** — provably thrashes on reuse graphs
  (decision 4); the full-remaining-scan is the honest guard.
- **`H` = one tensor's bytes** — under-banks by the staging factor; never
  fires on the gate workloads (decision 3).
- **Forward next-op need as `H`** — oscillates the threshold; rejected for
  the monotone backward max (decision 3).
- **Keeping `run()` reactive and gating `W = 1` behind a flag** — violates the
  always-on-substrate intent; the enumeration showed exactly one caller needed
  pinning (decision 6).
- **One gate scenario with both arms maximal** — arithmetically impossible
  under the quota (decision 7).

## Consequences

- The placed path gains one per-op set computation (two BTree set builds over
  the remaining graph — O(n²) on op count, trivially small) and one demote
  consult per boundary on capacity devices.
- `H` is pass-global, not per-device: `bank_headroom_all` applies the largest
  need seen anywhere to every capacity device. Exact for today's single
  capacity device (accel_sim); on a future heterogeneous multi-accelerator
  system it could demote earlier than necessary on a small device — always
  safe (dead tensors only), revisit with P4's multi-device work.
- `evict.rs`'s single-mutator coherence note now records that P3 added the
  proactive decision *without* relaxing the invariant; P4 is the named build
  that relaxes and re-proves it on the migration-reservation primitive.
- BUILD-P4's inputs are staged: the predictor, the residency policy shape,
  per-device capacity/residency/peak, and two proactive gates to regress
  against.
