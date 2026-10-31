# Dispatch - Phase 3 BUILD-4 Stability Fix Before BUILD-4B

Date: 2026-05-16
Agent: Jarvis
Status: AUTHORIZED IMPLEMENTATION
Authority:

- `Plans/CI-SMP-RACE-TRIAGE-FINDINGS-2026-05-16.md`
- `Plans/PHASE-3-BUILD-4A-CLOSE-EVIDENCE-2026-05-16.md`
- `docs/ADR-021-phase-3-design-substrate.md`

Substrate:

```text
00df29b docs: correct BUILD-4A close evidence
9324f4b phase3 build4a: directory readdir filecaps
```

## 1. PM Disposition

BUILD-4A is accepted and closed. BUILD-4B is blocked.

The residual retry-pass and CI failures are not BUILD-4A readdir regressions,
but they are no longer acceptable background noise. Before any mount syscall,
foreign filesystem, or BUILD-4B work starts, land a stabilization lane that
addresses the known timing surfaces:

1. CI currently drops the serial evidence needed to diagnose assertion failures.
2. `scheduler::kill_thread_by_pid` only searches the calling CPU's ready queue.
3. The NeoFS writeback kthread busy-spins when held or idle, which degrades I/O
   progress under SMP and changed KVM host timing.

This lane is allowed to make small, mechanism-tied code changes only. It is not
a new feature build.

## 2. Scope

### Fix 1 - CI serial log artifacts

Update `.github/workflows/ci.yml` so failed integration-test jobs upload the
serial logs from `target/neo-test/*.serial.log`.

Required shape:

```yaml
- uses: actions/upload-artifact@v4
  if: failure()
  with:
    name: neo-test-serial-logs
    path: target/neo-test/*.serial.log
    retention-days: 14
```

If the workflow later moves the output directory, keep the artifact path aligned
with `tools/xtask/src/main.rs`'s `TEST_OUTPUT_REL`.

### Fix 2 - Cross-CPU Ready-thread kill

`kernel/src/sched/scheduler.rs::kill_thread_by_pid` must no longer inspect only
`current_scheduler()`. It must scan initialized per-CPU scheduler ready queues
and remove the matching thread wherever it is Ready.

Required semantics:

- scan all initialized per-CPU scheduler pointers;
- hold at most one `PerCpuScheduler.inner` lock at a time;
- if a Ready victim is found, remove it from that scheduler's ready queue;
- set `victim.state = ThreadState::Zombie`;
- push it onto that same scheduler's zombie queue;
- increment that same scheduler's `zombie_count`;
- return `true`;
- return `false` only if no Ready-state thread for the pid exists on any CPU.

Do not introduce a multi-scheduler-lock cycle. Do not add a cross-CPU IPI
protocol for this lane unless the one-lock-at-a-time scan is proven insufficient.

Update the function comment: the operation is no longer local to the current CPU.

### Fix 3 - NeoFS writeback worker parks instead of busy-spinning

`kernel/src/fs/neofs/cache.rs::spawn_writeback_worker` currently does:

```rust
drain_worker_cycle();
yield_now();
```

When `writeback_hold == true`, when foreground writers are active, or when no
dirty work exists, this can become a tight AP loop because `yield_now()` is a
no-op on an otherwise idle per-CPU run queue. Replace that behavior with a real
park/wake discipline.

Required semantics:

- the worker must park when there is no useful worker drain work to perform;
- parking must not lose a wake if dirty work arrives concurrently;
- the worker must wake when dirty work is added;
- the worker must wake when `writeback_hold` transitions from true to false;
- the worker must wake when `foreground_writers` drops to zero;
- the worker must wake on stop request so `stop_writeback_worker` can join;
- foreground `drain_all()` remains mount-wide drain, not per-file dirty
  attribution;
- existing fsync behavior from BUILD-3E must remain unchanged.

Preferred mechanism: add a single worker waiter field to the NeoFS cache inner
state and use the existing scheduler wake-token machinery
(`block_current_with_token_and_guard` / `wake_thread_token`) so the idle worker
is actually blocked, not spinning.

The lost-wake rule is load-bearing: the worker must test the idle condition and
install its waiter while holding the same cache lock that dirty/hold/release
paths use to decide whether to wake it.

### Fix 4 - Deterministic `ipc_heap_kill_smoke` only if still needed

After Fix 2, rerun the targeted `ipc_heap_kill_smoke` loop. If it still fails
or retry-times out under `NEO_SMP=4`, make the test's blocked-kill precondition
deterministic.

Preferred shape: add an ACK synchronization point so the parent kills the worker
only after the worker has actually entered the blocking receive path. Do not pin
the worker to CPU 0 unless PM approves; pinning hides the SMP surface rather
than proving it.

### Fix 5 - `cap_revoke_local` is diagnostic unless evidence appears

