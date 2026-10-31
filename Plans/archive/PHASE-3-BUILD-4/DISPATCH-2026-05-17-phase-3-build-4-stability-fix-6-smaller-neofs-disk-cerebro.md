# Dispatch - Phase 3 BUILD-4 Stability Fix 6: Smaller Valid NeoFS Test Disk

Date: 2026-05-17
Agent: Cerebro
Status: AUTHORIZED IMPLEMENTATION
Authority:

- `Plans/PHASE-3-BUILD-4-STABILITY-FIX-5-BLOCKER-2026-05-17.md`
- `Plans/DISPATCH-2026-05-17-phase-3-build-4-stability-fix-5-targeted-timeout-cerebro.md`
- `Plans/RESEARCH-2026-05-16-phase-3-build-4-virtio-kvm-timing-prometheus.md`

Substrate:

```text
8f1a4b2 plans: authorize Phase 3 BUILD-4 targeted timeout policy
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
- Fix 11: `NEO_QEMU_STORAGE_MODE` env gate, default writeback;
- Fix 12: `NEO_TEST_RUN_ID` suffix;
- Fix 14: targeted 360s timeout for I/O-heavy NeoFS;
- stale-fsync progress frames.

## 1. PM Disposition

The targeted timeout policy is not sufficient. It moved the tail:

- under Fix 9 / 180s, stale-fsync timeouts clustered after `mount_done`, in
  seed fsync;
- under Fix 14 / 360s, all four timed-out attempts stalled before `mount_done`.

That makes mount validation the current dominant tail contributor. Mount
validation scales with NeoFS image size. The next authorized lane is therefore
a smaller valid NeoFS test disk, not a larger timeout and not a storage
architecture refactor.

This remains test-infrastructure scope. Do not change kernel NeoFS semantics.

## 2. Scope

### Fix 15 - Add per-scenario NeoFS image-size policy

Add xtask support for selecting NeoFS image size per scenario.

Required behavior:

- default NeoFS image size remains the mkfs default (`128 MiB`);
- selected I/O-heavy NeoFS scenarios use a smaller valid image;
- image-size choice is printed in xtask output;
- mkfs invocation passes `--size-mib <N>` only when a scenario-specific policy
  requires it;
- normal/non-I/O-heavy NeoFS scenarios keep default size unless explicitly
  justified.

Recommended size:

```text
72 MiB
```

Rationale:

- current NeoFS format requires all three superblock copies;
- fixed third superblock copy is at 64 MiB;
- `neofs-mkfs` minimum is just over 64 MiB;
- 72 MiB preserves the three-copy invariant while reducing mount validation
  work by about 44% versus 128 MiB.

Do not use 16 MiB. It is invalid under the current three-superblock policy.

### Scenario allowlist

Apply the smaller disk to the same I/O-heavy set that receives the 360s timeout
unless evidence shows a scenario needs the 128 MiB default.

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

If you use a narrower allowlist, justify it. The stale-fsync scenario is
mandatory.

### Fix 16 - Prove mkfs and mount accept the smaller image

Before loops, prove:

- `neofs-mkfs create --size-mib 72 --variant normal` succeeds;
- `neofs-mkfs verify` succeeds;
- output still reports `superblock_copies=3`;
- the kernel mounts the 72 MiB image and reports the expected superblock,
  root, and model policy behavior through existing smoke scenarios.

## 3. Verification

### Build gates

```text
cargo build -p xtask
cargo k
cargo k --no-default-features
```

Warnings must not increase over the 72-warning baseline.

### Mkfs proof

Run host-side:

```text
cargo run -p neofs-mkfs -- create --output target/neo-test/neofs-72m-smoke.raw --size-mib 72 --variant normal
cargo run -p neofs-mkfs -- verify target/neo-test/neofs-72m-smoke.raw
```

Capture:

- image size;
- `superblock_copies=3`;
- `root_inode_block`;
- `next_free_block`;
- `free_map_block`;
- verify PASS line.

### Timeout and disk-size smoke

Run:

```text
docker compose run --rm -e NEO_SMP=4 -e NEO_TEST_RUN_ID=small-disk-smoke neo cargo neo-test phase3_neofs_fsync_filecap_stale_regression
```

Evidence must show:

- timeout = 360s;
- disk size = 72 MiB, or exact selected smaller size;
- default writeback QEMU storage mode;
- PASS frame.

Also run one NeoFS scenario outside the smaller-disk allowlist and prove it
uses the default 128 MiB size. Suggested:

```text
phase3_neofs_mount_readonly_root
```

### Targeted loops

Run under default writeback mode, `NEO_SMP=4`, 360s timeout, smaller disk,
unique `NEO_TEST_RUN_ID`:

```text
cargo neo-test phase3_neofs_fsync_filecap_stale_regression
```

Minimum:

- 50 runs;
- stop on first final failure;
- preserve all retry/failure attempt logs.

If stale-fsync reaches 50/50 final PASS, then run:

```text
cargo neo-test phase3_sched_wake_token_abandon
cargo neo-test ipc_heap_kill_smoke
```

Minimum:

- wake-token abandon: 30 runs;
- ipc-kill: 20 runs.

### Regression scenarios

Only after targeted loops pass:

```text
cargo neo-test phase3_neofs_mount_readonly_root
cargo neo-test phase3_neofs_fsync_issues_flush
cargo neo-test phase3_neofs_fsync_dirty_persists_after_crash
cargo neo-test phase3_neofs_fsync_coordinates_with_writeback_gc
cargo neo-test phase3_neofs_writeback_worker_stop_join
cargo neo-test phase3_build1_reboot_persistence_smoke
cargo neo-test phase3_neofs_reboot_persistence_small_file
cargo neo-test phase3_neofs_crash_mid_overwrite_recovery
cargo neo-test phase3_neofs_superblock_rotation_recovery
cargo neo-test phase3_neofs_crash_mid_create_recovery
cargo neo-test phase3_vfs_readdir_testfs_root
cargo neo-test phase3_neofs_readdir_root
cargo neo-test phase3_readdir_buffer_too_small
```

### Final full-suite gate

Only after targeted loops and regressions pass:

```text
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test --no-meta
```

Required: exit 0. Report all `PASS after retry` lines.

## 4. Stop Conditions

Stop and report before committing if:

- `neofs-mkfs --size-mib 72` fails;
- verify does not report three superblock copies;
- kernel mount rejects the smaller image;
- smaller-disk policy applies globally instead of the allowlist;
- stale-fsync still gets a final double-timeout;
- timeout policy changes beyond the already-authorized 360s allowlist;
- storage mode default changes away from writeback;
- implementation drifts into pipelined fsync or multi-in-flight virtio;
- any BUILD-4A readdir scenario regresses.

## 5. Close Or Blocker Artifact

If successful, file:

```text
Plans/PHASE-3-BUILD-4-STABILITY-FIX-6-CLOSE-EVIDENCE-2026-05-17.md
```

If blocked, file:

```text
Plans/PHASE-3-BUILD-4-STABILITY-FIX-6-BLOCKER-2026-05-17.md
```

Include:

- HEAD and `git log --oneline -12`;
- `git diff --stat` by commit;
- build-gate results;
- mkfs/verify 72 MiB proof;
- timeout and disk-size smoke proof;
- targeted-loop table;
- retry/failure log paths and last progress frames;
- regression scenario PASS lines;
- final full-suite result if reached;
- explicit recommendation: close, accept with caveat, or authorize storage
  architecture lane.

## 6. Commit Policy

Preferred commits:

1. `tools: preserve neo-test logs across repeated runs`
2. `sched: prioritize virtio block I/O wake tokens`
3. `tools: add QEMU storage mode gate`
4. `tools: tune NeoFS I/O-heavy test budgets`
5. `docs: record Phase 3 BUILD-4 stability fix evidence`

Commit 4 may include both the 360s timeout policy and the smaller-disk policy
if they are implemented in the same xtask area and verified together.

Do not push. Do not merge. Do not start BUILD-4B until PM accepts evidence.
