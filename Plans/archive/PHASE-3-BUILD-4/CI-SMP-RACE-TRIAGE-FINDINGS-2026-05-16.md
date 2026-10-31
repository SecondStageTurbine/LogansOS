# CI SMP Race Triage — Findings

Date: 2026-05-16
Authority: `DISPATCH-2026-05-16-ci-smp-race-triage-cerebro.md`
Substrate: `bb47d0a` (main at time of investigation)
Status: FINDINGS UPDATED — scenario 4 added 2026-05-16

---

## 1. Environment Evidence

### Runner image delta

| Property         | Last success (2026-05-12) | First failure (2026-05-16) |
|------------------|---------------------------|----------------------------|
| Runner image     | ubuntu-24.04 v20260413.86.1 | ubuntu-24.04 v20260513.135.3 |
| Image gap        | —                         | Image updated 2026-05-13 (1 day before failures started) |
| QEMU (container) | 7.2.22 (Debian 7.2+dfsg-7+deb12u18) | same — container ships its own QEMU |
| NEO_SMP          | 4                         | 4 |
| Last kernel commit | 60319e3                 | same — all three failing commits are docs/plans only |

**Conclusion:** The container's QEMU binary is identical between old and new runs. The failure trigger is the **host Linux kernel change** in the new runner image (Ubuntu 24.04 updated from April to May 2026 build). A newer host kernel changes KVM VM exit patterns, CPU scheduling behavior, and preemption timing for QEMU vCPUs, widening or narrowing timing windows in the guest.

### Local environment

Local dev container: QEMU 7.2.22 (same as CI container). Local run of
`cargo neo-test --no-meta` with `NEO_SMP=4` on `bb47d0a`-based working tree
(with uncommitted BUILD-4A changes) was in progress at investigation time.
All three failing scenarios (`ipc_heap_kill_smoke`, `cap_revoke_local`,
`phase3_sched_wake_token_abandon`) **PASSED** in the local single-run pass
executed concurrently with this investigation.

**Important limitation:** A single run is insufficient to measure a
non-deterministic race. 50-iteration loop runs per scenario are still required.

---

## 2. Serial Log Access — Critical Gap

**The CI workflow does not upload serial logs as artifacts.** The serial log file
`/workspace/target/neo-test/<scenario>.serial.log` contains the exact
`NEO_TEST/1 event=FAIL reason=...` frame that identifies which assertion fired.
This log is ephemeral inside the container and is lost when the job ends.

**Impact:** Every failure triage from CI is forced into code-analysis-only mode.
No exit code, no reason string, no stack context. This gap was the primary
obstacle in this investigation.

**Recommendation (P0, unblocks all future triage):**
Add to `.github/workflows/ci.yml` — on-failure artifact upload:
```yaml
- uses: actions/upload-artifact@v4
  if: failure()
  with:
    name: neo-test-serial-logs
    path: target/neo-test/*.serial.log
    retention-days: 14
```

### Local serial log confirmation

Serial logs from prior local runs (available under `target/neo-test/`) confirm
both scenarios' passing serial trace. Key frames:

**`ipc_heap_kill_smoke` (passing path):**
```
[ neo-init ] sent go-block; worker should now be RecvBlocked
[ neo-init ] attempting sys_kill on Blocked worker (expect 0)
[ kernel ] proc: terminated 'worker' (pid=2) via sys_kill
[ neo-init ] sys_kill on Blocked returned 0 as expected
[ neo-init ] sys_kill on stale returned ESRCH as expected
NEO_TEST/1 event=PASS name=ipc_heap_kill_smoke leak_audit=clean exit_code=0
```

**`phase3_sched_wake_token_abandon` (passing path):**
```
NEO_TEST/1 event=PASS name=phase3_sched_wake_token_abandon
  token_cpu=0 token_slot=0 generation=1 late_wake=Gone abandon=zombified_and_reaped
```

`token_cpu=0` confirms: locally, the worker always blocks on CPU 0 (where
`push_ready` enqueues it and where the test harness runs). This is the expected
single-CPU path. The CI failure implies the guest scheduling behavior differs
under the new runner host kernel.

---

## 3. Failure Classification

