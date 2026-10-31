# PM-DISPOSITION: q7c_concurrent_munmap_smp hardware FAIL diagnosis accepted

- **Date:** 2026-05-25
- **Author:** PM
- **Scenario:** `q7c_concurrent_munmap_smp`
- **Evidence:** `Plans/evidence/phase2-bare-metal-optiplex-7080-20260525-220000-12-q7c_concurrent_munmap_smp.serial.log`
- **Related blocker:** `Plans/BLOCKER-2026-05-25-phase2-hardware-q7c-concurrent-munmap-smp.md`
- **Status:** RESOLVED BY FIXED BUILD AND BARE-METAL RERUN

---

## 1. Decision

PM accepts the combined Cerebro and Prometheus diagnosis:

- the OptiPlex log's later `event=FAIL` is a test-harness terminal-semantics artifact;
- the kernel behavior under test appears correct because the expected reader CPU page fault was observed and reported as `mechanism=q7c_munmap_shootdown`;
- the old hardware run is still not accepted as PASS because the final serial marker is `event=FAIL`;
- a fixed build and new bare-metal rerun are required before the sweep can proceed past this scenario.

---

## 2. Evidence

Hardware log:

```text
Plans/evidence/phase2-bare-metal-optiplex-7080-20260525-220000-12-q7c_concurrent_munmap_smp.serial.log
```

Observed terminal sequence:

```text
NEO_TEST/1 event=PASS name=q7c_concurrent_munmap_smp trap=page_fault mechanism=q7c_munmap_shootdown
NEO_TEST/1 event=FAIL name=q7c_concurrent_munmap_smp reason=reader_no_terminal_after_unmap
```

The PASS line proves the reader CPU took the expected post-unmap page fault path. The later FAIL is emitted by the BSP after its post-unmap wait loop expires.

---

## 3. Root Cause

`q7c_concurrent_munmap_smp` reaches its correctness verdict on the reader CPU, not the BSP.

In QEMU:

- `debug_exit(EXIT_SUCCESS)` writes the QEMU isa-debug-exit port;
- the VM exits machine-wide;
- the reader CPU PASS line is terminal.

On bare metal:

- the same port write is not a machine-wide exit mechanism;
- `debug_exit()` falls through to `halt_forever()` on the reader CPU;
- CPU 1 halts, while the BSP keeps running;
- the BSP's fixed wait loop expires and emits `reason=reader_no_terminal_after_unmap`.

This explains the observed PASS-then-FAIL sequence without requiring a q7c kernel correctness regression.

---

## 4. Implementation Direction

PM accepts the narrow fix direction reported by Prometheus:

- add a q7c-scoped signal that records when the reader CPU has reached the expected munmap page-fault verdict;
- ensure the signal is stored before the QEMU debug-exit port write so QEMU behavior is preserved;
- have the BSP converge to `debug_exit(EXIT_SUCCESS)` if it observes the reader verdict on bare metal;
- keep the change scoped to `q7c_concurrent_munmap_smp` terminal handling and shared debug-exit factoring as needed.

Prometheus noted one naming/value concern:

- `Q7C_STAGE_MUNMAP_FAULT_HANDLED = 3` overlaps `Q7C_STAGE_EXIT_DONE = 3`;
- this is safe if the cfg gates make them mutually exclusive, but the final implementation should either document that or use a distinct value to reduce review friction.

---

## 5. Required Before Rerun

Jarvis must return:

1. committed source fix;
2. QEMU gate result for `cargo neo-test q7c_concurrent_munmap_smp --no-meta`;
3. `cargo k` result, if touched source requires it;
4. rebuilt physical ISO path;
5. rebuilt ISO SHA256.

Only after that may PL rerun the OptiPlex 12-CPU scenario.

Acceptance for the rerun:

- serial log shows `smp: Limine reports 12 cpu(s)`;
- serial log shows `smp: started 11 AP(s)`;
- serial log shows the expected q7c munmap page-fault PASS path;
- serial log does not contain a later `event=FAIL`;
- the scenario's final accepted terminal state is PASS.

---

**End of disposition.**

---

## 6. Resolution

Jarvis committed and pushed the narrow harness fix:

```text
4ae4b0184071b5b8dff03836344e7a4f9184a78c Fix q7c hardware terminal handoff
```

Reported validation:

- `docker compose run --rm neo cargo k`: PASS.
- `docker compose run --rm -e NEO_SMP=4 neo cargo neo-test q7c_concurrent_munmap_smp --no-meta`: PASS.
- `docker compose run --rm -e NEO_SMP=12 neo cargo neo-test --build-only q7c_concurrent_munmap_smp`: PASS.

Rebuilt ISO:

```text
target/neo-test/vmware/q7c_concurrent_munmap_smp/q7c_concurrent_munmap_smp.iso
SHA256 CA9764150B9C861DBA6C3B1CED1A6F5812BA433251C5E9300B0272FB8812B4B8
```

PL reran the OptiPlex 7080 12-CPU scenario and PM verified the log:

```text
Plans/evidence/phase2-bare-metal-optiplex-7080-20260525-222500-12-q7c_concurrent_munmap_smp-rerun.serial.log
```

Observed accepted markers:

```text
[ kernel ] smp: Limine reports 12 cpu(s); bsp_lapic_id=0
[ kernel ] smp: started 11 AP(s)
NEO_TEST/1 event=PASS name=q7c_concurrent_munmap_smp trap=page_fault mechanism=q7c_munmap_shootdown
```

PM scan found no later `event=FAIL` in the rerun log.

Disposition: blocker resolved. Continue the Phase 2 OptiPlex 12-CPU hardware sweep with the next scenario.
