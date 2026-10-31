# CLOSE REPORT - Phase 3 BUILD-3: NeoFS GC, write-back, and durable fsync

**Date:** 2026-05-16
**Author:** PM / Codex
**Status:** CLOSED
**Authority:**
- `Plans/PM-DISPOSITION-2026-05-15-phase-3-build-3-post-3e-close-plan-consumption.md`
- `Plans/PHASE-3-BUILD-3-CLOSE-EVIDENCE-2026-05-15.md`

---

## 1. Closure Summary

Phase 3 BUILD-3 is closed.

BUILD-3 delivers the storage durability layer above BUILD-2 NeoFS native v1:
owned kthread lifecycle substrate, virtio-blk FLUSH negotiation, NeoFS
free-space GC, write-back buffer cache, and durable `sys_fsync` semantics over
FLUSH-capable virtio-blk devices. The final close evidence records clean
build/check gates, aarch64 runtime barrier evidence, and a full
`NEO_SMP=4 cargo neo-test --no-meta` sweep with exit `0`.

BUILD-3F CAS/dedup was not admitted. It remained measurement-gated throughout
BUILD-3, and no duplicate-block workload or ratio artifact was produced to
justify starting it. Closing BUILD-3 through 3E is therefore consistent with
the accepted BUILD-3 scope.

No BUILD-4, BUILD-3F, CAS/dedup, AHCI/NVMe, foreign filesystem support,
rootfs pivot, mmap/exec, broad POSIX expansion, merge, or push is claimed by
this close report.

The implementation branch is `codex/phase-3-build-3-3e-durable-fsync`, with
close-evidence HEAD at `c799cec`.

---

## 2. Commit Series

| SHA | Sub-commit | Description |
|---|---|---|
| `c799cec` | close evidence | Record Phase 3 BUILD-3 close evidence |
| `747c3a6` | 3E docs fix | Fix BUILD-3E timeout resolution formatting |
| `3779491` | 3E resolution | Resolve BUILD-3E SMP timeout investigation |
| `1da2d9a` | 3E blocker fix | Fix BUILD-3E blocker formatting |
| `6b53e49` | 3E blocker | File BUILD-3E full SMP sweep blocker |
| `60319e3` | 3E evidence hardening | Tighten BUILD-3E fsync evidence |
| `9ef3599` | 3E verification | Record Phase 3 BUILD-3E verification |
| `8cd3549` | 3E | Make NeoFS fsync durable |
| `abd98ed` | 3D verification | Record Phase 3 BUILD-3D verification |
| `b2a93f2` | 3D | Add NeoFS write-back cache |
| `67913b7` | 3C verification | Record Phase 3 BUILD-3C verification |
| `a7eeb21` | 3C | Add NeoFS free-space GC |
| `64693de` | 3B verification | Verify Phase 3 BUILD-3B virtio FLUSH |
| `a3023a0` | 3B | Negotiate virtio-blk FLUSH |
| `e4ac074` | 3A | Add owned kthread lifecycle substrate |

Base from BUILD-2 closure: `096414c`.

No merge to `main` and no push are claimed by this close report.

---

## 3. Verification Artifacts

| Sub-commit | Artifact | Status |
|---|---|---|
| 3A | `Plans/PHASE-3-BUILD-3-SUB-COMMIT-3A-VERIFICATION-2026-05-14.md` | PASS; kthread lifecycle substrate |
| 3B | `Plans/PHASE-3-BUILD-3-SUB-COMMIT-3B-VERIFICATION-2026-05-14.md` | PASS; virtio-blk FLUSH |
| 3C | `Plans/PHASE-3-BUILD-3-SUB-COMMIT-3C-VERIFICATION-2026-05-14.md` | PASS; NeoFS free-space GC |
| 3D | `Plans/PHASE-3-BUILD-3-SUB-COMMIT-3D-VERIFICATION-2026-05-14.md` | PASS; write-back cache |
| 3E | `Plans/PHASE-3-BUILD-3-SUB-COMMIT-3E-VERIFICATION-2026-05-15.md` | Implementation PASS; close initially blocked on clean SMP exit |
| 3E timeout resolution | `Plans/PHASE-3-BUILD-3-SUB-COMMIT-3E-SMP-TIMEOUT-RESOLUTION-2026-05-15.md` | Accepted; no code changes; timeout classified as full-suite/QEMU pressure |
| Close evidence | `Plans/PHASE-3-BUILD-3-CLOSE-EVIDENCE-2026-05-15.md` | PASS; final full SMP sweep exit `0` |