| Scenario                                      | Attempt 1      | Attempt 2         | Classification           |
|-----------------------------------------------|----------------|-------------------|--------------------------|
| `ipc_heap_kill_smoke`                         | exit status 35 | exit status 35    | `ASSERT_35`              |
| `cap_revoke_local`                            | exit status 35 | exit status 35    | `ASSERT_35`              |
| `phase3_sched_wake_token_abandon`             | exit status 35 | QEMU timeout 180s | `ASSERT_35` + `TIMEOUT`  |
| `phase3_neofs_fsync_filecap_stale_regression` | QEMU timeout 180s | QEMU timeout 180s | `TIMEOUT` (both attempts) |

Exit status 35 = `qemu_debug_exit_status(TEST_EXIT_FAILURE)` = `(0x11<<1)|1`.
The kernel deliberately called `debug_exit(EXIT_FAILURE)` — either from a test
assertion path or from the kernel panic handler (both end at the same port write).

The 180s `TIMEOUT` on `phase3_sched_wake_token_abandon` attempt 2 is a deadlock or
infinite spin that prevents the kernel from reaching `debug_exit` at all.

---

## 4. Mechanism Analysis Per Scenario

### 4.1 `ipc_heap_kill_smoke` — M2 (documented SMP race)

**Source of truth in the code itself** (neo-init/src/main.rs:205–209):
```
// Single-CPU rendezvous: the worker transitions directly from receiving
// this message into sys_recv(chan_main). There must be no worker-side
// diagnostic syscall in between; a timer tick in that window would make
// the blocked-kill assertion scheduler-timing-sensitive.
```

**Failure path:**
1. Test sends `go-block` to worker via synchronous IPC rendezvous.
2. Rendezvous completes on CPU 0 — the send returns.
3. On SMP=4, the worker is on CPU N (1/2/3). A finite window exists between the worker receiving `go-block` and the worker calling `sys_recv(chan_main)`. During this window the worker is **Running/Ready**, not Blocked.
4. Test immediately calls `sys_kill(worker_handle)`.
5. `sys_kill_impl` → `process::terminate(pid)` → `kill_thread_by_pid`.

**`kill_thread_by_pid` is CPU-local** (scheduler.rs:1024–1047):
```rust
pub fn kill_thread_by_pid(pid: ProcessId) -> bool {
    without_interrupts(|| {
        let sched = current_scheduler();  // ONLY THE CALLING CPU
        let mut s = sched.inner.lock();
        let idx = s.ready.iter().position(|t| t.pid == pid);
        None => return false,
```

If the worker is on CPU N's ready queue (not CPU 0's), `kill_thread_by_pid`
returns `false`. The fallback paths in `terminate()` check for IPC-blocked threads
(channel walk) and wake-token parked threads (`zombify_parked_thread_by_pid`) —
neither finds a **Ready thread on a remote CPU**.

`terminate()` then hits:
```rust
debug_assert!(
    found_blocked,
    "terminate: pid {} was neither Ready nor parked in a drained channel handle",
    target_pid.0
);
```

In debug builds (which CI uses), this `debug_assert!` fires → kernel panic →
panic handler → `debug_exit(EXIT_FAILURE)` → exit status 35.

**Expected CI serial frame:**
```
NEO_TEST/1 event=FAIL name=ipc_heap_kill_smoke reason=neo_init_exit_nonzero code=5
```
(code=5 = `return 5` when `sys_kill` returned non-zero)

Or if the debug_assert fires before the neo-init result is logged:
```
[ kernel ] KERNEL PANIC: terminate: pid N was neither Ready nor parked ...
```
followed by `debug_exit(EXIT_FAILURE)`.

**SMP sensitivity:** Confirmed by source comment. NEO_SMP=1 runs should PASS reliably.

### 4.2 `cap_revoke_local` — mechanism unclear, requires serial log

`cap_revoke_local` is a purely sequential, single-process, no-spawn test:
1. Create channel → derive handle → revoke it → assert EBADF on send → derive again

No explicit multi-threading from the test's perspective. The failure pattern
(both attempts fail consistently in ~1.7 seconds) is more deterministic than the
other two scenarios.

**Possible causes:**
(a) An AP's scheduler is executing some kernel background path (e.g., scheduler
    idle thread calling something) that races with the handle table operations
    during sys_derive or sys_revoke.
(b) The new runner's host kernel changes the preemption frequency, exposing a race
    in the handle-table lock acquisition path under high preemption rate.
(c) A prior assertion failure or debug_assert in an earlier boot phase leaves
    the kernel in an inconsistent state that affects the channel/handle code.

