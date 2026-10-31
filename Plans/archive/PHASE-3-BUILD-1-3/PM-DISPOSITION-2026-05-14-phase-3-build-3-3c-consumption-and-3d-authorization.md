# PM Disposition - Phase 3 BUILD-3C Consumption and 3D Authorization

**Date:** 2026-05-14
**Status:** 3C ACCEPTED; 3D authorized by separate dispatch
**Consumes:**
- `Plans/PHASE-3-BUILD-3-SUB-COMMIT-3C-VERIFICATION-2026-05-14.md`
- `Plans/PM-DISPOSITION-2026-05-14-phase-3-build-3-3b-consumption-and-3c-authorization.md`
- `Plans/DISPATCH-2026-05-14-phase-3-build-3-3c-neofs-gc-free-space-jarvis.md`

---

## 1. Decision

PM accepts BUILD-3 sub-commit 3C.

Implementation commit:

```text
a7eeb2135161778479961b323562580a6d42bfaa kernel/fs: add NeoFS free-space GC
```

Verification artifact commit:

```text
67913b7595bdd8c2666b93261d5e64d78bce870e docs: record Phase 3 BUILD-3C verification
```

Branch:

```text
codex/phase-3-build-3-3c-neofs-gc-free-space
```

BUILD-3 may proceed to 3D only:

```text
Plans/DISPATCH-2026-05-14-phase-3-build-3-3d-neofs-writeback-cache-jarvis.md
```

## 2. 3C Acceptance Basis

Jarvis reports and local PM checks confirm:

- NeoFS superblock free-map extension landed while preserving legacy all-zero BUILD-2 extension compatibility;
- free-map metadata is hash-covered and selected through superblock validation;
- mkfs initializes deterministic BUILD-3C free-map metadata;
- quiescent mark-sweep GC marks root/inode/directory/direct-data/superblock/free-map blocks;
- orphaned CoW blocks can be reclaimed and safely reused;
- corrupt free-map candidates are rejected by mount selection;
- GC publication uses the 3B block FLUSH primitive after superblock copies are written;
- `sys_fsync` remains the documented no-op and does not issue new flush commands;
- targeted NeoFS/block/aarch64 regressions passed;
- full `NEO_SMP=4 cargo neo-test --no-meta` passed.

Changed implementation files:

```text
docs/specs/neofs-format.md
kernel/Cargo.toml
kernel/src/fs/mod.rs
kernel/src/fs/neofs/format.rs
kernel/src/fs/neofs/inode.rs
kernel/src/fs/neofs/mod.rs
kernel/src/fs/neofs/mount.rs
kernel/src/test_harness.rs
tools/neofs-mkfs/src/format.rs
tools/neofs-mkfs/src/main.rs
tools/xtask/src/main.rs
```

Verification artifact file:

```text
Plans/PHASE-3-BUILD-3-SUB-COMMIT-3C-VERIFICATION-2026-05-14.md
```

## 3. Artifact Caveat

The 3C verification artifact ends with:

```text
git diff --cached --check must be rerun after staging this artifact for the verification commit.
```

Jarvis's closeout states `git diff --cached --check` passed, and local PM checks confirm current tracked diff whitespace is clean. This stale wording is not a correctness blocker, but future verification artifacts should not retain pre-commit reminders after the artifact commit is created.

## 4. 3D Authorization

3D is the NeoFS write-back buffer-cache slice using the 3A kthread lifecycle substrate.

3D may:

- add a mount-owned NeoFS FS-block cache;
- distinguish clean and dirty cached blocks;
- add a write-back worker owned by the NeoFS mount through `KthreadHandle`;
- route suitable NeoFS reads/writes through the cache;
- drain dirty cache state to disk through the existing CoW/free-map/superblock publication path;
- provide private/test-only wait/drain hooks for runtime scenarios;
- preserve GC quiescence by forcing or requiring a clean cache before GC runs.

3D must not:

- change `sys_fsync` behavior;
- claim durable per-file fsync semantics;
- add a public `sys_gc`, `sys_sync`, or unmount syscall;
- implement CAS/dedup;
- start AHCI/NVMe, foreign FS, rootfs pivot, mmap/exec, or broad POSIX work.

## 5. Carry-Forward Gates

After 3D, dirty data may be cached and drained by the write-back worker, but `sys_fsync` remains a no-op until 3E explicitly changes the contract.

3D must make the following distinction visible in tests:

- background write-back/drain makes data eventually persistent under clean execution;
- `sys_fsync` itself still does not force the drain and does not issue a flush command;
- crashes before a dirty transaction is drained may lose the uncommitted write but must not corrupt the mountable filesystem.

## 6. Closure

BUILD-3C is accepted. BUILD-3D may begin under the dedicated Jarvis dispatch. No merge to `main` and no push are authorized by this disposition.
