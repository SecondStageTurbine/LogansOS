<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# ADR-044: Phase 7 — BUILD-O2, the Dedup-to-Slot Re-key (the production load path comes under governance)

**Status:** Accepted
**Date:** 2026-06-11
**Closes:** ADR-043 §finding (dedup candidacy Weak-pinned every production-loaded weight)
**Builds on:** ADR-042 (the arena), ADR-035 (BUILD-3 sharing/dedup), ADR-043 (the ownership conversion)
**ANSPEC:** §9.7 amended; §9.13 pointer note

## Decision

The §9.7 dedup registry is re-keyed from `hash → Weak<Tensor>` to
`hash → SlotKey`. The arena's generation check (I-13) replaces
`Weak::upgrade` as the liveness probe: a freed slot is a clean miss
(removed lazily, with a same-key re-check so a re-published candidate under
the same hash is never clobbered); a live slot is a hit. **Candidacy pins
nothing** — the map entry is just a name.

- **Probe** (`dedup_probe`): copy the key out under the map lock, `mint`
  OUTSIDE it (mint can park across an in-flight migration — the recorded
  park-under-lock class), return `(SlotKey, Arc)` on a hit.
- **Hit** (`share_or_allocate_weight`): `arena::retain(key)` and the model
  stores the *same* `SlotKey` — N models, ONE slot, one resting `Arc`, one
  physical copy. (This also retires the O1b two-slots-per-deduped-weight
  nuance.) A slot that dies between the probe's mint and the retain falls
  through to fresh allocation.
- **Miss:** allocate, `register` the §9.5 count-once accounting (no Weak),
  `alloc` into the arena, publish `hash → key`.
- **Teardown:** the existing `on_tensor_drop` hook removes the registry +
  dedup entries when the physical copy dies (last model release → slot free
  → drain → `Arc` drop) — idempotent with the lazy removal. An unconditional
  hash removal can drop a *newer* same-hash candidate whose predecessor's
  `Arc` lingered; benign (a future load allocs fresh and re-publishes).

## The consequence (the point)

After a default-path (`dedup_owner = Some`) `sys_model_load`, an idle
weight's slot is `strong == 1 && weak == 0` — **claimable, demotable,
daemon-managed**. The I-4 carve-out ("deduped tensors stay unclaimable") is
retired from the ledger and the module docs: this is a strengthening — the
predicate is unchanged; one class of permanent `Weak` holder ceased to
exist. The `weak == 0` screen remains as defense against any future
Weak-holder.

**The §9.7 one-copy invariant is strengthened, not weakened:** N sharers →
1 `SlotKey` → 1 slot → 1 resting `Arc` → 1 physical copy — and the copy can
now *move* (demote/refetch) without breaking the share, because every
sharer resolves through the same slot. "Share-survives-movement" is the new
correctness surface, gated by `accel_dedup_movement`.

## The gates

- **`accel_dedup_movement` (share-survives-movement):** A loads (dedup
  path); every idle weight slot proves claimable (the claim admits); the
  weights are moved device↔CPU through the claim path (the daemon's own
  try_migrate — pressure-driven demotion is the payoff gate's coverage); B
  loads the same bytes against *demoted* slots — the probe HITS (the two
  models' weight-key lists are identical; zero new tensor allocations);
  both principals' inferences byte-identical; §9.5 counts the copy once
  (owner = A) throughout; last drop frees and deregisters. Race leg N≥50 at
  NEO_SMP=4: a prober thread hammers `dedup_probe` while the mover
  ping-pongs the slot — every probe hits (the share never breaks under
  movement), bytes tag-exact (no torn hit), zero duplicate allocations, and
  parks observed (the probe's mint parking on a mid-flight migration — the
  P4a discriminator, plus a deterministic held-claim park phase).
- **`accel_model_residency` re-run on the default path:** the O1b payoff
  scenario now loads `Some(owner)` — prefetch/pin/demote/refetch
  byte-identical on the path production actually uses. ADR-043's finding:
  closed.

## Audited carries (Forge, both MINOR — document-and-carry)

- **The retain-refusal transient:** a hit whose slot dies between the
  probe's mint and the retain falls through to fresh registration while the
  old copy's registry entry (and its count-once `resident_add`) persists
  until the held `Arc` drops — a transient CPU-share double-count that
  self-heals deterministically via `on_tensor_drop`. ACCEPTED: closing it
  means holding a lock across mint→retain — reintroducing the
  park-under-lock hazard this design exists to avoid, strictly worse than
  the benign transient.
- **The register→publish window:** two concurrent identical first-loads can
  both miss and allocate two valid copies (the second publish wins the
  candidacy) — the same outcome class as the pre-O2 Weak probe-then-register
  race, with a wider window (publish is deferred past the arena alloc).
  Never wrong bytes, never broken accounting (count-once per copy, each
  reclaimed by its own drop). Flagged for a future atomic register-publish
  tightening if concurrent same-model loads become a real workload.

## Scope honored

Arena protocol untouched (consumes mint/retain/release + the existing probe
seams). Sharing *semantics* untouched: rights, grant, revoke, the one-copy
trace, count-once accounting — only the registry's key type changed. The
non-dedup (`None`) path is diff-untouched. No new locks (the `DEDUP`
SwitchMutex persists; never held across a park), zero new `unsafe`, no
policy changes.

## Consequences

- O2 completes the Phase 7 spine: every governance line and every policy
  decision reaches the production-default load path.
- The serving surface inherits a fully-governed load path.
- The ledger sheds a carve-out instead of gaining a row.
