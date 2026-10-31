# BLOCKER: q8d_phase_a_coverage hardware TLB shootdown timeout

Date: 2026-05-26
From: PM
Status: RESOLVED - accepted rerun evidence recorded
Scenario: `q8d_phase_a_coverage`
Hardware lane: OptiPlex 7080 full-count 12-CPU path
Commit under test: `87b1345639a521bd6a4f8bcabacaa91d8d64a60c`

## 1. Evidence

Log:

```text
Plans/evidence/phase2-bare-metal-optiplex-7080-20260526-205055-12-q8d_phase_a_coverage.serial.log
```

Build artifact:

```text
E:\AI Automation\AltimitNeo\target\neo-test\vmware\q8d_phase_a_coverage\q8d_phase_a_coverage.iso
SHA256 78DB00EA3AB211F2D581F0AF18D7AEA291BB8FDEE943995903E431C8C3BD4FE1
```

Observed terminal sequence:

```text
NEO_TEST/1 event=BEGIN name=q8d_phase_a_coverage phase=dispatch
[ kernel ] sched: reaped 'q8d-phase-a-worker' (id=14)
!!  KERNEL PANIC  !!
panicked at kernel/src/mm/tlb_shootdown.rs:236:13:
tlb_shootdown_timeout pending=2 targets=0x0000000000000ffe
NEO_TEST/1 event=FAIL name=q8d_phase_a_coverage reason=kernel_panic
```

PM disposition: this is **not accepted as PASS**. The final terminal marker is `event=FAIL`, and the failure is a real panic, not a terminal-marker race.

## 2. What Failed

The q8d worker appears to run and exit far enough to be reaped:

```text
[ kernel ] sched: reaped 'q8d-phase-a-worker' (id=14)
```

The failure happens after that reap and before q8d can print its expected PASS marker:

```text
NEO_TEST/1 event=PASS name=q8d_phase_a_coverage mechanism=thread_pmap_phase_a cr3_source=thread_pmap registry_hot_path=retired
```

Source path at current HEAD:

- `kernel/src/test_harness.rs:2327-2335` - worker checks that loaded CR3 matches `Thread.pmap`, sets `Q8D_STAGE=1`, then exits.
- `kernel/src/test_harness.rs:2363-2379` - BSP waits for worker, calls `reap()`, then expects to print PASS.
- `kernel/src/sched/scheduler.rs:1277-1286` - `reap()` frees the zombie and calls `process::on_thread_reap(pid)`.
- `kernel/src/sched/process.rs:1099-1102` - natural process cleanup drops the process Arc and its Pmap before frame audit.
- `kernel/src/mm/pmap.rs:447-451` - `Pmap::Drop` performs full teardown shootdown before freeing page-table frames.
- `kernel/src/mm/tlb_shootdown.rs:227-240` - shootdown wait loop panics on timeout.

## 3. Working Diagnosis

This looks like a boot/online-readiness race exposed by the 12-CPU hardware path.

In the failing log, AP readiness messages are still interleaving after the scenario begins and after q8d reaps its worker:

```text
[ kernel ] sched: smoke test passed (5 ping, 5 pong, hog reaped, ticks=28)
[ kernel ] sched: AP per-CPU scheduler ready (cpu=9, bootstrap='ap-main', ready_queue empty)
[ kernel ] smp: AP cpu_id=8 ready (lapic_id=5, processor_id=9)
NEO_TEST/1 event=BEGIN name=q8d_phase_a_coverage phase=dispatch
[ kernel ] smp: AP cpu_id=11 ready (lapic_id=11, processor_id=12)
[ kernel ] sched: reaped 'q8d-phase-a-worker' (id=14)
[ kernel ] sched: AP per-CPU scheduler ready (cpu=8, bootstrap='ap-main', ready_queue empty)
[ kernel ] sched: AP per-CPU scheduler ready (cpu=11, bootstrap='ap-main', ready_queue empty)
```

The timeout target mask was:

```text
targets=0x0000000000000ffe
```

That mask targets CPUs 1 through 11. The panic reports:

```text
pending=2
```

The two pending acknowledgements line up with the two APs whose scheduler-ready lines are still appearing around the q8d cleanup window.

The likely structural issue:

- `tlb_shootdown::online_cpus_mask()` uses `PER_CPU[cpu].scheduler_ptr != null` as the definition of online.
- `sched::init_cpu_scheduler()` publishes `scheduler_ptr` before it prints scheduler-ready.
- `sched::run_ap_bootstrap()` enables interrupts only after `init_cpu_scheduler()` returns.
- Therefore an AP can become visible to `online_cpus_mask()` before it can reliably service the TLB shootdown IPI.

