# PM DECISION - Phase 3 BUILD-2 2A consumption and 2B authorization

**Date:** 2026-05-13
**Status:** 2A RATIFIED; 2B AUTHORIZED
**Consumes:** `Plans/PHASE-3-BUILD-2-SUB-COMMIT-2A-VERIFICATION-2026-05-13.md`
**2A commit:** `f781957ed2cde7fa9166ed231dc8025f1eef6a0e`

---

## 1. Decision

BUILD-2 sub-commit 2A is accepted.

The committed 2A scope matches authorization:

- `docs/specs/neofs-format.md`
- `tools/neofs-mkfs/**`
- root workspace membership and lockfile changes
- 2A verification artifact

No kernel mount path, runtime disk mutation, or later BUILD-2 sub-commit work
was started.

BUILD-2 sub-commit 2B is now authorized.

---

## 2. 2A Evidence Accepted

PM accepts the following 2A evidence:

- commit `f781957` (`tools/fs: add NeoFS v1 mkfs skeleton`);
- deterministic sparse image size `134217728`;
- deterministic image hash
  `dd845cb0e9a4065e3d548274f4259da948ee4762d8335eb620ddad55175b1c85`;
- three valid superblock copies at offsets `0`, `1 MiB`, and `64 MiB`;
- normalized byte-array magic values;
- normalized 512-byte superblock layout with non-overlapping
  `next_generation`;
- 256-byte inode layout;
- 64-byte directory entry layout;
- `model.neom` emitted as a `Model` inode;
- zeroed reserved fields and zero indirect pointers;
- host-side BLAKE3 dependency resolved for mkfs;
- `cargo fmt -p neofs-mkfs --check`, `cargo check -p neofs-mkfs`, mkfs
  sanity twice, `git diff --check`, `cargo k`, and
  `cargo k --no-default-features` reported PASS.

---

## 3. 2B Authorization

2B scope:

**Kernel read-only NeoFS mount of a mkfs-created virtio-blk image.**

2B must prove:

- kernel reads the mkfs-created disk through the existing BUILD-1 block device;
- all present superblock copies are validated with BLAKE3 checksums;
- mount selects the highest valid superblock generation;
- root inode block and root inode hash validate;
- root directory entries validate;
- `hello.txt` can be opened/read through VFS from the NeoFS mount;
- `model.neom` is recognized as `Model` type in the parser/mount path;
- corrupt superblock copies are rejected rather than accepted silently;
- the kernel performs no NeoFS disk writes in 2B.

2B should mount NeoFS under a temporary path prefix such as `/neofs` while
preserving existing BUILD-1 `testfs` behavior for current scenarios. This keeps
`phase3_vfs_*` and `phase3_block_vfs_smoke` stable.

---

## 4. Still Not Authorized

The following remain unauthorized:

- runtime NeoFS mutation;
- file create/write;
- bump allocator allocation from kernel runtime;
- CoW transaction path;
- `sys_fsync`;
- FileCap generation enforcement;
- typed executable-mapping refusal beyond read-only type parsing;
- CAS, dedup, GC, write-back cache, kthreads;
- AHCI/NVMe;
- foreign FS;
- rootfs pivot;
- BUILD-2 2C/2D/2E.

---

## 5. Dispatch

Jarvis dispatch:

`Plans/DISPATCH-2026-05-13-phase-3-build-2-2b-neofs-readonly-mount-jarvis.md`

