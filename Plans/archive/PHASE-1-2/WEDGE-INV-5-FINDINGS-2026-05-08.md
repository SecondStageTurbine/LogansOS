# WEDGE-INV-5 findings: GDB-stub pre-flight stopped at soft-heisenbug indicator

Date: 2026-05-08

Dispatch: `Plans/DISPATCH-2026-05-08-wedge-investigation-5-gdb-capture-cerebro.md`

Substrate: `05e15c9 boot: skip Limine boot menu (timeout: 0) to eliminate H7 pre-kernel stall`

## 1. Summary

WEDGE-INV-5 stopped in pre-flight, per dispatch §9. The observer path ran 60 boots of `smp_per_cpu_scheduler_smoke` at `NEO_SMP=4` with QEMU's GDB stub enabled and no GDB client attached unless a timeout occurred. All 60 runs passed; there were 0 `TIMEOUT_IN_KERNEL_SMOKE` instances, so no state dumps were captured and the 200-boot main loop was not run.

Verdict: soft heisenbug indicator / observer-path masking is plausible. This is not proof that the GDB stub alone masks the bug: the selected §3.1 path used direct QEMU invocation after one scenario build, so the observer path also removed the per-run `cargo neo-test` rebuild/ISO assembly overhead that existed in WEDGE-INV-4. The next direction should be chosen by PM/project lead; this dispatch did not localize the kernel mechanism.

## 2. Methodology

No kernel code, scenario code, userland code, or `tools/xtask` code was modified. `xtask` has no existing GDB-stub gate, but direct QEMU invocation was practical, so this dispatch used §3.1 option 2 instead of adding the permitted `NEO_QEMU_GDB_PORT` gate.

Scratch tooling location:

- `tools/wedge-investigation/wedge_inv5_capture.py` (untracked investigation scaffolding)

The scratch runner:

- Built `neo-init`, `neo-worker`, and the kernel with `--features neo-test,neo-test-smp-per-cpu-scheduler-smoke`.
- Assembled `boot/altimit-neo.iso` directly from the built artifacts and `boot/limine.conf`.
- Launched QEMU directly with the same `neo-test` arguments plus `-gdb tcp::1234`.
- Removed each prior source serial log before booting.
- Classified serial logs with the canonical H7 classifier from `Plans/H7-INVESTIGATION-FINDINGS-2026-05-08.md` §5.1.
- Would have attached `rust-gdb` on timeout and written `target/verify/WEDGE-INV-5-state-dump-run-N.txt`; no timeout occurred, so no GDB client attached.

Substrate caveat: pre-flight did not test "cargo neo-test plus GDB stub" because that would have required an `xtask` change. It tested "direct QEMU plus GDB stub." This distinction matters because WEDGE-INV-4's 4/100 failures occurred under per-run `cargo neo-test`, which rebuilds and reassembles the ISO each run and changes host-side timing/load.

Pre-existing tracked-tree caveat: before this dispatch started, `Plans/INDEX.md` already had tracked doc updates for the current wedge campaign. I left that file untouched. It does not affect the kernel/boot/test substrate.

## 3. Measurement 1 - GDB-stub pre-flight

Scenario: `smp_per_cpu_scheduler_smoke`

Environment: `NEO_SMP=4`

QEMU observer path: direct QEMU, `-gdb tcp::1234`, no `-S`, no GDB client attached on PASS runs.

Logs:

- Serial logs: `target/verify/WEDGE-INV-5-run-*.serial.log`
- Results JSONL: `target/verify/WEDGE-INV-5-results.jsonl`
- State dumps: none (`0` files)

| Run | Classification | Signature | Dump |
|---:|---|---|---|
| 1 | PASS | | |
| 2 | PASS | | |
| 3 | PASS | | |
| 4 | PASS | | |
| 5 | PASS | | |
| 6 | PASS | | |
| 7 | PASS | | |
| 8 | PASS | | |
| 9 | PASS | | |
| 10 | PASS | | |
| 11 | PASS | | |
| 12 | PASS | | |
| 13 | PASS | | |
| 14 | PASS | | |
| 15 | PASS | | |
| 16 | PASS | | |
| 17 | PASS | | |
| 18 | PASS | | |
| 19 | PASS | | |
| 20 | PASS | | |
| 21 | PASS | | |
| 22 | PASS | | |
| 23 | PASS | | |
| 24 | PASS | | |
| 25 | PASS | | |
| 26 | PASS | | |
| 27 | PASS | | |
| 28 | PASS | | |
| 29 | PASS | | |
| 30 | PASS | | |
| 31 | PASS | | |
| 32 | PASS | | |
| 33 | PASS | | |
| 34 | PASS | | |
| 35 | PASS | | |
| 36 | PASS | | |
| 37 | PASS | | |
| 38 | PASS | | |
| 39 | PASS | | |
| 40 | PASS | | |
| 41 | PASS | | |
| 42 | PASS | | |
| 43 | PASS | | |
| 44 | PASS | | |
| 45 | PASS | | |
| 46 | PASS | | |
| 47 | PASS | | |
| 48 | PASS | | |
| 49 | PASS | | |
| 50 | PASS | | |
| 51 | PASS | | |
| 52 | PASS | | |
| 53 | PASS | | |
| 54 | PASS | | |
| 55 | PASS | | |
| 56 | PASS | | |
| 57 | PASS | | |
| 58 | PASS | | |
| 59 | PASS | | |
| 60 | PASS | | |

