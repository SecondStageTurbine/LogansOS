<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: OT-9 observability slice — scheduler stats + memory pressure via the proven counter pattern → Janus

**Date:** 2026-07-05
**From:** PM (Claude)
**To:** Janus (main-tier coder — fresh off closing the OT-17 memory arc). **Branch:** `codex/ot9-observability-slice-janus` off `origin/main` HEAD.
**Status:** DISPATCHED — the long-deferred **OT-9** (kernel observability, ADR-014 Q1–Q7) gets its first Phase-8 slice. The pattern is already proven and AAG-cleared this week: the IPC-frequency build extended `kernel/src/observability.rs` with read-only `AtomicU64` counters behind `SYS_OBSERVE` (the `KernelObservabilityV1` v1→v2 bump). This slice repeats that exact shape for **scheduler stats + memory pressure**. **One slice (OT-22): counters only — TRACING is explicitly out of scope** (a different, bigger axis; note it, don't build it).

## 0. Ground in ADR-014 first
Read `docs/ADR-014-*.md` (the Q1–Q7 question list) and state in the RETURN which questions this slice answers and which remain open — OT-9's ledger row should close *partially and precisely*, not vaguely.

## ISCs
- **ISC-1 — Scheduler stats.** Per-CPU (or global with per-CPU where cheap): context switches, preemptions (timer-driven vs voluntary yield), idle ticks, run-queue depth high-water. Counters live in `observability.rs` as `AtomicU64`/`Relaxed` `fetch_add` — **the timer-ISR increments must be lock-free single atomics** (I-14(a): nothing in an ISR path takes any lock; state this in the RETURN the way IPC-freq did).
- **ISC-2 — Memory pressure.** Heap high-water mark, current heap use, frame-allocator frames used/free, alloc-failure count (some of these exist in the v2 struct — extend, don't duplicate; `heap_alloc_failures` is already there). Read-side only where the allocator already tracks the number; do NOT add accounting inside allocation hot paths beyond a single relaxed atomic.
- **ISC-3 — The ABI bump, both sides.** `KernelObservabilityV1` v2→v3: new `u64` fields appended or inserted with the SAME byte-discipline as the v1→v2 bump (the two `#[repr(C)]` copies in `kernel/src/observability.rs` and `userland/libneo/src/lib.rs` stay field-for-field identical; `VERSION` bumped in both; the size check holds). This is also live case-study #2 for the OT-44 SDK ABI-tier work — do it cleanly.
- **ISC-4 — A userland consumer (small).** Extend `neo-shell`'s existing `status` builtin to display the new stats via `sys_observe` — the counters earn their keep by being *readable by a person* (Article 1.5: the machine reports its own state legibly). Keep it a formatting change in `neo-shell`, nothing structural.
- **ISC-5 — The QEMU proof.** A scenario (or extension of an existing cheap one) asserting: the struct version reads 3, scheduler counters are nonzero after known activity (spawns/yields force context switches), memory counters are sane (used ≤ total, high-water ≥ current), and — the read-only discipline — two consecutive reads don't perturb governance state. Kill-switch: feature off ⇒ v2 behavior bit-for-bit (the existing scenarios stay green unmodified).
- **ISC-6 — Not an actuator.** Nothing reads these counters to make a kernel decision. Grep-checkable: no branch on any new counter outside the observe/read path. (The OT-15 lesson: attribution/actuation is a future, separately-gated build.)

## Scope guard
No tracing, no per-principal attribution (OT-15's), no scheduler behavior change, no new lock/invariant (Rule 2). **Fences:** do not touch `mm/heap_policy.rs` (Cerebro's 3B lane — read existing allocator stats, don't restructure them), the `arthur_memory_*` scenario family or the `-cpu max` allowlist region of xtask (Jarvis's OT-43 lane), `dhcpv4.rs`/`boot_frame.rs` (Oracle's consolidation, merging). Cargo/xtask table additions are rebase-level.

## Gate + close
- **Kernel-touching ⇒ the 4-shard QEMU `workflow_dispatch` gate is mandatory.** Runner order: Oracle's consolidation gate is on the box now and Jarvis's OT-43 gate comes next — **fire yours third**; per the runbook's shared-box discipline, don't stack.
- **Close-gate `scripts/worker-close-gate.sh`** (rebase-freshness, full host set, SPDX, self-stat; diff the RETURN against the artifact). RETURN: both green CI URLs + diff-stat + the ADR-014 Q-coverage statement + the ISC-6 no-actuator grep + which ledger text OT-9's row should now read.
- **Disk reclaim** after the PM confirms merge.

---
## Relay — what to tell Janus
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-07-05-ot9-observability-slice-janus.md` (+ ADR-014 for Q1–Q7, and the IPC-frequency build as the pattern template — `kernel/src/observability.rs`, the v1→v2 bump). **Branch:** `codex/ot9-observability-slice-janus` off `origin/main` HEAD.
- **One-line:** **Build the first OT-9 observability slice: scheduler stats + memory pressure, exactly the proven IPC-frequency counter pattern.** Read ADR-014 first and state which of Q1–Q7 this answers. Scheduler: context switches, preemptions (timer vs voluntary), idle ticks, run-queue high-water — ISR-side increments are single lock-free relaxed atomics, nothing in an ISR path takes a lock (I-14(a), state it). Memory: heap high-water/current, frames used/free, extending the existing fields without duplicating them; no new accounting in allocation hot paths beyond one relaxed atomic. ABI: `KernelObservabilityV1` v2→v3 with the same both-sides byte-discipline as v1→v2. Small userland consumer: `neo-shell status` displays the new stats. QEMU proof: version==3, counters move under known activity, sane invariants, read-doesn't-perturb; kill-switch off ≡ v2 bit-for-bit. **Not an actuator — no kernel decision reads these; grep-checkable. Tracing and per-principal attribution are OUT (future slices).** Fences: heap_policy.rs (Cerebro), arthur_memory_* + the allowlist xtask region (Jarvis), dhcpv4/boot_frame (Oracle). **Gate order: yours fires THIRD, behind Oracle's consolidation gate and Jarvis's OT-43 gate — the shared-box rule.** Close-gate; RETURN both green URLs + Q-coverage + the no-actuator grep.