Supporting artifacts consumed:

- `Plans/OBSERVE-2026-05-14-phase-3-build-3-prereq-audit-jarvis.md`
- `Plans/OBSERVE-2026-05-15-phase-3-build-3-post-3e-close-plan-prometheus.md`
- `Plans/PREP-2026-05-15-phase-3-build-3-3e-evidence-matrix-prometheus.md`
- `Plans/PM-DISPOSITION-2026-05-15-phase-3-build-3-3d-consumption-and-3e-authorization.md`
- `Plans/PM-DISPOSITION-2026-05-15-phase-3-build-3-3e-final-acceptance.md`
- `Plans/PM-DISPOSITION-2026-05-15-phase-3-build-3-post-3e-close-plan-consumption.md`

---

## 4. Capability Summary

BUILD-3 delivers:

- Owned kthread lifecycle substrate with spawn, stop, join, detached worker, and
  parked-worker wake coverage.
- Virtio-blk FLUSH negotiation with `VIRTIO_BLK_T_FLUSH` request path,
  completion accounting, and no fake data buffer for FLUSH requests.
- Explicit policy that `VIRTIO_BLK_F_CONFIG_WCE` remains not negotiated.
- NeoFS free-space metadata and mark-sweep GC over BUILD-2 CoW block history.
- Reclaimed-block reuse without live-content aliasing.
- Mount-time rejection of corrupt free-map metadata.
- Legacy BUILD-2 image compatibility through the all-zero superblock extension
  path.
- NeoFS write-back cache with cache hit/miss accounting, dirty-block drain,
  worker stop/join, reboot-after-drain persistence, and crash-before-drain loss
  semantics.
- GC/write-back coordination: GC drains dirty cache state before mark/sweep.
- Durable `sys_fsync` for Regular NeoFS files through `fsync_regular`.
- Dirty fsync persistence across reboot with observed cache drain and virtio
  FLUSH command.
- Clean fsync no-extra-FLUSH behavior from a fully clean mount.
- FileCap stale check before durability work, returning `ESTALE=-116` before
  draining or flushing.

---

## 5. Runtime Evidence

### 5.1 Kthread Lifecycle

3A introduced and verified:

```text
NEO_TEST/1 event=PASS name=phase3_kthread_spawn_join_smoke kthread_slot=0 generation=1 runs=1 joined=true reaped=true
NEO_TEST/1 event=PASS name=phase3_kthread_stop_join_smoke kthread_slot=0 generation=1 stop_observed=true joined=true
NEO_TEST/1 event=PASS name=phase3_kthread_stop_wakes_parked_worker kthread_slot=0 generation=1 parked=true stop_wake=true joined=true
NEO_TEST/1 event=PASS name=phase3_kthread_detached_worker_smoke kthread_slot=0 generation=1 detached=true reaped=true
NEO_TEST/1 event=PASS name=phase3_kthread_no_thread_arc_lifetime kthread_slot=0 generation=1 handle_control_arc=true thread_owner=scheduler thread_arc=false raw_thread=false
```

### 5.2 Virtio FLUSH

3B introduced virtio-blk FLUSH negotiation and command execution:

