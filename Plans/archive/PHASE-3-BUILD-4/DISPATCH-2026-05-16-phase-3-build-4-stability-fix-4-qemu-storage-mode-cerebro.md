# Dispatch - Phase 3 BUILD-4 Stability Fix 4: QEMU Storage Mode Experiment

Date: 2026-05-16
Agent: Cerebro
Status: AUTHORIZED IMPLEMENTATION / EXPERIMENT
Authority:

- `Plans/PHASE-3-BUILD-4-STABILITY-FIX-3-BLOCKER-2026-05-16.md`
- `Plans/RESEARCH-2026-05-16-phase-3-build-4-virtio-kvm-timing-prometheus.md`
- `Plans/DISPATCH-2026-05-16-phase-3-build-4-stability-fix-3-attempt-logs-and-io-wake-cerebro.md`

Substrate:

```text
014e5d7 plans: authorize Phase 3 BUILD-4 stability fix 3
584d3e8 plans: authorize Phase 3 BUILD-4 stability fix 2
be85ff1 fs: park NeoFS writeback worker when idle
cbe919f sched: scan all CPU ready queues for process kill
ada2de8 ci: upload neo-test serial logs on failure
```

Current active diff:

```text
Fix 8: per-attempt serial logs + NEO_TEST_RUN_ID suffix
Fix 9: wake_thread_token_front, used only by virtio-blk completions
Observation: stale-fsync PROGRESS frames
```

## 1. PM Disposition

Stability Fix 3 is blocked, but its partial work is accepted as the substrate
for the next experiment:

- keep Fix 8;
- keep the `NEO_TEST_RUN_ID` suffix, after smoke verification;
- keep Fix 9's narrow virtio-only `wake_thread_token_front`;
- keep stale-fsync progress frames.

Do not call the lane closed. Do not run the final full suite yet. BUILD-4B
remains blocked.

Prometheus's research changes the next step: do not jump to pipelined fsync or
multi-in-flight virtio. That would be a storage architecture change with a high
blast radius. The next lower-risk mechanism is host/QEMU storage backend
configuration, because the remaining tail is broad across mount, seed fsync,
and refreshed fsync, and the per-attempt logs show successful but slow I/O.

## 2. Scope

### Fix 11 - Env-gated QEMU storage mode

Add an xtask environment gate for the storage-drive backend used by x86 QEMU
storage scenarios.

Suggested env var:

```text
NEO_QEMU_STORAGE_MODE
```

Required behavior:

- unset or empty: preserve the current drive string exactly:
  `if=none,id=hd0,format=raw,cache=writeback,file=<disk>`;
- `writeback`: explicit alias for the current behavior;
- `direct-native`: use `cache=none,aio=native`;
- optional if cheap: `writeback-io-uring` with `cache=writeback,aio=io_uring`;
- optional if cheap: `writeback-threads` with `cache=writeback,aio=threads`.

If QEMU rejects any mode, report the exact stderr and do not continue that
mode's loop.

Important: `aio=native` only has the intended meaning with direct I/O
(`cache=none` or equivalent). Do not add `aio=native` to `cache=writeback` and
claim it tested native AIO; QEMU/libvirt prior art says it falls back or warns
without direct cache mode.

### Fix 12 - Finish log-preservation smoke tests

Before storage-mode loops:

- prove default serial naming still works;
- prove `NEO_TEST_RUN_ID=run-smoke` creates suffixed per-attempt logs;
- prove canonical `<scenario>.serial.log` still points to the latest attempt;
- preserve the smoke output in the blocker/close artifact.

### Fix 13 - Storage-mode comparison loop

Run the stale-fsync scenario under `NEO_SMP=4` with Fixes 8/9 active.

Baseline for comparison:

```text
Current mode with Fix 9: 41/42 PASS, 5 retry-pass, 1 final double-timeout.
```

Run this matrix:

1. `NEO_QEMU_STORAGE_MODE=direct-native`
2. If supported and not worse, extend to 50 stale-fsync runs.
3. If rejected or worse by run 15, stop that mode and try `writeback-io-uring`
   if implemented.
4. If `writeback-io-uring` is unsupported or worse, try `writeback-threads` if
   implemented.

Every loop invocation must set a unique `NEO_TEST_RUN_ID`, e.g.:

```text
NEO_TEST_RUN_ID=direct-native-run-01
```

No watcher should be required after Fix 12. If a watcher is still required,
Fix 12 is incomplete.

## 3. Decision Rules

### Candidate acceptance threshold

A storage mode can become the new default only if it proves:

