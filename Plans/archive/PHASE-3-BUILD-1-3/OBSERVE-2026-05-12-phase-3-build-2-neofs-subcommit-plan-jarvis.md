# OBSERVE - Phase 3 BUILD-2 NeoFS sub-commit plan

**Date:** 2026-05-12
**Author:** Jarvis
**Status:** OBSERVE COMPLETE
**Mode:** read-only planning artifact
**Dispatch:** `Plans/DISPATCH-2026-05-12-phase-3-build-2-observe-and-subcommit-plan-jarvis.md`

---

## 1. Current Substrate Map

BUILD-1 gives BUILD-2 useful seams. BUILD-2 should extend these seams, not
replace them.

### Block substrate

- `kernel/src/drivers/block/mod.rs` exposes `BlockDevice`, `BlockGeometry`,
  `BlockRequest`, `BlockOp`, `BlockError`, `read_block`, and `write_block`.
- The API is synchronous from the filesystem caller's view, while virtio-blk
  completes internally through an asynchronous completion/wake path.
- `BlockRequest.buffers: &[Arc<DmaBuffer>]` is already scatter/gather-shaped
  even though BUILD-1 virtio-blk currently accepts exactly one data buffer.
- `get_device(0)` is the current root-device selection seam for test scenarios.
  BUILD-2 can initially mount device 0 without adding mount namespaces or device
  capability plumbing.

### DMA and lifetime substrate

- `kernel/src/drivers/block/dma.rs` provides `DmaBuffer::alloc_zeroed`,
  physical addresses, raw/slice accessors, and Drop-based frame return.
- `Arc<DmaBuffer>` is already the lifetime carrier across storage completion.
  NeoFS block read/write should keep that model and avoid borrowing stack/local
  buffers across driver submission.

### Virtio-blk substrate

- `kernel/src/drivers/block/virtio_blk.rs` already enforces the BUILD-1
  durability posture: `F_FLUSH` must be offered, `F_FLUSH` is not negotiated,
  and `CONFIG_WCE` is not negotiated.
- Descriptor-publish and notification-suppression barriers are separated via
  `virtio_barrier::{publish_descriptors,before_notification_check}` and have an
  aarch64 runtime lane.
- The driver currently supports one in-flight request per device and
  single-block/single-buffer operations. NeoFS native v1 should assume this
  narrow queueing model until BUILD-3/BUILD-5 changes it.

### VFS and capability substrate

- `kernel/src/fs/mod.rs` defines `FileRights(u32)`, `InodeKind`, `VfsError`,
  the `Inode` trait, and `OpenFile`.
- BUILD-1 already distinguishes `TypeForbidden` from `InsufficientRights` and
  maps them to distinct errno values (`EISDIR` and `EACCES`) in the syscall
  layer.
- `OpenFile` stores an `Arc<dyn Inode>`, rights, and offset. This should remain
  the core file-handle object for BUILD-2.
- The current mount table is a single in-memory `TestFs` root. BUILD-2 should
  replace the root backend with a mountable filesystem object, not replace the
  `OpenFile` or `Inode` shape wholesale.

### Syscall and libneo substrate

- Existing syscall numbers are:
  - `SYS_WRITE = 1`
  - `SYS_OPEN = 12`
  - `SYS_READ = 13`
  - `SYS_CLOSE = 14`
- `sys_write(1, ...)` is an explicit serial-stdout namespace rule. It must
  remain stable: `sys_open` must continue not returning fd 1, and fd 1 must not
  resolve through the process handle table.
- `sys_read`, `sys_write` on file handles, and `sys_close` already go through
  `Handle::File { file, cap_id, rights }`.
- `libneo` mirrors the syscall numbers and exposes `FileHandle` plus
  `FileRights(u32)`. Existing wrappers must remain source-compatible for
  BUILD-1 tests.

### Test and xtask substrate

- `tools/xtask/src/main.rs` exposes `TestScenario` entries and makes every
  scenario whose name starts with `phase3_` receive a raw storage disk.
- `prepare_storage_disk` currently creates a blank 16 MiB image with a BUILD-1
  marker. BUILD-2 should extend this seam to seed mkfs-created NeoFS images.
- `phase3_build1_reboot_persistence_smoke` already has a two-boot disk reuse
  path. BUILD-2 should reuse that shape for mount/recovery tests.
- `kernel/src/test_harness.rs` already validates block read/write, VFS syscalls,
  storage completion, barrier counts, and two-boot persistence.

---

## 2. Extension Seams and Files Likely Touched

### Recommended module placement

NeoFS should live under `kernel/src/fs/`, because it consumes block devices and
implements VFS inodes. It should not live under `drivers/block/`.

Recommended tree:

```text
kernel/src/fs/
  mod.rs
  neofs/
    mod.rs
    format.rs
    mount.rs
    inode.rs
    alloc.rs
    cow.rs
    policy.rs
```