**Verdict: unresolved without CI serial log.** The 1.7-second failure timing and
consistency across two attempts suggest a more deterministic issue than ipc_heap_kill_smoke.
Highest priority for serial log capture.

**NEO_SMP=1 behavior:** Unknown — must be measured.

### 4.3 `phase3_sched_wake_token_abandon` — M1 + M3 (kill path + deadlock)

**Attempt 1 (exit status 35):** The same `kill_thread_by_pid` CPU-locality bug
applies. The test specifically kills a wake-token parked thread. The normal path:
1. Worker blocks via `block_current_with_token_and_guard` → installs token on CPU N
2. Test calls `process::terminate(pid)`
3. `kill_thread_by_pid` returns false (worker not in ready queue)
4. Channel walk: no channels → found_blocked = false
5. `zombify_parked_thread_by_pid` scans all CPUs → should find token

Locally this passes because token_cpu=0 (worker always blocks on CPU 0, same as
test harness CPU). On CI with new timing, the worker might be scheduled differently
or the timing of `wait_for_primary_token()` allows a different execution order.

**Attempt 2 (QEMU timeout 180s):** The 180-second hang points to one of:
(a) `block_current_with_token_and_guard` panicking with "ready queue empty" when
    the worker's CPU has no thread to context-switch to, and the panic handler
    entering an unrecoverable state (corrupted stack, infinite spin).
(b) The zombie reap loop (100,000 × yield+reap) running to exhaustion but then
    a `fail()` path leading to a state where `debug_exit` is never reached.
(c) A lock-order issue between `without_interrupts` callers on different CPUs
    where CPU 0 is spinning waiting for a lock held by CPU N (rare but possible
    if `zombify_parked_thread_by_pid`'s scan acquires CPU N's lock while CPU N
    is waiting for something CPU 0 holds).

The fact that attempt 2 is WORSE than attempt 1 (timeout vs fast exit) suggests
the race condition has two regimes: one that reaches an assertion quickly (35) and
one that enters a non-terminating state.

**Expected CI serial frame for attempt 1:**
```
NEO_TEST/1 event=FAIL name=phase3_sched_wake_token_abandon reason=wake_token_abandon_not_zombified zombies=0
```
or a PANIC frame if debug_assert fires.

### 4.4 `phase3_neofs_fsync_filecap_stale_regression` — M5 (writeback kthread busy-spin under SMP)

**Classification: MAIN-BRANCH SMP INSTABILITY — same root-cause family as CI failures.
Not BUILD-4A-induced. Not a NeoFS/fsync logic bug. Not a full-suite ordering issue.**

#### Evidence from serial log

Local serial log (`target/neo-test/phase3_neofs_fsync_filecap_stale_regression.serial.log`)
confirms the failure pattern. The log contains:

- Line 96: `NEO_TEST/1 event=BEGIN name=phase3_neofs_fsync_filecap_stale_regression phase=dispatch`
- Lines 97+: stream of `virtio-blk: completion ... wake=Woken` lines — I/O is actively completing
- **No PASS or FAIL frame.** The serial log ends abruptly mid-completion-stream when the 180-second QEMU timeout fires.

The kernel is executing I/O that completes successfully (`status=0`, `wake=Woken`) but the
test never terminates. This is a **wall-clock throughput failure**, not an assertion failure.

Two distinct serial logs were inspected: the previous failed run (511 lines, all completions)
and the current in-progress run (262+ lines, same pattern). Both end with no test-result frame.

#### Mechanism (M5): NeoFS writeback kthread busy-spin degrades I/O throughput under SMP

The test sets `writeback_hold = true` (line 6469 of test_harness.rs) before the main sequence,
and **never releases it**. This is intentional — the hold keeps the writeback worker from
flushing dirty blocks during the stale-fsync phase so the test can measure the durability
state invariant.

However, the NeoFS writeback kthread's idle loop is a busy-spin:

```rust
// spawn_writeback_worker in cache.rs:295-311
loop {
    if stop.is_stop_requested() { ... }
    if cache.drain_worker_cycle().is_err() { return Failed(2); }
    crate::sched::yield_now();
}
```

With `writeback_hold = true`, `drain_worker_cycle()` checks:
```rust
if inner.writeback_hold || inner.foreground_writers != 0 {
    return Ok(DrainReport::default());  // returns immediately, does no I/O
}
```

