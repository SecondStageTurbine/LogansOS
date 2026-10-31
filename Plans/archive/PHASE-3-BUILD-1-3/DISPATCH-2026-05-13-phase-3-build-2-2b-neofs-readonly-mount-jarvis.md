# DISPATCH - Phase 3 BUILD-2 sub-commit 2B: NeoFS read-only mount

**Date:** 2026-05-13
**To:** Jarvis
**From:** PM
**Status:** READY
**Branch:** create/switch to `codex/phase-3-build-2-2b-neofs-readonly-mount` from `f781957`

---

## 1. Mission

Implement BUILD-2 sub-commit 2B only:

**Kernel read-only mount of a mkfs-created NeoFS image from virtio-blk.**

No runtime NeoFS mutation. No file create/write. No 2C/2D/2E work.

---

## 2. Inputs

Read first:

- `Plans/PM-DECISION-2026-05-13-phase-3-build-2-2a-consumption-and-2b-authorization.md`
- `docs/specs/neofs-format.md`
- `Plans/PHASE-3-BUILD-2-SUB-COMMIT-2A-VERIFICATION-2026-05-13.md`
- `Plans/OBSERVE-2026-05-12-phase-3-build-2-neofs-subcommit-plan-jarvis.md`
- `docs/ADR-021-phase-3-design-substrate.md`

Treat `docs/specs/neofs-format.md` from 2A as binding for 2B. If the kernel
parser needs to change the on-disk format, stop and file a deviation.

---

## 3. Required Work

### 3.1 Kernel NeoFS read-only parser/mount

Add a NeoFS module under:

```text
kernel/src/fs/neofs/
```

Recommended files:

```text
format.rs
mount.rs
inode.rs
mod.rs
```

2B code must:

- read superblock copies from offsets `0`, `1 MiB`, and `64 MiB` where present;
- validate magic, fixed fields, block size, checksum, generation, and root
  pointer/hash;
- select the highest-generation valid superblock;
- read and validate the root directory inode;
- validate directory entries against child inode type;
- expose read-only inodes through the existing `Inode` trait;
- reject unsupported types and non-zero indirect pointers in BUILD-2;
- verify `content_hash` on read for `Regular` and `Model` files;
- never call `write_block` from the NeoFS 2B path.

If kernel BLAKE3 is unavailable in `no_std`, stop and file a blocker. Do not
substitute SHA-256 or a custom hash without PM approval.

### 3.2 VFS integration

Preserve BUILD-1 `testfs` behavior.

Mount NeoFS at a temporary BUILD-2 prefix, preferably `/neofs`, while leaving
existing paths such as `/hello.txt`, `/scratch`, `/dir`, and `/link` routed to
testfs.

`open_root_temporary("/neofs/hello.txt", OPEN_ROOT_TEMPORARY, READ)` should
return an `OpenFile` backed by NeoFS. Existing `sys_open` ABI and fd 1 stdout
rules must remain stable.

2B is read-only:

- `READ` on `hello.txt` succeeds.
- `WRITE` on NeoFS files returns `InsufficientRights` or another documented
  read-only denial consistent with existing VFS errors.
- create/truncate flags are not introduced in 2B.

### 3.3 Test orchestration

Extend the test runner so 2B scenarios receive a 2A mkfs-created NeoFS image
instead of the BUILD-1 raw marker disk.

You may extend `tools/neofs-mkfs` or `tools/xtask` with host-side helpers for
test image variants, including:

- normal image;
- primary-superblock-corrupted image;
- image with valid superblock copies at different generations, to prove highest
  valid generation selection.

Do not require QEMU for the existing host-side 2A sanity test.

---

## 4. Required Scenarios

Add and pass these targeted scenarios:

1. `phase3_neofs_mount_readonly_root`
   - QEMU boots with mkfs-created NeoFS image.
   - Kernel mounts NeoFS read-only.
   - Serial PASS includes:
     `neofs_mount=ok`, `selected_superblock=<slot>`, `generation=<n>`,
     `root_kind=Directory`, `hello_read=ok`, `model_kind=Model`,
     `runtime_mutation=false`.

2. `phase3_neofs_mount_selects_highest_generation`
   - QEMU boots with an image containing at least two valid superblock copies
     with different generations.
   - Kernel selects the highest valid generation.
   - Serial PASS names selected slot and generation.

3. `phase3_neofs_mount_rejects_bad_superblock`
   - QEMU boots with the primary superblock checksum corrupted and at least one
     valid redundant copy.
   - Kernel rejects the corrupted copy and mounts from a valid copy.
   - Serial PASS includes `corrupt_primary_rejected=true`.

If one of these cannot be implemented without exceeding 2B scope, stop and
file a deviation. Do not replace it with mutation coverage.

---

## 5. Allowed Files

Allowed create/update set:

- `kernel/src/fs/mod.rs`
- `kernel/src/fs/neofs/**`
- `kernel/Cargo.toml`
- `Cargo.lock`
- `kernel/src/test_harness.rs`
- `tools/xtask/src/main.rs`
- `tools/neofs-mkfs/**` for test image variants only
- `Plans/PHASE-3-BUILD-2-SUB-COMMIT-2B-VERIFICATION-2026-05-13.md`

Do not add unrelated untracked `Plans/` material.

---

## 6. Required Verification

Run and record exact commands/results:

- `cargo k`
- `cargo k --no-default-features`
- `cargo check -p xtask`
- `cargo check -p neofs-mkfs`
- `cargo neo-test phase3_neofs_mount_readonly_root`
- `cargo neo-test phase3_neofs_mount_selects_highest_generation`
- `cargo neo-test phase3_neofs_mount_rejects_bad_superblock`
- `cargo neo-test phase3_block_vfs_smoke`
- `cargo neo-test phase3_virtio_blk_raw_rw_smoke`
- host-side 2A mkfs sanity command
- scoped rustfmt for new/edited Rust files
- `git diff --check`

If the host requires Docker/Limine for QEMU scenarios, use the existing Docker
path and record that command exactly.

Global `cargo fmt --check` is not required; broad pre-existing formatting drift
is known.

---

## 7. Commit

If verification passes, commit only the allowed files.

Suggested commit message:

`kernel/fs: mount NeoFS images read-only`

After committing, report:

- commit SHA;
- files changed;
- exact verification results;
- selected superblock evidence;
- hello/model read/type evidence;
- confirmation that 2B performed no runtime NeoFS writes;
- confirmation that no 2C/2D/2E work started.

---

## 8. Stop Conditions

Stop and file a blocker/deviation instead of committing if:

- kernel BLAKE3 cannot be used in `no_std`;
- the kernel parser needs to change `docs/specs/neofs-format.md`;
- `phase3_neofs_mount_readonly_root` requires runtime disk mutation;
- the VFS integration breaks existing BUILD-1 testfs scenarios or fd 1 stdout;
- corrupt superblock data is accepted silently;
- highest-generation valid superblock selection cannot be proven;
- the implementation needs create/write/allocator/CoW/fsync/generation-enforce
  work to pass 2B;
- any anti-scope item appears: CAS, dedup, GC, write-back cache, kthreads,
  AHCI/NVMe, foreign FS, rootfs pivot, broad POSIX expansion.

