# Dispatch - Phase 3 BUILD-3E Durable NeoFS Fsync

Date: 2026-05-15
Agent: Jarvis
Status: AUTHORIZED
Authority:
- `Plans/PM-DISPOSITION-2026-05-15-phase-3-build-3-3d-consumption-and-3e-authorization.md`
- `Plans/PHASE-3-BUILD-3-SUB-COMMIT-3D-VERIFICATION-2026-05-14.md`
- `Plans/PREP-2026-05-15-phase-3-build-3-3e-evidence-matrix-prometheus.md`
- `Plans/RESEARCH-2026-05-15-phase-3-build-3-3e-durable-fsync-cerebro.md`

---

## 1. Mission

Implement BUILD-3 sub-commit 3E only: make `sys_fsync(fd)` durable for NeoFS
files by coordinating with the BUILD-3D mount-owned write-back cache and the
BUILD-3B virtio-blk FLUSH path.

Base:

```text
abd98ed5686a96ec0f01a4ea8e276e1c2e8bf42c
```

Recommended branch:

```text
codex/phase-3-build-3-3e-durable-fsync
```

## 2. Required Outcome

3E turns `sys_fsync(fd)` from the BUILD-2/3D no-op into a real durability
operation for NeoFS Regular files.

Required semantics:

- validate `fd` as an existing file handle capability;
- reject stale NeoFS FileCaps before performing durability work;
- for dirty NeoFS Regular files, drain the dirty cache state required to make
  the file and its required metadata durable;
- wait for the relevant write-back work to complete before returning success;
- issue and observe `VIRTIO_BLK_T_FLUSH` when fsync drains dirty data or
  metadata to a FLUSH-capable block device;
- preserve `VIRTIO_BLK_F_FLUSH` negotiation and continue not negotiating
  `VIRTIO_BLK_F_CONFIG_WCE`;
- preserve 3C/3D CoW publication ordering: data, inode/root/free-map metadata,
  superblock copies, then FLUSH;
- coordinate with the background write-back worker and GC so fsync cannot race
  into torn publication or mark/sweep dirty unpublished blocks;
- keep clean-file fsync cheap and deterministic: do not issue a spurious FLUSH
  when there is no relevant dirty NeoFS cache state;
- preserve existing stdout/testfs/VFS compatibility unless a narrower error is
  explicitly required by the current VFS model.

If per-file dirty attribution is not yet available, a conservative mount-wide
dirty drain is acceptable for 3E, but the implementation and verification
artifact must state that explicitly. Do not claim per-file precision unless the
cache actually tracks it.

PM sidecar calibration:

- Prometheus E1-E13 are all hard close gates.
- Cerebro reports no blocker: the 3D substrate is sufficient.
- Mount-wide drain is the accepted 3E implementation posture. The verification
  artifact must say "mount-wide drain, not per-file dirty attribution"
  explicitly.
- The existing 3D `fsync_regular` stub is the intended insertion point; keep
  the implementation narrow.
- The in-flight worker race where `pause_worker` cannot stop an already-started
  `drain_pass` is acceptable only because foreground `drain_all` loops until
  the cache is fully clean. Jarvis must state this in the verification artifact.
- Stale FileCap checking must remain directory-entry anchored against the
  current reachable inode generation.

## 3. Non-Goals

Do not implement:

- CAS/dedup;
- AHCI or NVMe;
- foreign filesystems;
- rootfs pivot;
- mmap or exec;
- broad POSIX semantics;
- native directory creation, unlink, truncate, rename, or symlink support;
- intent log unless PM separately authorizes it;
- background GC expansion beyond existing 3C/3D behavior;
- BUILD-4 or BUILD-3F+ work.

## 4. Insertion Guidance

Likely insertion points:

- syscall path in `kernel/src/arch/x86_64/syscall.rs`;
- VFS/file-handle dispatch in `kernel/src/fs/mod.rs`;
- NeoFS mount/file implementation in `kernel/src/fs/neofs/mount.rs`;
- NeoFS cache drain hooks in `kernel/src/fs/neofs/cache.rs`;
- CoW/generation helpers in `kernel/src/fs/neofs/inode.rs`;
- scenario wiring in `kernel/Cargo.toml`, `kernel/src/test_harness.rs`, and
  `tools/xtask/src/main.rs`.

Prefer extending the existing 3D cache API with a narrow fsync/drain primitive
instead of duplicating write-back logic in syscall code. Fsync should use the
same ordering discipline as the background worker and foreground GC drains.

## 5. Required Runtime Scenarios

Add focused QEMU scenarios and xtask wiring. Names may vary, but cover these
behaviors:

1. `phase3_neofs_fsync_dirty_persists_after_crash`
   - create/write a NeoFS Regular file;
   - call `sys_fsync(fd)`;
   - exit/crash without relying on mount teardown;
   - reboot the same image;
   - prove the fsync'd file data and required metadata survive.

