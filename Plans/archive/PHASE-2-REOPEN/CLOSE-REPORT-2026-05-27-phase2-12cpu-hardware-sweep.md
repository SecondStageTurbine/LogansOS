# CLOSE REPORT: Phase 2 12-CPU bare-metal hardware sweep

Date: 2026-05-27
From: PM
Status: PASS - 43/43 accepted
Hardware: Dell OptiPlex 7080 full-count 12-CPU path
Serial capture: COM3 / 38400 / 8N1
Final `origin/main`: `557c2936051c2ab65b023c522f01418f159bca2a`

## 1. Disposition

The Phase 2 12-CPU bare-metal hardware sweep is accepted as PASS.

All 43 required scenario boots produced accepted terminal PASS evidence. The final accepted run was:

```text
NEO_TEST/1 event=PASS name=irqsafe_mutex_bench spin_mean_ns=103 irqsafe_mean_ns=675 switch_mean_ns=335 overhead_ns=572 threshold_ns=6000
```

No accepted terminal evidence contains a same-scenario `event=FAIL` after the accepted PASS marker.

## 2. Scope

This close report covers the OptiPlex 7080 full-count 12-CPU hardware lane. The 4-CPU ambiguity is not a blocker for this close report per:

```text
Plans/PM-DISPOSITION-2026-05-25-phase2-hardware-12cpu-satisfies-minimum-tier.md
```

## 3. Defects Found During Sweep

Three blockers were found and resolved during the hardware run:

| Blocker | Root cause | Fix commit | Accepted rerun |
|---|---|---:|---|
| `q7c_concurrent_munmap_smp` PASS-then-FAIL | Worker CPU reached PASS, but bare metal did not machine-exit from QEMU debug port; BSP later emitted fallback FAIL | `4ae4b0184071b5b8dff03836344e7a4f9184a78c` | `Plans/evidence/phase2-bare-metal-optiplex-7080-20260525-222500-12-q7c_concurrent_munmap_smp-rerun.serial.log` |
| `q7c_missing_shootdown_detection` PASS-then-FAIL | Same worker-terminal handoff class on the negative detector path | `87b1345639a521bd6a4f8bcabacaa91d8d64a60c` | `Plans/evidence/phase2-bare-metal-optiplex-7080-20260526-191754-12-q7c_missing_shootdown_detection-rerun.serial.log` |
| `q8d_phase_a_coverage` TLB shootdown timeout | TLB shootdown target mask used scheduler-online state before APs were guaranteed shootdown-ready | `557c2936051c2ab65b023c522f01418f159bca2a` | `Plans/evidence/phase2-bare-metal-optiplex-7080-20260526-212947-12-q8d_phase_a_coverage-rerun.serial.log` |

The q8d blocker was a real kernel-correctness issue. The committed fix separated per-CPU TLB-shootdown readiness from scheduler routing readiness. QEMU regression for `q7c_shootdown_timeout_panic` remained PASS after the fix.

## 4. Evidence Caveat

The accepted `q7c_process_exit_during_thread_run` log was later reused accidentally by PuTTY for the first `q7c_missing_shootdown_detection` failed boot. The accepted q7c process-exit marker is still valid as that scenario's terminal marker at the time it was run:

```text
Plans/evidence/phase2-bare-metal-optiplex-7080-20260526-184512-12-q7c_process_exit_during_thread_run.serial.log
```

The appended q7c missing-shootdown FAIL is tracked separately in:

```text
Plans/BLOCKER-2026-05-26-phase2-hardware-q7c-missing-shootdown-detection.md
```

and resolved by the accepted rerun listed above.

## 5. Accepted Evidence Matrix

