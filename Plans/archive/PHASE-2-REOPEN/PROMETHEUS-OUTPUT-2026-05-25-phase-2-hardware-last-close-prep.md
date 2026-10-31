# PROMETHEUS-OUTPUT: Phase 2 hardware-last close-prep map

- **Author:** Prometheus (Qwen3.6-27B — documenting, amending, document cleanup lane)
- **Date:** 2026-05-25
- **Binding constraint:** `Plans/PM-NOTE-2026-05-25-phase-2-hardware-last-execution-barrier.md` (hardware-last execution barrier)
- **PLAN source:** `Plans/PROMETHEUS-OUTPUT-2026-05-25-phase-2-plan-substance.md` (`e6a4cdf`)
- **PM acceptance:** `Plans/PM-DISPOSITION-2026-05-25-phase-2-plan-substance-close.md` (`b7281d0`)
- **PL ratification:** `Plans/PL-RATIFICATION-2026-05-25-phase-2-plan.md` (`a7bcacf`)
- **Repo state:** `origin/main` at `b3bb488`

---

## Purpose

This artifact maps every remaining Phase 2 work item up to the final hardware session, organized so PM can verify the pre-hardware barrier is satisfied before routing the OptiPlex session. It does not claim any active BUILD work has landed. It is a planning and tracking document.

---

## Phase 2 Execution State (as of this artifact)

| Subphase | Dispatch | Close report | Status |
|---|---|---|---|
| **1. Capability reserved-bit activation** | `Plans/DISPATCH-2026-05-25-phase-2-build-subphase-1-capability-reserved-bit-activation-jarvis.md` | — (placeholder below) | Dispatched to Jarvis/Codex; close report pending |
| **2. ADR-016 A2 scheduler heap-decoupling** | — (not yet filed) | — (placeholder below) | PLAN-ratified; dispatch not yet filed |
| **3. ADR-019 B/C type-level IRQ safety** | — (not yet filed) | — (placeholder below) | PLAN-ratified; dispatch not yet filed |
| **4. Formal close** | — (not yet filed) | — (placeholder below) | PLAN-ratified; dispatch not yet filed |

---

## Pre-Hardware Completion Checklist

Per `PM-NOTE-2026-05-25-phase-2-hardware-last-execution-barrier.md` §2. Every row must be satisfied before the OptiPlex hardware session is routed.

### Subphase 1: Capability Reserved-Bit Activation

| Item | Evidence required | Artifact |
|---|---|---|
| `sys_wait(handle)` implemented and rights-gated by WAIT | Commit with syscall implementation, libneo wrapper | Subphase 1 close report |
| `sys_inspect(handle)` implemented and rights-gated by INSPECT | Commit with syscall implementation, libneo wrapper | Subphase 1 close report |
| `sys_close` explicitly deferred | Close report statement; CLOSE bit not activated in code | Subphase 1 close report |
| WNOHANG/still-running ABI documented | Close report documents the return convention and proves no collision with valid `i32` exit codes | Subphase 1 close report |
| Inspect snapshot is scalar-only (no Handle, CapId, pointer, cap identity) | Code review in close report confirms struct fields | Subphase 1 close report |
| Test: `sys_wait_blocking` | Scenario PASS at NEO_SMP=4 | Subphase 1 close report |
| Test: `sys_wait_wnohang` | Scenario PASS at NEO_SMP=4 | Subphase 1 close report |
| Test: `sys_wait_already_exited` | Scenario PASS at NEO_SMP=4 | Subphase 1 close report |
| Test: `sys_wait_multiple_waiters` | Scenario PASS at NEO_SMP=4 | Subphase 1 close report |
| Test: `sys_inspect_running` | Scenario PASS at NEO_SMP=4 | Subphase 1 close report |
| Test: `sys_inspect_exited` | Scenario PASS at NEO_SMP=4 | Subphase 1 close report |
| Test: `sys_inspect_rights_gate` | Scenario PASS at NEO_SMP=4 | Subphase 1 close report |
| `cap_revoke_spawn_inheritance_descendants` (INV-CAP-11) exact scenario name | Scenario PASS; or existing equivalent named with explanation and alias | Subphase 1 close report |
| Full `cap_*` regression suite PASS at NEO_SMP=4 | Verification log | Subphase 1 close report |
| `cargo k` PASS, no PANIC/BUG, frame leak audit clean | Verification log | Subphase 1 close report |

