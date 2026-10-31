<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: OT-9 — kernel observability slice (scheduler stats · memory pressure · tracing) → Jarvis

**Date:** 2026-07-03
**From:** PM (Claude)
**To:** Jarvis (main-tier — just landed OT-41 + the 1.5B boot). New branch off `origin/main` HEAD (`8d3e128`): `codex/ot9-observability-jarvis`.
**Status:** DISPATCHED — the **oldest coder-actionable observability debt** (OT-9, ADR-014 Q1–Q7, deferred to "Phase-8+" which is now active). Paying down oldest-first per PL direction. Directly useful: it makes the upcoming bare-metal runs + Arthur debugging far more legible. **Non-conflicting** with WiFi (Cerebro), Arthur-agentic (Janus), and the `mm/slab` internals (Oracle's OT-8) — this is read-only instrumentation of `sched`/`mm`. **Kernel-touching ⇒ 4-shard QEMU gate + AAG.**

## 0. What this build is
A **read-only** observability layer answering ADR-014's Q1–Q7: what is the scheduler doing, is memory under pressure, and a minimal trace to reconstruct "what happened." The discipline: observability must be **cheap and safe** — lock-free counters (atomics) or existing locks only, **never a new lock in a hot path or ISR** (I-5/I-14), and **zero-cost when disabled**. It reports; it does not govern (a QoS/rate-limit *actuator* is OT-15, explicitly NOT this build).

## 1. References
```
git fetch origin
git show origin/main:docs/ADR-014-kernel-observability.md   # THE SPEC — Q1..Q7 (scheduler stats, memory pressure, tracing scope)
git show origin/main:kernel/src/sched/scheduler.rs          # ready-queue / dispatch / context-switch — the counters to surface
git show origin/main:kernel/src/mm/heap.rs                  # heap watermark / alloc-failure signals
git show origin/main:kernel/src/mm/tlb_shootdown.rs         # the existing per-CPU counter pattern (:172-194) to MIRROR (sibling instrumentation)
git show origin/main:docs/OPEN-THREADS.md                   # OT-9 + OT-15 (the QoS actuator this must NOT become)
```

## 2. ISCs
- **ISC-1 — Scheduler stats (ADR-014 Q1–Q3).** Per-CPU: context-switch count, ready-queue depth, run/idle time, and dispatch counters. Mirror the existing per-CPU TLB-counter pattern (`tlb_shootdown.rs:172-194`) — **lock-free atomics**, incremented on the existing path, no new lock in the dispatch hot path.
- **ISC-2 — Memory pressure (ADR-014 Q4–Q5).** Heap high-water mark, allocation-failure count, and the buddy/free-page level. Read the existing allocator state (coordinate with Oracle's OT-8 — **read** the slab/heap counters, don't add your own into the slab internals he's touching; if a counter genuinely belongs in `mm/slab`, flag it so you two don't collide). Surface the residency/reclaim signal if one's exposed.
- **ISC-3 — Minimal tracing (ADR-014 Q6–Q7).** A **bounded** trace/breadcrumb ring (fixed-size, overwrite-oldest, lock-free or a leaf lock) capturing key kernel events (dispatch, fault, syscall class) — enough to reconstruct a short window post-hoc, drainable to serial. Bounded memory, no unbounded growth.
- **ISC-4 — The read surface.** Expose the counters/trace read-only (a stats syscall or a serial/breadcrumb dump — match the project's existing convention; a `sys_inspect`-style read if one fits). **Read-only** — no principal can mutate governance or another principal's state through it, and it adds no CPU-time authority (that's OT-15). Mind I-15/caps if it's a syscall (a read cap, no ambient exposure of another process's data).
- **ISC-5 — Kill-switch + cost.** Observability behind a default-off (or provably zero-cost-when-off) feature/flag ⇒ **disabled ≡ current behavior bit-for-bit** (the counters compile out / fold away; no hot-path cost when off). Ship the equality.

## 3. Standing-rule sections
- **Kill-switch (Rule 1):** ISC-5 — disabled ≡ today, bit-for-bit; the counters are zero-cost when off.
- **Invariant budget (Rule 2):** preserve the ledger; **no new invariant** (read-only instrumentation). **I-5 (lock order):** any lock the trace ring uses is a **leaf** — no new ordered lock, max lockdep level unchanged (re-measure if it moves; coordinate with Oracle so the lock census isn't double-counted). **I-14(a):** no observability counter/lock is acquired on a driver ISR path that could contend a governance lock — atomics only in ISR-reachable code.
- **I-14 / hot-path safety:** the whole point is cheap + safe — a counter that adds a lock to the dispatch path or an ISR is a defect, not a feature.
- **Scope guard:** **read-only observability ONLY.** **Do NOT:** build a QoS/rate-limiter/scheduler-fairness *actuator* (OT-15 — a separate, later, untrusted-principal-gated build); touch WiFi/agentic/forward surfaces; add an invariant; or add a new lock to a hot path/ISR. If Q1–Q7 imply an actuator, note it as an OT-15 line-item, don't build it.

## 4. Gate + close
- **Kernel-touching ⇒ mandatory 4-shard QEMU `NEO_SMP=4` + AAG.** A scenario that reads the stats/trace and asserts they advance (dispatch count > 0, a trace event captured) + the kill-switch equality. Push-CI green. **Green without the CI run URLs is not done.**
- **Close-gate `scripts/worker-close-gate.sh`** (rebase onto `origin/main` — A3/OT-8 may land in parallel; re-run after rebase; host set; SPDX; self-stat).
- **RETURN** with the QEMU shard URLs + SHA, the stats/trace read demo, the kill-switch equality, the lock-census statement (no new ordered lock; leaf-only; I-14(a) atomics-in-ISR), and confirmation it's read-only (no OT-15 actuator). **Disk reclaim** after merge.

---
## Relay — what to tell Jarvis
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-07-03-ot9-kernel-observability-jarvis.md` (+ ADR-014 Q1–Q7, `scheduler.rs`, `mm/heap.rs`, the `tlb_shootdown.rs:172-194` per-CPU-counter pattern to mirror). **Branch:** `codex/ot9-observability-jarvis` off `origin/main` HEAD (`8d3e128`).
- **One-line:** **The oldest observability debt (OT-9).** A **read-only** layer for ADR-014 Q1–Q7: per-CPU scheduler stats (mirror the TLB per-CPU-counter pattern — lock-free atomics) (ISC-1), memory-pressure signals — heap high-water / alloc-failures / free-page level (ISC-2), a bounded overwrite-oldest trace ring drainable to serial (ISC-3), and a read-only stats/trace read surface (ISC-4). **Cheap + safe:** no new lock in a hot path or ISR (atomics only), zero-cost when off, kill-switch disabled ≡ today bit-for-bit (ISC-5). **Read-only ONLY** — it reports, it does not govern; a QoS/rate-limiter actuator is OT-15, NOT this build. **Coordinate with Oracle (OT-8, `mm/slab`)** — read the allocator counters, don't add yours into his slab internals; flag if a counter belongs there. **No new invariant; no WiFi/agentic/forward touch.** **Gate:** 4-shard QEMU + AAG; rebase before close. RETURN shard URLs + SHA + the stats/trace demo + kill-switch equality + the lock-census statement.
