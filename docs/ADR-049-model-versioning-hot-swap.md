<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# ADR-049: Model Versioning and Hot-Swap Semantics

**Status:** Proposed (design of record for Phase 8 / production serving). The *primitives* this rests on are built and silicon/stress-proven (content-hash dedup §9.7, the slot arena §9.12/ADR-042, free-on-last-handle O1b, the migration reservation P4a, the residency daemon P4b, I-9, I-13). The *registry + swap + retirement* layer is **not yet built** — it is the named Phase-8 deliverable this ADR specifies.
**Date:** 2026-06-13
**Builds on:** ADR-035 (capability sharing + content-hash dedup), ADR-042 (slot arena, I-13), ADR-043/044 (ownership conversion, dedup-to-slot), ADR-037/039/040 (eviction, reservation, daemon), `docs/complexity-governance.md` I-9/I-13.

## Context
A model in production is updated (a fine-tune, a LoRA merge, a security patch). The question: how does the kernel retire the old version and bring up the new one **without downtime**, while N tenants may be mid-inference on the old weights, and without corrupting a running inference or leaking memory? This must hold under the residency daemon (weights move host↔device concurrently) and under capability isolation (tenants must not see each other's versions).

## Decision

### 1. Versioning is content-addressed — old slots are never "invalidated," they expire
The §9.7 dedup registry keys on the **BLAKE3 content hash** of each weight tensor (`hash → SlotKey`). Therefore a new version is, by definition, **different bytes → different hashes → its own new slots**. There is **no "invalidate the old slot" step**: the old version's slots stay valid for whoever still references them and **free automatically when the last reference drops** (O1b free-on-last-handle). Two versions are two content-hash sets that coexist — exactly like a content-addressed store (git, a CAS). This is the load-bearing simplification: *version safety is a property of content-addressing, not a protocol we maintain.*

### 2. Delta-only migration (the free win)
Because weights are content-addressed **per tensor**, the *unchanged* tensors of vN+1 hash **identically** to vN's → `dedup_probe` **hits** → vN+1 **shares vN's already-resident slots**; only the *changed* tensors allocate new slots and migrate. A 5%-changed model costs ~5% new device memory and migrates ~5% of the bytes. The daemon (P4b) **prefetches that delta before the swap**, so the new version is warm at the instant it goes live — no cold-start stall.

### 3. Hot-swap = immutable handles + atomic active-version pointer + reference-counted drain
Models are **immutable handles** (`Model::weights: Vec<SlotKey>`, O1b) — "updating" never mutates vN. The protocol (the RCU / DRM-fd-table shape used throughout this kernel):
1. Load vN+1 as a new handle (dedup makes it mostly vN's slots + the delta; daemon prefetches the delta).
2. **Atomic flip** of the named model's *active-version* pointer: new inference requests resolve to vN+1.
3. In-flight vN requests keep their `SlotKey` references and **complete on vN's still-resident weights**.
4. **Drain:** when the last in-flight vN request finishes and the vN handle drops, vN's *unique* slots free; the daemon reclaims the device memory. vN and vN+1 coexist only for the drain window — shrunk by dedup to the delta.

### 4. The invariants that make it safe
- **I-9 (pin protection) — the safety floor of the swap.** An in-flight inference *mints* its operand/weight tensors → `strong_count ≥ 2` → **unclaimable** → the daemon can never evict or migrate a weight out from under a running vN inference, *even mid-swap*. A version swap therefore cannot corrupt an in-flight inference: vN's weights stay pinned for exactly as long as a vN request holds them, no longer. This is why drain is safe and downtime is zero — old and new are both fully resident-and-pinned for their respective in-flight requests.
- **I-13 (stale handles fail safe) — no cross-version aliasing.** When a vN slot frees and the arena later reuses that index for a vN+1 weight, a *stale vN `SlotKey`* cannot resolve to the vN+1 occupant: the generation counter mismatches → clean `None`/miss, never a deref of the new version's bytes. The generational arena makes version reuse safe by construction — a late vN request that somehow held a freed key fails closed, it does not silently read vN+1's weights.
- **I-1 (result-invariance)** holds per version: each version's inference is byte-identical to its reference; the swap changes *which version a request routes to*, never *what a given version computes*.

### 5. Retirement: two modes
- **Graceful drain (default, zero-downtime):** §3 above. The old version expires on last-reference; no inference is ever interrupted.
- **Forced revoke (security — a vulnerable vN must be purged now):** the §9.7 capability revoke retires vN's capability immediately so no *new* request can route to it. In-flight vN requests are bounded by I-9 (their weights are pinned and cannot be yanked): the policy choice is **(a) let in-flight complete then free** (brief over-retention of a known-bad version) or **(b) explicitly abort in-flight vN inferences** (immediate purge, the requests fail cleanly). This is a deliberate per-deployment policy knob; the mechanism (revoke + I-9 + drain/abort) supports both. Forced revoke is the only path that does not honor "zero-downtime" — by design.

## User-facing behavior (the contract)
- **Zero-downtime swap:** issuing vN+1 never stalls or drops a request. New requests get vN+1 (warm — the delta was prefetched); in-flight requests finish on vN; the transition is an atomic pointer flip, not a reload.
- **Bounded transient memory:** the overlap window holds both versions' *unique* weights only — typically just the delta beyond the shared base.
- **Cross-tenant correct:** a tenant cannot observe or be affected by another tenant's version (capability isolation; each version is a distinct capability-secured handle); shared *unchanged* weights are one physical copy across versions and tenants (§9.7), counted once (§9.5).

## What is NOT built (the honest scope — the Phase-8 deliverable)
1. **A named, versioned model registry** — `(model-name) → active-version` routing. Today a model is identified by its handle/slots, with no stable name or version pointer.
2. **The atomic active-version flip + drain accounting** — the swap point and the last-vN-reference tracking.
3. **The retirement policy surface** — graceful-vs-forced, and the abort-in-flight path.
These are **substrate-completion on proven primitives, not new mechanism**, and belong to **Phase 8 (production serving)**. This ADR is the design of record they will implement; if any of §4's invariant guarantees turn out insufficient when built, that is a relax-and-re-prove event (Rule 2), not a silent change.

## Consequences
- The hard parts (safe concurrent movement, pin-on-use, generation-safe reuse, cross-tenant one-copy sharing) are *already proven*; versioning composes them rather than introducing new risk.
- Content-addressing gives delta-migration and version coexistence **for free** — a property a blob/whole-model-reload runtime cannot match, and a concrete instance of the governance thesis (the kernel *understands* the objects, so it can do what userspace orchestration cannot).
- The open risk is purely the unbuilt registry/swap layer's correctness under concurrency — to be gated, when built, by the same NEO_SMP stress + byte-oracle discipline as P4a/O1.
