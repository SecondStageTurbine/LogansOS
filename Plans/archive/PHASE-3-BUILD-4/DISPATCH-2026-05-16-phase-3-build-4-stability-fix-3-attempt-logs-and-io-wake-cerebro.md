# Dispatch - Phase 3 BUILD-4 Stability Fix 3: Preserve Retry Evidence And Test I/O Wake Priority

Date: 2026-05-16
Agent: Cerebro
Status: AUTHORIZED IMPLEMENTATION
Authority:

- `Plans/PHASE-3-BUILD-4-STABILITY-FIX-2-BLOCKER-2026-05-16.md`
- `Plans/DISPATCH-2026-05-16-phase-3-build-4-stability-fix-2-ap-idle-jarvis.md`
- `Plans/PHASE-3-BUILD-4-STABILITY-FIX-BLOCKER-2026-05-16.md`

Substrate:

```text
584d3e8 plans: authorize Phase 3 BUILD-4 stability fix 2
be85ff1 fs: park NeoFS writeback worker when idle
cbe919f sched: scan all CPU ready queues for process kill
ada2de8 ci: upload neo-test serial logs on failure
```

Current worktree note:

```text
Fix 6 AP idle halt and stale-fsync progress frames are implemented but uncommitted.
```

## 1. PM Disposition

Fix 6 is blocked and is not accepted as a commit in its current form.

Accepted from Fix 6:

- stale-fsync `event=PROGRESS` frames are useful and should be retained;
- the race-safety argument for `cli -> ready-check -> sti;hlt` is coherent.

Not accepted from Fix 6:

- AP idle halt did not clear the stale-fsync timing tail;
- the early post-fix sample worsened retry-pass rate;
- run 4 lost the only useful attempt-2 serial log because xtask overwrote it.

The immediate next fix is not another blind timing change. First preserve
per-attempt evidence so retry/final-failure logs cannot be overwritten. Then
test the next concrete scheduler/storage mechanism: block-I/O completion should
run the woken I/O waiter promptly instead of pushing it to the back of the same
CPU's ready queue and waiting for timer/yield cadence.

BUILD-4B remains blocked.

## 2. Required Worktree Handling

Before coding this lane:

1. Preserve the current uncommitted Fix 6 state in a named stash or patch file.
2. Restore the AP idle halt changes from:
   - `kernel/src/arch/x86_64/mod.rs`
   - `kernel/src/sched/scheduler.rs`
3. Keep the stale-fsync `event=PROGRESS` frames in `kernel/src/test_harness.rs`.

Rationale: the AP halt change is not accepted yet, and the next measurement must
not include it as an uncontrolled variable. The progress frames are observation,
not mechanism, and should stay.

If you cannot cleanly preserve progress frames while removing AP halt, stop and
report before continuing.

## 3. Scope

### Fix 8 - Preserve serial logs per retry attempt

`tools/xtask/src/main.rs` must stop overwriting the only serial log during retry
loops.

Required semantics:

- every QEMU attempt writes to a unique serial log path;
- regular scenario attempts use a stable shape such as:
  - `<scenario>.attempt1.serial.log`
  - `<scenario>.attempt2.serial.log`
- the final/canonical `<scenario>.serial.log` may remain as a convenience copy,
  but it must not be the only copy;
- error messages must include the exact serial log path for the failed attempt;
- retry-pass output must preserve both the failed attempt log and the passing
  attempt log;
- CI artifact upload must include the per-attempt logs.

Apply the same principle to aarch64 and two-boot retry wrappers if the change is
small and mechanical. If not, regular x86 scenario attempts are mandatory in
this lane, and the other wrappers get a documented follow-up.

Stop condition: a final failure must never again report "last progress stage
unknown because the serial log was overwritten."

### Fix 9 - Prefer the woken I/O waiter on virtio completion

The stale-fsync timeout tails show many successful virtio completions:

```text
virtio-blk: completion ... completion_cpu=0 token_cpu=0 ... wake=Woken
```

For same-CPU token wakes, `wake_thread_token` currently pushes the woken thread
to the back of the ready queue and sends no reschedule IPI because the target is
already the current CPU. In an I/O-heavy serialized path, that can add timer or
cooperative-yield latency to every block operation.

Add a narrow scheduler API for priority token wake, then use it only from the
virtio-blk completion path.

Required semantics:

- default `wake_thread_token` behavior remains unchanged for existing callers;
- add a separate function, for example `wake_thread_token_front`;
- `wake_thread_token_front` uses the same generation checks and stale/gone
  semantics as `wake_thread_token`;
- on successful wake, it pushes the woken thread to the front of the target
  CPU's ready queue instead of the back;
- if target CPU differs from current CPU, it still sends the reschedule IPI;
- use the front-wake function only in `kernel/src/drivers/block/virtio_blk.rs`
  completion handling;
- do not change kthread join, stop-token, or general scheduler wake-token
  semantics in this lane.

