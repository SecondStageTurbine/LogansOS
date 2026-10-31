<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Phase 7 BUILD-P3 — cost-aware proactive migration (the kernel anticipates) → Jarvis

**Date:** 2026-06-10
**From:** PM/Cerebro
**To:** Jarvis (new branch off `origin/main` `c7b3075`: `codex/phase-7-build-P3-cost-aware-migration-jarvis`)
**Status:** DISPATCHED. **QEMU — pure policy over proven mechanisms; no silicon, no concurrency, low risk (P1/P2-shaped).**
**Builds on:** BUILD-P2 (`197c41b`, LRU eviction, §9.10, ADR-037) + BUILD-P1 (placement, §9.9, ADR-036) + §9.5 accounting/recency + the AccelDriver registry.
**Ratified design:** `Plans/THINK-2026-06-10-phase-7-build-P3-cost-aware-migration.md` — PL ratified Q1–Q4 as recommended (2026-06-10): **Path A** (synchronous, decision-only); **structural** op-graph reuse signal; **retain + cold-demote only** (prefetch/promotion deferred to P4); **cost in bytes only** (no timing model).

## 0. The decision this build makes
P1 decides *where* an op runs; P2 decides *what leaves* a full device once pressure has arrived. Both are **reactive**. P3 is the first **proactive** decision: at the same in-pass seams P1/P2 already own, the kernel now (a) **retains** a tensor P2 *would have evicted* when it is predicted-hot — evicting it would only force a re-migration next op (net-negative), so a colder victim is chosen instead; and (b) **proactively cold-demotes** a not-predicted-hot tensor to bank a headroom margin, so the *next* placement avoids an eviction round-trip entirely. The kernel stops merely reacting to pressure and starts *anticipating* it. New trace: `residency:`.

## 1. The policy (minimal, Path A, retain + cold-demote only)
**Path A is mandatory:** every decision executes **synchronously inside the executor pass**, the pass remaining the only mutator of its value table. **No background thread, no async mover, no concurrency** — that is BUILD-P4. The `evict.rs` single-mutator invariant (lines 127–133) is **preserved, not relaxed** (ISC-5).

Reuse signal (**structural, not statistical** — Q2): from the op sequence the executor is running, a device-resident tensor is **predicted-hot** iff it is an operand of an upcoming op within a small lookahead window `W` (start **`W = 1`** — "needed by the next op"). No learned model, no access-counter sampling — only the op-dependency graph the pass already holds.

Cost is measured in **bytes only** (Q4) — keep-cost = device bytes held idle; evict-cost = bytes that must re-migrate if a predicted-hot tensor is evicted. No wall-clock, no transfer-latency estimate.

On the P2 `ensure_room` consult (device capacity `C`, resident `R`, need `N`):
1. **Retain:** when choosing victims, a **predicted-hot** tensor is *not evictable for this op* even if it is the LRU coldest — prefer a colder *not-predicted-hot* victim of sufficient size instead. If retaining predicted-hot tensors leaves no way to make room, fall through to P2's exact behavior (evict by LRU among the remaining; then `no-room-cpu-fallback` if still short). Reason: `retain-hot`.
2. **Proactive cold-demote (headroom banking):** maintain a **headroom target** `H` (a small fraction of `C`, e.g. one max-operand-tensor worth — pick a concrete default and justify it in the ADR). After a placement settles, if `resident > C − H` *and* there exists a **not-predicted-hot** resident tensor, demote the coldest such tensor toward the headroom target. **Bounded:** never demote a predicted-hot tensor; never demote below what the predicted-hot working set needs. Reason: `cold-demote`.
3. **No-op** otherwise. **P3 is a strict superset of P2:** with `W = 0` / no prediction available, it reproduces P2 *bit for bit*.

**Thrash guard (REQUIRED, first-class):** never make a proactive move whose own predicted consequence is a future eviction of a predicted-hot tensor. A proactive migration the model itself predicts will be undone is a *bug*, not a tradeoff.