`format.rs` should contain endian-aware on-disk parsing/serialization helpers.
`mount.rs` should own superblock/root validation. `inode.rs` should implement
the `Inode` trait. `alloc.rs` should own the physical block allocator. `cow.rs`
should own transaction/superblock rotation once 2D begins. `policy.rs` should
own typed-inode operation policy once 2E begins.

The current `TestFs` can stay in `fs/mod.rs` during 2A/2B. If it becomes noisy,
split it into `kernel/src/fs/testfs.rs` as a mechanical cleanup inside the same
sub-commit that adds the first NeoFS module.

### Mkfs tooling placement

Mkfs should be a host-side tool, not kernel code.

Recommended tree:

```text
tools/neofs-mkfs/
  Cargo.toml
  src/main.rs
  src/format.rs
```

`tools/xtask` should invoke this tool or share a small host-side format library
when preparing BUILD-2 test images. Do not put mkfs logic directly into the
kernel. Do not make `docs/specs/neofs-format.md` normative until the Cerebro
format challenge is consumed by PM.

### Likely touched files by area

| Area | Likely files |
|---|---|
| NeoFS kernel module | `kernel/src/fs/mod.rs`, `kernel/src/fs/neofs/*` |
| Block integration | `kernel/src/drivers/block/mod.rs` only if block errors need expansion; avoid virtio-blk changes in 2A/2B |
| Syscalls | `kernel/src/arch/x86_64/syscall.rs`, `kernel/src/errno.rs`, `userland/libneo/src/lib.rs` |
| Process handles | `kernel/src/sched/process.rs` for FileCap generation/type metadata if not stored inside `OpenFile` |
| User scenarios | `userland/neo-init/Cargo.toml`, `userland/neo-init/src/main.rs` |
| Test harness | `kernel/src/test_harness.rs` |
| Test orchestration | `tools/xtask/src/main.rs`, possibly `tools/neofs-mkfs/*` |
| Format docs | `docs/specs/neofs-format.md` after PM/Cerebro format acceptance |

### Syscall stability and change plan

Must remain stable:

- `sys_write(1, ...)` serial stdout compatibility.
- `SYS_OPEN`, `SYS_READ`, `SYS_WRITE`, and `SYS_CLOSE` numbers and existing
  BUILD-1 behavior for old tests.
- `sys_read`/`sys_write` file-handle semantics for opened regular files.
- `TypeForbidden` and `InsufficientRights` remain distinguishable.
- `FileRights` remains a `u32`.
- `sys_map_region` remains anonymous user memory allocation, not file mmap.

May change in BUILD-2:

- `sys_open` implementation may stop routing only to `TestFs` and may route to
  the NeoFS root mount once 2B is active.
- `sys_open` flags may gain minimal create/truncate/type bits after PM accepts
  the exact ABI. Existing `flags=0` root-open behavior must continue.
- `SYS_FSYNC` should be added in 2D for per-file intent-log flush semantics.
  Suggested next number: `15`, subject to PM ABI confirmation.

Avoid unless PM explicitly authorizes:

- A broad `sys_mmap`/`exec` surface in BUILD-2. Typed-inode executable-refusal
  can be tested through a kernel-side VFS policy check or narrow stub in 2E.
- `sys_mount`, `sys_umount`, `sys_readdir`, symlink resolution, hard links,
  `mmap`/exec implementation, grant/derive/revoke FileCap semantics, or mount
  namespaces.

---

## 3. Proposed Sub-commit Split

I recommend keeping the dispatch's 2A-2E shape, with one refinement: on-disk
type and generation fields must be reserved from 2A, even if enforcement waits
until 2E. That avoids a format migration inside BUILD-2.

### 2A - NeoFS format draft and mkfs image skeleton

Goal: define and generate the smallest PM-accepted NeoFS v1 image shape, with
no kernel mount.

Expected work after Cerebro/PM format acceptance:

- Draft `docs/specs/neofs-format.md`.
- Add host-side `tools/neofs-mkfs`.
- Generate an image with redundant superblock slots, root directory, at least
  one regular file, and reserved type/generation/content-hash fields.
- Add host-side image sanity checks through xtask or cargo tests.

No kernel mount. No runtime disk mutation.

### 2B - Kernel read-only NeoFS mount

Goal: kernel mounts a mkfs-created NeoFS image and validates superblock/root
state.

Expected work:

- Add `kernel/src/fs/neofs/*` read-only parser and mount object.
- Read candidate superblocks from block device 0.
- Validate magic, revision, checksum, block size, generation, root pointer/hash,
  and root inode type.
- Install NeoFS as the root backend while preserving existing file-handle
  semantics.
- Keep `TestFs` fallback or test-only mount path only if needed to preserve
  earlier scenarios.