This is intentionally narrower than "all wake tokens become front-of-queue."
Storage I/O completions are latency-sensitive and serialized; other wake-token
users may depend on the existing fairness profile.

### Fix 10 - Smaller NeoFS disk is deferred unless evidence demands it

Do not implement the smaller-disk option in this lane unless Fixes 8 and 9 still
leave stale-fsync timing out and PM explicitly authorizes it.

Reason:

- the blocker suggestion of a 16 MiB disk is incompatible with the current
  NeoFS format; `neofs-mkfs` requires all three superblock copies, including
  the fixed 64 MiB copy;
- the true minimum image is just over 64 MiB;
- the free map is already one block at 128 MiB, so disk shrink is not guaranteed
  to remove the observed tail.

If disk-size reduction becomes necessary, propose a concrete valid size
(probably 72 MiB or 80 MiB), prove `neofs-mkfs create/verify` accepts it, and
explain what I/O is reduced.

## 4. Verification

Run from the dev container.

### Build gates

```text
cargo k
cargo k --no-default-features
```

Both must pass. Warning count must not increase over the current 72-warning
baseline.

### Attempt-log smoke

Run one intentionally retry-prone or manually interrupted regular scenario if
needed to prove per-attempt logs are preserved. At minimum, prove that a normal
scenario writes:

```text
target/neo-test/<scenario>.attempt1.serial.log
```

and that retry paths preserve both attempt logs.

### Targeted loops

Run under `NEO_SMP=4`:

```text
cargo neo-test phase3_neofs_fsync_filecap_stale_regression
cargo neo-test phase3_sched_wake_token_abandon
cargo neo-test ipc_heap_kill_smoke
```

Minimum counts:

- `phase3_neofs_fsync_filecap_stale_regression`: 50 runs.
- `phase3_sched_wake_token_abandon`: 30 runs.
- `ipc_heap_kill_smoke`: 20 runs.

Close target:

- zero final failures;
- zero retry-pass timeouts for `phase3_neofs_fsync_filecap_stale_regression`;
- `phase3_sched_wake_token_abandon` remains no worse than the Fix 6 blocked
  sample and preferably stays clean.

If stale-fsync times out, preserve both attempt logs and classify by the last
observed `event=PROGRESS` frame.

### Regression scenarios

Run once under `NEO_SMP=4`:

```text
cargo neo-test phase3_storage_completion_cross_cpu_wake
cargo neo-test phase3_sched_wake_token_round_trip
cargo neo-test phase3_sched_wake_token_stale_generation
cargo neo-test phase3_neofs_writeback_dirty_drain
cargo neo-test phase3_neofs_writeback_worker_stop_join
cargo neo-test phase3_neofs_fsync_coordinates_with_writeback_gc
cargo neo-test phase3_neofs_fsync_dirty_persists_after_crash
cargo neo-test phase3_neofs_fsync_issues_flush
cargo neo-test smp_cross_cpu_migration_reap
cargo neo-test smp_cross_cpu_rendezvous_stress
```

### Final gate

Only after targeted loops and regressions pass:

```text
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test --no-meta
```

Required result: exit 0. Report every `PASS after retry` line explicitly.

## 5. Stop Conditions

Stop and report before committing if:

- per-attempt serial logs can still be overwritten during retries;
- the AP idle halt change remains in the implementation diff;
- `wake_thread_token_front` changes default token-wake behavior;
- non-storage wake-token users are switched to front-of-queue;
- a stale-fsync timeout occurs and the last progress stage is still unknown;
- stale-fsync still has any final failure after Fixes 8 and 9;
- stale-fsync still has retry-pass timeouts after Fixes 8 and 9;
- any scheduler wake-token scenario regresses;
- any BUILD-4A readdir scenario regresses.

## 6. Close Or Blocker Artifact

If successful, file:

```text
Plans/PHASE-3-BUILD-4-STABILITY-FIX-3-CLOSE-EVIDENCE-2026-05-16.md
```

If blocked, file:

```text
Plans/PHASE-3-BUILD-4-STABILITY-FIX-3-BLOCKER-2026-05-16.md
```

The artifact must include:

- HEAD commit hash and `git log --oneline -10`;
- exact build-gate results;
- `git diff --stat` for each commit;
- proof that per-attempt serial logs are preserved;
- targeted-loop tables with retry-pass counts;
- literal stale-fsync PASS frame from at least one passing run;
- if any timeout occurs, the failed attempt log path and last progress frame;
- final full-suite result if reached;
- explicit statement whether BUILD-4B remains blocked.

## 7. Commit Policy

Allowed implementation commits:

1. `tools: preserve neo-test serial logs per retry attempt`
2. `sched: prioritize virtio block I/O wake tokens`
3. `docs: record Phase 3 BUILD-4 stability fix 3 evidence`

Do not commit the AP idle halt change in this lane. Keep or commit stale-fsync
progress frames with the evidence/tooling commit if they remain useful.

Do not push. Do not merge. Do not start BUILD-4B.
