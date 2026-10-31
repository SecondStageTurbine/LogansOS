<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# ADR-037: Phase 7 — Eviction Under Device-Memory Pressure (BUILD-P2)

**Status:** Accepted
**Date:** 2026-06-10
**Phase:** 7 (BUILD-P2)
**Builds on:** ADR-036 (residency-aware placement, §9.9), ADR-033 (§9.5 accounting), ADR-035 (§9.7 sharing)
**Opens:** ANSPEC-001 §9.10

## Context

BUILD-P1 made the kernel *decide* where an op runs, and keeps a chain's
intermediates resident on the chosen accelerator. That residency has a cost P1
ignored: a bounded device memory (accel_sim's 256 KiB arena) fills, and the next
placed allocation simply fails the forward pass. The kernel needs the classic
scarce-resource decision — which resident tensor to evict to make room — plus a
graceful floor when no eviction can help.

## Decision

Reactive LRU eviction-to-CPU, wired into the placed dispatch path, with these
specific commitments:

1. **Recency extends §9.5; it does not duplicate it.** A pure, host-tested
   `RecencyTable` (`tensor_id → (device_abi, tick)`, monotonic table tick) lives
   with the accounting module. Touched on device allocation, migrate-onto-device,
   and per-op operand/output use; forgotten on migrate-off and tensor drop. Keyed
   by `tensor_id` alone because a tensor is resident on exactly one device —
   forget is O(log n) and the table is bounded by the live device-resident set.

2. **The policy is pure and host-tested** (`ai/evict_policy.rs`, path-mod'd into
   `tools/ai-ops-tests` like the placement policy):
   `select_victims(capacity, resident, need, candidates) → (victims, reason)` —
   LRU-first among evictable candidates, ascending tick, lowest-`tensor_id`
   tiebreak.

3. **No partial eviction.** Sufficiency is precomputed: the policy returns a
   victim set that makes room, or `NoRoomCpuFallback` with zero victims. Evicting
   without ending up with room is thrash with no benefit; the dispatch's "do not
   loop" is honored by construction.

4. **Evictability = uniquely-held.** A tensor is an eviction candidate iff the
   executor's value table holds the only `Arc` to it and it is not an operand of
   the op being placed (the explicit pin set). This makes pin protection
   *mechanical*: operands are protected by their live references in the executing
   node; weights, caller inputs, and §9.7-shared/deduped tensors are protected by
   their external holders (evicting them could not free storage anyway). The
   mutation gate is `Arc::get_mut`, which also refuses when any `Weak` exists
   (the dedup registry holds `Weak<Tensor>`); a refused victim is skipped and the
   post-eviction room re-check decides the outcome. No new `unsafe`, no
   interior-mutability rework.

5. **`need` includes arena staging.** accel_sim's dispatch transiently stages
   all inputs and the output in the same arena that holds resident tensors, so
   for `AccelMemoryKind::DeviceArena` devices the room check covers
   `output + (Σ operand bytes + output)`. Ops with scratch beyond this model are
   out of P2 scope and rely on the backstop (next point).

6. **OOM backstop = totality.** If a placed device allocation or dispatch still
   reports out-of-memory after a passing room check (SMP-concurrent allocator,
   alignment padding, unmodeled scratch), the op reroutes to accel_cpu instead of
   failing — byte-identical output, never a failed pass. The legacy fixed-driver
   path is untouched (errors propagate exactly as before BUILD-P2).

7. **Capacity is a driver property.** `AccelDriver::capacity_bytes()` defaults to
   `None` (unbounded → eviction never engages → §9.9 behavior bit-for-bit);
   accel_sim returns its arena size. This is the strict-superset guarantee:
   accel_cpu, virtio_accel, and accel_intel declare no capacity in P2.

8. **`R ≤ C` is provable post-hoc.** §9.5 gains per-device current/peak residency
   totals (pure table extension, host-tested, in lockstep with the per-principal
   entries). The gate asserts the high-water mark never exceeded capacity —
   "always," not "at the end."

## The decision trace

```
eviction: tensor=<id> device=Accel(<n>) → cpu reason=lru (resident=<R>/<C>, need=<N>)
eviction: device=Accel(<n>) reason=no-room-cpu-fallback (resident=<R>/<C>, need=<N>, evictable=<E>)
```

`resident` in the `lru` line is the residency immediately before that victim was
evicted, so a multi-victim sequence shows the pressure draining.

## The gate (QEMU, deterministic)

New scenario `accel_evict_pressure` + neomc fixture `valid_pressure_chain.neom`
(weight-free `input → Gelu → Relu → Gelu → Relu`, 1×12288 F32 = 48 KiB/tensor).
The sizing is pinned by two constraints: §9.6 `DISPATCH_QUOTA = 4` bounds the
pass to 4 accelerator dispatches, and the 256 KiB arena must force eviction
within them. At 48 KiB: ops 1–2 fit; op 3's need (48 KiB output + 96 KiB
staging) exceeds free space → evict out1 (LRU); op 4 → evict out2. Asserts:
exactly 2 evictions / 98 304 evicted bytes / 0 fallbacks, peak residency ≤
capacity, the externally-held input never evicted (still device-resident),
output byte-identical to the accel_cpu reference, storage/arena leak-free.
The no-room fallback and pinned-protection corners are host-tested in the pure
policy (5 named cases + tiebreaks).

## Rejected alternatives

- **Evict-then-discover (partial eviction):** pays migration cost that can fail
  to buy room; violates "do not loop." Precomputed sufficiency is strictly better.
- **Interior-mutability rework of `Tensor` storage** (lock around storage to
  migrate shared tensors): large unsafety/borrow surface for zero P2 benefit —
  shared tensors cannot yield free storage while another holder lives.
- **Eviction to another accelerator:** scope-guarded to P3+; CPU is the only
  victim target.
- **Counting staging via a driver-reported overhead API:** over-general for one
  arena-backed reference driver; the `AccelMemoryKind::DeviceArena` keyed model
  is honest and minimal, with the backstop covering the gap.

## Consequences

- The placed path gains one capacity consult per op on capacity-declaring
  devices; devices without capacity see a single `None` check (no behavior
  change, no measurable cost).
- Recency touches add two BTreeMap operations per accelerator-touching op —
  bounded, lock-disciplined identically to the §9.5 table.
- BUILD-P3 (proactive, cost-aware migration — the `residency:` trace) now has
  its inputs in place: recency, capacity, per-device residency, and a proven
  migration seam.