| # | Scenario | Accepted evidence |
|---:|---|---|
| 1 | `smp_per_cpu_scheduler_smoke` | `Plans/evidence/phase2-bare-metal-optiplex-7080-20260525-195901-12-smp_per_cpu_scheduler_smoke.serial.log` |
| 2 | `smp_same_cpu_ipc` | `Plans/evidence/phase2-bare-metal-optiplex-7080-20260525-201500-12-smp_same_cpu_ipc.serial.log` |
| 3 | `smp_block_release_smoke` | `Plans/evidence/phase2-bare-metal-optiplex-7080-20260525-202500-12-smp_block_release_smoke.serial.log` |
| 4 | `smp_send_blocks_first_then_completes` | `Plans/evidence/phase2-bare-metal-optiplex-7080-20260525-203500-12-smp_send_blocks_first_then_completes.serial.log` |
| 5 | `smp_recv_blocks_first_then_completes` | `Plans/evidence/phase2-bare-metal-optiplex-7080-20260525-204500-12-smp_recv_blocks_first_then_completes.serial.log` |
| 6 | `smp_grant_cross_cpu` | `Plans/evidence/phase2-bare-metal-optiplex-7080-20260525-210000-12-smp_grant_cross_cpu.serial.log` |
| 7 | `smp_cross_cpu_migration_reap` | `Plans/evidence/phase2-bare-metal-optiplex-7080-20260525-211000-12-smp_cross_cpu_migration_reap.serial.log` |
| 8 | `smp_cross_cpu_rendezvous_stress` | `Plans/evidence/phase2-bare-metal-optiplex-7080-20260525-212000-12-smp_cross_cpu_rendezvous_stress.serial.log` |
| 9 | `smp_concurrent_reap_after_terminate` | `Plans/evidence/phase2-bare-metal-optiplex-7080-20260525-213000-12-smp_concurrent_reap_after_terminate.serial.log` |
| 10 | `q7b_cr3_cache_skip_count` | `Plans/evidence/phase2-bare-metal-optiplex-7080-20260525-214000-12-q7b_cr3_cache_skip_count.serial.log` |
| 11 | `phase2_perf_proxy_baseline` | `Plans/evidence/phase2-bare-metal-optiplex-7080-20260525-215000-12-phase2_perf_proxy_baseline.serial.log` |
| 12 | `q7c_concurrent_munmap_smp` | `Plans/evidence/phase2-bare-metal-optiplex-7080-20260525-222500-12-q7c_concurrent_munmap_smp-rerun.serial.log` |
| 13 | `q7c_process_exit_during_thread_run` | `Plans/evidence/phase2-bare-metal-optiplex-7080-20260526-184512-12-q7c_process_exit_during_thread_run.serial.log` |
| 14 | `q7c_missing_shootdown_detection` | `Plans/evidence/phase2-bare-metal-optiplex-7080-20260526-191754-12-q7c_missing_shootdown_detection-rerun.serial.log` |
| 15 | `q7c_shootdown_timeout_panic` | `Plans/evidence/phase2-bare-metal-optiplex-7080-20260526-192502-12-q7c_shootdown_timeout_panic.serial.log` |
| 16 | `ipc_rendezvous_translate_fault` | `Plans/evidence/phase2-bare-metal-optiplex-7080-20260526-193029-12-ipc_rendezvous_translate_fault.serial.log` |
| 17 | `q8a_concurrent_install_revoke` | `Plans/evidence/phase2-bare-metal-optiplex-7080-20260526-200504-12-q8a_concurrent_install_revoke.serial.log` |
| 18 | `q8b_grant_vs_exit_race` | `Plans/evidence/phase2-bare-metal-optiplex-7080-20260526-201355-12-q8b_grant_vs_exit_race.serial.log` |
| 19 | `q8b_exit_state_cas` | `Plans/evidence/phase2-bare-metal-optiplex-7080-20260526-202409-12-q8b_exit_state_cas.serial.log` |
| 20 | `q8c_blocked_kill_walk` | `Plans/evidence/phase2-bare-metal-optiplex-7080-20260526-202938-12-q8c_blocked_kill_walk.serial.log` |
| 21 | `q8c_close_peer_my_side_extract` | `Plans/evidence/phase2-bare-metal-optiplex-7080-20260526-204450-12-q8c_close_peer_my_side_extract.serial.log` |
| 22 | `q8d_phase_a_coverage` | `Plans/evidence/phase2-bare-metal-optiplex-7080-20260526-212947-12-q8d_phase_a_coverage-rerun.serial.log` |
| 23 | `cap_mint_and_use` | `Plans/evidence/phase2-bare-metal-optiplex-7080-20260526-213757-12-cap_mint_and_use.serial.log` |
| 24 | `cap_derive_subset` | `Plans/evidence/phase2-bare-metal-optiplex-7080-20260526-214542-12-cap_derive_subset.serial.log` |
| 25 | `cap_derive_rights_violation` | `Plans/evidence/phase2-bare-metal-optiplex-7080-20260526-215518-12-cap_derive_rights_violation.serial.log` |
| 26 | `cap_derive_missing_meta` | `Plans/evidence/phase2-bare-metal-optiplex-7080-20260526-220103-12-cap_derive_missing_meta.serial.log` |
| 27 | `cap_grant_basic` | `Plans/evidence/phase2-bare-metal-optiplex-7080-20260526-220754-12-cap_grant_basic.serial.log` |
| 28 | `cap_grant_missing_meta` | `Plans/evidence/phase2-bare-metal-optiplex-7080-20260526-222210-12-cap_grant_missing_meta.serial.log` |
| 29 | `cap_revoke_local` | `Plans/evidence/phase2-bare-metal-optiplex-7080-20260526-223214-12-cap_revoke_local.serial.log` |
| 30 | `cap_revoke_cross_process` | `Plans/evidence/phase2-bare-metal-optiplex-7080-20260526-224012-12-cap_revoke_cross_process.serial.log` |
| 31 | `cap_revoke_descendants_walk` | `Plans/evidence/phase2-bare-metal-optiplex-7080-20260526-224734-12-cap_revoke_descendants_walk.serial.log` |
| 32 | `cap_revoke_spawn_inheritance_descendants` | `Plans/evidence/phase2-bare-metal-optiplex-7080-20260526-225915-12-cap_revoke_spawn_inheritance_descendants.serial.log` |
| 33 | `cap_survive_exit` | `Plans/evidence/phase2-bare-metal-optiplex-7080-20260526-230522-12-cap_survive_exit.serial.log` |
| 34 | `sys_wait_blocking` | `Plans/evidence/phase2-bare-metal-optiplex-7080-20260526-231120-12-sys_wait_blocking.serial.log` |
| 35 | `sys_wait_wnohang` | `Plans/evidence/phase2-bare-metal-optiplex-7080-20260526-232255-12-sys_wait_wnohang.serial.log` |
| 36 | `sys_wait_already_exited` | `Plans/evidence/phase2-bare-metal-optiplex-7080-20260527-171653-12-sys_wait_already_exited.serial.log` |
| 37 | `sys_wait_multiple_waiters` | `Plans/evidence/phase2-bare-metal-optiplex-7080-20260527-172413-12-sys_wait_multiple_waiters.serial.log` |
| 38 | `sys_inspect_running` | `Plans/evidence/phase2-bare-metal-optiplex-7080-20260527-172929-12-sys_inspect_running.serial.log` |
| 39 | `sys_inspect_exited` | `Plans/evidence/phase2-bare-metal-optiplex-7080-20260527-173605-12-sys_inspect_exited.serial.log` |
| 40 | `sys_inspect_rights_gate` | `Plans/evidence/phase2-bare-metal-optiplex-7080-20260527-184210-12-sys_inspect_rights_gate.serial.log` |
| 41 | `sched_ready_queue_capacity_pressure` | `Plans/evidence/phase2-bare-metal-optiplex-7080-20260527-184825-12-sched_ready_queue_capacity_pressure.serial.log` |
| 42 | `irqsafe_mutex_if_restore` | `Plans/evidence/phase2-bare-metal-optiplex-7080-20260527-190152-12-irqsafe_mutex_if_restore.serial.log` |
| 43 | `irqsafe_mutex_bench` | `Plans/evidence/phase2-bare-metal-optiplex-7080-20260527-191028-12-irqsafe_mutex_bench.serial.log` |

## 6. Final PM Judgment

The OptiPlex 7080 12-CPU hardware lane validates the Phase 2 SMP, TLB shootdown, process/capability, wait/inspect, scheduler-capacity, and IRQ-safe mutex scenarios listed above.

This close report does not commit or publish artifacts. It records local PM acceptance of the completed hardware sweep and identifies the evidence files required for a later publication/commit pass.
