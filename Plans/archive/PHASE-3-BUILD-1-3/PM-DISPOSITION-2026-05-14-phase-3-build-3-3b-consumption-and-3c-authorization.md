# PM Disposition - Phase 3 BUILD-3B Consumption and 3C Authorization

**Date:** 2026-05-14
**Status:** 3B ACCEPTED; 3C authorized by separate dispatch
**Consumes:**
- `Plans/PHASE-3-BUILD-3-SUB-COMMIT-3B-VERIFICATION-2026-05-14.md`
- `Plans/PM-DISPOSITION-2026-05-14-phase-3-build-3-3a-consumption-and-3b-authorization.md`
- `Plans/DISPATCH-2026-05-14-phase-3-build-3-3b-virtio-flush-jarvis.md`

---

## 1. Decision

PM accepts BUILD-3 sub-commit 3B.

Implementation commit:

```text
a3023a0259be4b1d1694442ddfa370fde7099b99 kernel/storage: negotiate virtio-blk flush
```

Verification artifact commit:

```text
64693dede3c56bf8177f9adc94d81ef077044ee7 Plans: verify Phase 3 BUILD-3B virtio flush
```

Branch:

```text
codex/phase-3-build-3-3b-virtio-flush
```

BUILD-3 may proceed to 3C only:

```text
Plans/DISPATCH-2026-05-14-phase-3-build-3-3c-neofs-gc-free-space-jarvis.md
```

## 2. 3B Acceptance Basis

Jarvis reports and local PM checks confirm:

- `BlockDevice::supports_flush()` and `BlockDevice::flush()` landed;
- virtio-blk negotiates `VIRTIO_BLK_F_FLUSH`;
- `VIRTIO_BLK_F_CONFIG_WCE` remains not negotiated;
- real `VIRTIO_BLK_T_FLUSH = 4` requests complete through the existing virtqueue and wake-token path;
- FLUSH requests use header + status descriptors only, with no fake data buffer;
- `sys_fsync` remains a BUILD-2 no-op over the current synchronous NeoFS path;
- targeted block/VFS/fsync/aarch64 regressions passed;
- full `NEO_SMP=4 cargo neo-test --no-meta` passed.

The `kernel/src/arch/x86_64/syscall.rs` change is an explanatory comment update only. It does not alter `sys_fsync_impl` behavior.

## 3. 3B Scope Confirmation

Implementation changed:

```text
kernel/Cargo.toml
kernel/src/arch/x86_64/syscall.rs
kernel/src/drivers/block/mod.rs
kernel/src/drivers/block/virtio_blk.rs
kernel/src/test_harness.rs
tools/xtask/src/main.rs
```

Artifact commit changed:

```text
Plans/PHASE-3-BUILD-3-SUB-COMMIT-3B-VERIFICATION-2026-05-14.md
```

No NeoFS write-back cache, mark-sweep GC, free-space format change, durable fsync contract, CAS/dedup, AHCI/NVMe, foreign filesystem, rootfs pivot, file-backed mmap/exec, or broad POSIX work is accepted as part of 3B.

## 4. 3C Authorization

3C is the NeoFS free-space format and quiescent mark-sweep GC slice.

3C may:

- add the smallest NeoFS free-space metadata extension needed for reclamation;
- keep mount compatibility for BUILD-2 images if practical, treating an all-zero free-space extension as legacy bump-only state;
- update `docs/specs/neofs-format.md` and `tools/neofs-mkfs` only as needed for the new free-space metadata;
- implement a mark phase from the selected root inode through reachable directory entries, inode records, and direct data blocks;
- implement a sweep phase that returns unreachable allocated blocks to the free-space allocator;
- add quiescent GC entry points for mount-time, unmount-time, or test-only explicit GC;
- use the 3B block flush primitive for GC metadata/superblock publication where appropriate.

3C must not introduce concurrent GC with active writes. GC must run only at write quiescence.

3C must not change `sys_fsync` behavior. Durable fsync over write-back remains a later slice.

## 5. Carry-Forward Gates

After 3C, the allocator must no longer be permanently bump-only for images carrying the BUILD-3 free-space metadata. Orphaned CoW blocks must be reclaimable and safely reusable.

3C closure requires both:

- reachable-block correctness: live root/inode/data blocks are never reclaimed;
- reclamation correctness: orphaned blocks can be returned to allocation and reused without corrupting active files or mount selection.

CAS/dedup remains deferred to Phase 5 unless PM explicitly re-admits it.

## 6. Closure

BUILD-3B is accepted. BUILD-3C may begin under the dedicated Jarvis dispatch. No merge to `main` and no push are authorized by this disposition.
