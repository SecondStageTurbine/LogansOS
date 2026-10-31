# Blocker - Phase 3 BUILD-3E Full SMP Sweep

Date: 2026-05-15
Dispatch: `Plans/DISPATCH-2026-05-15-phase-3-build-3-3e-clean-smp-sweep-jarvis.md`
HEAD: `60319e32d348082fa1e5f90ee98288cfcc5bcf42`

## Command

```text
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test --no-meta
```

Captured log:

```text
target/verify-phase3-build3e-clean-smp-sweep.log
```

Timestamps:

```text
start=2026-05-15T14:11:49.5448370-04:00
end=2026-05-15T15:04:25.8847885-04:00
exit=1
```

## Result

The full SMP sweep exited non-zero. Terminal failing scenario:

```text
phase3_neofs_fsync_dirty_persists_after_crash
```

Failure type: two-boot scenario timeout on both attempts.

Sweep log excerpt:

```text
66750: [xtask] neo-test: `phase3_storage_completion_cross_cpu_wake` attempt 1/2 failed: QEMU timed out after 180s for
71040: [xtask] neo-test: `phase3_neofs_mount_rejects_bad_superblock` attempt 1/2 failed: QEMU timed out after 180s for
75008: [xtask] neo-test: `phase3_neofs_reboot_persistence_small_file` two-boot attempt 1/2 failed: QEMU timed out after 180s
86506: [xtask] neo-test: `phase3_neofs_gc_reclaims_orphan_after_overwrite` attempt 1/2 failed: QEMU timed out after 180s for
91246: [xtask] neo-test: `phase3_neofs_cache_hit_miss_smoke` attempt 1/2 failed: QEMU timed out after 180s for
96904: [xtask] neo-test: `phase3_neofs_fsync_dirty_persists_after_crash` two-boot attempt 1/2 failed: QEMU timed out after
96905: 180s for `phase3_neofs_fsync_dirty_persists_after_crash`
97772: xtask: `phase3_neofs_fsync_dirty_persists_after_crash` failed after 2 two-boot attempts; last error: QEMU timed out
97773: after 180s for `phase3_neofs_fsync_dirty_persists_after_crash`; previous error: QEMU timed out after 180s for
97774: `phase3_neofs_fsync_dirty_persists_after_crash`
```

## Serial Evidence

Expected serial paths from the failing sweep:

```text
target/neo-test/phase3_neofs_fsync_dirty_persists_after_crash.boot1.serial.log
target/neo-test/phase3_neofs_fsync_dirty_persists_after_crash.boot2.serial.log
```

Important caveat: these fixed xtask serial paths were overwritten by the required targeted rerun after the full-sweep failure. Before the targeted rerun, the observed failing `boot1` tail had reached the test dispatch and then printed repeated virtio-blk completion lines without a terminal `NEO_TEST/1 event=PASS` or `event=FAIL` before QEMU timed out. The excerpt observed before overwrite was:

```text
NEO_TEST/1 event=BEGIN name=phase3_neofs_fsync_dirty_persists_after_crash phase=dispatch
[ kernel ] sched: spawned 'virtio-blk-completion' (id=7)
[ kernel ] virtio-blk: completion bdf=0x000300 status=0 completion_cpu=0 token_cpu=0 token_slot=0 wake=Woken
[ kernel ] virtio-blk: completion bdf=0x000300 status=0 completion_cpu=0 token_cpu=0 token_slot=0 wake=Woken
[ kernel ] virtio-blk: completion bdf=0x000300 status=0 completion_cpu=0 token_cpu=0 token_slot=0 wake=Woken
```

The post-overwrite targeted rerun serial logs now contain PASS frames and should not be treated as the failed sweep serial contents.

## Targeted Rerun

Command:

```text
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test phase3_neofs_fsync_dirty_persists_after_crash
```

Captured log:

```text
target/verify-phase3-build3e-clean-smp-targeted-dirty-persists.log
```

Result: exit 0.

Targeted rerun PASS excerpt:

```text
[xtask] neo-test: `phase3_neofs_fsync_dirty_persists_after_crash` PASS
[xtask] neo-test: `phase3_neofs_fsync_dirty_persists_after_crash` PASS
[xtask] neo-test: `phase3_neofs_fsync_dirty_persists_after_crash` two-boot persistence PASS
```

Targeted rerun serial PASS frames:

```text
NEO_TEST/1 event=PASS name=phase3_neofs_fsync_dirty_persists_after_crash stage=write neofs_fsync_persist=prepared path=/neofs/fsync-3e-persist.txt bytes=56 dirty_blocks_before_fsync=10 dirty_blocks_after_fsync=0 drained_blocks_delta=10 cache_flushes_delta=1 flush_commands_delta=1 drain_scope=mount_wide selected_superblock=0 generation=1 image=fresh
NEO_TEST/1 event=PASS name=phase3_neofs_fsync_dirty_persists_after_crash stage=readback neofs_fsync_persist=ok path=/neofs/fsync-3e-persist.txt bytes=56 selected_superblock=0 generation=3 image=reused
```

## Classification

This does not currently look like a deterministic BUILD-3E scenario logic failure:

- The same scenario passes when rerun directly under `NEO_SMP=4`.
- Earlier full-sweep runs also showed multiple unrelated attempt-level QEMU timeouts that later retried to PASS.
- This clean-sweep run accumulated unrelated transient timeouts before reaching the terminal failure (`phase3_storage_completion_cross_cpu_wake`, `phase3_neofs_mount_rejects_bad_superblock`, `phase3_neofs_reboot_persistence_small_file`, `phase3_neofs_gc_reclaims_orphan_after_overwrite`, and `phase3_neofs_cache_hit_miss_smoke`).

The failure appears most consistent with an inherited SMP/QEMU/harness timing or storage-completion stress issue that can become terminal when the final scenario exhausts both retry attempts. It is not proven environmental-only because the failing serial was overwritten before durable capture, so the exact in-kernel wait point is not preserved locally.

## Recommended Next Action

Do not close BUILD-3E from this run.

Recommended PM choice:

- Run one more clean SMP sweep only if PM accepts the current evidence as likely transient but wants a clean hard gate.
- If PM wants root cause before another long sweep, authorize a verification-only harness change that preserves per-attempt serial logs with attempt suffixes for retrying scenarios. The fixed serial paths caused the failed-attempt serial evidence to be overwritten by the targeted rerun, which weakens blocker fidelity.
- If PM treats repeated timeout pressure as a kernel/harness defect, open a focused investigation on repeated `NEO_SMP=4` storage/NeoFS timeouts under full-suite pressure before BUILD-3F.
