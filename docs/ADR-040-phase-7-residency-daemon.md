<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# ADR-040: Phase 7 — The Async Residency Daemon (BUILD-P4b)

**Status:** Accepted
**Date:** 2026-06-10
**Phase:** 7 (BUILD-P4b — closes the P1→P4 concurrent-residency line)
**Builds on:** ADR-039 (the migration-reservation primitive, §9.12), ADR-038 (§9.11), kthreads
**Ratified design:** `Plans/THINK-2026-06-10-phase-7-build-P4-concurrent-residency.md` §9 (Q1–Q5)
**Extends:** ANSPEC-001 §9.12 (the daemon subsection)
**Amended by:** ADR-041 (BUILD-C1): the monotonic H latch is replaced by a decaying hint behind a Rule-1 kill-switch (decision 3's named follow-on, taken); the mispredict sweep triggers on prediction-withdrawal alone, decoupled from the high watermark (the original coupling would have flaked under decay); production-surface wiring (the decision-1 follow-on) is BLOCKERed as its own build — see ADR-041.

## Context

P4a proved the primitive; P4b is its first consumer — the kswapd/TPP-model
kernel thread that banks headroom and stages data while inference runs. The
three pre-recorded P4a→P4b seams (teardown wake-not-kill, TensorSlot surfaces,
per-device H) are all taken in this build.

## Decisions

1. **The demotion surface is the managed-slot registry, not the executor's
   value table.** The pass's values are private Arcs whose single-mutator
   discipline is the P4a soundness lemma — the daemon cannot and must not
   reach them. Tenants park daemon-eligible tensors in registered
   `TensorSlot`s; the multi-tenant gate makes this the honest value story:
   tenant B's idle pool squeezes the arena, is invisible to tenant A's
   `ensure_room` (which can only evict A's own tensors), and only the daemon
   can drain it. Production wiring of syscall-held tensors and model caches
   into managed slots is named follow-on integration, not P4b mechanism.

2. **The critical-path metric is `ensure_room` evictions + no-room fallbacks**
   — the events that block a placement. The §9.11 op-boundary demote seams are
   proactive housekeeping and are **reported separately per phase** in the
   gate (baseline 0 / daemon 2 on the gate workload), so the metric's
   disjointness is visible in evidence, never assumed (advisor mandate).

3. **Watermarks: `low = H_dev`, `high = min(2·H_dev, C)`, per device.**
   `H_dev` is the device's noted max placement-need, fed by every placement
   consult — replacing P3's pass-global figure (seam 3 taken; value-identical
   on every existing gate by the note-before-seam ordering, proven by the
   regression batch — NOTE the proof is order-dependent: the asserted W=1
   pass runs before the W=0 baseline in the P3 gates, and the W=0 path never
   reads the seam; reordering those passes would break value-identity). On
   accel_sim the band degenerates (`2H ≈ 1.1·C`) to
   drain-all-cold — correct at that scale, oscillation-free because prefetch
   fires only on an explicit publish. **The monotonic-latch hazard is real and
   named:** `H_dev` only grows, so one outsized placement permanently inflates
   the low watermark and over-drains cold tensors thereafter. Blast radius is
   bounded by C and strictly policy-quality (cache-cold reloads — never
   correctness, no thrash: the prefetch gate still holds). A persisted-but-
   decaying estimate (EWMA / windowed-max / max-with-idle-decay) is the named
   follow-on; do NOT build eviction policy assuming the hint tracks the
   working set.

4. **Prefetch needs an explicit consumption signal.** The daemon cannot
   distinguish a consumed prefetch from a misprediction by observation, so
   `ManagedSlot` carries `prefetched` (set at stage-commit) and `mark_used`
   (the tenant's consumption signal, clearing it): mispredict-rollback fires
   only for staged-then-withdrawn-without-use — making the misprediction
   counter mean something. Production analogue: the executor's operand touch.

5. **The daemon parks on its own control state** (token + pending-kick flag
   under one lock — a kick racing the park either finds the token or sets the
   flag the parker tests before sleeping; no lost wake). `kick()` is
   non-blocking and called from the placement consult when free < low: the
   backstop never waits on the daemon.

6. **Teardown = request-stop + kick + join.** Every daemon movement is one
   atomic `try_migrate` (commit/rollback wakes its waiters before returning),
   so "drained" is the natural state between cycles — stop simply prevents the
   next cycle. The gate asserts parked-count == 0 and no stale reservations
   post-join (seam 1 taken).

## The gate

`accel_residency_daemon`: 60 iterations at NEO_SMP=4. Per iteration: B-pool
staged resident (96 KiB squeeze) → baseline pass with the daemon paused (pays
exactly 1 no-room fallback — the backstop completing correctly) → daemon
enabled + kicked, drains B (bg-demote traces), pass re-run (pays 0 — strictly
lower) → prefetch B1 (publish→stage→consume) → deliberate misprediction B2
(publish→stage→withdraw→rollback, byte-checked) → P4a matrix hygiene (parked
balance, deltas, generation/byte consistency via the racing B-accessor
thread) → residency drained to zero. Loop-wide: bg-demotes ≥ 60, prefetches
== 120, mispredicts == 60, heartbeat advanced, teardown quiesce. Per-phase
heartbeat + P3-seam counts in the serial log from run 1 (the P4a lesson,
pre-applied).

## Rejected alternatives

- **Demoting from the executor's value table** — violates the P4a lemma; the
  registry is the honest surface (decision 1).
- **Daemon-initiated (unpublished) prefetch** — reintroduces the §3(a)
  oscillation risk the THINK warned about; publish-only is the structural
  guard.
- **`high = C/4`** (the dispatch's parenthetical) — yields high < low whenever
  H > C/4 (true on accel_sim by 2.2×): broken hysteresis; `min(2H, C)` keeps
  the invariant `high ≥ low` always.
- **Counting P3 seams in the critical-path metric** — charges the daemon phase
  for P3's own banking; reported-not-counted keeps the comparison honest
  (decision 2).
- **A timer-driven daemon tick** — kick-driven suffices for the gate and avoids
  a new periodic source; a timer wake is trivial follow-on if a consumer needs
  unprompted scanning.

## Consequences

- The Phase 7 policy arc is complete as THINK'd: P1 places, P2 reclaims, P3
  anticipates, P4a makes movement safe, P4b moves — the residency trace now
  reads like a mature memory manager (`placement:` / `eviction:` /
  `residency: RETAINED` / `cold-demote` / `bg-demote` / `prefetch`).
- The named remaining lifts: device-side copy-engine DMA overlap (Q3, a
  hardware build), production surface wiring into managed slots, and the
  decaying H estimate (decision 3).
