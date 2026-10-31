# SC-3B verification log

Date: 2026-05-09
Commit under test: a61e318d93321c9e84732869f5de9da96abe3d5f
Backup after Sub-step 5 commit: Z:\AltimitNeo\2026-05-09-144314

## Gate steps

1. `docker compose run --rm neo cargo k`
   - Result: PASS
   - Host log: `target/verify/sc3b-gate-01-cargo-k.host.log`
   - Warning count: 18
   - Observed line: `warning: `altimit-neo-kernel` (bin "kernel") generated 18 warnings`

2. `docker compose run --rm -e NEO_SMP=4 neo cargo neo-tty`
   - Result: clean boot observed; wrapper command timed out because production boot reaches the expected halt loop.
   - Host log: `target/verify/sc3b-gate-02-neo-tty.host.log`
   - Observed: `[ kernel ] paging: GLOBAL fixup complete - 4K leaves: 4578, 2M leaves: 251, 1G leaves: 0`
   - Observed: `[ kernel ] paging: sanity OK - kernel .text sample virt VirtAddr(0xffffffffe11456b0) -> phys PhysAddr(0x1f1e06b0) (global=true)`
   - Observed: `[ kernel ] frame: leak audit clean (baseline=1026 final=1026)`
   - Observed: `[ kernel ] phase 0 boot complete`
   - Observed final line: `[ kernel ] no scheduler yet - halting forever`
   - No `BUG:` or `KERNEL PANIC` observed.

3. Single NEO_SMP=4 run of each new/SC-3B scenario
   - Result: PASS
   - Host log: `target/verify/sc3b-gate-03-single.host.log`
   - Serial logs: `target/verify/sc3b-gate-03-single/*.serial.log`
   - PASS: `NEO_TEST/1 event=PASS name=smp_block_release_smoke blocked_cpu=0 completer_cpu=1 resumed_cpu=1 release_path=asm`
   - PASS: `NEO_TEST/1 event=PASS name=smp_send_blocks_first_then_completes blocked_cpu=0 completer_cpu=1 resumed_cpu=1`
   - PASS: `NEO_TEST/1 event=PASS name=smp_recv_blocks_first_then_completes blocked_cpu=0 completer_cpu=1 resumed_cpu=1`
   - PASS: `NEO_TEST/1 event=PASS name=smp_grant_cross_cpu blocked_cpu=0 completer_cpu=1 resumed_cpu=1 grant_rights=0x101 handle_idx=0`
   - PASS: `NEO_TEST/1 event=PASS name=smp_cross_cpu_migration_reap blocked_cpu=0 resumed_cpu=1 exit_reaped=ok`
   - PASS: `NEO_TEST/1 event=PASS name=smp_cross_cpu_rendezvous_stress iterations=1000 send_first=500 recv_first=500 cpus=4`

4. Full default `cargo neo-test` suite at NEO_SMP=4
   - Result: PASS
   - Host log: `target/verify/sc3b-gate-04-default-suite.host.log`
   - Serial logs: `target/verify/sc3b-gate-04-default-suite/*.serial.log`
   - Serial logs copied: 34
   - PASS terminal frames: 33
   - FAIL terminal frames: 1 expected meta-test frame from `expect_pass_but_fail`.
   - Observed meta-test line: `[xtask] neo-test meta: deliberate failure was detected`
   - No unexpected `FAIL`, `BUG:`, or `KERNEL PANIC` observed.

5. 50-run supplementary loops at NEO_SMP=4
   - Result: PASS
   - Host log: `target/verify/sc3b-gate-05-supplementary.host.log`
   - Serial logs: `target/verify/sc3b-gate-05-supplementary/<scenario>/run-*.serial.log`
   - `smp_block_release_smoke`: 50/50 PASS, 0 FAIL/BUG/PANIC
   - `smp_send_blocks_first_then_completes`: 50/50 PASS, 0 FAIL/BUG/PANIC
   - `smp_recv_blocks_first_then_completes`: 50/50 PASS, 0 FAIL/BUG/PANIC
   - `smp_grant_cross_cpu`: 50/50 PASS, 0 FAIL/BUG/PANIC
   - `smp_cross_cpu_migration_reap`: 50/50 PASS, 0 FAIL/BUG/PANIC

6. 200-run `smp_cross_cpu_rendezvous_stress` loop at NEO_SMP=4
   - Result: FAIL at run 036
   - Host log: `target/verify/sc3b-gate-06-stress-neo-smp4.host.log`
   - Serial logs: `target/verify/sc3b-gate-06-stress-neo-smp4/run-*.serial.log`
   - Clean PASS runs before failure: 35/35
   - Failure run serial log: `target/verify/sc3b-gate-06-stress-neo-smp4/run-036.failure.serial.log`
   - Observed timeout: `xtask: QEMU timed out after 180s for `smp_cross_cpu_rendezvous_stress``
   - Failure serial terminal state: `NEO_TEST/1 event=BEGIN name=smp_cross_cpu_rendezvous_stress phase=dispatch`
   - No `NEO_TEST/1 event=PASS`, `NEO_TEST/1 event=FAIL`, `BUG:`, or `KERNEL PANIC` observed in the failure serial log.

7. 25-run `smp_cross_cpu_rendezvous_stress` loop at NEO_SMP=2
   - Result: NOT RUN
   - Reason: dispatch section 9.5 says to stop on a single timeout in the 200-run stress loop, classify, report, and bring to PM for triage.

## Failure classification

Primary classification: `Missing terminal frame` - kernel hung pre-teardown after the scenario BEGIN frame and before any terminal PASS/FAIL frame.

Secondary possibilities not distinguishable from the serial-only artifact:

- `Lost wakeup` - a stress sender/receiver may have blocked and not resumed.
- `Deadlock with waiter published but not woken` - endpoint waiter may have been visible but not woken.
- `Deadlock with one waiter unpublished` - block-site publication could still have a race under the stress scenario.

Negative findings:

- Not `Wrong CPU migration`: no terminal PASS frame existed to report a wrong `resumed_cpu`.
- Not `Duplicate terminal frame`: no terminal frame existed.
- Not `Panic or BUG`: neither string was observed.
- Not `Rights mismatch in grant`: stress scenario does not exercise grant rights.
- Not `Unexpected EBADF or EPIPE`: neither string was observed.

## Conclusion

SC-3B Sub-step 5 implementation is not closed. The pre-commit build and sanity checks passed, the default suite passed, and the five supplementary 50-run loops passed. The closure signal failed: `smp_cross_cpu_rendezvous_stress` timed out on run 036 of the required 200-run NEO_SMP=4 loop.

Per dispatch section 9.5, the run stopped immediately. No NEO_SMP=2 supplementary loop was run. Next action is PM triage or a targeted investigation of the stress timeout before Sub-step 6 docs/close can proceed.
