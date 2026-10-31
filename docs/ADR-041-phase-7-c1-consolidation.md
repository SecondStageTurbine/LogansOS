<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# ADR-041: Phase 7 — BUILD-C1, the First Consolidation Build

**Status:** Accepted
**Date:** 2026-06-10
**Authority:** `docs/complexity-governance.md` Rule 4 (first invocation — this ADR is the precedent for every future C-build)
**Amends:** ADR-040 (decaying H; the mispredict-sweep trigger)

## What was collapsed

- **One candidate-construction site.** `evict::candidate_from` is now the only
  place a live tensor joins its §9.5 recency into a `ResidencyCandidate`;
  `evict_policy::coldest_eligible` is the only LRU victim ordering. The
  residency daemon's former ad-hoc min-scan is deleted — it builds candidates
  through the shared join and picks through the shared pure selector. (The
  six-line `coldest_eligible` extraction is the one pure-tier touch this build
  makes, required by the unification; `select_demotion` delegates to it and
  its host tests pass unchanged.)
- **`exec.rs` split three ways** — `exec.rs` (the pass driver: value table,
  lookahead, op loop, boundary seam), `exec_ops.rs` (per-op shape/dtype
  rules), `exec_dispatch.rs` (one op's place/room/dispatch journey). The split
  surfaced a real consolidation: every op's dispatch views are simply *all
  operands in order*, so the former ~130-line per-op match collapsed into a
  uniform driver plus one validation function. 723 lines became 244+200+233
  with less total logic.
- **`ai/mod.rs` is the table of contents** (the fresh-reader contract): three
  tiers, one line per module, dependency order — the layering reconstructible
  without git archaeology.

## What was deleted or narrowed

- All six blanket `#![allow(dead_code)]` in `ai/` are gone. Genuinely dead
  items were deleted; gate entry points, named seams, and discriminators carry
  narrow item-level allows with a one-line justification each (Rule 5:
  discriminators are never dead code — they are the ledger's guard column).

**Known selection-order drift (reviewed, accepted):** the daemon's victim
tie-break changed — the old ad-hoc min-scan kept the first REGISTRY-order slot
on a `last_use_tick` tie; the unified `coldest_eligible` picks the lowest
`tensor_id` (the same tie-break the synchronous seam always used — that
asymmetry was itself part of the duplication being collapsed). Byte-preserving
(any victim choice is output-invariant by I-1) and the daemon gate passes
unchanged; logged because a future scenario asserting *which* tensor lands on
CPU could see the other twin.

## The two named debts

- **Decaying H (paid).** The per-device need hint loses an eighth per **demand
  event** — `note_need` composes decay-then-latch (`watermark::decay_step` +
  `latch`, pure, host-tested) — so ongoing traffic itself erodes an outlier
  toward the recent max while quiet windows freeze the band. (The first C1
  gate run caught the naive wake-driven form: kick-heavy drain waits decayed
  `low` below `free` with no notes to oppose it, and the hysteresis hold
  blocked the very drain the gate awaited — decay must be anchored to demand,
  not wakes.) Kill-switch (Rule 1): decay disabled is the identity — unit
  equality over arbitrary latch/decay interleavings. **Consequential fix
  folded in:** the daemon's mispredict sweep no longer gates on the high
  watermark (a decaying `high` would have made rollback timing a flake);
  the withdrawal of an unconsumed prefetch is itself the rollback trigger —
  gate counters unchanged.
- **Production-surface wiring (BLOCKERed, per the dispatch's own boundary).**
  `Handle::Tensor`/`Handle::Model` store bare `Arc<Tensor>` and `Handle` is
  `Clone`: a handle-held tensor always has `strong ≥ 1`, so registering such
  a surface gives the daemon nothing claimable — ever. That is ledger I-4
  working exactly as designed. Real wiring means the handle/model types adopt
  slot indirection (store a managed slot, mint references through
  `guarded_clone`), which changes handle-clone and `sys_tensor_query`
  refcount semantics — an ownership redesign requiring its own dispatch,
  THINK, and gate. Returned to the PM as a named build candidate.

## The ledger audit (Rule 4, rows I-1…I-12)

| Row | Guard named | Runs in |
|-----|-------------|---------|
| I-1 byte-oracle | byte-identical asserts in every gate scenario | all 22 QEMU scenarios, incl. both stress loops |
| I-2 kill-switch | W=0≡P2 unit equality; decay-off≡latch unit equality (new); prior-layer family | host suite + full family every build |
| I-3 single-mutator | P4a stress gate + gen counter/quiesce/owner-uniqueness | `accel_migration_reservation` at NEO_SMP=4 |
| I-4 refcount-as-safety | claim truth-table tests; zero-unsafe diff scrutiny | host suite + CI |
| I-5 lock order | the debug lockdep (panics at the offending acquire) | every dev-profile QEMU run (it has caught two real bugs) |
| I-6 accounting exactness | per-migration delta asserts + balance-to-zero | every gate, NEO_SMP=4 |
| I-7 residency ≤ capacity | §9.5 peak asserts | every pressure gate |
| I-8 anti-thrash | no-partial-eviction + thrash-guard unit tests; oscillation/mispredict counters | host suite + P3/P4b gates |
| I-9 pin protection | falls out of I-4 + pinned-set checks; no-needed-tensor-moved asserts | P2/P3 gates |
| I-10 temporal isolation | admission scenarios + INFLIGHT attribution checks | `accel_admission*` scenarios |
| I-11 no lost wakeup | park/retry balance + quiesce + teardown drain asserts | P4a/P4b gates |
| I-12 byte-layout oracle | the committed `.neom` hand-laid fixture tests | host suite/CI |

All 12 rows verified: every guard is named and runs. One drift found and
fixed: the §1 structural facts (lock census 31, not ~13; pure-share figures
re-measured and split honestly). Zero rows added (Rule 2).

## The fresh-reader test (alarm condition 4)

Performed as a deliberate cold read of `ai/mod.rs` + module heads after the
split. What it forced: the three-tier TOC in `ai/mod.rs` (the layering was
previously reconstructible only from ADR order), explicit tier labels on each
module line, and the §-pointer corrections in module heads (several still
described their build's *forecast* rather than what landed).

## Net effect

Reported in the BUILD-C1 close: net code LOC −295 (269 insertions / 564
deletions across kernel + tools, *after* the narrow justified allows were
added back), six blanket allows removed, one duplicated path collapsed, one
723-line file split, 12/12 ledger rows verified, two debts dispositioned
(one paid, one honestly BLOCKERed).
