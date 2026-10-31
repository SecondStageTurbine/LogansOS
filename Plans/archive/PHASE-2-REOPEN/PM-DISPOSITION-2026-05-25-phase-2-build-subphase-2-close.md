# PM-DISPOSITION: Phase 2 BUILD subphase 2 close

- Author: PM
- Date: 2026-05-25
- Status: Jarvis subphase 2 ACCEPTED. **Phase 2 BUILD subphase 3 may be dispatched.**
- Inputs consumed:
  - `Plans/CLOSE-REPORT-2026-05-25-phase-2-build-subphase-2-adr-016-a2-scheduler-ring-jarvis.md` (`950ece5`) - Jarvis close report
  - `5aa5baaff6029769323a0ced3e4e088ccbbf9d43` - implementation commit
  - `950ece5fb4df4a7805d5e04e064afe881e23d1c3` - ADR-016 amendment and close report commit
  - `Plans/DISPATCH-2026-05-25-phase-2-build-subphase-2-adr-016-a2-scheduler-ring-jarvis.md` (`14b3697`) - dispatch
  - `Plans/PM-NOTE-2026-05-25-phase-2-hardware-last-execution-barrier.md` (`b3bb488`) - hardware-last barrier

---

## 1. Acceptance-Criteria Verification

| Criterion | Status | Evidence |
|---|---|---|
| `PerCpuSchedulerInner.ready` and `.zombies` use fixed-capacity rings | PASS | Close report records `ThreadRing<const N: usize>` with `SCHEDULER_RING_CAPACITY = 64`; PM spot-check found `SchedulerThreadRing` in `kernel/src/sched/scheduler.rs` |
| `VecDeque<Box<Thread>>` removed from scheduler ready/zombie storage | PASS | Close report records removal; PM spot-check found no `VecDeque` import/use in scheduler implementation |
| Box-pointer-stability preserved | PASS | Ring stores `Box<Thread>` values, so moving slots moves ownership/pointers, not `Thread` payloads |
| User spawn fails gracefully at capacity | PASS | `process::spawn_user` returns `SpawnError::SchedulerFull`; `sys_spawn` maps it to `-EAGAIN` |
| `reap()` avoids allocation under scheduler lock | PASS | Close report records one-zombie pop under lock followed by `Thread::free` and `process::on_thread_reap` after lock release |
| `INV-SYNC-5` exact scenario present | PASS | `sched_ready_queue_capacity_pressure` added and wired; close report records PASS at NEO_SMP=4 |
| Existing SMP regression gate | PASS | Close report records listed SMP scenarios PASS at NEO_SMP=4 |
| Subphase 1 anti-regression preserved | PASS | Close report records wait/inspect scenarios, exact `INV-CAP-11`, and full `cap_*` suite PASS at NEO_SMP=4 |
| ADR-016 close documentation landed | PASS | `docs/ADR-016-irq-safe-heap-and-scheduler-decoupling.md` has `Amendment 2026-05-25 - A2 implementation landed` |
| Hardware anti-scope honored | PASS | Close report explicitly records no hardware evidence collected and no perf/hardware work started |

No correction pass is required.

## 2. PM Rulings

### 2.1 Subphase 2 accepted

PM accepts Jarvis's Phase 2 BUILD subphase 2 close. ADR-016 A2 scheduler heap-decoupling is complete for Phase 2:

- fixed-capacity scheduler ready/zombie rings are implemented
- spawn capacity pressure is visible as `-EAGAIN`
- `INV-SYNC-5` is covered by `sched_ready_queue_capacity_pressure`
- ADR-016 has a close amendment recording the landed implementation

### 2.2 Source-level caveat

Internal scheduler wake/exit paths still treat ring overflow as a kernel invariant failure. PM accepts this as matching the dispatch: user-driven spawn pressure must return `-EAGAIN`; internal scheduler overflow indicates a violated kernel scheduling invariant.

### 2.3 Scope caveats preserved

This close does not claim:

- ADR-019 B/C lock migration
- `Channel.state` migration
- perf harness work
- hardware evidence
- process/channel/capability `sys_close`
- NVMe FLUSH durability diagnosis

### 2.4 Hardware-last checklist update

The subphase 2 row in `Plans/PM-NOTE-2026-05-25-phase-2-hardware-last-execution-barrier.md` is satisfied, subject to this PM acceptance.

Hardware remains blocked. Phase 2 now proceeds to subphase 3: ADR-019 B/C type-level IRQ safety.

## 3. Current Phase 2 State

| Area | State |
|---|---|
| Subphase 1: capability reserved-bit activation | COMPLETE and accepted |
| Subphase 2: ADR-016 A2 scheduler heap-decoupling | COMPLETE and accepted |
| Subphase 3: ADR-019 B/C IRQ safety | Ready for dispatch |
| Subphase 4: formal close | Non-hardware prep may continue; hardware session blocked |
| Hardware testing | Final Phase 2 action only |

## 4. Next Steps

1. PM dispatches subphase 3 to Jarvis/Codex.
2. Subphase 3 must preserve subphase 1 and subphase 2 gates.
3. No hardware session is routed until the hardware-last barrier checklist is complete.

## 5. References

- `Plans/CLOSE-REPORT-2026-05-25-phase-2-build-subphase-2-adr-016-a2-scheduler-ring-jarvis.md` (`950ece5`) - accepted close report.
- `5aa5baaff6029769323a0ced3e4e088ccbbf9d43` - accepted implementation commit.
- `950ece5fb4df4a7805d5e04e064afe881e23d1c3` - accepted ADR-016 amendment and close report commit.
- `Plans/DISPATCH-2026-05-25-phase-2-build-subphase-2-adr-016-a2-scheduler-ring-jarvis.md` (`14b3697`) - dispatch.
- `Plans/PROMETHEUS-OUTPUT-2026-05-25-phase-2-hardware-last-close-prep.md` (`87a4620`) - active pre-hardware tracking map.