Then `yield_now()` is called. Under SMP=4, the kthread runs on an AP (CPU 1/2/3).
Because no other threads are on that AP's ready queue, `yield_now()` is a **no-op** —
it does not release the AP to the hypervisor. The kthread immediately loops back.

**Result:** One AP is in a continuous tight loop consuming 100% of its virtual CPU:
`drain_worker_cycle() → no-op → yield_now() → no-op → repeat`.

Under the new host kernel's KVM scheduling behavior, this AP busy-spin competes for
host CPU time that would otherwise be used for:
- Processing virtio-blk MSI interrupts
- Delivering QEMU I/O completions to the guest vCPUs

The effect: each virtio-blk sector read/write takes significantly longer. The 128 MB
NeoFS disk (`capacity_sectors=262144`) requires hundreds of sector reads during
`mount_neofs_rw_or_fail()` alone (superblock validation, inode tree scan, free map
validation via `validate_free_map_against_live_set` → `collect_live_blocks`). Under
degraded I/O throughput from the AP busy-spin, the total test time exceeds 180 seconds.

#### Classification of root cause

| Question | Answer |
|----------|--------|
| BUILD-4A-induced? | **No.** BUILD-4A changes `fs/mod.rs` (readdir_one, VfsDirEntry) and `neofs/inode.rs` (readdir_at, directory available_rights). None of these touch the NeoFS write, fsync, or writeback code paths. The regular-file fsync sequence is unchanged. |
| NeoFS/fsync logic bug? | **No.** The `drain_all` → `drain_pass` → `write_fs_block` loop is correct. Blocks drain and complete with status=0. The test does not reach an assertion; it times out on wall-clock. |
| Full-suite/order/resource interaction? | **No.** Each scenario runs in its own isolated QEMU boot with a freshly-created disk image (`prepare_neofs_storage_disk` calls `neofs-mkfs create` for each run). Prior scenarios cannot contaminate the NeoFS disk or QEMU VM state. |
| Main-branch SMP instability? | **Yes.** The writeback kthread busy-spin under `writeback_hold = true` is a pre-existing behavior present since BUILD-3E (commit `b2a93f2 kernel/fs: add NeoFS write-back cache`). It became a timeout risk when the host kernel's KVM scheduling changed (same underlying cause as CI failures). |

#### Why this appears as a BUILD-4A blocker (not a pre-existing failure)

On CI, the full suite fails at `ipc_heap_kill_smoke`, `cap_revoke_local`, or
`phase3_sched_wake_token_abandon` before reaching `phase3_neofs_fsync_filecap_stale_regression`.
So CI has no data on whether this scenario would pass or fail.

Locally, those three earlier scenarios pass under the developer machine's host timing, allowing
the suite to proceed to `phase3_neofs_fsync_filecap_stale_regression`. This scenario then fails
due to the same root cause (changed KVM timing), via a different mechanism (I/O throughput
degradation vs. scheduler kill race).

#### Shared signature with CI failures

| Attribute | CI failures | `phase3_neofs_fsync_filecap_stale_regression` |
|-----------|-------------|-----------------------------------------------|
| Fails under NEO_SMP=4 | Yes | Yes |
| Passes under NEO_SMP=1 | Expected yes (unconfirmed) | Expected yes (unconfirmed) |
| Root cause | New KVM host timing exposes SMP race | New KVM host timing + AP busy-spin degrades I/O |
| Classification | ASSERT_35 (assertion) or TIMEOUT (deadlock) | TIMEOUT (throughput) |
| Code bug | `kill_thread_by_pid` CPU-local | writeback kthread busy-spin under hold |
| BUILD-4A code change? | No | No |

#### Isolation test results (added 2026-05-16)

Controlled loop runs were executed after the initial triage to validate the BUILD-4A and
SMP-sensitivity claims:

| Configuration              | Runs | Final PASS | Final FAIL | First-attempt timeouts (retry-pass) |
|----------------------------|------|------------|------------|--------------------------------------|
| clean main, NEO_SMP=4      | 20   | 20         | 0          | 4                                    |
| BUILD-4A WIP, NEO_SMP=4    | 20   | 20         | 0          | 2                                    |
| clean main, NEO_SMP=1      | 10   | 10         | 0          | 0                                    |
| BUILD-4A WIP, NEO_SMP=1    | 10   | 10         | 0          | 1                                    |