```text
NEO_TEST/1 event=PASS name=phase3_virtio_blk_flush_negotiated_gate bdf=0x000300 offered_features=0x0000010130007e54 driver_features=0x0000000100000200 F_FLUSH_offered=true F_FLUSH_negotiated=true F_CONFIG_WCE_negotiated=false block_flush_supported=true block_count=32768 block_size=512 attach=rw
NEO_TEST/1 event=PASS name=phase3_virtio_blk_flush_command_smoke bdf=0x000300 request_type=4 VIRTIO_BLK_T_FLUSH=true completions_delta=1 flush_commands_delta=1 flush_completions_delta=1 wake_result=1 retained_dma_buffers=2 has_data_buffer=false readback=ok descriptor_publish_barriers_delta=1 notification_suppression_barriers_delta=1 live_before=6 live_after=6 F_FLUSH_offered=true F_FLUSH_negotiated=true F_CONFIG_WCE_negotiated=false
```

### 5.3 NeoFS Free-Space GC

3C added free-space metadata and foreground mark-sweep GC. The verification
artifact records six new GC scenarios, including mark-live-set, orphan
reclamation after overwrite, reclaimed-block reuse, reboot persistence,
crash-before-publish recovery, and corrupt-free-map rejection.

The reuse scenario proves reclaimed physical blocks can be allocated again
without aliasing live file content: a later file reused block `6`, below
`high_water_before_reuse=16404`, after GC made reclaimed space available.

### 5.4 Write-Back Cache

3D added the NeoFS write-back cache and worker lifecycle:

```text
NEO_TEST/1 event=PASS name=phase3_neofs_cache_hit_miss_smoke neofs_cache=hit_miss path=/neofs/hello.txt misses_delta_first=2 hits_delta_second=6 cached_blocks=2 dirty_blocks=0 selected_superblock=0 generation=1
NEO_TEST/1 event=PASS name=phase3_neofs_writeback_dirty_drain neofs_writeback=dirty_drain path=/neofs/wb-drain.txt dirty_blocks_before=10 dirty_blocks_after=0 drained_blocks_delta=10 cache_flushes_delta=1 data_dirty_marks=1 inode_dirty_marks=2 root_dirty_marks=4 superblock_dirty_marks=6 selected_superblock=0 generation=1
NEO_TEST/1 event=PASS name=phase3_neofs_writeback_worker_stop_join neofs_writeback=worker_stop_join path=/neofs/wb-stop.txt dirty_blocks_before_stop=10 dirty_blocks_after_stop=0 stopped=true joined=true exit_success=true cache_flushes=1 selected_superblock=0 generation=1
NEO_TEST/1 event=PASS name=phase3_neofs_writeback_reboot_after_drain stage=write neofs_writeback_reboot=prepared path=/neofs/wb-persist.txt bytes=57 dirty_blocks_after_drain=0 cache_flushes=1 selected_superblock=0 generation=1 image=fresh
NEO_TEST/1 event=PASS name=phase3_neofs_writeback_reboot_after_drain stage=readback neofs_writeback_reboot=ok path=/neofs/wb-persist.txt bytes=57 selected_superblock=0 generation=3 image=reused
NEO_TEST/1 event=PASS name=phase3_neofs_writeback_crash_before_drain stage=write neofs_writeback_crash=prepared marker=/neofs/wb-crash-marker.txt target=/neofs/wb-crash-dirty.txt marker_dirty_blocks_after_drain=0 dirty_blocks_before_exit=10 writeback_hold=true selected_superblock=0 generation=1 image=fresh
NEO_TEST/1 event=PASS name=phase3_neofs_writeback_crash_before_drain stage=readback neofs_writeback_crash=ok marker=/neofs/wb-crash-marker.txt target=/neofs/wb-crash-dirty.txt dirty_target_absent=true selected_superblock=0 generation=3 image=reused
NEO_TEST/1 event=PASS name=phase3_neofs_gc_waits_for_clean_cache neofs_gc_clean_cache=ok path=/neofs/wb-gc.txt dirty_blocks_before_gc=14 forced_drain_blocks_delta=14 reclaimed_blocks=16390 dirty_blocks_after_gc=4 dirty_blocks_after_release=0 cache_flushes=2 selected_superblock=0 generation=1
```

