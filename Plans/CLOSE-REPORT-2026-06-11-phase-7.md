<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# CLOSE-REPORT: Phase 7 — kernel governance of accelerator memory

**Date:** 2026-06-11
**From:** PM/Cerebro
**Status:** Phase 7 **CLOSED** at `db88652` (BUILD-O2). Opened 2026-06-07 (`Plans/THINK-2026-06-07-phase-7-inference-scheduler-governance.md`).
**Successor:** the pre-mandated **Phase 7.5 connectivity lane** (PL decisions 2026-05-30 USB re-deferral + 2026-06-10 I-14 mandate). Opening artifact: the 7.5 THINK, with the **DMA-isolation decision (VT-d/IOMMU vs fenced DMA arena)** as task one.

---

## 1. The thesis, and the verdict

Phase 7 set out to prove the project's core claim — that a kernel which *understands* AI objects can **govern** them between mutually-untrusting tenants in ways userspace cannot: the necessity test, governance at a hardware boundary userspace can't police, via capability authority fused with kernel-owned device memory.

**The thesis is proven, end to end, on the production-default path.** The kernel measures, arbitrates, owns, and isolates AI work (the four governance lines — the isolation line enforced by the GPU's own page tables on real silicon); it places, reclaims, anticipates, and concurrently moves tensor residency (the policy layer); and as of the O-arc, all of it reaches the tensors that matter — **model weights, loaded the way production loads them** — not just gate scratch. Every layer is result-invariant (byte-oracle), provably switch-off-able (kill-switch), stress-proven at NEO_SMP=4, with zero new `unsafe` across the entire phase.

## 2. The build ledger (16 builds, in three arcs + one digest)

**Arc 1 — the governance mechanisms (BUILD-1 → 4X-3):** accounting (§9.5, ADR-033) → admission scheduler, live-path blocking (§9.6, ADR-034) → capability sharing + content-hash dedup (§9.7, ADR-035) → per-context PPGTT/execlist spatial isolation on silicon (§9.8; the 4X arc, after the legacy-ring PPGTT falsification — a cross-tenant GPU read **page-faults in hardware**). Includes the deep-research execlist/LRC bring-up that collapsed a likely 8-iteration blind slog to 2.

**Arc 2 — the policy layer (P1 → P4b):** residency-aware placement (§9.9, ADR-036) → LRU eviction under pressure (§9.10, ADR-037) → proactive retention + headroom banking (§9.11, ADR-038) → the migration-reservation primitive (§9.12, ADR-039; the single-mutator invariant relaxed-and-re-proven) → the async residency daemon (ADR-040; the same pressure pass that pays ≥1 critical-path reclaim paused pays **zero** awake, 60/60).

**Digest — BUILD-C1 (ADR-041):** the first Rule-4 consolidation; −295 production LOC, the ledger's first row-by-row audit (one drift found and fixed), the decaying-H debt paid, ISC-4 honestly BLOCKERed back — which became the O-arc.

**Arc 3 — the ownership arc (O1a → O2):** the generational slot arena (ADR-042; I-13 added, I-4 relaxed-and-re-proven to unique-owner-at-rest; 480 stale keys generated, 0 resolved) → the Handle/Model conversion (§9.13, ADR-043; idle weights demoted + refetched byte-identical while in-flight weights never move) → the dedup-to-slot re-key (ADR-044; the production-default load path's last pin removed; the shared copy can now *move* without breaking the share).

## 3. What is honestly true at close (and what is not claimed)

**True and proven:** all of §1, under QEMU NEO_SMP=4 stress gates and (for the isolation line and the Phase-6 substrate) bare metal on the Dell OptiPlex 7080's UHD 630. The complexity-governance ledger holds **14 invariants on one page**, each with a named, running guard; every policy layer collapses bit-for-bit to its predecessor.

**Not claimed:** (a) **scale** — proven on one node, one consumer GPU, small models; *designed* for 70B/datacenter density, not demonstrated there. (b) **cost** — per the §4 performance-falsifiability commitment, all performance claims are **"designed-for, not measured"** until the latency oracle reports (a tracked obligation, scheduled after 7.5). (c) **breadth** — one accelerator vendor path on silicon; the `AccelDriver` class is device-agnostic by construction but multi-vendor is future work.

**Carried out of Phase 7 (named, tracked):**
1. **The latency oracle + performance budget** (governance §4) — after 7.5.
2. **Device-side copy-engine DMA overlap** — hardware lift, deferred per P4-THINK Q3.
3. **The production inference-serving surface** — unblocked by the O-arc (it now inherits a fully-governed load path); Phase 8 territory.
4. **Phase-6 SIMD32 backport** + post-Phase-6 hardening (stateful-BTI/DRM) — tracked, not blocking.
5. **ADR-044's two Forge MINOR carries** + O1a's `live_slots()` scaling watch (partially addressed in O1b) — small, recorded in their ADRs.
6. **I-14(c) DMA isolation** — mandated, guard ships with the first driver-DMA build (the 7.5 opening decision).

## 4. Process verdict (what the phase taught, now standing discipline)

The phase converted four one-off lessons into permanent mechanism: the **blind-hardware discriminator rule** (born of the iter7 pushback; made every subsequent blind run conclusive), the **complexity-governance regime** (kill-switch / invariant budget / consolidation cadence — Rule 4's first cycle completed on schedule), the **concurrency analogue** of the discriminator rule (P4a: five gate failures, five one-run self-diagnoses), and **performance falsifiability** (§4 — the one axis not yet instrumented, now a named obligation instead of an assumption). Honesty corrections were made *in the record, not just in chat*: the lock-census re-count (31, not ~13), the QEMU-runnable family framing, the ADR-042 novelty disclaimer (a textbook pattern well-applied is not an invention), and ADR-043's dedup-pin finding carried verbatim until O2 closed it.

## 5. Close

Phase 7 closes 16-for-16 with every gate green, the invariant ledger audited and one page long, and the governance thesis demonstrated where it counts: **the kernel governs intelligence as a resource, between untrusting tenants, down to the silicon, on the path production actually uses.** The next lane is already constrained before it begins: Phase 7.5 opens under I-14, with the DMA-isolation architecture as its first decision — the integrity of the boundary preceding the features that need it.
