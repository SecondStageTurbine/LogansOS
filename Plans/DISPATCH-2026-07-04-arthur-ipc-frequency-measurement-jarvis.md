<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Arthur agentic-path IPC-frequency measurement (HarmonyOS lesson → OT-15) → Jarvis

**Date:** 2026-07-04
**From:** PM (Claude)
**To:** Jarvis (main-tier — just built OT-9 observability + the CI de-flake). New branch off `origin/main` HEAD (`78c5455`): `codex/arthur-ipc-frequency-jarvis`.
**Status:** DISPATCHED — a **measurement/instrumentation pass** that turns the top HarmonyOS lesson into data, and it **leverages the OT-9 observability Jarvis just landed**. The HongMeng finding (`THINK-2026-07-03-harmonyos-hongmeng-lessons.md` §1, verified): microkernel overhead is dominated by **IPC *frequency*** + state double-bookkeeping, not per-IPC latency. So: **how often does Arthur's NL→tool-call→cap-check→action path cross a protection boundary per action?** Measure it before anyone optimizes it. **Non-conflicting** with Oracle (forward/SMP) and Janus (memory) — this is instrumentation of the existing agentic path + a report. **Kernel-touching (counters) ⇒ 4-shard QEMU + AAG** (small surface).

## 0. What this build is
Instrument the agentic action path to **count boundary crossings (syscalls / IPC / cap-checks) per natural-language action**, using the OT-9 observability counters + a bounded trace. Deliverable = the instrumentation (read-only, kill-switched) + a short **findings note** (`docs/`) reporting: crossings-per-action for the A1/A2/A3 tiers, where the hot spots are, and whether any governance state is double-bookkept between kernel and userland (the HM antipattern). This is the data that feeds **OT-15** (per-principal CPU-time QoS accounting) and tells us whether Arthur's path warrants call-coalescing.

## 1. References
```
git fetch origin
git show origin/main:docs/THINK-2026-07-03-harmonyos-hongmeng-lessons.md   # §1 the cost-reframing lesson this measures
git show origin/main:kernel/src/observability.rs                            # the OT-9 read-only counters + trace ring (2596b36) to reuse
git show origin/main:docs/OPEN-THREADS.md                                   # OT-15 (the QoS lane this feeds) + OT-9
git show origin/main:userland/neo-arthur/src/                               # the agentic path: conversation_logic + the execute_agentic_* handlers (A1/A2/A3)
git show origin/main:userland/libneo/src/action.rs                          # the execute_action cap-check the path crosses
```

## 2. ISCs
- **ISC-1 — Boundary-crossing counters on the agentic path.** Extend the OT-9 observability (lock-free atomics, read-only) to count, per agentic action: syscalls issued, IPC/cap-check crossings, and the tool-call→execute round-trips. Attribute to the A1/A2/A3 tier. **Read-only, atomics-only** (no new lock in the hot path — the OT-9 discipline), zero-cost when off.
- **ISC-2 — A scenario that exercises + reports.** A neo-test scenario (or extend `arthur_agentic_actions`) that runs representative actions in each tier and **emits the crossing counts** to serial (a breadcrumb line per action) so the numbers are captured deterministically. No new assertion on the *count value* (it's a measurement, not a threshold) — just that the counters advance and are read.
- **ISC-3 — The double-bookkeeping scan (the second HM lesson).** A quick audit: is any governance/agentic state mirrored between a kernel structure and a userland copy on the agentic path? List any found (a candidate for single-owner state). Analysis, not a fix.
- **ISC-4 — The findings note.** `docs/NOTE-2026-07-04-arthur-ipc-frequency.md`: the measured crossings-per-action per tier, the hot spots, the double-bookkeeping findings, and a recommendation (coalesce / leave / a specific OT-15 follow-on). Honest numbers (TCG-virtualized — report crossings *counts*, which are exact, not wall-clock).

## 3. Standing-rule sections
- **Kill-switch / invariant:** the counters are behind the OT-9 observability feature (default-off / zero-cost when off) ⇒ **disabled ≡ today bit-for-bit**; **no new invariant** (read-only instrumentation). No new lock, no ISR-path governance-lock (I-14(a) — atomics only).
- **Read-only:** the counters report; they do not gate or govern (no OT-15 *actuator* — that's the separate, later build; this is the *measurement* that informs it).
- **Scope guard:** instrumentation + scenario + the findings note ONLY. **Do NOT:** build a QoS/rate-limiter actuator (OT-15 proper — later); change the agentic-action behavior/grammar (Janus/Oracle surfaces stay untouched — you only *count*, never alter the path's semantics); touch the forward/memory/WiFi; add an invariant or a hot-path lock.

## 4. Gate + close
- **Kernel-touching (counters on the agentic path) ⇒ 4-shard QEMU `NEO_SMP=4` + AAG.** The AAG centers on: read-only + no hot-path/ISR lock (I-14(a)) + kill-switch ≡ today + no behavior change to the agentic path (you measure, not modify). Push-CI green. **Green without the CI run URLs is not done.**
- **Close-gate `scripts/worker-close-gate.sh`** (rebase; re-run; host set; SPDX; self-stat).
- **RETURN** the QEMU shard URLs + SHA, the findings note path + the headline numbers (crossings/action per tier), the double-bookkeeping list, the kill-switch/read-only statement, and the recommended OT-15 follow-on. **Disk reclaim** after merge.

---
## Relay — what to tell Jarvis
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-07-04-arthur-ipc-frequency-measurement-jarvis.md` (+ the HarmonyOS lessons THINK §1, your `observability.rs`, OT-15, the agentic path). **Branch:** `codex/arthur-ipc-frequency-jarvis` off `origin/main` HEAD (`78c5455`).
- **One-line:** **Measure how often Arthur's NL→tool-call→cap-check→action path crosses a protection boundary per action** — turning the top HongMeng lesson (overhead = IPC *frequency*, not latency) into data, using the OT-9 observability you just built. Extend the read-only atomic counters to tally crossings/syscalls/cap-checks per agentic action by A1/A2/A3 tier (ISC-1), a scenario that runs actions + emits the counts to serial (ISC-2), a quick kernel↔userland double-bookkeeping scan (ISC-3), and a `docs/` findings note with the numbers + an OT-15 recommendation (ISC-4). **Read-only, atomics-only, zero-cost when off — you COUNT, never alter the path.** **Not the OT-15 actuator (later); no agentic-behavior/forward/memory/WiFi change; no hot-path lock.** 4-shard QEMU + AAG. RETURN shard URLs + SHA + the findings note + the crossings/action numbers + the double-bookkeeping list + kill-switch/read-only.
