<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# ADR-042: Phase 7 — the Generational Slot Arena (BUILD-O1a: the kernel owns; handles point)

**Status:** Accepted
**Date:** 2026-06-10
**Design:** `Plans/THINK-2026-06-10-phase-7-build-O1-slot-indirected-ownership.md` (ratified)
**Builds on:** ADR-039 (the P4a migration reservation — `TensorSlot`, `guarded_clone`, take-with-retry), ADR-040/-041 (the daemon + its consolidation)
**Amends the ledger:** I-4 relax-and-re-proven; **I-13 added** (`docs/complexity-governance.md`)

## Decision

Replace the residency daemon's append-only `'static` slot registry with a
**generational slot arena**: the kernel becomes the unique strong owner of a
tensor *at rest*, named by a `SlotKey { index: u32, generation: u32 }`. Use
mints a short-lived `Arc` through the proven P4a gate; existence of a name
no longer pins residency. O1a ships the mechanism and its race proof only;
the ownership-surface conversion (`Handle::Tensor`, `Model::weights`) is
BUILD-O1b, gated on this green — the P4a/P4b mechanism-before-consumer split.

This is the industry-converged shape re-derived for NeoPlutarch (THINK §3):
DRM GEM/TTM's two-counter ownership (handle count ≠ pin; eviction allowed
while handles exist; `dma_resv` ↔ our P4a reservation), the fd table's
mint-on-use (`fget`/`inc_not_zero` ↔ our generation-checked `guarded_clone`),
and slotmap generational indices (the ABA defense).

### Prior art and novelty (stated plainly — no overclaim)

The core data structure here — a **generational slot map** (`{index,
generation}` keys, `free` bumps the generation, a mismatched lookup fails
instead of aliasing) — is **textbook, not a discovery.** It is the standard
solution to the ABA / stale-handle problem and is used daily across the
industry:

- **ECS game engines** (Bevy, Unity; the `slotmap` / `generational-arena`
  crates): the canonical use — if entity A dies and B reuses its slot, a
  delayed command for A must not silently hit B. Identical in shape to our
  "a stale `SlotKey` must not resolve to the new occupant" (I-13).
- **High-speed networking** (DPDK-style buffer/index pools): generational or
  versioned indices manage buffers without garbage-collector overhead.

We claim **no novelty in the data structure.** What this build contributes is
a **composition**, and even that is engineering, not science: the generational
slot map is here (1) the *ownership root* for capability-secured kernel objects
(the `SlotKey` lives inside a `Handle`, behind cap rights), and (2) wired to
the P4a **migration reservation** so that `mint` does not merely version-check
but *parks across an in-flight background migration and re-resolves* — the
generation gate and the concurrent-residency gate become the same gate. The
value is in that wiring (and in proving it under the stress matrix), not in the
map. Recorded here so the project's history does not mistake a well-known
pattern, well-applied, for an invention.

## The protocol (one screen)

- **`alloc(tensor, home) → SlotKey`** — install into a recycled-or-grown slot
  (LIFO free list — deliberately adversarial: stale keys collide with hot
  indices); the slot holds the only resting `Arc`; handles = 1.
- **`mint(key) → Option<Arc<Tensor>>`** — the `fget`: generation check under
  the table lock, snapshot the slot reference, **drop the table lock**, then
  `guarded_clone` (parks across an in-flight migration; `None` on a drained
  slot). A racing free-then-realloc drains the *snapshotted* slot — the new
  occupant is unreachable because the stale path never re-derefs the table.
- **`free(key) → bool`** — generation-checked (stale ⇒ counted no-op);
  removes the occupant from the table, **bumps the generation**, recycles the
  index, then drains outside the lock via the P4a take protocol (a free
  racing a migration parks until the commit — never tears). Idempotent
  against stale keys by the bump itself.
- **`retain(key)` / `release(key)`** — the GEM `handle_count` analogue;
  the last release frees. Overflow and double-release are refused, never
  wrapped (`arena_logic::retain_count` / `release_count`, host-proven).
- **Legacy registration** (`residency_daemon::register(&'static ManagedSlot)`)
  is adopted by the arena as a permanent entry — never freed, no key issued,
  outside the alloc/free balance. The daemon walks `arena::live_slots()`
  (index order ≡ registration order for never-freed entries).

`ManagedSlot` moved from `residency_daemon.rs` to `arena.rs` (re-exported,
so scenario code is source-identical). The table lock is ordered at
`lock_order::ARENA` (12, below `TENSOR_SLOT` 15) under the I-5 lockdep and
is never held across a park. Pure decisions (`key_matches`,
`bump_generation`, `alloc_pick`, `recycle_slot`, the handle counts) live in
`arena_logic.rs` — kernel-free, host-tested truth tables. Zero new `unsafe`.

## The ledger changes (Rule 2, paid in-build)

- **I-4 amended (relax-and-re-prove):** from "Arc-in-the-handle" framing to
  **unique-strong-owner-at-rest** — the slot arena holds the only resting
  `Arc`; all minting goes through the generation-checked `guarded_clone`
  gate. The claim predicate (`strong == 1 && weak == 0`) is *unchanged*;
  what changed is its meaning for arena-owned tensors: exactly "no use in
  flight". Re-proven under the O1a stress gate.
- **I-13 added (the one new invariant): stale handles fail safe.** A
  freed-then-reused slot is never reachable through an old key — generation
  mismatch ⇒ clean `None`/no-op, never a deref of the new occupant. Guard
  shipped in the same build: the `gen_mismatches` counter (gate demands > 0
  generated and **0 stale resolves ever**), the host reuse truth tables, and
  the keyed byte-tag oracle in the race gate.

**Generation wraparound (documented bound):** the bump is `wrapping_add(1)`
on a `u32`. A stale key aliases only if the *same slot index* is freed
exactly 2³² times while that key is still held. At a free/realloc per
microsecond — far beyond any real churn — that is ~71 minutes of *continuous
single-slot recycling* with a key pinned the whole time; any other index or
count fails the match. Accepted as benign for O1a/O1b; if a future surface
hands keys to untrusted holders for unbounded lifetimes, widen to u64 then.

## Kill-switch (Rule 1)

`arena::set_reuse_enabled(false)` ⇒ `free` drains and bumps but never
recycles an index — the arena degenerates to the append-only registry it
replaced. Proof at the pure level (recycle-disabled ⇒ free list stays empty
⇒ alloc always grows ⇒ indices monotonic = registration order, host-tested)
and at the behavioral level: the full P4b stress family runs unchanged (its
pool registers once and never frees — the kill-switch shape by
construction).

## The race proof (gate `accel_slot_arena`, N=60 ≥ 50 at NEO_SMP=4)

Three roles per the dispatch: a **minter** (continuous keyed mints,
byte-checked against the tag captured *with* the key — a stale key resolving
to a slot's new occupant surfaces as a byte mismatch), a **migrator** (the
daemon role: keyed claim-or-skip ping-pong device↔CPU), and the **churner**
(free + realloc the whole pool every iteration, then replay every retired
key as a reuse attack). Each iteration also runs the P4a held-claim probe so
an accessor park is observed structurally, not statistically.

Discriminators, all armed in-build: `gen_mismatches > 0` with 0 stale
resolves and 0 stale frees (I-13); minted bytes tag-exact (no use-after-free
— a mint returns the live pre-free tensor or `None`, never a draining one);
slot balance `allocs == frees` at teardown (the leak guard); the standing
P4a matrix (parks > 0, quiesce == 0, delta violations == 0, storage/arena
balance) demonstrably firing.

**A standing probe re-validated (found by this gate's run 2):** the P4a
per-migration delta probe brackets the copy window against *device totals*
— valid only when the device is otherwise accounting-quiescent, which the
P4a/P4b gates guaranteed structurally and the arena's free-while-migrating
deliberately breaks (the churner freeing *another* slot's device-resident
tensor mid-copy confounded the equality; I-6 itself held throughout). The
probe is now scoped mechanically instead of weakened: an **accounting
mutation epoch** (`accounting::mutation_epoch`, bumped by every
`resident_add`/`resident_sub`) brackets the window; strict equality is
enforced when only the migration's own mutations landed
(`deltas_enforced`), confounded windows are counted (`deltas_unmeasured`),
and the gate demands both `delta_violations == 0` and
`deltas_enforced > 0` — the probe stays armed at full pre-O1a strength in
every quiescent window (all legacy gates) and never false-positives under
designed concurrency. The lesson generalizes: new concurrency must
re-validate every standing discriminator's *measurement assumptions*, not
just the invariants.

## Scope honored

No `Handle`/`Model` changes (O1b). No dedup re-keying (§9.7 keeps its
`Weak`; deduped tensors stay unclaimable exactly as today — named
follow-on). No syscall/capability semantics change. No new lock primitive,
no reservation-protocol change (the arena *consumes* `guarded_clone` /
`take` / `try_migrate` / the probe seams), no new `unsafe`.

## Consequences

- O1b can convert the production ownership surfaces onto `SlotKey` and put
  model weights — the largest memory object in any AI workload — under the
  proven policy layer for the first time: idle weights demotable, mid-use
  weights pinned by the same mechanism that pins pass intermediates.
- The daemon registry generalizes from gate-pool plumbing to the kernel's
  tensor-ownership surface with one walk API (`live_slots`).
- The ledger grows by exactly one row (I-13), with its guard standing.