## 2. ISCs
- **ISC-1 — The reuse predictor (pure, host-testable).** From the remaining op sequence + each op's operand tensor ids, compute the **predicted-hot set** within lookahead `W`. Pure/deterministic. Unit-tested: operand-of-next-op is hot; a tensor used only beyond `W` is cold; output-not-yet-produced is not hot. Keep it in the kernel-free policy file so `tools/ai-ops-tests` can path-`mod` it (the P1/P2 pattern).
- **ISC-2 — The residency policy (pure, host-testable).** `residency_policy::decide(capacity, resident, need, candidates_with_recency, predicted_hot_set, headroom_target) → (victims, demotions, reason)` where `reason ∈ {NoEviction, RetainHot, Lru, ColdDemote, NoRoomCpuFallback}`. **Extend `evict_policy.rs` rather than duplicate it** — `select_victims` gains the predicted-hot exclusion; add the cold-demote selector. Unit-tested across: retain-hot-picks-colder-victim, retain-hot-leaves-no-room→fall-through-to-LRU, cold-demote-banks-headroom, never-demote-hot, `W=0`→identical-to-P2, thrash-guard-refuses-self-defeating-move.
- **ISC-3 — Wire into the executor pass (synchronous, single-mutator).** The executor (which already holds the op sequence and the value table) supplies the predicted-hot set to `ensure_room`, and runs the post-placement cold-demote step **within the same pass**, reusing `Tensor::migrate` (no parallel residency path, exactly as P2). Pin protection (operands/output of the current op) and the `Arc` strong==1 / weak==0 evictability gate from P2 are **unchanged and still apply** — predicted-hot is an *additional* protection, never a relaxation.
- **ISC-4 — Result-invariance (REQUIRED — same discipline as P1/P2).** Retention and proactive demotion change *where* a tensor lives, never its bytes. **The forward-pass output must stay byte-identical to the P2/unbounded reference** regardless of retain/demote decisions. A wrong result, or a predicted-hot tensor moved out from under its op, is a *bug*, not a perf tradeoff.
- **ISC-5 — Invariant preservation (REQUIRED — the SMP seam).** Path A introduces **no second mutator**: the device stays run-to-completion one-per-device, and the pass stays the only mutator of its value table. **Update the `evict.rs:127–133` coherence note** to record that the proactive decision was added *without* relaxing the single-mutator property (and that concurrent movement remains deferred to P4 + the migration-reservation primitive). No new `unsafe`, no new lock-order edges.
- **ISC-6 — The decision trace + the gate scenario (QEMU).** Traces:
  `residency: tensor=<id> device=Accel(n) RETAINED reason=reuse-soon (next-use≤W, would-evict=<bytes>)`
  `residency: tensor=<id> device=Accel(n) → cpu reason=cold-demote (resident=<R>/<C>, headroom-target=<H>)`
  Gate scenario: a forward pass on a bounded device (accel_sim's arena) constructed so that **P2 would evict a tensor the very next op needs**. Assert (a) the `residency: … RETAINED` trace fires and a *colder* tensor is demoted instead; (b) proactive cold-demote banks headroom so the next placement takes **zero** eviction round-trips (eviction count strictly lower than a P2 baseline on the same workload); (c) residency stays `≤ C` throughout; (d) **no predicted-hot tensor is ever migrated off-device while an upcoming op needs it**; (e) **output byte-identical** to the P2/unbounded reference. Drive the prediction deterministically (a fixed op sequence + sized tensors + known capacity), not by race.
- **ISC-7 — Standards.** Always-on substrate but a **strict superset** (`W=0`/no prediction → P2 verbatim); SPDX; **no new `unsafe`**; `cargo fmt`; host suite (predictor + residency-policy unit tests + the proactive scenario + P1/P2/`infer_*`/accounting/admission/sharing regressions) green; `shard-coverage`; `clippy -D warnings` (CI/Linux); builds on+off. Open **ANSPEC-001 §9.11** (the proactive residency model — structural reuse prediction, the retain + cold-demote decision, the headroom target, the thrash guard, result-invariance, the `residency:` trace, and an explicit note that movement stays synchronous/single-mutator — P4 owns concurrency) + **ADR-038**.

## 3. Gate (two-stage — QEMU, no OptiPlex)
Pure policy logic; no hardware dependency, **no concurrency** (no blind-hardware run and no SMP-race surface → no failure-mode/discriminator section; the single-mutator invariant is preserved per ISC-5, so there is no new race to discriminate).
- **Stage A (you):** builds (on+off); predictor + residency-policy unit tests + the proactive scenario + regressions green; clippy; fmt; shard-coverage.
- **Stage B (QEMU):** the forward pass shows the `residency: RETAINED` + `cold-demote` decisions, a strictly-lower eviction count than the P2 baseline on the same workload, residency bounded by capacity, no predicted-hot tensor evicted, and byte-identical output. No silicon.

## 4. Scope guard
Synchronous, decision-only retain + proactive cold-demote. **Do NOT** add a background/async mover, a residency daemon, or any migration concurrent with dispatch (**BUILD-P4** — that is the build that introduces the migration-reservation primitive and *relaxes* the `evict.rs` invariant); **do NOT** add early-prefetch/promotion (deferred to P4 per Q3); **do NOT** use a learned/statistical predictor or access-counter sampling (structural op-graph only, Q2); **do NOT** add a timing/bandwidth cost model (bytes only, Q4); **do NOT** add eviction-to-another-accelerator or sub-tensor block migration; **do NOT** change *what* any op computes (ISC-4); **do NOT** relax the single-mutator invariant (ISC-5); **do NOT** duplicate §9.5 (read recency) or re-implement P2 (extend `evict_policy.rs`).

## 5. Merge (PM, on Stage-B PASS)
Close-gate: `git fetch origin && git rebase origin/main`, re-run Stage-A, push the rebased branch. PM `pm-merge-check` → `--ff-only` → `main`. Then **BUILD-P4** (concurrent residency management — the migration-reservation primitive from the THINK §3(c)/§4 that makes a concurrent accessor block-and-retry, plus the async residency daemon for prefetch/promotion; the build that relaxes and re-proves the `evict.rs` single-mutator invariant, with its own NEO_SMP concurrency gate and the migration-race discriminator). With P3, the kernel anticipates pressure rather than only reacting to it — the last reactive→proactive step the policy layer can take *without* concurrency.

---

## Relay — what to tell Jarvis
- **Dispatch (authoritative, on `main`):** `Plans/DISPATCH-2026-06-10-phase-7-build-P3-cost-aware-migration-jarvis.md`
- **THINK (ratified design rationale + prior art):** `Plans/THINK-2026-06-10-phase-7-build-P3-cost-aware-migration.md`
- **Branch:** `codex/phase-7-build-P3-cost-aware-migration-jarvis` off `origin/main` `c7b3075`.
- **One-line:** Path A only — synchronous retain + proactive cold-demote, structural op-graph reuse signal (`W=1`), bytes-only cost; preserve the `evict.rs` single-mutator invariant (no concurrency — that's P4); strict superset of P2; open §9.11 + ADR-038; QEMU gate, no silicon.