**Close report placeholder:** `Plans/CLOSE-REPORT-phase-2-subphase-1-capability-reserved-bit-activation.md`

### Subphase 2: ADR-016 A2 Scheduler Heap-Decoupling

| Item | Evidence required | Artifact |
|---|---|---|
| Fixed-capacity per-CPU ring of `Box<Thread>` replaces `VecDeque` in `PerCpuSchedulerInner` | Commit with ring implementation and scheduler refactor | Subphase 2 close report |
| Box-pointer-stability invariant preserved (captured `rsp` survives enqueue/dequeue) | Code review in close report; no test failure of existing context-switch paths | Subphase 2 close report |
| Spawn returns `-EAGAIN` at per-CPU capacity (default 64/CPU, configurable at init) | Scenario evidence or verification log | Subphase 2 close report |
| No `alloc`/`dealloc` reachable while holding `PerCpuScheduler.inner` | Code review in close report confirms no heap calls under lock | Subphase 2 close report |
| Test: `sched_ready_queue_capacity_pressure` (INV-SYNC-5) | Scenario PASS — drives to capacity, asserts `-EAGAIN`, no realloc/alloc-under-lock/panic | Subphase 2 close report |
| Full SMP regression suite PASS at NEO_SMP=4 | Verification log | Subphase 2 close report |
| `cargo k` PASS, no PANIC/BUG, frame leak audit clean | Verification log | Subphase 2 close report |
| ADR-016 amendment recording A2 implementation choices | Documentation commit | Subphase 2 close report |

**Close report placeholder:** `Plans/CLOSE-REPORT-phase-2-subphase-2-adr-016-a2-scheduler-ring.md`

### Subphase 3: ADR-019 B/C Type-Level IRQ Safety

| Item | Evidence required | Artifact |
|---|---|---|
| Sub-phase B: all mixed-context `SwitchMutex` callers audited and classified | Audit table in close report listing each site, classification (mixed-IRQ or process-only), and resulting type | Subphase 3 close report |
| Sub-phase B: mixed-context sites migrated to `IrqSafeMutex` | Commit with migration | Subphase 3 close report |
| Sub-phase C: `Channel.state` migrated from `SwitchMutex` to `IrqSafeMutex` | Commit with migration | Subphase 3 close report |
| Sub-phase C: five `without_interrupts(...)` wrappers removed (discipline becomes structural) | Commit diff confirms removal; close report names the sites | Subphase 3 close report |
| Test: `ipc_rendezvous_translate_fault` (INV-IPC-5) absorbed in full suite gate | Scenario PASS as part of suite | Subphase 3 close report |
| Sub-phase B gate: full suite green + ADR-019 IrqSafeMutex micro-bench captured | Verification log; micro-bench numbers recorded | Subphase 3 close report |
| Sub-phase C gate: SC-3B 200/200 NEO_SMP=4 `smp_cross_cpu_rendezvous_stress` PASS | Verification log | Subphase 3 close report |
| `cargo k` PASS, no PANIC/BUG, frame leak audit clean | Verification log | Subphase 3 close report |
| ADR-019 amendment recording sub-phase B/C completion | Documentation commit | Subphase 3 close report |

**Close report placeholder:** `Plans/CLOSE-REPORT-phase-2-subphase-3-adr-019-b-c-irq-safety.md`

### Perf Harness Non-Hardware Tier (Subphase 4, pre-hardware)

| Item | Evidence required | Artifact |
|---|---|---|
| QEMU-TCG deterministic proxy gate implemented and runnable | Commit with proxy counter instrumentation; proxies run as part of existing CI | Perf harness close report |
| Proxy counters: context-switch count, CR3-write count, allocation count, scheduling-decision count for a fixed workload | Counter values logged in verification evidence | Perf harness close report |
| Baseline proxy values captured | Recorded in perf baseline artifact | Perf harness close report |
| Proxy change = CI BLOCKER enforcement wired | CI configuration or xtask integration confirms gate behavior | Perf harness close report |
| VMware/reproducible middle-tier path documented or explicitly deferred by PM | Documentation or PM disposition | Perf harness close report |