### 5.5 Durable Fsync

3E changed `sys_fsync` from BUILD-2/3C/3D no-op semantics to durable Regular
NeoFS fsync. The implementation uses the existing narrow `fsync_regular`
insertion point.

Accepted PASS frames include:

```text
NEO_TEST/1 event=PASS name=phase3_neofs_fsync_dirty_persists_after_crash stage=write neofs_fsync_persist=prepared path=/neofs/fsync-3e-persist.txt bytes=56 dirty_blocks_before_fsync=10 dirty_blocks_after_fsync=0 drained_blocks_delta=10 cache_flushes_delta=1 flush_commands_delta=1 drain_scope=mount_wide selected_superblock=0 generation=1 image=fresh
NEO_TEST/1 event=PASS name=phase3_neofs_fsync_dirty_persists_after_crash stage=readback neofs_fsync_persist=ok path=/neofs/fsync-3e-persist.txt bytes=56 selected_superblock=0 generation=3 image=reused
NEO_TEST/1 event=PASS name=phase3_neofs_unfsynced_dirty_may_be_lost stage=write neofs_unfsynced_dirty=prepared marker=/neofs/fsync-3e-marker.txt target=/neofs/fsync-3e-unfsynced.txt marker_dirty_blocks_after_fsync=0 dirty_blocks_before_exit=10 writeback_hold=true no_target_fsync=true selected_superblock=0 generation=1 image=fresh
NEO_TEST/1 event=PASS name=phase3_neofs_unfsynced_dirty_may_be_lost stage=readback neofs_unfsynced_dirty=lost_without_corruption marker=/neofs/fsync-3e-marker.txt target=/neofs/fsync-3e-unfsynced.txt dirty_target_absent=true selected_superblock=0 generation=3 image=reused
NEO_TEST/1 event=PASS name=phase3_neofs_fsync_issues_flush neofs_fsync=issues_flush path=/neofs/fsync-3e-flush.txt device_supports_flush=true dirty_blocks_before=10 dirty_blocks_after=0 drained_blocks_delta=10 cache_flushes_delta=1 flush_commands_delta=1 flush_completions_delta=1 drain_scope=mount_wide selected_superblock=0 generation=1
NEO_TEST/1 event=PASS name=phase3_neofs_fsync_clean_no_extra_flush neofs_fsync=clean_no_extra_flush path=/neofs/hello.txt dirty_blocks_before=0 dirty_blocks_after=0 drained_blocks_delta=0 cache_flushes_delta=0 flush_commands_delta=0 flush_completions_delta=0 selected_superblock=0 generation=1
NEO_TEST/1 event=PASS name=phase3_neofs_fsync_coordinates_with_writeback_gc neofs_fsync_gc=coordinated path=/neofs/fsync-3e-gc.txt dirty_blocks_before_fsync=14 dirty_blocks_after_fsync=0 dirty_blocks_after_gc=4 dirty_blocks_after_gc_fsync=0 dirty_blocks_after_release=0 drained_blocks_delta=14 gc_fsync_drained_blocks_delta=4 reclaimed_blocks=16390 cache_flushes=2 writeback_hold_during_fsync=true drain_scope=mount_wide selected_superblock=0 generation=1
NEO_TEST/1 event=PASS name=phase3_neofs_fsync_filecap_stale_regression path=/neofs/fsync-3e-stale.txt stale_fsync=ESTALE stale_errno=-116 stale_before_durability=true old_cap_generation=7 refreshed_generation=9 current_inode_generation=9 dirty_blocks_before_stale=7 dirty_blocks_after_stale=7 dirty_blocks_after_refreshed_fsync=0 flush_commands_after_stale_delta=0 cache_flushes=2 selected_superblock=0 generation=1
```

