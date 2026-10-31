# CLOSE REPORT: Phase 2 BUILD subphase 4 - formal close pre-hardware

- From: Phase 2 worker lane
- To: PM
- Date: 2026-05-25
- Dispatch: `Plans/DISPATCH-2026-05-25-phase-2-build-subphase-4-formal-close-pre-hardware.md`
- Status: PRE-HARDWARE WORK COMPLETE
- Hardware execution: NOT PERFORMED

---

## 1. Commit

Implementation/procedure commit:

- `b7d27a66b1256456b098183d1fa50c7b219ab896` - `phase2: add pre-hardware close gates`

Close-report commit: this file is expected to be committed separately after filing.

## 2. Files Changed

Source and test runner:

- `kernel/Cargo.toml`
- `kernel/src/ipc/mod.rs`
- `kernel/src/mm/heap.rs`
- `kernel/src/sched/process.rs`
- `kernel/src/sched/scheduler.rs`
- `kernel/src/sched/thread.rs`
- `kernel/src/test_harness.rs`
- `tools/xtask/src/main.rs`

Procedure:

- `Plans/PROCEDURE-2026-05-25-phase-2-bare-metal-optiplex-7080-verification.md`

## 3. QEMU-TCG Perf Proxy Tier

New runnable scenario:

```powershell
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test phase2_perf_proxy_baseline --no-meta
```

Structural counters enforced by the scenario:

- context switches: `3`
- scheduling decisions: `3`
- CR3 writes performed: `2`
- CR3 writes skipped: `1`
- allocation calls: `1`
- deallocation calls: `1`

Observed PASS line:

```text
NEO_TEST/1 event=PASS name=phase2_perf_proxy_baseline context_switches=3 scheduling_decisions=3 cr3_performed=2 cr3_skipped=1 alloc_calls=1 dealloc_calls=1 baseline=enforced
```

Blocker behavior:

- The scenario compares runtime counters to the committed constants.
- Any unexpected drift emits `event=FAIL name=phase2_perf_proxy_baseline reason=perf_proxy_baseline_drift ...` and the normal `cargo neo-test` runner returns nonzero.
- Legitimate updates require investigation first, then an intentional source update to the scenario constants and this baseline evidence. QEMU elapsed time is not an acceptance signal.

## 4. F-004 Evidence

Exact scenario added and run:

```powershell
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test smp_concurrent_reap_after_terminate --no-meta
```

Observed PASS line:

```text
NEO_TEST/1 event=PASS name=smp_concurrent_reap_after_terminate inv=INV-PROC-5 f004=concurrent_reap_after_terminate kill_rc=0 racer_done=1 process_absent=true zombies_reaped=true channel_closed=true
```

Implemented property:

- CPU0 drives `terminate(pid)`.
- CPU1 is scheduled into `on_thread_reap(pid)` while termination is paused at the test-only hook.
- The scenario verifies no double cleanup, process registry removal, zombie reap completion, endpoint closure, and no frame-audit regression.

## 5. VMware Middle Tier Disposition

Current repo status:

- The repo has the normal QEMU/xtask test runner and procedure documentation.
- There is no executable VMware-shaped harness in the current runner that can be invoked like `cargo neo-test`.
- VMware evidence must therefore be collected as a manual middle-tier procedure, not claimed by this dispatch.

Existing procedure reference:

- `Plans/PROCEDURE-2026-05-20-phase-2-and-3-vmware-hypervisor-vm-verification.md`

PM deferral proposal:

- Defer VMware baseline execution until PL/PM schedules the manual hypervisor-VM session.
- File future VMware evidence under `Plans/PHASE-2-AND-3-VMWARE-HYPERVISOR-VM-VERIFICATION-<date>.md` or a Phase 2-specific successor report.
- This does not block the pre-hardware close work because the dispatch allowed documentation or explicit PM deferral.

## 6. OptiPlex Procedure

Prepared:

- `Plans/PROCEDURE-2026-05-25-phase-2-bare-metal-optiplex-7080-verification.md`

The procedure covers:

- OptiPlex 7080 target assumptions
- UEFI and Secure Boot posture
- `EFI/BOOT/BOOTX64.EFI` media expectations
- boot smoke and recovery path
- serial-log path and naming convention
- scenario order for `>=4` logical CPUs and full logical CPU count
- frame-audit and PASS/fail vocabulary
- per-AP GS-base sentinel evidence
- required bare-metal perf captures
- PM ingestion requirements after the hardware session

No OptiPlex or other hardware execution was performed.

## 7. Required Gates

All commands below were run from `E:\AI Automation\AltimitNeo`.

### 7.1 Compile

```powershell
docker compose run --rm neo cargo k
```

Result: PASS.

### 7.2 New Gates

```powershell
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test phase2_perf_proxy_baseline --no-meta
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test smp_concurrent_reap_after_terminate --no-meta
```

Result: PASS/PASS.

### 7.3 Accepted Subphase 2/3 Preservation