**Close report placeholder:** `Plans/CLOSE-REPORT-phase-2-perf-harness-non-hardware-tiers.md`

### F-004 and Remaining Audit Chain (Subphase 4, pre-hardware)

| Item | Evidence required | Artifact |
|---|---|---|
| QEMU-reproducible portion of `F-004` (`smp_concurrent_reap_after_terminate`) executed | Scenario PASS at NEO_SMP=4 in QEMU; or explicit note that the scenario requires hardware-only evidence | Subphase 4 close report |

**Close report placeholder:** `Plans/CLOSE-REPORT-phase-2-subphase-4-formal-close.md` (covers perf harness, F-004, and procedure preparation)

### Hardware Procedure Preparation

| Item | Evidence required | Artifact |
|---|---|---|
| Bare-metal procedure document written | Procedure file at `Plans/PROCEDURE-2026-05-25-phase-2-bare-metal-optiplex-7080-verification.md` (or equivalent date) | Procedure document |
| Boot steps specified | Section in procedure | Procedure document |
| Scenario execution order specified | Section in procedure | Procedure document |
| Evidence-capture instructions (serial-log path, frame-audit command, PASS vocabulary) | Section in procedure | Procedure document |
| PASS/fail strings documented | Per-scenario PASS markers and evidence-shape criteria | Procedure document |
| Escalation path for hardware-exposed bugs | Section in procedure | Procedure document |

**Procedure placeholder:** `Plans/PROCEDURE-2026-05-25-phase-2-bare-metal-optiplex-7080-verification.md`

### Close Documentation Preparation

| Item | Evidence required | Artifact |
|---|---|---|
| ADR-016 close amendment (post-A2 implementation) | Filed as part of subphase 2 close | Subphase 2 close report |
| ADR-019 close amendment (post-B/C migration) | Filed as part of subphase 3 close | Subphase 3 close report |
| Phase 2 close disposition plan (post-hardware-evidence ingestion) | Outline in subphase 4 close report or separate close-prep artifact | Subphase 4 close report |
| `docs/phases/PHASE-2.md` status header update (post-close) | Planned as post-hardware documentation task | Close documentation prep note |

---

## Hardware-Only Remainder (Post-Barrier)

Per `PM-NOTE-2026-05-25-phase-2-hardware-last-execution-barrier.md` §3. Once the pre-hardware checklist is fully satisfied, the remaining Phase 2 work is hardware-only or hardware-evidence consumption:

| Item | Session | Evidence | Artifact |
|---|---|---|---|
| OptiPlex 7080 bare-metal multi-CPU verification at ≥4 logical CPUs | PL hardware session | SMP scenario set PASS, serial-log capture, frame-audit-clean, per-AP GS-base sentinel PASS | Bare-metal evidence log |
| OptiPlex 7080 bare-metal multi-CPU verification at full logical-CPU count | PL hardware session | Same evidence vocabulary at full core count | Bare-metal evidence log |
| Hardware-tier absolute-latency baseline (boot time, syscall latency ns, context-switch ns, scheduling fairness) | Shared PL hardware session | n-trial median + IQR for each metric | Perf baseline artifact |
| ADR-019 50ns IrqSafeMutex micro-bench on bare metal | Shared PL hardware session | Acquire/release cycle cost on real hardware | Perf baseline artifact |
| Hardware-only `F-004` evidence if QEMU cannot fully satisfy it | PL hardware session (if needed) | Evidence that `smp_concurrent_reap_after_terminate` holds on real hardware | Subphase 4 close report |
| Close report evidence ingestion | PM post-session | All hardware evidence consumed into subphase 4 close report | Subphase 4 close report |
| PM/PL Phase 2 close disposition | PM + PL | Formal Phase 2 closure artifact | `Plans/CLOSE-DISPOSITION-phase-2.md` (placeholder) |

---

## Execution Sequencing Under Hardware-Last Barrier

```
Subphase 1 (Jarvis/Codex) ──→ Subphase 2 (Jarvis/Codex) ──→ Subphase 3 (Jarvis/Codex)
                                                                         │
Perf harness non-hardware tier (Prometheus) ─────────────────────────────┤
                                                                         │
F-004 QEMU portion (Jarvis/Codex or Prometheus) ────────────────────────┤
                                                                         │
Hardware procedure preparation (Prometheus) ─────────────────────────────┤
                                                                         ▼
                                                                 Pre-hardware barrier check
                                                                 (PM verifies every row above
                                                                  is complete or hardware-blocked)
                                                                         ▼
                                                                 PL hardware session (OptiPlex 7080)
                                                                         ▼
                                                                 Evidence ingestion + close disposition
```