Dirty FLUSH evidence asserts `device_supports_flush=true` before using FLUSH
command/completion counters as durability evidence. Clean fsync starts from a
fully clean mount and observes zero dirty drain and zero extra FLUSH. The stale
FileCap test remains anchored to current reachable inode generation and checks
`ESTALE` before durability work.

---

## 6. Regression Evidence

The final close evidence reran the required hard gates on HEAD
`747c3a6afc495d775719ce7152fb7d89f9691bb8` before committing the evidence
artifact:

```text
git diff --check
exit=0

docker compose run --rm neo cargo check -p xtask
exit=0

docker compose run --rm neo cargo check -p neofs-mkfs
exit=0

docker compose run --rm neo cargo k
exit=0
warning_summary=`neoplutarch-kernel` (bin "kernel") generated 74 warnings

docker compose run --rm neo cargo k --no-default-features
exit=0
warning_summary=`neoplutarch-kernel` (bin "kernel") generated 74 warnings

docker compose run --rm neo cargo neo-test phase3_aarch64_virtio_barrier_lane
exit=0

docker compose run --rm -e NEO_SMP=4 neo cargo neo-test --no-meta
exit=0
```

Final full SMP sweep timestamps:

```text
start=2026-05-15T19:48:47.2939626-04:00
end=2026-05-15T20:32:43.9674784-04:00
exit=0
```

The full SMP sweep included transient QEMU timeouts that retried to PASS:

```text
[xtask] neo-test: `phase3_neofs_mount_selects_highest_generation` attempt 1/2 failed: QEMU timed out after 180s for
[xtask] neo-test: `phase3_neofs_mount_selects_highest_generation` PASS after retry attempt 2/2
[xtask] neo-test: `phase3_neofs_reboot_persistence_small_file` two-boot attempt 1/2 failed: QEMU timed out after 180s
[xtask] neo-test: `phase3_neofs_reboot_persistence_small_file` two-boot PASS after retry attempt 2/2
[xtask] neo-test: `phase3_neofs_fsync_filecap_stale_regression` attempt 1/2 failed: QEMU timed out after 180s for
[xtask] neo-test: `phase3_neofs_fsync_filecap_stale_regression` PASS after retry attempt 2/2
```

No `failed after`, `event=FAIL`, `KERNEL PANIC`, or `BUG:` was observed in the
full SMP sweep extraction.

The final staged diff gate for the close-evidence artifact passed:

```text
git diff --cached --check
exit=0
```

---

## 7. AArch64 Runtime Barrier Lane

The aarch64 runtime barrier lane remained a named close gate because the
BUILD-3 prereq audit treated compile-only fallback as a stop condition.

Final close-evidence PASS frame:

```text
NEO_TEST/1 event=PASS name=phase3_aarch64_virtio_barrier_lane runtime=aarch64 qemu_machine=virt transport=virtio_mmio event_idx_negotiated=true feature_low=0x30006e54 feature_high=0x00000101 sector=8 block_size=512 write_ok=true read_ok=true data_integrity=ok publish_descriptors=2 before_notification_check=2 notification_decisions=2 notifications_sent=2 compile_only=false sentinel=runtime_qemu_aarch64
```

This is runtime QEMU evidence, not compile-only evidence.

---

## 8. Course Corrections During BUILD

### 8.1 BUILD-3E full SMP timeout blocker

BUILD-3E implementation was accepted at
`60319e32d348082fa1e5f90ee98288cfcc5bcf42`, but closure was blocked when a
full SMP sweep exited non-zero with `phase3_neofs_fsync_dirty_persists_after_crash`
as the terminal scenario.

The investigation made no kernel code changes. It reran the targeted scenario,
the immediate predecessor mini-suite, the broader NeoFS 3C/3D cluster, and a
full `NEO_SMP=4 cargo neo-test --no-meta` sweep. The later full sweep exited
`0`. PM accepted the classification as full-suite/QEMU timeout pressure with
successful retry behavior, not a deterministic BUILD-3E fsync correctness
failure.

