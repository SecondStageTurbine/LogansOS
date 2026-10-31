# PM-DISPOSITION: Phase 2 BUILD subphase 1 close

- Author: PM
- Date: 2026-05-25
- Status: Jarvis subphase 1 ACCEPTED. **Phase 2 BUILD subphase 2 may be dispatched.**
- Inputs consumed:
  - `Plans/CLOSE-REPORT-2026-05-25-phase-2-build-subphase-1-capability-reserved-bit-activation-jarvis.md` (`f87a26e`) - Jarvis close report
  - `6587cd607222bb3ed9d2ac77d5d1dfa9940d69e3` - implementation commit
  - `Plans/DISPATCH-2026-05-25-phase-2-build-subphase-1-capability-reserved-bit-activation-jarvis.md` (`a7bcacf`) - dispatch
  - `Plans/PM-NOTE-2026-05-25-phase-2-hardware-last-execution-barrier.md` (`b3bb488`) - hardware-last barrier

---

## 1. Acceptance-Criteria Verification

| Criterion | Status | Evidence |
|---|---|---|
| `SYS_WAIT = 19` and `SYS_INSPECT = 20` implemented in kernel and libneo | PASS | Close report syscall ABI section; PM spot-check found constants and dispatcher arms in `kernel/src/arch/x86_64/syscall.rs` and `userland/libneo/src/lib.rs` |
| WAIT/INSPECT rights gates are operation-specific | PASS | Close report names KILL/WAIT/INSPECT split and EBADF/EPERM/ESRCH behavior |
| `sys_wait` WNOHANG result cannot collide with `i32` exit codes | PASS | `-EAGAIN` for still running; successful exits use `WAIT_EXITED_TAG | exit_code` and libneo decodes to `i32` |
| Blocking wait does not busy-poll | PASS | Close report records parked waiter support with preallocated per-process wait queue |
| Wait behavior covered | PASS | `sys_wait_blocking`, `sys_wait_wnohang`, `sys_wait_already_exited`, and `sys_wait_multiple_waiters` PASS at NEO_SMP=4 |
| Inspect behavior covered | PASS | `sys_inspect_running`, `sys_inspect_exited`, and `sys_inspect_rights_gate` PASS at NEO_SMP=4 |
| Inspect snapshot is scalar-only | PASS | Close report lists scalar fields only and excludes Handle, CapId, pointers, cap identities, and authority-bearing values |
| Exact `INV-CAP-11` scenario present | PASS | `cap_revoke_spawn_inheritance_descendants` added, wired, and PASS at NEO_SMP=4 |
| Full `cap_*` regression gate | PASS | Close report records full cap scenario set PASS at NEO_SMP=4 |
| `sys_close` remains deferred | PASS | Existing file `sys_close` remains `SYS_CLOSE = 14`; process/channel/capability close remains explicitly deferred |
| Verification commands recorded | PASS | `docker compose run --rm neo cargo k`, targeted scenarios, cap suite, serial scan, and `git diff --check` all recorded PASS |

No correction pass is required.

## 2. PM Rulings

### 2.1 Subphase 1 accepted

PM accepts Jarvis's Phase 2 BUILD subphase 1 close. The capability reserved-bit activation milestone is complete:

- `sys_wait` is callable and WAIT-gated.
- `sys_inspect` is callable and INSPECT-gated.
- The exact `INV-CAP-11` scenario is present and verified.
- `sys_close` remains deferred.

### 2.2 ABI choice accepted

The wait ABI is accepted:

- `-EAGAIN` means WNOHANG still-running or wait-queue capacity pressure.
- Completed waits return a tagged success value in kernel ABI space.
- libneo decodes the tagged success value back to `i32`.

This satisfies the dispatch requirement that a WNOHANG still-running result not collide with any valid process exit code.

### 2.3 Scope caveats preserved

This close does not claim:

- full FM-D14 structural retirement through handle-presence-gated slot lifetime
- PID recycling
- broad stale-handle revocation beyond wait/inspect behavior
- process/channel/capability `sys_close`
- ADR-016 A2 scheduler heap-decoupling
- ADR-019 B/C IRQ-safety migration
- perf harness or hardware evidence

### 2.4 Hardware-last checklist update

The subphase 1 row in `Plans/PM-NOTE-2026-05-25-phase-2-hardware-last-execution-barrier.md` is satisfied, subject to this PM acceptance.

Hardware remains blocked. Phase 2 now proceeds to subphase 2: ADR-016 A2 scheduler heap-decoupling.

## 3. Current Phase 2 State

| Area | State |
|---|---|
| Subphase 1: capability reserved-bit activation | COMPLETE and accepted |
| Subphase 2: ADR-016 A2 scheduler heap-decoupling | Ready for dispatch |
| Subphase 3: ADR-019 B/C IRQ safety | Pending subphase 2 |
| Subphase 4: formal close | Non-hardware prep may continue; hardware session blocked |
| Hardware testing | Final Phase 2 action only |

## 4. Next Steps

1. PM dispatches subphase 2 to Jarvis/Codex.
2. Subphase 2 must preserve subphase 1 wait/inspect and cap-regression gates.
3. No hardware session is routed until the hardware-last barrier checklist is complete.

## 5. References

- `Plans/CLOSE-REPORT-2026-05-25-phase-2-build-subphase-1-capability-reserved-bit-activation-jarvis.md` (`f87a26e`) - accepted close report.
- `6587cd607222bb3ed9d2ac77d5d1dfa9940d69e3` - accepted implementation commit.
- `Plans/DISPATCH-2026-05-25-phase-2-build-subphase-1-capability-reserved-bit-activation-jarvis.md` (`a7bcacf`) - dispatch.
- `Plans/PROMETHEUS-OUTPUT-2026-05-25-phase-2-hardware-last-close-prep.md` (`87a4620`) - active pre-hardware tracking map.