No kernel disk mutation yet.

### 2C - Physical allocator and small-file create/read/write

Goal: first kernel persistent disk mutation.

Expected work:

- Add physical block allocator according to the accepted format.
- Add inode create/open/read/write for small regular files.
- Extend `sys_open` flags or add the PM-approved create path.
- Persist small regular file data and metadata to the NeoFS image.
- Keep writes synchronous through the existing `BlockDevice` API.

This is the first sub-commit that should mutate disk at runtime. It should not
start until Cerebro answers the allocator and inode/tree format questions.

### 2D - CoW transaction, `sys_fsync`, and recovery

Goal: make BUILD-2 file mutation crash-consistent under the BUILD-1 virtio
write-through posture.

Expected work:

- Add CoW transaction path for create, overwrite, and any accepted truncate
  subset.
- Add per-file `sys_fsync` intent-log semantics. Do not make fsync a global
  filesystem transaction flush.
- Add redundant superblock rotation and mount-time highest-valid-generation
  selection.
- Add two-boot recovery scenarios and superblock-corruption fallback tests.

Do not negotiate `F_FLUSH` yet. BUILD-2 remains on BUILD-1's write-through
virtio Case-(1) path. Explicit FLUSH negotiation belongs to BUILD-3.

### 2E - Typed inode policy and FileCap generation checks

Goal: land BUILD-2 typed-inode enforcement and stale FileCap groundwork without
implementing retype.

Expected work:

- Enforce typed inode discriminants for `Regular`, `Directory`, `Model`, and
  reserved/future types.
- Add or activate FileCap generation checks against inode generation.
- Prove Model inodes cannot be mapped/executed through the available policy
  check or narrow stub, regardless of filename.
- Prove type-first failure is distinguishable from rights failure.
- Prove stale generation detection through a test-controlled stale cap path.

Retype stays out of scope. 2E should consume the fields reserved in 2A and
carried through 2B/2C rather than changing the on-disk layout.

---

## 4. Scenario/Test Matrix by Sub-commit

Suggested xtask scenario names:

| Sub-commit | Scenario | Purpose |
|---|---|---|
| 2A | `phase3_neofs_mkfs_image_sanity` | Host-created image has expected magic, block size, superblock copies, root inode, checksum, and reserved fields. |
| 2A | `phase3_neofs_mkfs_rejects_bad_args` | mkfs rejects impossible block sizes, too-small images, and unsupported type IDs. |
| 2B | `phase3_neofs_mount_readonly_root` | QEMU mounts mkfs image, validates superblock/root, and reads a known root file. |
| 2B | `phase3_neofs_mount_rejects_bad_superblock` | Corrupt magic/checksum/revision is rejected without falling back silently. |
| 2B | `phase3_neofs_mount_selects_highest_generation` | Mount chooses highest valid redundant superblock generation. |
| 2C | `phase3_neofs_create_small_file` | `sys_open` create path creates a regular file and returns a file handle. |
| 2C | `phase3_neofs_write_readback_same_boot` | Create/write/close/reopen/read returns persisted bytes in one boot. |
| 2C | `phase3_neofs_allocator_reuse_guard` | Allocator does not allocate the same block to two live inodes. |
| 2C | `phase3_neofs_reboot_persistence_small_file` | Two-boot disk reuse reads back a file created in boot 1. |
| 2D | `phase3_neofs_fsync_file_intent` | `sys_fsync(file)` persists the file's accepted intent-log semantics only. |
| 2D | `phase3_neofs_cow_overwrite_recovery` | Overwrite uses CoW ordering and recovers either old or new valid file state. |
| 2D | `phase3_neofs_superblock_rotation_recovery` | Corrupt newest superblock copy falls back to older valid generation. |
| 2D | `phase3_neofs_reboot_after_fsync` | Two-boot recovery after fsync sees the committed file state. |
| 2E | `phase3_neofs_typed_inode_exec_refusal` | Model inode rejects executable mapping/check, independent of filename. |
| 2E | `phase3_neofs_regular_exec_policy_stub` | Regular inode passes the same policy check when rights allow it. |
| 2E | `phase3_neofs_type_vs_rights_policy_errors` | Type-forbidden and insufficient-rights failures remain distinguishable. |
| 2E | `phase3_neofs_filecap_generation_stale` | FileCap generation mismatch returns a stale-cap failure and does not operate on the inode. |
| 2E | `phase3_neofs_reserved_type_refusal` | Reserved/future inode type is rejected or mounted inert per accepted format policy. |

Scenario orchestration changes:

- Extend `tools/xtask` with a NeoFS image-preparation path instead of only
  `prepare_storage_disk`.
- Reuse the existing two-boot persistence runner shape for 2C and 2D.
- Keep aarch64 barrier coverage in the regression set, but BUILD-2 does not
  need a new aarch64-only scenario unless block ordering or barrier code changes.
