# DISPATCH: Phase 2 final hardware session - OptiPlex 7080 verification

- From: PM
- To: Hardware execution lane
- Date: 2026-05-25
- Status: READY WHEN PHYSICAL ACCESS IS PRESENT
- Prior gate: `Plans/PM-DISPOSITION-2026-05-25-phase-2-build-subphase-4-close.md`
- Procedure: `Plans/PROCEDURE-2026-05-25-phase-2-bare-metal-optiplex-7080-verification.md`

---

## 1. Mission

Execute the final Phase 2 hardware-only remainder on the Dell OptiPlex 7080:

- boot smoke on physical hardware
- targeted Phase 2 scenario pass at `>=4` logical CPUs
- targeted Phase 2 scenario pass at full logical CPU count
- hardware-tier perf captures
- full serial evidence capture
- hardware verification report

Do not substitute QEMU, VMware, theory, or prior logs for this dispatch. If physical access to the OptiPlex and capture setup is not available, stop and report `hardware_unavailable`.

## 2. Authoritative Inputs

Read these before execution:

- `Plans/PM-DISPOSITION-2026-05-25-phase-2-build-subphase-4-close.md`
- `Plans/PROCEDURE-2026-05-25-phase-2-bare-metal-optiplex-7080-verification.md`
- `Plans/CLOSE-REPORT-2026-05-25-phase-2-build-subphase-4-formal-close-pre-hardware.md`
- `Plans/PM-NOTE-2026-05-25-phase-2-hardware-last-execution-barrier.md`
- `Plans/PL-RATIFICATION-2026-05-25-phase-2-plan.md`

Use live source as truth. Record the exact commit used for the hardware session.

## 3. Preconditions

Before touching boot media:

```powershell
cd 'E:\AI Automation\AltimitNeo'
git status --short --branch
git rev-parse HEAD
git log --oneline -5
```

Required state:

- checkout is on `main`
- `origin/main` is at or after `39a804815019a740238e821ecd69040746a6ef87`
- no uncommitted source changes are present except explicitly documented local-only evidence files

If the tree is dirty with source changes, stop and classify them before building media.

## 4. Build And Media

Run:

```powershell
docker compose run --rm neo cargo k
docker compose run --rm neo cargo iso
docker compose run --rm neo xorriso -indev /workspace/boot/neoplutarch.iso -report_el_torito plain
```

Record:

- build result
- ISO path
- ISO hash
- El Torito/UEFI report
- media write method
- confirmation that the target media exposes `EFI/BOOT/BOOTX64.EFI`

For Phase 2 scenario media, build one ISO per scenario:

```powershell
docker compose run --rm neo cargo neo-test --build-only <scenario>
```

Use `target/neo-test/vmware/<scenario>/<scenario>.iso` as the physical boot ISO for that scenario and record its SHA-256 hash. Do not use `--bare-metal-mode` for the Phase 2 scenario set; current tooling reserves that mode for Phase 3 AHCI/NVMe storage exemplars.

Secure Boot must be disabled for the unsigned Limine path. If Secure Boot cannot be disabled, stop and report the firmware state; do not treat a Secure Boot rejection as a kernel failure.

## 5. Required Hardware Runs

Execute the procedure in this order:

1. Production ISO boot smoke.
2. Test-harness targeted pass at `>=4` logical CPUs.
3. Test-harness targeted pass at full logical CPU count.
4. Full non-ignored test-harness sweep if the procedure can run it within the hardware window.
5. Hardware perf captures:
   - boot time
   - syscall latency smoke measurement
   - context-switch latency or count-derived scheduler timing
   - scheduling fairness evidence
   - `irqsafe_mutex_bench`

## 6. Required Scenario Set

At both `>=4` logical CPUs and full logical CPU count, cover:

```text
smp_per_cpu_scheduler_smoke
smp_same_cpu_ipc
smp_block_release_smoke
smp_send_blocks_first_then_completes
smp_recv_blocks_first_then_completes
smp_grant_cross_cpu
smp_cross_cpu_migration_reap
smp_cross_cpu_rendezvous_stress
smp_concurrent_reap_after_terminate
q7b_cr3_cache_skip_count
phase2_perf_proxy_baseline
q7c_concurrent_munmap_smp
q7c_process_exit_during_thread_run
q7c_missing_shootdown_detection
q7c_shootdown_timeout_panic
ipc_rendezvous_translate_fault
q8a_concurrent_install_revoke
q8b_grant_vs_exit_race
q8b_exit_state_cas
q8c_blocked_kill_walk
q8c_close_peer_my_side_extract
q8d_phase_a_coverage
cap_mint_and_use
cap_derive_subset
cap_derive_rights_violation
cap_derive_missing_meta
cap_grant_basic
cap_grant_missing_meta
cap_revoke_local
cap_revoke_cross_process
cap_revoke_descendants_walk
cap_revoke_spawn_inheritance_descendants
cap_survive_exit
sys_wait_blocking
sys_wait_wnohang
sys_wait_already_exited
sys_wait_multiple_waiters
sys_inspect_running
sys_inspect_exited
sys_inspect_rights_gate
sched_ready_queue_capacity_pressure
irqsafe_mutex_if_restore
irqsafe_mutex_bench
```

Expected-panic exception:

- `q7c_shootdown_timeout_panic` may show `KERNEL PANIC` only if the same scenario emits `event=PASS expected_panic=tlb_shootdown_timeout`.

## 7. Evidence Capture

Preferred path pattern:

```text
Plans/evidence/phase2-bare-metal-optiplex-7080-YYYYMMDD-HHMMSS-<cpu-count>-<scenario-or-sweep>.serial.log
```

Every run must preserve:

- full serial log
- firmware mode and Secure Boot state
- CPU count used
- source commit
- ISO hash
- PASS/FAIL table
- raw perf lines
- frame-audit result

## 8. Stop Conditions

Stop and file blocker evidence on:

- unexpected `KERNEL PANIC`
- unexpected `BUG:`
- frame leak or dirty frame audit
- GS-base sentinel mismatch
- AP bring-up failure
- boot hang after serial progress
- media not visible as UEFI boot target after layout verification
- scenario `event=FAIL` in a canonical final log

If hardware access is missing, do not improvise with emulators. Report `hardware_unavailable`.

## 9. Required Report

After execution, file:

```text
Plans/PHASE-2-BARE-METAL-OPTIPLEX-7080-VERIFICATION-YYYY-MM-DD.md
```

The report must include:

- source commit and ISO hash
- hardware/firmware details
- media layout evidence
- serial-log paths
- targeted scenario table for `>=4` CPUs
- targeted scenario table for full logical CPU count
- full sweep verdict, if run
- hardware perf captures
- serial-log scan output
- expected-panic exception handling
- conclusion: `PASS`, `PASS-WITH-INCIDENTAL`, or `FAIL`

## 10. Tell-Worker Text

Phase 2 pre-hardware work is accepted. Execute the final OptiPlex 7080 hardware verification per `Plans/DISPATCH-2026-05-25-phase-2-hardware-optiplex-7080-verification.md` and `Plans/PROCEDURE-2026-05-25-phase-2-bare-metal-optiplex-7080-verification.md`. This is a real-hardware-only dispatch: do not substitute QEMU, VMware, or theory. If physical access or serial capture is unavailable, stop and report `hardware_unavailable`. Preserve full serial evidence, exact source commit, ISO hash, firmware/Secure Boot state, CPU counts, scenario tables, perf captures, and scan output.

---

**End of dispatch.**
