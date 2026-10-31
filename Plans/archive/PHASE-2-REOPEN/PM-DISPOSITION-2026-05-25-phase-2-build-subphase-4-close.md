# PM-DISPOSITION: Phase 2 BUILD subphase 4 pre-hardware close

- Author: PM
- Date: 2026-05-25
- Status: Subphase 4 pre-hardware work ACCEPTED. Phase 2 is ready for the final hardware-only session.
- Inputs consumed:
  - `Plans/CLOSE-REPORT-2026-05-25-phase-2-build-subphase-4-formal-close-pre-hardware.md` (`39a8048`) - worker close report
  - `b7d27a66b1256456b098183d1fa50c7b219ab896` - implementation/procedure commit
  - `39a804815019a740238e821ecd69040746a6ef87` - close report commit
  - `Plans/DISPATCH-2026-05-25-phase-2-build-subphase-4-formal-close-pre-hardware.md` (`e5cc026`) - dispatch
  - `Plans/PM-NOTE-2026-05-25-phase-2-hardware-last-execution-barrier.md` (`b3bb488`) - hardware-last barrier

---

## 1. Acceptance-Criteria Verification

| Criterion | Status | Evidence |
|---|---|---|
| QEMU-TCG proxy perf gate exists and is runnable | PASS | Close report records `phase2_perf_proxy_baseline` command and PASS |
| Proxy baseline values captured | PASS | `context_switches=3`, `scheduling_decisions=3`, `cr3_performed=2`, `cr3_skipped=1`, `alloc_calls=1`, `dealloc_calls=1` |
| Proxy drift is a blocker | PASS | Scenario compares runtime counters to committed constants and fails the normal `cargo neo-test` runner on drift |
| Exact F-004 scenario exists | PASS | `smp_concurrent_reap_after_terminate` PASS with `inv=INV-PROC-5 f004=concurrent_reap_after_terminate` |
| VMware middle tier documented or deferred | PASS | Existing VMware procedure is cited; PM accepts explicit deferral because no executable VMware-shaped harness exists |
| OptiPlex 7080 hardware procedure exists | PASS | `Plans/PROCEDURE-2026-05-25-phase-2-bare-metal-optiplex-7080-verification.md` filed |
| Accepted subphase 1-3 gates preserved | PASS | Close report records wait/inspect, cap regression, scheduler capacity, `INV-IPC-5`, IRQ-safe mutex, and cross-CPU rendezvous PASS |
| Full `NEO_SMP=4` suite passed | PASS | `docker compose run --rm -e NEO_SMP=4 neo cargo neo-test all --no-meta` returned PASS; wrapper wall time `3235.6s` |
| Serial-log scan clean | PASS | Canonical final logs: `201` scanned, `0` unexpected fail/panic/BUG/leak matches |
| Expected-panic handling correct | PASS | `q7c_shootdown_timeout_panic` raw `KERNEL PANIC` accepted only because the final log emits `event=PASS expected_panic=tlb_shootdown_timeout` |
| No hardware executed | PASS | Close report and procedure both explicitly state no hardware evidence was claimed |
| `git diff --check` | PASS | Worker ran `git diff --check`; PM reran before filing this disposition with no output |

No correction pass is required.

## 2. PM Rulings

### 2.1 Subphase 4 pre-hardware work accepted

PM accepts subphase 4 pre-hardware work as complete. The non-hardware formal-close rows are done:

- deterministic QEMU-TCG structural proxy tier exists and gates drift
- exact QEMU-reproducible F-004 scenario exists and passes
- VMware middle tier is documented and execution is explicitly deferred
- OptiPlex 7080 procedure is ready
- accepted subphase 1-3 regression gates remain green

### 2.2 VMware deferral accepted

PM accepts the worker's VMware disposition. Current repo tooling has no executable VMware-shaped harness equivalent to `cargo neo-test`; VMware remains a manual middle-tier procedure and is not a blocker for routing the hardware-only Phase 2 remainder.

### 2.3 Retry-attempt caveat accepted

PM accepts the full-sweep caveat: `sys_wait_multiple_waiters.attempt1.serial.log` contained a retry-attempt failure, while `attempt2` and canonical `sys_wait_multiple_waiters.serial.log` passed and the full command returned zero. This is not a final successful-log failure, but the retry evidence remains useful for future flake triage.

### 2.4 Hardware-last barrier satisfied

Every checklist row in `Plans/PM-NOTE-2026-05-25-phase-2-hardware-last-execution-barrier.md` is now either complete or hardware-only:

- Subphase 1 accepted
- Subphase 2 accepted
- Subphase 3 accepted
- Subphase 4 non-hardware rows accepted by this disposition

The only remaining Phase 2 work is the OptiPlex hardware session, hardware perf capture, evidence ingestion, and final Phase 2 close disposition.

## 3. Current Phase 2 State

| Area | State |
|---|---|
| Subphase 1: capability reserved-bit activation | COMPLETE and accepted |
| Subphase 2: ADR-016 A2 scheduler heap-decoupling | COMPLETE and accepted |
| Subphase 3: ADR-019 B/C IRQ safety | COMPLETE and accepted |
| Subphase 4: pre-hardware formal close | COMPLETE and accepted |
| Hardware testing | READY TO ROUTE as final Phase 2 action |
| Phase 2 formal close | BLOCKED only on hardware evidence ingestion |

## 4. Next Steps

1. Route the OptiPlex 7080 hardware session using `Plans/PROCEDURE-2026-05-25-phase-2-bare-metal-optiplex-7080-verification.md`.
2. Preserve full serial evidence and perf capture artifacts.
3. File `Plans/PHASE-2-BARE-METAL-OPTIPLEX-7080-VERIFICATION-YYYY-MM-DD.md`.
4. PM consumes hardware evidence and files the final Phase 2 close disposition.

## 5. References

- `Plans/CLOSE-REPORT-2026-05-25-phase-2-build-subphase-4-formal-close-pre-hardware.md` (`39a8048`) - accepted close report.
- `Plans/PROCEDURE-2026-05-25-phase-2-bare-metal-optiplex-7080-verification.md` (`b7d27a6`) - hardware procedure.
- `Plans/PM-NOTE-2026-05-25-phase-2-hardware-last-execution-barrier.md` (`b3bb488`) - barrier now satisfied.

---

**End of disposition.**