```powershell
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test sched_ready_queue_capacity_pressure ipc_rendezvous_translate_fault irqsafe_mutex_if_restore irqsafe_mutex_bench smp_cross_cpu_rendezvous_stress --no-meta
```

Result: PASS.

Representative PASS lines:

```text
NEO_TEST/1 event=PASS name=sched_ready_queue_capacity_pressure sched_capacity_scenario=ok exit_code=0
NEO_TEST/1 event=PASS name=ipc_rendezvous_translate_fault inv=INV-IPC-5 translate_fault=clean_unwind exit_code=0
NEO_TEST/1 event=PASS name=irqsafe_mutex_if_restore cpu=0 thread_id=0 site=0xa4bdccd8 if_restore=ok
NEO_TEST/1 event=PASS name=irqsafe_mutex_bench spin_mean_ns=442 irqsafe_mean_ns=3738 switch_mean_ns=1535 overhead_ns=3296 threshold_ns=6000
NEO_TEST/1 event=PASS name=smp_cross_cpu_rendezvous_stress iterations=1000 send_first=500 recv_first=500 cpus=4
```

### 7.4 Accepted Subphase 1 Wait/Inspect

```powershell
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test sys_wait_blocking sys_wait_wnohang sys_wait_already_exited sys_wait_multiple_waiters sys_inspect_running sys_inspect_exited sys_inspect_rights_gate --no-meta
```

Result: PASS.

### 7.5 Capability Regression

```powershell
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test cap_mint_and_use cap_derive_subset cap_derive_rights_violation cap_derive_missing_meta cap_grant_basic cap_grant_missing_meta cap_revoke_local cap_revoke_cross_process cap_revoke_descendants_walk cap_revoke_spawn_inheritance_descendants cap_survive_exit --no-meta
```

Result: PASS.

### 7.6 Full Sweep

```powershell
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test all --no-meta
```

Result: PASS. Wall time observed by the tool wrapper: `3235.6s`.

Note: `sys_wait_multiple_waiters.attempt1.serial.log` from the full sweep contains one retry-attempt `event=FAIL`; `sys_wait_multiple_waiters.attempt2.serial.log` and canonical `sys_wait_multiple_waiters.serial.log` PASS. The command returned zero. This is preserved as retry evidence, not counted as a final successful-log failure.

### 7.7 Serial-Log Scan

Scan scope: canonical final serial logs modified during the full `all` run, excluding `.attemptN` intermediates.

Command shape:

```powershell
$since = Get-Date '2026-05-25T12:35:00'
$logs = Get-ChildItem 'target/neo-test' -Filter '*.serial.log' |
  Where-Object { $_.LastWriteTime -ge $since -and $_.Name -notmatch '\.attempt\d+\.serial\.log$' }
$logs | Select-String -Pattern 'event=FAIL|KERNEL PANIC|BUG:|frame leak detected|frame_leak|leak_audit=dirty|frame_audit.*leak'
```

Result:

```text
canonical_logs_since=201
raw_matches=1
unexpected_matches=0
```

The one raw match is `q7c_shootdown_timeout_panic.serial.log`, which is the expected-panic scenario and emits:

```text
NEO_TEST/1 event=PASS name=q7c_shootdown_timeout_panic expected_panic=tlb_shootdown_timeout
```

### 7.8 Whitespace

```powershell
git diff --check
git diff --cached --check
```

Result before implementation/procedure commit: PASS.

## 8. Hardware-Last Barrier Status

Satisfied pre-hardware rows:

- QEMU-TCG deterministic proxy perf tier implemented and runnable.
- Proxy structural counters have committed baseline evidence.
- Proxy drift is a test-runner blocker.
- Exact F-004 `smp_concurrent_reap_after_terminate` exists and passes at `NEO_SMP=4`.
- VMware middle tier is documented with explicit PM deferral for execution evidence.
- OptiPlex 7080 procedure exists.
- Accepted subphase 1-3 QEMU gates remain passing.
- Full `NEO_SMP=4` non-ignored `cargo neo-test all --no-meta` passes.
- Current canonical successful serial-log scan has no unexpected fail/panic/BUG/leak marker.

Remaining hardware-only rows:

- Build final boot media for the hardware session.
- Execute OptiPlex 7080 boot smoke.
- Execute targeted scenario order at `>=4` logical CPUs.
- Execute targeted scenario order at full logical CPU count.
- Capture required bare-metal perf data.
- Run hardware-session serial-log scan.
- File hardware verification report and PM ingestion disposition.

## 9. Caveats

- Hardware evidence is still absent by design.
- VMware evidence was not executed; the repo has procedure coverage, not an executable VMware harness.
- QEMU-TCG proxy counters are structural regression guards only. They are not hardware performance claims.
- The pre-existing untracked `ai_digest.py` was not touched.

## 10. PM Recommendation

Accept subphase 4 pre-hardware work as complete. Keep Phase 2 formally open only for the hardware-last rows listed in section 8.

---

**End of close report.**
