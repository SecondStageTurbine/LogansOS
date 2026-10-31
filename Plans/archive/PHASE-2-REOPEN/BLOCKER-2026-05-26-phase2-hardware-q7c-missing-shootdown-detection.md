# BLOCKER: q7c_missing_shootdown_detection hardware PASS-then-FAIL

Date: 2026-05-26
From: PM
Status: RESOLVED - accepted rerun evidence recorded
Scenario: `q7c_missing_shootdown_detection`
Hardware lane: OptiPlex 7080 full-count 12-CPU path

## 1. Evidence

Expected scenario log path was:

`Plans/evidence/phase2-bare-metal-optiplex-7080-20260526-185351-12-q7c_missing_shootdown_detection.serial.log`

That exact file was not created. PuTTY appears to have appended the run to the prior scenario log:

`Plans/evidence/phase2-bare-metal-optiplex-7080-20260526-184512-12-q7c_process_exit_during_thread_run.serial.log`

Relevant terminal markers in the appended log:

```text
NEO_TEST/1 event=BEGIN name=q7c_missing_shootdown_detection phase=dispatch
NEO_TEST/1 event=PASS name=q7c_missing_shootdown_detection detected=stale_tlb_read_after_unmap negative_feature=neo-test-missing-shootdown-negative value=0xa7
NEO_TEST/1 event=FAIL name=q7c_missing_shootdown_detection reason=negative_reader_no_terminal_after_unmap
```

PM disposition: this run is **not accepted as PASS** because the final terminal marker is `event=FAIL`.

## 2. Diagnosis

The first PASS line proves the intended negative detector fired: with the `neo-test-missing-shootdown-negative` feature active, the reader CPU retained a stale TLB mapping after the BSP unmapped the VA and read the original marker value `0xa7`.

The later FAIL is a harness terminal-semantics artifact, matching the previously fixed `q7c_concurrent_munmap_smp` class:

- `q7c_missing_shootdown_reader()` prints the PASS marker on the worker CPU after detecting the stale read.
- It then calls `debug_exit(EXIT_SUCCESS)`.
- In QEMU, the debug-exit port terminates the whole VM, so the PASS is terminal.
- On bare metal, the debug-exit port write returns and `debug_exit()` halts only the worker CPU.
- The BSP continues its fixed post-unmap loop and falls through to `fail("reason=negative_reader_no_terminal_after_unmap")`.

Relevant source locations at current HEAD `4ae4b0184071b5b8dff03836344e7a4f9184a78c`:

- `kernel/src/test_harness.rs:3435-3460` - reader prints PASS and calls `debug_exit(EXIT_SUCCESS)`.
- `kernel/src/test_harness.rs:3507-3511` - BSP waits for no terminal signal, then emits fallback FAIL.
- `kernel/src/test_harness.rs:11695-11714` - `write_debug_exit_port()` and `debug_exit()` semantics; QEMU exits, bare metal returns then halts the calling CPU.

## 3. Kernel-Correctness Verdict

The mechanism under test is a negative detector, not production shootdown correctness. The PASS line means the test successfully detected the deliberately introduced stale-TLB failure mode:

```text
detected=stale_tlb_read_after_unmap negative_feature=neo-test-missing-shootdown-negative value=0xa7
```

The kernel production path is not implicated by this negative-feature scenario. The hardware sweep remains blocked only because the harness emits a later FAIL on bare metal.

## 4. Required Fix Direction

Dispatch Jarvis for a narrow harness-only fix analogous to the q7c munmap reader handoff already committed in `4ae4b0184071b5b8dff03836344e7a4f9184a78c`:

1. Add a q7c-missing-shootdown terminal stage, gated to `neo-test-q7c-missing-shootdown-detection`.
2. In `q7c_missing_shootdown_reader()`, replace the direct `debug_exit(EXIT_SUCCESS)` tail with:
   - scenario PASS phase/frame emission,
   - log flush,
   - `write_debug_exit_port(EXIT_SUCCESS)`,
   - stage store indicating the reader reached the PASS verdict,
   - `halt_forever()`.
3. In the BSP post-unmap wait loop, check that stage and call `debug_exit(EXIT_SUCCESS)` without printing a second PASS/FAIL marker.
4. Preserve QEMU behavior: QEMU must still exit from the reader's debug-exit port write before the BSP convergence path runs.

Expected rerun acceptance after fix:

```text
NEO_TEST/1 event=PASS name=q7c_missing_shootdown_detection detected=stale_tlb_read_after_unmap negative_feature=neo-test-missing-shootdown-negative value=0xa7
```

with no later `event=FAIL`.

## 5. PM Gate

Do not proceed to `q7c_shootdown_timeout_panic` until this scenario has:

- committed fix on `origin/main`;
- QEMU gate PASS for `q7c_missing_shootdown_detection`;
- rebuilt ISO SHA256 recorded;
- clean OptiPlex 7080 12-CPU rerun evidence with no later `event=FAIL`.

## 6. Resolution

Status: RESOLVED 2026-05-26.

Jarvis implemented and pushed the narrow harness-only terminal handoff fix:

```text
87b1345639a521bd6a4f8bcabacaa91d8d64a60c
```

Reported validation:

- `cargo k`: PASS, exit 0.
- `NEO_SMP=4 cargo neo-test q7c_missing_shootdown_detection`: PASS, exit 0; serial ended at expected PASS marker with no later `event=FAIL`.

Rebuilt ISO:

```text
E:\AI Automation\AltimitNeo\target\neo-test\vmware\q7c_missing_shootdown_detection\q7c_missing_shootdown_detection.iso
SHA256 F37C8B81C5945D9FF4012C1BE11B57D0DAD4212FC22597AE53D8ED4854ED293F
```

Accepted hardware rerun evidence:

```text
Plans/evidence/phase2-bare-metal-optiplex-7080-20260526-191754-12-q7c_missing_shootdown_detection-rerun.serial.log
```

Observed terminal marker:

```text
NEO_TEST/1 event=PASS name=q7c_missing_shootdown_detection detected=stale_tlb_read_after_unmap negative_feature=neo-test-missing-shootdown-negative value=0xa7
```

PM scan found no later `event=FAIL` in the rerun log. The blocker is closed and the hardware sweep may proceed to `q7c_shootdown_timeout_panic`.