On QEMU/4-CPU and on some hardware timings, the AP reaches interrupt-enabled state fast enough. On this 12-CPU hardware run, q8d's Pmap teardown shootdown caught two APs in the gap and timed out.

## 4. Kernel-Correctness Verdict

This is not a q8d invariant failure as such. The worker reached reap, so the Phase A `Thread.pmap` path likely executed. The failure is that Pmap teardown's production TLB shootdown can target APs that are not yet shootdown-ready during early SMP bring-up.

That is kernel-correctness relevant. Unlike the prior q7c terminal-semantics blockers, this is a real panic on a production cleanup path and must be fixed before continuing the hardware sweep.

## 5. Required Fix Direction

Dispatch Jarvis for a narrow structural fix. Do not paper over this by relaxing the timeout only.

Required properties:

1. A CPU must not be included in the TLB shootdown target mask until it can service TLB shootdown IPIs.
2. The publication point for "shootdown-ready" must be after the AP has IDT/LAPIC/per-CPU/slab/scheduler state and interrupts enabled.
3. Existing scheduler routing semantics must not be broken; if `scheduler_ptr` remains the scheduler-online marker, add a separate shootdown-ready marker or boot barrier rather than overloading it.
4. Preserve the expected-panic scenario `q7c_shootdown_timeout_panic`; the timeout panic must still occur when a shootdown target is deliberately made non-responsive by the test.
5. Re-run the relevant QEMU gates and rebuild the q8d ISO before hardware rerun.

Suggested implementation shapes for Jarvis to evaluate:

- Add a per-CPU `shootdown_ready` flag set at the end of `run_ap_bootstrap()` after `enable_interrupts()`, and make TLB shootdown use a readiness mask rather than `scheduler_ptr`.
- Or add a BSP boot barrier after AP bring-up that waits for APs to reach shootdown-ready before the scheduler smoke/scenario path can begin.

The first option is narrower and matches the invariant directly.

## 6. PM Gate

Do not proceed to capability scenarios until `q8d_phase_a_coverage` has:

- committed fix on `origin/main`;
- `cargo k` PASS;
- QEMU gate PASS for `q8d_phase_a_coverage`;
- regression PASS for `q7c_shootdown_timeout_panic`;
- rebuilt q8d ISO SHA256 recorded;
- clean OptiPlex 7080 12-CPU rerun evidence with the expected PASS marker and no later `event=FAIL`.

## 7. Fix Report

Jarvis implemented and pushed the narrow structural fix:

```text
557c2936051c2ab65b023c522f01418f159bca2a
```

Fix shape:

- Added a separate per-CPU `tlb_shootdown_ready` marker.
- TLB shootdown masks now use the shootdown-ready marker instead of `scheduler_ptr`.
- Scheduler routing semantics remain unchanged.

Reported validation:

- `cargo k`: PASS, exit 0.
- `q8d_phase_a_coverage` QEMU gate: PASS, exit 0.
- `q7c_shootdown_timeout_panic` QEMU regression: PASS, exit 0.

Rebuilt ISO:

```text
E:\AI Automation\AltimitNeo\target\neo-test\vmware\q8d_phase_a_coverage\q8d_phase_a_coverage.iso
SHA256 1757332F5866237F56AD9E2F2824A57564924523AEBA2F8E38D95F473727670A
```

Hardware rerun pending. Required accepted marker:

```text
NEO_TEST/1 event=PASS name=q8d_phase_a_coverage mechanism=thread_pmap_phase_a cr3_source=thread_pmap registry_hot_path=retired
```

with no later `event=FAIL`.

## 8. Resolution

Status: RESOLVED 2026-05-26.

Accepted hardware rerun evidence:

```text
Plans/evidence/phase2-bare-metal-optiplex-7080-20260526-212947-12-q8d_phase_a_coverage-rerun.serial.log
```

Observed terminal sequence:

```text
NEO_TEST/1 event=BEGIN name=q8d_phase_a_coverage phase=dispatch
[ kernel ] sched: reaped 'q8d-phase-a-worker' (id=15)
[ kernel ] pmap: dropped - freed 0 frames + 1 PML4
[ kernel ] proc: deregistered 'q8d-phase-a' (pid=1)
NEO_TEST/1 event=PASS name=q8d_phase_a_coverage mechanism=thread_pmap_phase_a cr3_source=thread_pmap registry_hot_path=retired
```

PM scan found no later `event=FAIL` and no `tlb_shootdown_timeout` in the rerun log. The blocker is closed and the hardware sweep may proceed to capability scenarios.
