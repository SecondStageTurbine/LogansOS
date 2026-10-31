<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# ADR-043: Phase 7 — BUILD-O1b, the Ownership Conversion (model weights under the policy layer)

**Status:** Accepted
**Date:** 2026-06-11
**Design:** `Plans/THINK-2026-06-10-phase-7-build-O1-slot-indirected-ownership.md` (ratified); ADR-042 (the O1a arena this consumes)
**ANSPEC:** §9.13 (new — the ownership model)

## Decision

`Handle::Tensor` stops holding `Arc<Tensor>` and becomes `{ slot: SlotKey,
meta: TensorMeta, cap_id, rights }`; `Model::weights` becomes
`Vec<SlotKey>` (+ cached weight ids for the §9.7 reader scans). Every
byte-touching use mints through `arena::mint` and drops when the use
completes; metadata paths answer from cached `meta` and never mint.

**Ownership rides table membership, not value copies.** `Handle` values
are freely copied through syscall paths (the pre-O1b idiom); a copy now
carries no ownership at all — the `HandleTable` is the chokepoint
(`install` retains; `remove`/`remove_cap_ids`/`drain` release; the table's
entries are private, so the chokepoints are exhaustive by construction,
not by audit). Creators (`sys_tensor_create`, `sys_inf_run` outputs)
transfer their alloc count to the table by releasing immediately after
install — uniform, no special cases, leak-free on the error path. Models
hold their weight slots' creator counts and release them on drop
(free-on-last-Model-reference). This is the GEM `handle_count` model: the
count counts *handles*, kernel transients re-validate through the
generation check (I-13).

## Semantics preserved (the ISC-4 kill-switch)

- With the daemon off and no pressure, every syscall-observable result is
  bit-identical: proven by the full prior family (sharing/grant/revoke/
  query/infer scenarios unchanged).
- The `sys_tensor_query` logical refcount is now `arena::handle_count` —
  which equals live table entries, exactly the pre-O1b `Arc`-derived
  figure at every quiescent point.
- A grant copies the `SlotKey` (N handles → 1 slot → 1 physical copy —
  §9.7 intact); the grant-completion share registration mints for the
  registration's duration (the grantor's entry pins the slot live).
- **One intentional race tightening (documented, not drift):** a syscall
  racing a revoke previously completed on its already-cloned `Arc`; a
  stale handle copy now fails the generation check (`EBADF`) if the revoke
  fully freed the slot first — the revoke wins earlier. Strictly safer;
  unobservable in the quiescent equivalence family.

## I-9 unification

While minted, `strong ≥ 2` ⇒ unclaimable: production tensors are pinned by
the *same mechanism* as pass intermediates. The executor mints weights
into its value table at pass start and drops them with it — the pass IS
the use. The payoff gate's pin probe demonstrates both directions
deterministically: the daemon's rollback of a withdrawn-prefetch weight is
refused while the scenario holds a mint (`claims_referenced` advances, the
weight never moves) and proceeds the moment the mint drops.

## The finding: dedup candidacy pins production-loaded weights

`sys_model_load`'s dedup path (`share_or_allocate_weight`) Weak-registers
**every fresh immutable weight** as a dedup candidate — not just actual
dedup hits — and `weak > 0` is unclaimable by I-4 (an `Arc::get_mut`
fact, not a policy choice). Consequence: weights loaded through the
production syscall remain unclaimable until the **§9.7 dedup-to-slot
re-key** (explicitly out of this build's scope; removing candidate Weaks
instead would break BUILD-3's dedup family). The payoff gate therefore
loads through the existing non-dedup path (`from_neom_bytes(None)` — the
documented behavior-must-not-change load), which exercises the full
mechanism end-to-end. **The dedup re-key is hereby promoted from optional
polish to the gate on the full production payoff** — returned to the PM as
the next named build on this line.

## The `live_slots` fix (the O1a carried item, within bound)

The daemon walk now snapshots pure `SlotKey`s under the table lock
(`live_keys`) and resolves each candidate lazily outside it (`slot_ref`)
— zero `Arc` bumps under the lock; a slot freed between snapshot and
resolve misses cleanly (a legitimate I-13 generation-mismatch event).
Reference discipline only; the daemon's logic is untouched.

## The payoff gate (`accel_model_residency`, N=50 at NEO_SMP=4)

Per iteration, all deterministic (the arithmetic is walked against
accel_sim's 256 KiB arena with H = 147456 from the pass's own notes):
prefetch admits exactly 2 of 4 published weights (thrash guard refuses the
rest — counted); warm inference byte-identical with weights pass-pinned;
the I-9 pin probe (refused-then-rolled-back, same weight); tenant-B arena
pressure → the daemon's coldest victim is A's idle W2 (**an idle model
weight bg-demoted under another tenant's pressure**); residency ≤ capacity
throughout; the cold re-run byte-identical; slot/storage balance exact at
teardown.

## Scope honored

Arena protocol consumed unchanged (mint/free/retain/release/debug seams +
two additive read-only accessors: `handle_count`, the keyed
publish/clear/mark tenant signals). Dedup registry untouched. Capability
rights, grant/revoke model, intended syscall semantics unchanged. No new
locks, zero new `unsafe`, no daemon redesign.