- 50/50 stale-fsync final PASS;
- zero stale-fsync retry-pass timeouts, or a PM-accepted retry rate strictly
  lower than current Fix 9 behavior;
- `phase3_neofs_fsync_issues_flush` PASS;
- `phase3_neofs_fsync_dirty_persists_after_crash` PASS;
- `phase3_build1_reboot_persistence_smoke` PASS.

If no storage mode beats Fix 9, do not change the default.

### Timeout policy is not authorized yet

Do not raise `TEST_TIMEOUT` in this lane. If all storage modes fail to clear
the tail, file a blocker and recommend either:

- targeted I/O-scenario timeout policy; or
- smaller valid NeoFS test disk; or
- a real storage architecture change.

### Pipelined fsync is not authorized yet

Do not implement pipelined fsync drain or multi-in-flight virtio. Prometheus's
research explicitly flags this as high blast radius. If you still believe it is
the right move, file a design note with the exact required changes, invariants,
and tests before code.

## 4. Verification

Run from the dev container.

### Build gates

```text
cargo k
cargo k --no-default-features
```

Both must pass. Warning count must not increase over the 72-warning baseline.

### Storage-mode proof

For each implemented mode, capture:

- the exact QEMU `-drive` string in xtask output;
- whether QEMU accepted or rejected it;
- any QEMU stderr;
- `phase3_virtio_blk_flush_command_smoke` result;
- `phase3_neofs_fsync_issues_flush` result.

### Targeted loops

For the chosen candidate mode, run:

```text
cargo neo-test phase3_neofs_fsync_filecap_stale_regression
```

Minimum:

- 30 runs before PM check-in;
- extend to 50 only if no final failures;
- all with unique `NEO_TEST_RUN_ID`.

Then run:

```text
cargo neo-test phase3_sched_wake_token_abandon
cargo neo-test ipc_heap_kill_smoke
```

Minimum:

- 30 runs for wake-token abandon;
- 20 runs for ipc-kill.

### Regression scenarios

Only after stale-fsync reaches 50/50 final PASS:

```text
cargo neo-test phase3_storage_completion_cross_cpu_wake
cargo neo-test phase3_virtio_blk_flush_command_smoke
cargo neo-test phase3_neofs_fsync_issues_flush
cargo neo-test phase3_neofs_fsync_dirty_persists_after_crash
cargo neo-test phase3_neofs_fsync_coordinates_with_writeback_gc
cargo neo-test phase3_neofs_writeback_worker_stop_join
cargo neo-test phase3_build1_reboot_persistence_smoke
```

### Final gate

Only after targeted loops and regression scenarios pass:

```text
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test --no-meta
```

Required result: exit 0. Report every `PASS after retry` line explicitly.

## 5. Stop Conditions

Stop and file a blocker if:

- `NEO_TEST_RUN_ID` does not preserve logs across repeated scenario invocations;
- the selected QEMU storage mode changes flush semantics or breaks fsync tests;
- `direct-native` is silently used with cache mode that makes native AIO inert;
- stale-fsync has any final double-timeout under the candidate mode;
- stale-fsync retry-pass rate is not materially better than Fix 9;
- any QEMU mode requires broad CI environment assumptions not present locally;
- any BUILD-4A readdir scenario regresses;
- implementation drifts into pipelined fsync or multi-in-flight virtio.

## 6. Close Or Blocker Artifact

If successful, file:

```text
Plans/PHASE-3-BUILD-4-STABILITY-FIX-4-CLOSE-EVIDENCE-2026-05-16.md
```

If blocked, file:

```text
Plans/PHASE-3-BUILD-4-STABILITY-FIX-4-BLOCKER-2026-05-16.md
```

The artifact must include:

- HEAD and active diff stat;
- build-gate results;
- `NEO_TEST_RUN_ID` smoke proof;
- storage-mode matrix results;
- targeted-loop tables;
- literal stale-fsync PASS frame for the winning mode, if any;
- all retry/failure attempt log paths and last progress frames;
- final full-suite result if reached;
- explicit PM recommendation: accept mode, keep current mode, or authorize
  timeout/disk-size/storage-architecture follow-up.

## 7. Commit Policy

Allowed commits after verification:

1. `tools: preserve neo-test logs across repeated runs`
2. `sched: prioritize virtio block I/O wake tokens`
3. `tools: add QEMU storage mode gate`
4. `docs: record Phase 3 BUILD-4 stability fix 4 evidence`

Do not push. Do not merge. Do not start BUILD-4B.