### 8.2 Evidence hardening for BUILD-3E

PM required 3E evidence to state and prove the following explicitly:

- durable fsync uses mount-wide drain, not per-file dirty attribution;
- clean fsync starts from a fully clean mount;
- unfsync'd dirty-loss test holds write-back before the dirty write;
- dirty FLUSH evidence asserts `device.supports_flush() == true`;
- `pause_worker()` / already-started `drain_pass()` race is acceptable because
  foreground `drain_all()` loops until fully clean;
- stale FileCap check remains current reachable inode generation anchored.

Commit `60319e3` tightened tests and evidence accordingly.

---

## 9. Caveats

- C1: 3E durable fsync uses mount-wide drain, not per-file dirty attribution.
- C2: GC remains quiescent.
- C3: no background GC worker landed.
- C4: `VIRTIO_BLK_F_CONFIG_WCE` remains not negotiated.
- C5: transient QEMU timeouts occurred under SMP and were resolved by clean rerun evidence.
- C6: fatal kernel panic/no unwind behavior remains.
- C7: single-thread-per-process invariant remains.
- C8: legacy BUILD-2 image compatibility caveat remains.

Additional detail:

- Mount-wide fsync drain is conservative and correct for BUILD-3E, but it can
  over-drain unrelated dirty files on the same mount.
- GC is foreground and quiescent; it holds metadata/entry-table locks through
  the mark/sweep/publish cycle.
- There is a write-back worker, but no dedicated background GC worker.
- Device write-cache negotiation remains deferred; BUILD-3 relies on explicit
  FLUSH commands.
- xtask currently uses fixed per-scenario serial paths, so failed retry-attempt
  serial logs can be overwritten. Future timeout investigations should preserve
  per-attempt serial logs.
- Kernel panics in worker context remain fatal. BUILD-3 does not add unwind or
  panic recovery.
- Single-thread-per-process remains the scheduler invariant.
- Legacy BUILD-2 images remain mountable through the BUILD-3C all-zero
  extension path.

---

## 10. Deferred Work

Not started in BUILD-3:

- BUILD-3F CAS/dedup. This remains measurement-gated and unadmitted.
- BUILD-4 foreign filesystem read-only support, mount/unmount, directory
  enumeration, unlink/rename, or broader POSIX surface.
- BUILD-5 AHCI, NVMe, bare-metal storage validation, and retype.
- Rootfs pivot from Limine modules to NeoFS.
- File-backed mmap/exec.
- Per-file dirty attribution for fsync cost narrowing.
- Incremental or background GC.
- Device write-cache negotiation via `VIRTIO_BLK_F_CONFIG_WCE`.
- xtask per-attempt serial-log preservation.
- Merge to `main`, push, `docs/phases/PHASE-3.md` updates, or `Plans/INDEX.md`
  updates. Those require separate PM authorization.

---

## 11. Closure Statement

Phase 3 BUILD-3 is **CLOSED**.

NeoFS now has the BUILD-3 durability substrate: owned kernel workers,
FLUSH-capable virtio-blk, free-space GC, write-back cache, and durable fsync
over Regular files. Dirty fsync drains the mount-wide cache and issues FLUSH on
FLUSH-capable devices; clean fsync stays cheap; stale FileCaps fail before
durability work; GC composes with write-back by forcing clean state before
mark/sweep. The final close evidence records successful build/check gates,
runtime aarch64 barrier evidence, and a full `NEO_SMP=4 --no-meta` sweep with
exit `0`.

No BUILD-4, BUILD-3F, CAS/dedup, AHCI/NVMe, foreign filesystem support,
rootfs pivot, mmap/exec, broad POSIX expansion, merge, or push is authorized by
this report.

---

**End of close report.**
