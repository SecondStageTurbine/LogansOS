# PM-DISPOSITION: Phase 2 BUILD subphase 3 close

- Author: PM
- Date: 2026-05-25
- Status: Jarvis subphase 3 ACCEPTED. **Phase 2 BUILD subphase 4 pre-hardware work may be dispatched.**
- Inputs consumed:
  - `Plans/CLOSE-REPORT-2026-05-25-phase-2-build-subphase-3-adr-019-b-c-irq-safety-jarvis.md` (`a202ed4`) - Jarvis close report
  - `e7094a860d0b928a5bb08046009357fd8eb2f6d7` - implementation commit
  - `a202ed4894e9f4e2e0bad0944e3e591a39b756c6` - close report commit
  - `Plans/DISPATCH-2026-05-25-phase-2-build-subphase-3-adr-019-b-c-irq-safety-jarvis.md` (`22cb54d`) - dispatch
  - `Plans/PM-NOTE-2026-05-25-phase-2-hardware-last-execution-barrier.md` (`b3bb488`) - hardware-last barrier

---

## 1. Acceptance-Criteria Verification

| Criterion | Status | Evidence |
|---|---|---|
| Production `SwitchMutex` audit table complete | PASS | Close report lists every production site, classification, action, and rationale; test-harness fixtures are excluded as dispatched |
| Mixed IRQ-context production locks migrated where the type fits | PASS | `ipc::Channel.state`, `AHCI_DEVICES`, and `VIRTIO_DEVICES` are recorded as migrated; PM spot-check found `IrqSafeMutex` declarations in the live source |
| Retained `SwitchMutex` sites are justified | PASS | PM spot-check found declaration-site comments naming process-context-only reachability or scheduler/context-switch release-token requirements |
| `Channel.state` migrated to structural IRQ safety | PASS | PM spot-check found `state: IrqSafeMutex<[EndpointState; 2]>` in `kernel/src/ipc/mod.rs` |
| Channel-state-only `without_interrupts(...)` wrappers removed or narrowed | PASS | Close report names removed/narrowed sites; retained wrappers are scoped to complete IPC handoff or scheduler wake/zombify work, not state-lock discipline alone |
| `INV-IPC-5` exact scenario present | PASS | `ipc_rendezvous_translate_fault` is wired in kernel/userland/xtask and close report records PASS with `inv=INV-IPC-5 translate_fault=clean_unwind` |
| `irqsafe_mutex_bench` evidence captured | PASS | Close report records `spin_mean_ns=864`, `irqsafe_mean_ns=3834`, `switch_mean_ns=1473`, `overhead_ns=2970`, threshold `6000` |
| SC-3B 200/200 stress gate passed | PASS | Close report records final container `551a8704c662...`, `docker wait` status 0, and completion line; PM spot-check found the local saved log ending with `COMPLETE 200/200` |
| Full suite and prior subphase gates preserved | PASS | Close report records `cargo neo-test all --no-meta` at `NEO_SMP=4`, wait/inspect, full cap regression, exact `INV-CAP-11`, and scheduler capacity gate PASS |
| Successful serial-log scan clean | PASS | Close report records clean scan over current successful logs and explicitly bounds retained stale/expected-failure logs |
| ADR-019 close documentation landed | PASS | `docs/ADR-019-switchmutex-irq-off-by-construction.md` has the 2026-05-25 B/C landed amendment |
| Hardware/perf anti-scope honored | PASS | Close report explicitly records no hardware evidence and no perf harness implementation |
| `git diff --check` | PASS | Worker recorded PASS; PM re-ran `git diff --check` on the accepted checkout with no output |

No correction pass is required.

## 2. PM Rulings

### 2.1 Subphase 3 accepted

PM accepts Jarvis's Phase 2 BUILD subphase 3 close. ADR-019 B/C is complete for Phase 2:

- production `SwitchMutex` sites are audited and classified
- mixed-context list locks that fit the type moved to `IrqSafeMutex`
- `Channel.state` is structurally IRQ-safe by type
- state-only manual IRQ wrappers were removed or narrowed
- exact `INV-IPC-5` coverage exists as `ipc_rendezvous_translate_fault`
- the SC-3B 200/200 `NEO_SMP=4` stress gate passed after the migration

### 2.2 API-adaptation caveat accepted

`IrqSafeMutex` now delegates to `SwitchMutex<T>` internally and exposes release-token support for the IPC context-switch handoff. PM accepts this as the minimal adaptation needed to preserve the parked-thread publication path while making IF-off acquisition structural. This does not reopen ADR-019 or authorize merging `SwitchMutex` and `IrqSafeMutex` into one abstraction.

### 2.3 Evidence caveat

PM did not rerun the full suite or the 200-run loop. Acceptance is based on the pushed close report, source spot-checks, the committed ADR amendment, local presence of the final 200-run log, and a fresh `git diff --check`.

### 2.4 Scope caveats preserved

This close does not claim:

- subphase 4 perf harness completion
- OptiPlex or other hardware evidence
- VMware middle-tier execution
- `F-004` absorption
- process/channel/capability `sys_close`
- NVMe FLUSH durability diagnosis
- scheduler-ring behavior beyond preservation of the accepted subphase 2 implementation

### 2.5 Hardware-last checklist update

The subphase 3 row in `Plans/PM-NOTE-2026-05-25-phase-2-hardware-last-execution-barrier.md` is satisfied, subject to this PM acceptance.

Hardware remains blocked. Phase 2 now proceeds to subphase 4 pre-hardware work: non-hardware perf harness tier, QEMU-reproducible `F-004`, bare-metal procedure preparation, and close-prep artifacts. The OptiPlex session is still the final Phase 2 action only.

## 3. Current Phase 2 State

| Area | State |
|---|---|
| Subphase 1: capability reserved-bit activation | COMPLETE and accepted |
| Subphase 2: ADR-016 A2 scheduler heap-decoupling | COMPLETE and accepted |
| Subphase 3: ADR-019 B/C IRQ safety | COMPLETE and accepted |
| Subphase 4: formal close | Ready for pre-hardware dispatch |
| Hardware testing | Blocked until every non-hardware Phase 2 row is complete or explicitly hardware-only |

## 4. Next Steps

1. PM dispatches subphase 4 pre-hardware work.
2. The worker completes all QEMU/docs/procedure actions that do not require physical hardware.
3. PM performs the pre-hardware barrier check before routing the OptiPlex session.

## 5. References

- `Plans/CLOSE-REPORT-2026-05-25-phase-2-build-subphase-3-adr-019-b-c-irq-safety-jarvis.md` (`a202ed4`) - accepted close report.
- `e7094a860d0b928a5bb08046009357fd8eb2f6d7` - accepted implementation commit.
- `a202ed4894e9f4e2e0bad0944e3e591a39b756c6` - accepted close report commit.
- `Plans/DISPATCH-2026-05-25-phase-2-build-subphase-3-adr-019-b-c-irq-safety-jarvis.md` (`22cb54d`) - dispatch.
- `Plans/PROMETHEUS-OUTPUT-2026-05-25-phase-2-hardware-last-close-prep.md` - active pre-hardware tracking map.
- `Plans/PM-NOTE-2026-05-25-phase-2-hardware-last-execution-barrier.md` (`b3bb488`) - binding hardware-last barrier.
