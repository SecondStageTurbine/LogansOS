<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# ADR-039: Phase 7 — The Migration-Reservation Primitive (BUILD-P4a)

**Status:** Accepted
**Date:** 2026-06-10
**Phase:** 7 (BUILD-P4a)
**Builds on:** ADR-038 (§9.11), ADR-037 (§9.10), ADR-019 (`SwitchMutex`), the scheduler wake-token primitive
**Ratified design:** `Plans/THINK-2026-06-10-phase-7-build-P4-concurrent-residency.md` (Q1–Q5)
**Opens:** ANSPEC-001 §9.12

## Context

P4b's residency daemon must move tensors concurrently with dispatch, which
relaxes the single-mutator invariant every policy build since P1 rested on.
The THINK split the work: P4a builds and race-proves the primitive alone (this
ADR); P4b builds the mover on top. The gate is a concurrency stress loop with
every §6-matrix failure mode self-diagnosing — the SMP analogue of the
blind-hardware discriminator rule.

## Decisions

1. **Claim = possession (the `PG_isolated` analogue).** The claim takes the
   `Arc` out of the owning `TensorSlot`, leaving a `Reserved{tensor_id}`
   marker, and the copy runs with **no locks held** — the mover owns the only
   reference, so `Arc::get_mut` cannot fail and the container is never
   monopolized. The first design draft held the slot lock across the migrate;
   that was discarded because accessors would serialize on the slot spinlock
   and never observe the reservation — the park path would be dead code and
   the gate's `retries > 0` assertion unfalsifiable. Possession is also
   exactly the shape the P4b daemon needs.

2. **Keyed by `tensor_id`** (device recorded implicitly by the tensor) — the
   P2 recency-table insight: a tensor is resident on exactly one device.

3. **The claim gate is the §9.10 evictability predicate, contention checked
   first.** `decide_claim(reserved, strong, weak)` is pure and host-tested.
   `weak == 0` is load-bearing: a `Weak::upgrade` (the §9.7 dedup registry)
   could mint a reference without the container lock; a `Weak`-bearing tensor
   is therefore never claimable. Confirmed: no `Arc::into_raw`-class escape
   exists on tensor `Arc`s (the only raw-pointer constructors are
   lifetime-bound byte-slice views), so the reference census is closed.

4. **Park via the proven wake-token pattern.** The accessor installs its token
   while the reservation lock is held (`block_current_with_token_and_guard` —
   the admission/kthread/NeoFS-writeback precedent), releasing the slot lock
   first (outer before inner park; lock-order safe because both sides acquire
   `SLOT → RESERVATION`). Commit and rollback take-and-wake identically — no
   token is ever stranded. On wake the accessor re-resolves from scratch.

5. **"One critical section" realized as observer-atomicity.** The §9.5 delta
   is asserted at the commit boundary; within a migration the same transient
   source+target accounting overlap exists that every synchronous migrate has
   had since P2 (new storage accounted before old frees). No observer can see
   it inconsistently with the storage — every observer parks until commit.

6. **The lockdep (discriminator #3).** `SwitchMutex::new_ordered(value, level)`
   + a `debug_assertions`-gated per-thread held-levels table (64 slots,
   lock-free, thread-keyed so a preempted holder cannot false-positive another
   thread; full table degrades to skip). Levels:
   `QUEUES(10) → SLOT(15) → RESERVATION(20) → INFLIGHT(22) → TABLE(25) →
   RECENCY(28)` — levels occupy the lockdep's 1..=31 bitmask domain (the gate's
   first run caught level 40 panicking the lockdep's own range assertion: a
   discriminator discriminating its author); the scheduler lock stays
   unordered (always acquired last via
   the park primitive). Wired: admission `QUEUES`, accounting
   `INFLIGHT`/`TABLE`/`RECENCY`, the reservation table, `TensorSlot`.
   Release hooks on both the guard `Drop` and the context-switch token path
   (idempotent — whichever runs).

7. **The deterministic park.** A purely probabilistic race gate could pass
   with the park path never exercised. The gate's double-claim probe holds a
   claim **until the accessor is observed parked** (gauge ≥ 1), then releases
   — guaranteeing `retries > 0` at any SMP width while the rest of the loop
   races freely. The watchdog is the mover's bounded retry budget (every spin
   loop fails with a named reason on exhaustion) plus the harness timeout.

8. **Storage generation counter on `Tensor`** (#1): `AtomicU64`, bumped on
   every storage swap in both migrate directions, read with acquire ordering.
   Benign for all earlier builds (monotonic, never read by them).

## The gate

`accel_migration_reservation`: 60 iterations (≥ 50, the `sys_wait` precedent),
NEO_SMP=4. Mover (scenario main) does 4 device↔CPU migrations per iteration of
a 64 KiB patterned tensor in a shared `TensorSlot`; a long-lived accessor
thread continuously `guarded_clone`s and checksums it. Per iteration: owner
uniqueness + double-claim refusal, deterministic park + quiesce-to-zero, exact
§9.5 deltas, zero generation-inconsistent reads, zero byte divergences,
device residency zero between iterations. Loop-wide: retries > 0, commit count
exact (240), zero rollbacks, storage/arena leak-free. Any failed assertion or
exhausted budget names its §6 failure mode in the `fail` reason.

## Rejected alternatives

- **Hold the slot lock across the migrate** — kills the park path (decision 1).
- **Interior mutability over `TensorStorage`** — large unsafe surface for no
  benefit; possession already makes `get_mut` infallible (Q1).
- **RCU/seqlock or a new lock primitive** — Q4: `SwitchMutex` + the documented
  order suffice at this scale; the lockdep is diagnostics, not a new primitive.
- **Per-CPU lockdep tracking** — false-positives across preemption; per-thread
  keying fixes it at the same cost.
- **A timeout-based park watchdog** — the scheduler has no timed parks; the
  bounded-budget + harness-timeout pair localizes a hang without new
  scheduler surface.

## Consequences

- P4b's daemon has its primitive: claim/skip semantics for the mover loop,
  `guarded_clone` for any surface it shares with accessors, and the §9.12
  stress gate to regress against.
- The `TensorSlot` container is the pattern for daemon-managed tensors; the
  executor's value table stays single-mutator (un-instrumented by design —
  the lemma covers it).
- Every `SwitchMutex` in the kernel can opt into the lock order incrementally;
  only the six AI-residency locks are ordered today.