Aggregate:

| Classification | Count | Rate |
|---|---:|---:|
| PASS | 60 | 100.0% |
| TIMEOUT_IN_KERNEL_SMOKE | 0 | 0.0% |
| TIMEOUT_PRE_KERNEL / H7 | 0 | 0.0% |
| TIMEOUT_BOOT_KERNEL_PRE_SMOKE | 0 | 0.0% |
| FAIL / PANIC / OTHER | 0 | 0.0% |

Average QEMU-only run time across the 60 PASS runs was 2.922 seconds.

## 4. Measurement 2 - Main capture loop

Not run. Dispatch §4.1 and §9 require stopping after 0/60 in-kernel timeouts during pre-flight because stub-enabled observation may be masking the bug.

No state dumps were produced:

```text
target/verify/WEDGE-INV-5-state-dump-run-*.txt = 0 files
```

## 5. Comparison to WEDGE-INV-4

WEDGE-INV-4 M2:

- 100 boots under `cargo neo-test smp_per_cpu_scheduler_smoke`
- 96 PASS
- 4 `TIMEOUT_IN_KERNEL_SMOKE`
- Rate: 4.0%

WEDGE-INV-3 + WEDGE-INV-4 combined:

- 150 uninstrumented observations
- 4 `TIMEOUT_IN_KERNEL_SMOKE`
- Rate: 2.67%

WEDGE-INV-5 pre-flight:

- 60 boots under direct QEMU with `-gdb tcp::1234`
- 60 PASS
- 0 `TIMEOUT_IN_KERNEL_SMOKE`
- Rate: 0.0%

Under the combined 4/150 rate (2.67%), observing 0 failures in 60 runs has probability about 19.5%. Under the WEDGE-INV-4 M2-only rate (4.0%), observing 0 failures in 60 runs has probability about 8.6%. This is not decisive, but it is enough to trigger the dispatch's soft-heisenbug stop condition.

## 6. Cross-dump synthesis and case verdict

No dumps exist, so the §6 lock/wakeup/reap/fault/idle-workstanding decision matrix cannot be applied to kernel state. The correct verdict is outside Cases A-F: pre-flight stop condition, observer path may be masking the residual wedge.

Mechanism localized: no.

Supported finding: the direct-QEMU + GDB-stub observer path did not reproduce `POST_SMOKE_SMP_SCENARIO` across 60 boots.

Unsupported claims:

- This does not prove the GDB stub alone masks the bug.
- This does not prove the wedge is gone.
- This does not identify the missing wakeup/reap/scheduler mechanism.

## 7. Recommendation

Stop here and ask PM/project lead for the next observation strategy, as required by dispatch §9.

The cleanest next test would isolate variables:

- Option A: add the minimal `NEO_QEMU_GDB_PORT` `xtask` gate and rerun pre-flight through `cargo neo-test`, preserving per-run build/ISO timing while adding only the GDB stub.
- Option B: run direct QEMU without `-gdb` for 100-200 boots to determine whether direct QEMU alone masks the bug.
- Option C: use a non-GDB out-of-band strategy, such as QEMU monitor/core dump on timeout, if the project lead treats the GDB path as too perturbing.

I do not recommend proceeding to the 200-boot GDB capture loop from the current observer path without direction; the pre-flight stop condition exists specifically to prevent spending a long run on an observation path that may suppress the target bug.

## 8. Cross-references

- `Plans/DISPATCH-2026-05-08-wedge-investigation-5-gdb-capture-cerebro.md`
- `Plans/WEDGE-INV-4-FINDINGS-2026-05-08.md`
- `Plans/PM-NOTE-2026-05-08-wedge-inv-4-case-iii-verdict.md`
- `Plans/PM-NOTE-2026-05-08-patch1-signature-analysis.md`
- `Plans/H7-INVESTIGATION-FINDINGS-2026-05-08.md`
- `Plans/WEDGE-INV-3-BASELINE-FINDINGS-2026-05-08.md`
- `Plans/PM-NOTE-2026-05-07-wedge-fix-commitment.md`
- `target/verify/WEDGE-INV-5-results.jsonl`
- `target/verify/WEDGE-INV-5-run-*.serial.log`
- `tools/wedge-investigation/wedge_inv5_capture.py`
