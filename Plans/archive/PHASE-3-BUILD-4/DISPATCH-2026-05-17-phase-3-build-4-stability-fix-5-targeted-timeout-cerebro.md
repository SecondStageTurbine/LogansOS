# Dispatch - Phase 3 BUILD-4 Stability Fix 5: Targeted NeoFS I/O Timeout Policy

Date: 2026-05-17
Agent: Cerebro
Status: AUTHORIZED IMPLEMENTATION
Authority:

- `Plans/PHASE-3-BUILD-4-STABILITY-FIX-4B-BACKEND-MATRIX-2026-05-17.md`
- `Plans/PHASE-3-BUILD-4-STABILITY-FIX-4-BLOCKER-2026-05-16.md`
- `Plans/PHASE-3-BUILD-4-STABILITY-FIX-3-BLOCKER-2026-05-16.md`
- `Plans/RESEARCH-2026-05-16-phase-3-build-4-virtio-kvm-timing-prometheus.md`

Substrate:

```text
2377d4d plans: authorize Phase 3 BUILD-4 storage backend matrix
dbe59a3 plans: authorize Phase 3 BUILD-4 stability fix 4
014e5d7 plans: authorize Phase 3 BUILD-4 stability fix 3
584d3e8 plans: authorize Phase 3 BUILD-4 stability fix 2
be85ff1 fs: park NeoFS writeback worker when idle
cbe919f sched: scan all CPU ready queues for process kill
ada2de8 ci: upload neo-test serial logs on failure
```

Active uncommitted substrate to retain:

- Fix 8: per-attempt serial logs;
- Fix 9: narrow virtio-only `wake_thread_token_front`;
- Fix 11: `NEO_QEMU_STORAGE_MODE` env gate, but **default remains writeback**;
- Fix 12: `NEO_TEST_RUN_ID` suffix;
- stale-fsync progress frames.

## 1. PM Disposition

The QEMU backend matrix is complete enough. Storage backend mode is not the
lever:

- `direct-native`: accepted by QEMU but worse final-failure rate;
- `writeback-io-uring`: rejected by the local host/container;
- `writeback-threads`: accepted but 3x worse retry-pass rate and new pre-mount
  stalls.

Do not pursue more QEMU backend variants in this lane. Do not implement
pipelined fsync drain or multi-in-flight virtio. The evidence now supports a
narrow timeout policy for I/O-heavy NeoFS scenarios.

This is not "hide a kernel bug." The invariants pass on completed attempts, and
the timing tail is distributed across mount, seed fsync, and refreshed fsync.
The proximate failure is the 180s per-attempt budget colliding twice with an
I/O-heavy path under `NEO_SMP=4`.

## 2. Scope

### Fix 14 - Per-scenario timeout policy

Add xtask support for per-scenario QEMU timeout selection.

Required behavior:

- default timeout remains 180 seconds for all scenarios;
- I/O-heavy NeoFS scenarios get 360 seconds;
- timeout choice is printed in xtask launch output;
- timeout choice appears in timeout error messages through existing
  `run_with_timeout` reporting;
- allow an optional env override such as `NEO_QEMU_TEST_TIMEOUT_SECS` for
  manual diagnostics, but do not require it for CI;
- invalid env override values must fail clearly before launching QEMU.

Recommended implementation shape:

```text
fn timeout_for_scenario(scenario: TestScenario) -> Duration
```

Use this instead of the global `TEST_TIMEOUT` at the QEMU invocation sites.

### I/O-heavy NeoFS scenario allowlist

Apply 360s only to scenarios that:

- use NeoFS storage disks; and
- perform fsync/flush/writeback/GC or reboot persistence work dense enough to
  run hundreds of virtio operations or durable flushes.

Minimum allowlist:

```text
phase3_neofs_fsync_filecap_stale_regression
phase3_neofs_fsync_dirty_persists_after_crash
phase3_neofs_unfsynced_dirty_may_be_lost
phase3_neofs_fsync_issues_flush
phase3_neofs_fsync_clean_no_extra_flush
phase3_neofs_fsync_coordinates_with_writeback_gc
phase3_neofs_writeback_reboot_after_drain
phase3_neofs_writeback_crash_before_drain
phase3_neofs_gc_reboot_persistence
phase3_neofs_gc_crash_before_publish
phase3_build1_reboot_persistence_smoke
phase3_neofs_reboot_persistence_small_file
phase3_neofs_crash_mid_overwrite_recovery
phase3_neofs_superblock_rotation_recovery
phase3_neofs_crash_mid_create_recovery
```