**Parallelism notes:**
- Subphases 1 → 2 → 3 are sequential by ratified ordering (A2 before ADR-019 B/C per THINK v0.2 Q7(d)).
- Perf harness non-hardware tier can proceed in parallel with subphases 1–3 (no code dependency).
- Hardware procedure preparation can proceed in parallel with subphases 1–3 (pure documentation).
- F-004 QEMU portion can proceed in parallel with subphases 1–3 if the scenario is implementable without A2/ADR-019 changes; otherwise it sequences after the subphase it depends on.

---

## Barrier Verification Protocol

When PM is ready to route the hardware session, verify against this checklist:

1. **Subphase 1 close report exists and is accepted.** `sys_wait`, `sys_inspect`, INV-CAP-11 all verified in QEMU at NEO_SMP=4.
2. **Subphase 2 close report exists and is accepted.** Scheduler rings verified, INV-SYNC-5 PASS, full SMP regression clean.
3. **Subphase 3 close report exists and is accepted.** B/C migration verified, INV-IPC-5 PASS, SC-3B 200/200 stress gate PASS.
4. **Perf harness non-hardware tier is runnable.** QEMU-TCG proxies integrated and gating. VMware path documented or deferred.
5. **F-004 QEMU portion executed** (or explicitly hardware-blocked with explanation).
6. **Hardware procedure document is written.** Boot steps, scenario order, evidence capture, PASS vocabulary, escalation path all present.
7. **Close documentation plan is prepared.** ADR amendment placeholders identified; Phase 2 close disposition outline exists.

If any row is still actionable without hardware, the hardware session waits.

---

## Close Report Placeholders

These are the expected close report filenames for PM to use when each subphase completes:

| Subphase | Close report path |
|---|---|
| 1 | `Plans/CLOSE-REPORT-phase-2-subphase-1-capability-reserved-bit-activation.md` |
| 2 | `Plans/CLOSE-REPORT-phase-2-subphase-2-adr-016-a2-scheduler-ring.md` |
| 3 | `Plans/CLOSE-REPORT-phase-2-subphase-3-adr-019-b-c-irq-safety.md` |
| 4 | `Plans/CLOSE-REPORT-phase-2-subphase-4-formal-close.md` |
| Phase 2 close | `Plans/CLOSE-DISPOSITION-phase-2.md` |

---

## References

- `Plans/PM-NOTE-2026-05-25-phase-2-hardware-last-execution-barrier.md` — binding sequencing constraint (normative)
- `Plans/PROMETHEUS-OUTPUT-2026-05-25-phase-2-plan-substance.md` (`e6a4cdf`) — four-subphase PLAN with gates, audit mapping, hardware session plan
- `Plans/PM-DISPOSITION-2026-05-25-phase-2-plan-substance-close.md` (`b7281d0`) — PM acceptance of PLAN substance + downstream clarifications
- `Plans/PL-RATIFICATION-2026-05-25-phase-2-plan.md` (`a7bcacf`) — PL ratification of four-subphase PLAN
- `Plans/PHASE-2-THINK-v0.2-2026-05-24.md` (`349151e`) — ratified answered dispositions (Q1–Q7)
- `Plans/DISPATCH-2026-05-25-phase-2-build-subphase-1-capability-reserved-bit-activation-jarvis.md` — subphase 1 BUILD dispatch (filed, in flight)
- `Plans/PROCEDURE-2026-05-20-phase-2-and-3-vmware-hypervisor-vm-verification.md` — VMware procedure template for bare-metal procedure authoring
- `docs/ADR-016-irq-safe-heap-and-scheduler-decoupling.md` — A2 architectural constraints
- `docs/ADR-019-switchmutex-irq-off-by-construction.md` — IrqSafeMutex type and sub-phase structure
- `Plans/PM-DISPOSITION-2026-05-24-pre-phase-2-audit-close.md` — audit (b) item definitions (F-004, INV-IPC-5, INV-CAP-11, INV-SYNC-5)