Do not invent a cap-table fix without a serial frame or local reproduction. The
CI artifact upload is the first move. If `cap_revoke_local` still fails after
Fixes 1-3, file a blocker with the serial log and exact `NEO_TEST/1 event=FAIL`
or panic evidence.

## 3. Anti-Scope

Do not:

- start BUILD-4B;
- add `SYS_MOUNT`;
- add FAT32 or ext4;
- refactor the mount table;
- alter retry counts or the 180s timeout to hide the issue;
- remove scenarios from the default suite;
- mark flaky scenarios ignored;
- broaden the scheduler API beyond what the kill-path fix needs;
- change filecap, readdir, fsync, or CoW semantics except as directly required
  by writeback worker parking;
- perform unrelated warning cleanup in the same commit unless PM explicitly
  authorizes a tiny separate cleanup.

## 4. Required Verification

Run every command from inside the dev container unless noted otherwise.

### Build gates

```text
cargo k
cargo k --no-default-features
```

Both must pass. Warning count must not increase.

### Targeted loops

Run these under `NEO_SMP=4`. Capture both final result and retry-pass evidence.
The target is zero final failures and zero retry-pass timeouts for the implicated
scenarios unless explicitly justified in the close artifact.

```text
cargo neo-test ipc_heap_kill_smoke
cargo neo-test phase3_sched_wake_token_abandon
cargo neo-test phase3_neofs_fsync_filecap_stale_regression
cargo neo-test phase3_neofs_fsync_noop_write_through
cargo neo-test phase3_build1_reboot_persistence_smoke
```

Minimum loop counts:

- 30 runs each for `ipc_heap_kill_smoke`,
  `phase3_sched_wake_token_abandon`, and
  `phase3_neofs_fsync_filecap_stale_regression`;
- 20 runs each for `phase3_neofs_fsync_noop_write_through` and
  `phase3_build1_reboot_persistence_smoke`.

If a scenario emits a retry-pass, record the run number, scenario, attempt count,
and serial-log path. Do not summarize it away.

### Regression scenarios

Run these once under `NEO_SMP=4` after the targeted loops:

```text
cargo neo-test cap_revoke_local
cargo neo-test smp_cross_cpu_migration_reap
cargo neo-test smp_cross_cpu_rendezvous_stress
cargo neo-test q8c_blocked_kill_walk
cargo neo-test phase3_sched_wake_token_round_trip
cargo neo-test phase3_sched_wake_token_stale_generation
cargo neo-test phase3_neofs_writeback_dirty_drain
cargo neo-test phase3_neofs_writeback_worker_stop_join
cargo neo-test phase3_neofs_fsync_coordinates_with_writeback_gc
cargo neo-test phase3_neofs_writeback_reboot_after_drain
cargo neo-test phase3_neofs_fsync_dirty_persists_after_crash
cargo neo-test phase3_neofs_fsync_issues_flush
```

### Final full gate

```text
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test --no-meta
```

Required result: exit 0.

Also inspect the xtask output for `PASS after retry`. The close artifact must
state whether any retry-pass occurred. If retry-pass remains in the previously
implicated scenarios, this lane is not closed without PM disposition.

## 5. Stop Conditions

Stop and report before committing if:

- the scheduler kill fix requires holding two scheduler locks at once;
- `kill_thread_by_pid` can kill the currently running thread;
- writeback worker parking can sleep while dirty blocks exist and neither
  `writeback_hold` nor `foreground_writers` blocks drain;
- stop/join of the writeback worker can hang because the worker remains parked;
- foreground `drain_all()` stops looping to fully clean;
- `cap_revoke_local` fails and no serial log is available;
- any fix relies on retry/timeout increases rather than removing the timing
  surface;
- any BUILD-4A readdir scenario regresses.

## 6. Close Artifact

File:

```text
Plans/PHASE-3-BUILD-4-STABILITY-FIX-CLOSE-EVIDENCE-2026-05-16.md
```

The artifact must include:

- HEAD commit hash and `git log --oneline -5`;
- `git diff --stat` for each code/CI commit;
- literal build-gate output summaries;
- targeted-loop tables with run counts and retry-pass counts;
- literal `NEO_TEST/1 event=PASS ...` lines for the implicated scenarios;
- explicit statement that CI serial artifacts are configured;
- explicit statement that `kill_thread_by_pid` scans all CPU ready queues one
  lock at a time;
- explicit statement that NeoFS writeback idle/held state parks instead of
  busy-spinning;
- final `NEO_SMP=4 cargo neo-test --no-meta` exit code and retry-pass evidence;
- any remaining instability, named as a blocker or PM-disposition item.

## 7. Commit Policy

Preferred commit split:

1. `ci: upload neo-test serial logs on failure`
2. `sched: scan all CPU ready queues for process kill`
3. `fs: park NeoFS writeback worker when idle`
4. `docs: record Phase 3 BUILD-4 stability fix evidence`

If the implementation is smaller than expected, commits 2 and 3 may be combined
only if the close artifact still separates their verification evidence.

No merge or push until PM accepts the close evidence.
