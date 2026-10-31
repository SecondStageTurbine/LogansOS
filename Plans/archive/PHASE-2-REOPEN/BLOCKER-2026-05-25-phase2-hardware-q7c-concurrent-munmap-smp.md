# BLOCKER: Phase 2 hardware q7c_concurrent_munmap_smp emits PASS then FAIL

- **Date:** 2026-05-25
- **Author:** PM
- **Scenario:** `q7c_concurrent_munmap_smp`
- **CPU tier:** OptiPlex 7080 full-count, 12 logical CPUs
- **ISO:** `target/neo-test/vmware/q7c_concurrent_munmap_smp/q7c_concurrent_munmap_smp.iso`
- **ISO SHA256:** `2CDCB41845E91A4DCE4669D16027214AAD017E1A5D738093738586E20CCA01B3`
- **Evidence:** `Plans/evidence/phase2-bare-metal-optiplex-7080-20260525-220000-12-q7c_concurrent_munmap_smp.serial.log`
- **Status:** RESOLVED 2026-05-25 - accepted rerun evidence recorded

---

## 1. Observed Result

The log contains both an expected PASS marker and a later FAIL marker:

```text
NEO_TEST/1 event=PASS name=q7c_concurrent_munmap_smp trap=page_fault mechanism=q7c_munmap_shootdown
NEO_TEST/1 event=FAIL name=q7c_concurrent_munmap_smp reason=reader_no_terminal_after_unmap
```

The scenario cannot be accepted as PASS because the final terminal marker is FAIL.

Other relevant evidence:

```text
[ kernel ] smp: Limine reports 12 cpu(s); bsp_lapic_id=0
[ kernel ] smp: started 11 AP(s)
```

No unexpected raw `PAGE_FAULT`, `KERNEL PANIC`, or `BUG:` line was observed in the PM scan. The PASS marker itself represents the scenario's expected page fault path.

---

## 2. PM Initial Diagnosis

This appears to be a hardware test-harness terminal-semantics issue, not immediate proof of a q7c kernel correctness regression.

Source path:

- `kernel/src/test_harness.rs::run_q7c_concurrent_munmap_smp()`
- `kernel/src/test_harness.rs::q7c_concurrent_munmap_reader()`
- `kernel/src/test_harness.rs::handle_expected_page_fault_for_test()`
- `kernel/src/test_harness.rs::debug_exit()`

Relevant current behavior:

1. BSP starts the q7c scenario.
2. Reader is placed on CPU 1.
3. BSP unmaps the user VA and sets `Q7C_STAGE_UNMAPPED`.
4. Reader faults as expected when reading the unmapped VA.
5. Test page-fault handler sees `EXPECT_Q7C_MUNMAP_PF` and calls:

```rust
pass("trap=page_fault mechanism=q7c_munmap_shootdown");
```

6. `pass()` calls `debug_exit(EXIT_SUCCESS)`.
7. `debug_exit()` writes the QEMU debug-exit port, flushes logs, and then halts forever.

On QEMU, the debug-exit port exits the VM, so the PASS marker is terminal.

On bare metal, the debug-exit port is not a machine-wide exit mechanism. The reader CPU appears to halt, but the BSP continues its post-unmap loop and then emits:

```text
NEO_TEST/1 event=FAIL name=q7c_concurrent_munmap_smp reason=reader_no_terminal_after_unmap
```

This explains the observed "PASS then FAIL" screen behavior.

---

## 3. Stop Condition

Per hardware procedure, `event=FAIL` is a stop condition. The Phase 2 hardware sweep is paused here.

Do not continue to `q7c_process_exit_during_thread_run` until Jarvis either:

- fixes the hardware terminal semantics for this scenario and provides a rebuilt ISO; or
- files a PM-acceptable disposition explaining why this scenario should be classified as harness-only and how to record it without weakening the evidence standard.

---

## 4. Jarvis Dispatch

Jarvis - hardware sweep blocker. `q7c_concurrent_munmap_smp` on the OptiPlex 7080 12-CPU tier emitted PASS then FAIL:

```text
NEO_TEST/1 event=PASS name=q7c_concurrent_munmap_smp trap=page_fault mechanism=q7c_munmap_shootdown
NEO_TEST/1 event=FAIL name=q7c_concurrent_munmap_smp reason=reader_no_terminal_after_unmap
```

Evidence:

```text
Plans/evidence/phase2-bare-metal-optiplex-7080-20260525-220000-12-q7c_concurrent_munmap_smp.serial.log
```

PM initial read: the expected page fault handler calls `pass(...) -> debug_exit(EXIT_SUCCESS)` from the reader CPU. In QEMU the debug-exit port terminates the VM, but on real hardware it does not provide global scenario termination; the reader CPU halts and the BSP continues to fail with `reader_no_terminal_after_unmap`.

Task:

1. Confirm or refute this diagnosis from source.
2. Do not change unrelated code.
3. If confirmed, implement the narrowest hardware-compatible terminal mechanism for this scenario/test-harness class.
4. Preserve QEMU behavior.
5. Rebuild and run the relevant QEMU scenario gate.
6. Return a pushed commit or, if implementation is not appropriate, file a PM disposition explaining the classification and required evidence handling.

Acceptance for a fix:

- `cargo k` passes.
- `cargo neo-test q7c_concurrent_munmap_smp --no-meta` passes in QEMU.
- A rebuilt physical ISO for `q7c_concurrent_munmap_smp` is produced with SHA256.
- PL reruns the OptiPlex 12-CPU scenario and the serial log contains exactly one terminal accepted marker with no later `event=FAIL`.

## 5. Resolution

Status: RESOLVED 2026-05-25.

Jarvis implemented and pushed the narrow q7c harness terminal handoff fix:

```text
4ae4b0184071b5b8dff03836344e7a4f9184a78c
```

Reported validation:

- `cargo k`: PASS.
- `NEO_SMP=4 cargo neo-test q7c_concurrent_munmap_smp --no-meta`: PASS.
- `NEO_SMP=12 cargo neo-test --build-only q7c_concurrent_munmap_smp`: PASS.

Rebuilt ISO:

```text
E:\AI Automation\AltimitNeo\target\neo-test\vmware\q7c_concurrent_munmap_smp\q7c_concurrent_munmap_smp.iso
SHA256 CA9764150B9C861DBA6C3B1CED1A6F5812BA433251C5E9300B0272FB8812B4B8
```

Accepted hardware rerun evidence:

```text
Plans/evidence/phase2-bare-metal-optiplex-7080-20260525-222500-12-q7c_concurrent_munmap_smp-rerun.serial.log
```

Observed terminal marker:

```text
NEO_TEST/1 event=PASS name=q7c_concurrent_munmap_smp trap=page_fault mechanism=q7c_munmap_shootdown
```

PM scan found no later `event=FAIL` in the rerun log. The blocker is closed.

---

**End of blocker.**
