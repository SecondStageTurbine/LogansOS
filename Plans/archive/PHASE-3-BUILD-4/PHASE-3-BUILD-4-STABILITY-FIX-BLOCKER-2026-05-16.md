# Phase 3 BUILD-4 Stability Fix Blocker

Date: 2026-05-16
Lane: Phase 3 BUILD-4 stability fix before BUILD-4B
Status: BLOCKED - targeted SMP loop failed before close gates

## HEAD

```text
be85ff1 fs: park NeoFS writeback worker when idle
cbe919f sched: scan all CPU ready queues for process kill
ada2de8 ci: upload neo-test serial logs on failure
75e00a5 plans: authorize Phase 3 BUILD-4 stability fix
00df29b docs: correct BUILD-4A close evidence
9324f4b phase3 build4a: directory readdir filecaps
```

## Implemented Fixes

```text
ada2de8 ci: upload neo-test serial logs on failure
 .github/workflows/ci.yml | 8 ++++++++
 1 file changed, 8 insertions(+)

cbe919f sched: scan all CPU ready queues for process kill
 kernel/src/sched/scheduler.rs | 57 ++++++++++++++++++++++++++-----------------
 1 file changed, 34 insertions(+), 23 deletions(-)

be85ff1 fs: park NeoFS writeback worker when idle
 kernel/src/fs/neofs/cache.rs | 93 +++++++++++++++++++++++++++++++++++++++-----
 kernel/src/fs/neofs/inode.rs |  4 +-
 2 files changed, 86 insertions(+), 11 deletions(-)
```

## Build Gates

Both build gates passed before targeted loops.

```text
target/verify-stability-fix/cargo-k.log
start 2026-05-16T16:10:13.9526073-04:00
end 2026-05-16T16:11:12.5756443-04:00
exit 0
warning summary: neoplutarch-kernel generated 72 warnings

target/verify-stability-fix/cargo-k-no-default-features.log
start 2026-05-16T16:11:18.4538251-04:00
end 2026-05-16T16:12:21.7607713-04:00
exit 0
warning summary: neoplutarch-kernel generated 72 warnings
```

## Targeted Loop Results

All targeted loops ran under `NEO_SMP=4`.

```text
ipc_heap_kill_smoke:
  runs: 30/30
  final PASS: 30
  retry-pass: 0
  final timeout/fail: 0
  summary: target/verify-stability-fix/loops-clean/ipc_heap_kill_smoke/summary.txt

phase3_sched_wake_token_abandon:
  runs: 30/30
  final PASS: 30
  retry-pass: 5
  retry runs: 07,09,21,22,25
  final timeout/fail: 0
  summary: target/verify-stability-fix/loops-clean/phase3_sched_wake_token_abandon/summary.txt

phase3_neofs_fsync_filecap_stale_regression:
  runs: stopped after 25/30 because the targeted gate had already failed
  final PASS: 23
  retry-pass: 4
  retry-pass runs: 06,13,17,23
  final timeout/fail: 2
  final timeout/fail runs: 19,20
```

`ipc_heap_kill_smoke` did not justify optional Fix 4. The required ACK synchronization was not implemented because the dispatch authorized it only if the scenario still failed or retry-timed out after Fix 2.

`phase3_sched_wake_token_abandon` still produced retry-pass evidence. That alone would require PM disposition before closure.

`phase3_neofs_fsync_filecap_stale_regression` produced final failures, so the lane is blocked before regression scenarios or the final full `NEO_SMP=4 cargo neo-test --no-meta` gate.

## Literal PASS Frames

Representative serial PASS frames from the completed/passing loop runs:

```text
NEO_TEST/1 event=PASS name=ipc_heap_kill_smoke leak_audit=clean exit_code=0
NEO_TEST/1 event=PASS name=phase3_sched_wake_token_abandon token_cpu=0 token_slot=0 generation=1 late_wake=Gone abandon=zombified_and_reaped
NEO_TEST/1 event=PASS name=phase3_neofs_fsync_filecap_stale_regression path=/neofs/fsync-3e-stale.txt stale_fsync=ESTALE stale_errno=-116 stale_before_durability=true old_cap_generation=7 refreshed_generation=9 current_inode_generation=9 dirty_blocks_before_stale=7 dirty_blocks_after_stale=7 dirty_blocks_after_refreshed_fsync=0 flush_commands_after_stale_delta=0 cache_flushes=2 selected_superblock=0 generation=1
```

Full per-run serial logs are under:

```text
target/verify-stability-fix/loops-clean/<scenario>/run-XX.serial.log
```

## Blocker Evidence

Run 19:

```text
path=target/verify-stability-fix/loops-clean/phase3_neofs_fsync_filecap_stale_regression/run-19.log
[xtask] neo-test: `phase3_neofs_fsync_filecap_stale_regression` attempt 1/2 failed: QEMU timed out after 180s for
[xtask] neo-test: retrying `phase3_neofs_fsync_filecap_stale_regression` after transient QEMU failure candidate
xtask: `phase3_neofs_fsync_filecap_stale_regression` failed after 2 attempts; last error: QEMU timed out after 180s
for `phase3_neofs_fsync_filecap_stale_regression`; previous error: QEMU timed out after 180s for
```

Run 19 serial facts:

```text
path=target/verify-stability-fix/loops-clean/phase3_neofs_fsync_filecap_stale_regression/run-19.serial.log
lines=196
last_line=[ kernel ] virtio-blk: completion bdf=0x000300 status=0 completion_cpu=0 token_cpu=0 token_slot=0 wake=Woken
pass_frames=0
fail_frames=0
```

Run 20:

```text
path=target/verify-stability-fix/loops-clean/phase3_neofs_fsync_filecap_stale_regression/run-20.log
[xtask] neo-test: `phase3_neofs_fsync_filecap_stale_regression` attempt 1/2 failed: QEMU timed out after 180s for
[xtask] neo-test: retrying `phase3_neofs_fsync_filecap_stale_regression` after transient QEMU failure candidate
xtask: `phase3_neofs_fsync_filecap_stale_regression` failed after 2 attempts; last error: QEMU timed out after 180s
for `phase3_neofs_fsync_filecap_stale_regression`; previous error: QEMU timed out after 180s for
```

Run 20 serial facts:

```text
path=target/verify-stability-fix/loops-clean/phase3_neofs_fsync_filecap_stale_regression/run-20.serial.log
lines=509
last_line=[ kernel ] virtio-blk: completion bdf=0x000300 status=0 completion_cpu=0 token_cpu=0 token_slot=0 wake=Woken
pass_frames=0
fail_frames=0
```

## Disposition

The lane cannot close in this state. The CI artifact, cross-CPU ready-queue kill, and NeoFS worker parking fixes are implemented and build, but the targeted stability gate still exposes residual SMP instability:

- `phase3_sched_wake_token_abandon`: 5 retry-pass events in 30 runs.
- `phase3_neofs_fsync_filecap_stale_regression`: 2 final double-timeout failures by run 25.

No retry/timeout counts were changed. No scenarios were removed or ignored. BUILD-4B remains blocked.
