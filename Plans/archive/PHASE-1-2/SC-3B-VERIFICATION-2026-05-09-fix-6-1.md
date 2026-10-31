# SC-3B Fix 6.1 verification log

Date: 2026-05-09
Status: FAILED gate; stop condition fired; no commit landed

## Substrate

Starting HEAD:

```text
a61e318d93321c9e84732869f5de9da96abe3d5f
```

Dispatch:

```text
Plans/DISPATCH-2026-05-09-sc-3b-fix-6-1-cerebro.md
```

Tracked code change under test:

```text
kernel/src/test_harness.rs | 6 +++++-
1 file changed, 5 insertions(+), 1 deletion(-)
```

Change summary:

- Added `STRESS_PAIR_DIRECTION: [AtomicBool; STRESS_MAX_PAIRS]`.
- Reset each pair direction to `false` in `reset_stress_state`.
- Replaced global sequence parity with per-pair alternation:

```rust
let send_first = STRESS_PAIR_DIRECTION[pair].fetch_xor(true, Ordering::Relaxed);
```

No kernel source outside `kernel/src/test_harness.rs` was modified.

Backup:

```text
Not run. Dispatch says backup runs after commit lands; no commit landed because the gate failed before commit.
```

## Pre-commit sanity

### Build

Command:

```text
docker compose run --rm neo cargo k
```

Result:

```text
PASS
`altimit-neo-kernel` (bin "kernel") generated 18 warnings
```

Warning interpretation:

```text
Matches the dispatch baseline of 18 warnings. No cargo k warning regression.
```

### Diff scope

Command:

```text
git diff --stat
```

Observed:

```text
kernel/src/test_harness.rs | 6 +++++-
1 file changed, 5 insertions(+), 1 deletion(-)
```

Command:

```text
git diff --numstat -- kernel/src/test_harness.rs
```

Observed:

```text
5	1	kernel/src/test_harness.rs
```

### Single-run stress sanity at NEO_SMP=4

Command:

```text
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test smp_cross_cpu_rendezvous_stress
```

Result:

```text
PASS
```

Note:

```text
The immediate single-run serial log was later overwritten by subsequent scenario runs. The xtask command exited 0 and printed:
[xtask] neo-test: `smp_cross_cpu_rendezvous_stress` PASS
```

### Sibling scenario sanity at NEO_SMP=4

All five sibling scenarios exited 0.

Observed PASS frames:

```text
smp_block_release_smoke: NEO_TEST/1 event=PASS name=smp_block_release_smoke blocked_cpu=0 completer_cpu=1 resumed_cpu=1 release_path=asm
smp_send_blocks_first_then_completes: NEO_TEST/1 event=PASS name=smp_send_blocks_first_then_completes blocked_cpu=0 completer_cpu=1 resumed_cpu=1
smp_recv_blocks_first_then_completes: NEO_TEST/1 event=PASS name=smp_recv_blocks_first_then_completes blocked_cpu=0 completer_cpu=1 resumed_cpu=1
smp_grant_cross_cpu: NEO_TEST/1 event=PASS name=smp_grant_cross_cpu blocked_cpu=0 completer_cpu=1 resumed_cpu=1 grant_rights=0x101 handle_idx=0
smp_cross_cpu_migration_reap: NEO_TEST/1 event=PASS name=smp_cross_cpu_migration_reap blocked_cpu=0 resumed_cpu=1 exit_reaped=ok
```

## Gate steps

### Step 1: 200-run NEO_SMP=4 loop

Command:

```text
docker compose run --rm -e NEO_SMP=4 neo bash -lc '<200-run loop; copies serial logs to /workspace/target/verify/sc3b-fix-6-1-gate/neo-smp4>'
```

Host artifact directory:

```text
target/verify/sc3b-fix-6-1-gate/neo-smp4/
```

Completed runs:

```text
5 PASS, then run 006 timed out.
```

Observed PASS frames:

```text
run-001.serial.log: NEO_TEST/1 event=PASS name=smp_cross_cpu_rendezvous_stress iterations=1000 send_first=500 recv_first=500 cpus=4
run-002.serial.log: NEO_TEST/1 event=PASS name=smp_cross_cpu_rendezvous_stress iterations=1000 send_first=500 recv_first=500 cpus=4
run-003.serial.log: NEO_TEST/1 event=PASS name=smp_cross_cpu_rendezvous_stress iterations=1000 send_first=500 recv_first=500 cpus=4
run-004.serial.log: NEO_TEST/1 event=PASS name=smp_cross_cpu_rendezvous_stress iterations=1000 send_first=500 recv_first=500 cpus=4
run-005.serial.log: NEO_TEST/1 event=PASS name=smp_cross_cpu_rendezvous_stress iterations=1000 send_first=500 recv_first=500 cpus=4
```

Run 006 xtask failure line:

```text
xtask: QEMU timed out after 180s for `smp_cross_cpu_rendezvous_stress`
```

Run 006 serial signature:

```text
NEO_TEST/1 event=BEGIN name=smp_cross_cpu_rendezvous_stress phase=dispatch
```

No terminal frame observed in run 006:

```text
No NEO_TEST PASS frame.
No NEO_TEST FAIL frame.
No BUG: line.
No KERNEL PANIC line.
No frame-leak audit line.
```

Run 006 tail:

```text
[ kernel ] sched: reaped 'pong' (id=5)
[ hog  ] preempted at least 5 ticks (start=3, now=10)
[ hog  ] HOG_TRIGGER seen, exiting
[ kernel ] sched: reaped 'ping' (id=4)
[ kernel ] sched: reaped 'hog' (id=6)
[ kernel ] sched: smoke test passed (5 ping, 5 pong, hog reaped, ticks=13)
NEO_TEST/1 event=BEGIN name=smp_cross_cpu_rendezvous_stress phase=dispatch
```

Classification:

```text
Run-36 signature reproduced: BEGIN frame followed by silence, no terminal frame, QEMU 180s timeout.
```

### Step 2: 25-run NEO_SMP=2 loop

Result:

```text
Not run. Dispatch stop condition fired on NEO_SMP=4 run 006.
```

## Statistical interpretation

Fix 6.1 did not close the SC-3B stress gate.

The required closure criterion was:

```text
200/200 PASS at NEO_SMP=4 with zero classifier-signature timeouts.
```

Observed:

```text
5/6 completed runs PASS; run 006 hit the same silent-timeout class.
```

This refutes Fix 6.1 as a sufficient fix for the gate. It may still reduce or perturb the failure distribution, but the dispatch explicitly forbids threshold relaxation or extending the loop after any timeout.

## Conclusion

Outcome:

```text
Outcome B: gate failed.
```

Required next path:

```text
Phase B instrumentation per Plans/SC-3B-INVESTIGATION-SYNTHESIS-2026-05-09.md section 3.
```

Commit status:

```text
No commit landed. kernel/src/test_harness.rs remains modified in the working tree for PM review.
```

Current tracked status at time of log:

```text
M kernel/src/test_harness.rs
```