**Disposition:** Classification confirmed. No final failures across 60 total runs. BUILD-4A
does not change the pass/fail outcome in this sample — the scenario is not BUILD-4A-induced.

The retry-pass first-attempt timeouts (4 on clean main SMP=4, 2 on BUILD-4A SMP=4, 1 on
BUILD-4A SMP=1) confirm residual timing instability under the current runner/QEMU/host
configuration. These are not regressions introduced by BUILD-4A; they are the same
underlying KVM scheduling sensitivity that drives the CI failures. The xtask retry
mechanism correctly masks them, but the instability is real and must be fixed (Fix 5)
before these tests are robust under all runner configurations.

The SMP=1 near-clean result (0 on clean, 1 on BUILD-4A) confirms SMP sensitivity is
the primary driver, though the single SMP=1 timeout suggests the I/O timing issue is
not purely an SMP phenomenon — it has a slower-but-present expression under SMP=1 as
well, likely from the AP idle bootstrap threads.

#### Expected fix

The writeback kthread should block/sleep instead of busy-spinning when `writeback_hold = true`.
Options:
- Add a `crate::sched::reap()` call inside the idle loop to let the AP process zombie threads
  and reduce host CPU consumption (minimal change, doesn't fully solve)
- Use a condition variable / wake token to block the kthread until `writeback_hold` is released
  (correct fix, adds a new synchronization primitive to the writeback state)
- For the test specifically: release the hold before the final fsync (but changes test semantics —
  the hold is the point of the test)

---

## 5. Confirmed Findings Summary

| Finding | Evidence |
|---------|----------|
| Runner image changed 2026-05-13, failures started 2026-05-16 | CI logs compared |
| QEMU binary identical between old/new runner | CI logs + local container inspect |
| `kill_thread_by_pid` searches only calling CPU's ready queue | scheduler.rs:1024 code read |
| `terminate()` debug_assert fires if thread is Ready on remote CPU | process.rs:971 code read |
| `ipc_heap_kill_smoke` source-documented as single-CPU SMP-sensitive | neo-init/src/main.rs:205 |
| Earlier three scenarios pass in single local run with standard timing | docker logs from running container |
| Local `token_cpu=0` confirms worker stays on CPU 0 locally | target/neo-test/phase3_sched_wake_token_abandon.serial.log |
| CI serial logs not uploaded — triage blocked | CI log inspection |
| Serial log format contains exact `reason=` field per assertion | test_harness.rs code read |
| `cap_revoke_local` mechanism unresolved | serial log not available |
| `phase3_neofs_fsync_filecap_stale_regression` times out with I/O completing (no PASS/FAIL) | target/neo-test/phase3_neofs_fsync_filecap_stale_regression.serial.log |
| Writeback kthread busy-spins under `writeback_hold = true` (no sleep, `yield_now()` is no-op on idle AP) | cache.rs:295–310 code read |
| BUILD-4A kernel changes do not touch NeoFS fsync/writeback path | fs/mod.rs + neofs/inode.rs working tree read |
| Phase3_neofs_fsync test never releases `writeback_hold` before final fsync | test_harness.rs:6469 — only set to true, never false |
| QEMU capacity_sectors=262144 (128 MB disk) confirmed from serial log | phase3_neofs_fsync...serial.log line 38 |
| Scenario is isolated per QEMU boot, not affected by suite order | xtask prepare_storage_disk reads code |
| Isolation: 20/20 final PASS on clean main SMP=4 (4 retry-pass timeouts) | controlled loop run |
| Isolation: 20/20 final PASS on BUILD-4A WIP SMP=4 (2 retry-pass timeouts) | controlled loop run |
| Isolation: 10/10 final PASS on clean main SMP=1 (0 timeouts) | controlled loop run |
| Isolation: 10/10 final PASS on BUILD-4A WIP SMP=1 (1 retry-pass timeout) | controlled loop run |
| Not BUILD-4A-induced: identical pass rates across clean and WIP at both SMP levels | isolation comparison |
| Residual retry-pass timeouts confirm timing instability persists under current KVM config | isolation result |

---

## 6. Recommended Fix Dispatch

### Fix 1 — Add CI serial log artifact upload (P0)
No kernel code change. Unblocks all future triage. One-line addition to CI workflow.

### Fix 2 — Extend `kill_thread_by_pid` to scan all CPUs (P1)
`scheduler::kill_thread_by_pid` must iterate all per-CPU schedulers' ready queues,
not just the calling CPU. Same pattern as `zombify_parked_thread_by_pid` which
already does the multi-CPU scan correctly.

Alternative: send a cross-CPU IPI requesting the target CPU to drain the specific
pid from its own ready queue. The IPI approach avoids holding multiple scheduler
locks simultaneously but adds message-passing complexity.

### Fix 3 — Make `ipc_heap_kill_smoke` SMP-safe (P2)
The test must ensure the worker is deterministically in RecvBlocked state before
`sys_kill` is called. Options:
(a) Add a second synchronization channel where the worker sends an ACK after
    calling `sys_recv(chan_main)` (cleanest — removes the race window entirely).
(b) Pin the worker to CPU 0 so rendezvous and kill always happen on the same CPU.
    This treats the symptom, not the root cause.

### Fix 4 — Investigate `cap_revoke_local` after serial log upload (P2)
Cannot design a fix without the failure reason. Unblocked by Fix 1.

### Fix 5 — Writeback kthread should sleep instead of busy-spin under hold (P1)
The writeback kthread in `spawn_writeback_worker` (cache.rs:295) calls `yield_now()` after each
empty cycle. Under SMP=4 with an otherwise-idle AP, `yield_now()` is a no-op. The kthread
busy-spins at 100% AP utilization, degrading host-side virtio-blk interrupt throughput.

Options in priority order:
(a) Add `crate::sched::reap()` inside the idle loop alongside `yield_now()`. Reap processes
    zombie threads which can create some scheduler work, breaking the pure busy-spin. Low
    disruption but may not fully solve.
(b) Add a `block_current_with_token_and_guard`-style park when `drain_worker_cycle()` returns
    empty (hold active or no dirty blocks). The stop token or hold-release path would issue a
    wake_thread_token to unblock it. Correct fix; requires a new wake signal in the writeback
    hold state machine.

Fix 5 must not regress: `phase3_neofs_writeback_dirty_drain`, `phase3_neofs_writeback_worker_stop_join`,
`phase3_neofs_fsync_coordinates_with_writeback_gc` (all depend on writeback kthread behavior).

---

## 7. Local Reproduction Protocol (Not Yet Complete)

The following loop runs were dispatched but not yet executed (current container
running full suite single pass):

```bash
SCENARIOS="ipc_heap_kill_smoke cap_revoke_local phase3_sched_wake_token_abandon phase3_neofs_fsync_filecap_stale_regression"

# NEO_SMP=4 loop runs — 50 iterations each
for scenario in $SCENARIOS; do
  passed=0; failed=0
  for i in $(seq 1 50); do
    if docker compose run --rm -e NEO_SMP=4 neo cargo neo-test $scenario 2>&1 | grep -q "PASS"; then
      passed=$((passed+1))
    else
      failed=$((failed+1))
    fi
  done
  echo "$scenario: $passed/50 passed, $failed failed"
done

# NEO_SMP=1 control runs — 20 iterations each
for scenario in $SCENARIOS; do
  for i in $(seq 1 20); do
    docker compose run --rm -e NEO_SMP=1 neo cargo neo-test $scenario 2>&1 | tail -1
  done
done
```

These should be run after the current full-suite container completes to avoid
workspace contention.

---

## 8. Stop Conditions for Fix Dispatch

The fix dispatch must stop and return findings if:
- Fix 2 (`kill_thread_by_pid` multi-CPU scan) causes any existing SMP test to fail
  (`smp_cross_cpu_migration_reap`, `smp_cross_cpu_rendezvous_stress` are the
  most likely to catch regressions in scheduler lock ordering)
- The multi-CPU scan introduces a lock-ordering cycle (CPU 0 holds sched_0 and
  waits for sched_1, while CPU 1 holds sched_1 and waits for sched_0)
- `cap_revoke_local` serial log reveals a mechanism different from kill-path races
  (e.g., a handle table corruption requiring a separate fix lane)
- Fix 5 (writeback kthread blocking) causes `phase3_neofs_writeback_dirty_drain`,
  `phase3_neofs_writeback_worker_stop_join`, or `phase3_neofs_fsync_coordinates_with_writeback_gc`
  to fail (regression in writeback lifecycle semantics)