- Keep existing BUILD-1 scenarios green throughout:
  `phase3_block_vfs_smoke`, `phase3_virtio_blk_raw_rw_smoke`, and
  `phase3_build1_reboot_persistence_smoke`.

---

## 5. Blockers Awaiting Cerebro or PM

Implementation should not start until PM consumes the Cerebro format challenge,
Jarvis observe plan, and Prometheus evidence matrix. Specific coding blockers:

1. Exact NeoFS v1 on-disk binary layout: magic, endian, revision, block-size
   limits, checksum algorithm, superblock slots, root pointer/hash, and content
   hash reservation width.
2. Allocator shape: bitmap, extent list, free tree, or another layout. This
   blocks 2C.
3. Inode/tree shape: direct blocks, extents, tiny tree, or other representation.
   This blocks both mkfs finalization and kernel mutation.
4. Crash-consistency ordering under BUILD-1 virtio write-through without
   negotiating FLUSH. Cerebro must name the ordering for create, overwrite,
   accepted truncate subset, fsync, and superblock rotation.
5. Minimal per-file intent-log record set for `sys_fsync`. Without this, 2D can
   accidentally become a global filesystem transaction flush.
6. FileCap generation invariant: exact generation width, stale errno, cap issue
   point, check point, and future retype relationship.
7. Typed-inode policy table and reserved type behavior: especially whether
   `Model` inodes can be created in BUILD-2 by mkfs only, kernel create path,
   or both.
8. Public syscall ABI for create/type/fsync: PM should ratify whether BUILD-2
   extends `sys_open` flags or adds a separate create syscall. `SYS_FSYNC`
   number should be confirmed.
9. Normative status of `docs/specs/neofs-format.md`: PM should decide when the
   draft becomes implementation-binding.

One note from the close report: its forward-marker phrase about BUILD-2
"journaling layer, write-back policy, F_FLUSH/F_CONFIG_WCE negotiation,
multi-device allocation, directory inodes, extended attributes" conflicts with
ADR-021's accepted BUILD-2 boundary. Treat ADR-021 and PM's BUILD-2 opening
decision as the normative source unless PM explicitly revises the scope.

---

## 6. Anti-scope List

Do not include in BUILD-2:

- BUILD-3 CAS/hash-as-address, dedup, content-hash index, bloom filter, or
  mark-sweep GC.
- Buffer-cache write-back.
- Virtio `F_FLUSH` negotiation, `CONFIG_WCE` negotiation, or FLUSH command
  implementation.
- Background GC, background writeback, or new kthread substrate.
- AHCI, NVMe, MSI/MSI-X, IOMMU, or non-identity DMA work.
- Foreign filesystem support: FAT32, ext4, xattrs, foreign-FS type import, or
  default-`noexec` mount behavior.
- Rootfs pivot from Limine modules to NeoFS.
- Mount namespaces, `sys_mount`, `sys_umount`, `sys_readdir`, symlink
  resolution, hard links, or directory enumeration beyond what the accepted
  NeoFS root lookup requires.
- Full file `mmap` or exec implementation unless PM explicitly authorizes a
  narrow typed-policy stub.
- FileCap grant/derive/revoke semantics for files.
- Retype. BUILD-2 only stores and checks generation/type invariants needed for
  future retype.
- POSIX UID/GID, chmod/chown/chgrp, advisory locks, ACLs, `/proc`, `/sys`,
  quotas, or broad POSIX syscall expansion.

---

## 7. Direct Answers to Dispatch Questions

1. Extend `BlockDevice`, `Arc<DmaBuffer>`, virtio completion wake, `OpenFile`,
   `Inode`, `FileRights(u32)`, `Handle::File`, and xtask scenario/disk seams.
2. NeoFS should live under `kernel/src/fs/neofs/`.
3. Mkfs should live as a host tool under `tools/neofs-mkfs/`, with xtask
   orchestration.
4. `sys_open` backend/flags and new `sys_fsync` must change; read/write/close,
   fd 1 stdout, syscall numbers for existing operations, and error distinction
   must remain stable.
5. Suggested scenario names are listed in section 4.
6. Safest split is 2A format/mkfs, 2B read-only mount, 2C first runtime
   mutation, 2D CoW/fsync/recovery, 2E typed/generation enforcement.
7. 2C is the first sub-commit that should touch persistent disk mutation at
   runtime.
8. 2E should introduce enforcement of typed inode policy and FileCap generation
   checks, but 2A must reserve the on-disk fields and 2B/2C must carry them.
9. Coding should block on Cerebro for exact format, allocator, inode/tree,
   crash ordering, fsync intent-log, typed policy, and generation/stale-cap
   invariants.

---

**End of observe artifact.**