2. `phase3_neofs_unfsynced_dirty_may_be_lost`
   - create a committed marker;
   - call `debug_set_writeback_hold(true)` before creating/writing the second
     dirty file;
   - do not call fsync on the dirty file;
   - crash/reboot;
   - prove the old committed marker remains mountable and the unfsync'd dirty
     file is either absent or old-state, not torn/corrupt.

3. `phase3_neofs_fsync_issues_flush`
   - perform a dirty NeoFS write;
   - call `sys_fsync(fd)`;
   - prove dirty cache count reaches zero;
   - prove `VIRTIO_BLK_T_FLUSH` command and completion counters advance.

4. `phase3_neofs_fsync_clean_no_extra_flush`
   - run on a fully-clean mount, not after unrelated dirty writes;
   - open/read or fsync an already-clean NeoFS file;
   - prove no dirty drain occurs;
   - prove no additional FLUSH command is issued solely for clean fsync.

5. `phase3_neofs_fsync_coordinates_with_writeback_gc`
   - arrange dirty cache state with background write-back/GC-sensitive blocks;
   - call fsync while the worker or GC path could otherwise compete;
   - prove fsync returns only after a clean, ordered checkpoint and GC still
     sees a clean cache before mark/sweep.

6. `phase3_neofs_fsync_filecap_stale_regression`
   - preserve ESTALE `-116` stale FileCap behavior;
   - prove fsync on a stale NeoFS handle fails before durability work;
   - prove a refreshed handle can fsync successfully.

All dirty-FLUSH scenarios must assert `device.supports_flush() == true` in the
QEMU setup path before using FLUSH counters as durability evidence.

Retain or update the 3D fsync no-op scenario so the evidence clearly shows the
contract changed only in 3E and only according to this dispatch.

## 6. Required Regression Gates

Run and report:

```text
rustfmt --edition 2021 <changed Rust files>
git diff --check
cargo check -p xtask
cargo check -p neofs-mkfs
cargo k
cargo k --no-default-features
```

Run the new 3E Docker/QEMU scenarios.

Run targeted regressions:

```text
cargo neo-test phase3_virtio_blk_flush_command_smoke
cargo neo-test phase3_virtio_blk_abandoned_io_no_leak
cargo neo-test phase3_block_vfs_smoke
cargo neo-test phase3_neofs_writeback_dirty_drain
cargo neo-test phase3_neofs_writeback_worker_stop_join
cargo neo-test phase3_neofs_gc_waits_for_clean_cache
cargo neo-test phase3_neofs_gc_reboot_persistence
cargo neo-test phase3_neofs_gc_corrupt_free_map_rejected
cargo neo-test phase3_neofs_filecap_generation_stale
cargo neo-test phase3_aarch64_virtio_barrier_lane
```

Because 3E touches syscall/VFS/cache/flush ordering, run:

```text
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test --no-meta
```

Use the established Docker path if host QEMU/Limine is blocked.

Before committing, run:

```text
git diff --cached --check
```

## 7. Verification Artifact

Create:

```text
Plans/PHASE-3-BUILD-3-SUB-COMMIT-3E-VERIFICATION-2026-05-15.md
```

Include:

- exact implementation commit SHA;
- changed files;
- syscall/VFS fsync contract summary;
- cache drain and worker coordination summary;
- dirty vs clean fsync behavior;
- explicit mount-wide drain statement, with no per-file dirty attribution claim;
- FLUSH command/completion evidence;
- crash/reboot persistence evidence for fsync'd data;
- crash/reboot evidence for unfsync'd dirty-cache behavior;
- GC coordination evidence;
- stale FileCap regression evidence;
- explanation that an already-started write-back `drain_pass` is safe because
  fsync/foreground `drain_all` loops to a fully clean checkpoint;
- targeted regression and full SMP sweep results;
- explicit statement that no CAS/dedup, AHCI/NVMe, foreign FS, rootfs pivot,
  mmap/exec, broad POSIX, BUILD-4, or unauthorized work landed.

Do not leave a placeholder commit SHA in the committed verification artifact.

## 8. Stop Conditions

Stop and file a blocker instead of committing if:

- fsync cannot force durability without weakening CoW/superblock ordering;
- fsync can return before the required dirty cache state and FLUSH complete;
- fsync can race background write-back into torn publication;
- fsync can race GC into mark/sweep over dirty unpublished blocks;
- stale FileCaps can fsync or mutate durability state;
- clean fsync issues unexplained/spurious FLUSH commands;
- crash after fsync loses fsync'd data;
- crash before fsync corrupts old committed state;
- the implementation requires CAS/dedup, AHCI/NVMe, foreign FS, rootfs pivot,
  mmap/exec, broad POSIX, BUILD-4, or other out-of-scope work.

If blocked, create:

```text
Plans/BLOCKER-2026-05-15-phase-3-build-3-3e-durable-fsync-jarvis.md
```

## 9. Commit Discipline

Commit only 3E implementation and its verification or blocker artifact.

Suggested implementation commit message:

```text
kernel/fs: make NeoFS fsync durable
```

Suggested verification commit message:

```text
docs: record Phase 3 BUILD-3E verification
```

Do not push. Do not merge to `main`. Do not start BUILD-3F or BUILD-4.