If you add or remove scenarios from this list, explain why in the evidence
artifact.

### Keep existing useful fixes

Retain and verify:

- per-attempt logs;
- `NEO_TEST_RUN_ID`;
- `wake_thread_token_front` only for virtio-blk completions;
- stale-fsync progress frames;
- `NEO_QEMU_STORAGE_MODE` gate defaulting to the original writeback mode.

Do not make direct-native, io_uring, or threads default.

## 3. Verification

### Build gates

```text
cargo build -p xtask
cargo k
cargo k --no-default-features
```

Warnings must not increase over the 72-warning baseline.

### Timeout-policy smoke

Run:

```text
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test ipc_heap_kill_smoke
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test phase3_neofs_fsync_filecap_stale_regression
```

Evidence must show:

- `ipc_heap_kill_smoke` uses 180s;
- stale-fsync uses 360s;
- both PASS.

Run invalid override smoke outside long loops:

```text
NEO_QEMU_TEST_TIMEOUT_SECS=not-a-number
```

Expected: xtask fails clearly before launching QEMU.

### Targeted loops

Run under default writeback mode, `NEO_SMP=4`, unique `NEO_TEST_RUN_ID`:

```text
cargo neo-test phase3_neofs_fsync_filecap_stale_regression
cargo neo-test phase3_sched_wake_token_abandon
cargo neo-test ipc_heap_kill_smoke
```

Minimum counts:

- stale-fsync: 50 runs;
- wake-token abandon: 30 runs;
- ipc-kill: 20 runs.

Targets:

- stale-fsync: 50/50 final PASS under the 360s policy;
- wake-token abandon: no final failures; retry-pass evidence reported;
- ipc-kill: remains clean.

Retry-pass under stale-fsync should be reported, but a retry-pass that succeeds
under the 360s per-attempt policy is no longer automatically a blocker. A final
failure remains a blocker.

### Regression scenarios

Run once under `NEO_SMP=4`:

```text
cargo neo-test phase3_storage_completion_cross_cpu_wake
cargo neo-test phase3_virtio_blk_flush_command_smoke
cargo neo-test phase3_neofs_fsync_issues_flush
cargo neo-test phase3_neofs_fsync_dirty_persists_after_crash
cargo neo-test phase3_neofs_fsync_coordinates_with_writeback_gc
cargo neo-test phase3_neofs_writeback_worker_stop_join
cargo neo-test phase3_build1_reboot_persistence_smoke
cargo neo-test phase3_vfs_readdir_testfs_root
cargo neo-test phase3_neofs_readdir_root
cargo neo-test phase3_readdir_buffer_too_small
```

### Final full-suite gate

Only after targeted loops and regressions pass:

```text
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test --no-meta
```

Required result: exit 0. Report every `PASS after retry` line explicitly.

## 4. Stop Conditions

Stop and report before committing if:

- timeout policy applies to all scenarios instead of the narrow allowlist;
- scheduler-only scenarios use 360s;
- timeout override can silently parse to zero or another unintended value;
- stale-fsync still gets a final failure under 360s;
- BUILD-4A readdir scenarios regress;
- storage mode default changes away from writeback;
- direct-native/io_uring/threads are made default;
- implementation drifts into pipelined fsync or multi-in-flight virtio.

## 5. Close Artifact

File:

```text
Plans/PHASE-3-BUILD-4-STABILITY-FIX-5-CLOSE-EVIDENCE-2026-05-17.md
```

Include:

- HEAD and `git log --oneline -10`;
- `git diff --stat` by commit;
- build-gate results;
- timeout allowlist and rationale;
- timeout smoke proof;
- invalid override proof;
- targeted-loop tables;
- stale-fsync retry/failure evidence, if any;
- regression scenario PASS lines;
- final full-suite result and retry-pass lines;
- explicit statement whether BUILD-4B is unblocked.

## 6. Commit Policy

Preferred commits:

1. `tools: preserve neo-test logs across repeated runs`
2. `sched: prioritize virtio block I/O wake tokens`
3. `tools: add QEMU storage mode gate`
4. `tools: use extended timeout for I/O-heavy NeoFS tests`
5. `docs: record Phase 3 BUILD-4 stability fix 5 evidence`

Do not push. Do not merge. Do not start BUILD-4B until PM accepts evidence.
