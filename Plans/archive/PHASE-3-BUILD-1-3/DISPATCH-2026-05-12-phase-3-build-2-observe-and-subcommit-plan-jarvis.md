# DISPATCH - Phase 3 BUILD-2 observe pass and sub-commit plan

**Date:** 2026-05-12
**To:** Jarvis
**From:** PM
**Status:** READY
**Mode:** observe / planning only

---

## 1. Mission

Perform a read-only observe pass for Phase 3 BUILD-2 and propose the exact
sub-commit split for NeoFS native v1 implementation.

BUILD-2 implementation is not authorized by this dispatch. Do not edit kernel
code, tools, docs, or tests except to create the single observe artifact named
below.

---

## 2. Inputs

Read:

- `docs/ADR-021-phase-3-design-substrate.md`
- `docs/phases/PHASE-3.md`
- `Plans/CLOSE-REPORT-2026-05-12-phase-3-build-1.md`
- `kernel/src/fs/mod.rs`
- `kernel/src/drivers/block/mod.rs`
- `kernel/src/drivers/block/dma.rs`
- `kernel/src/drivers/block/virtio_blk.rs`
- `userland/libneo/src/lib.rs`
- `userland/neo-init/src/main.rs`
- `tools/xtask/src/main.rs`

Optional if useful:

- `Plans/RESEARCH-2026-05-12-phase-3-vfs-typed-inode-capability-cerebro.md`
- `Plans/RESEARCH-2026-05-12-phase-3-storage-driver-prior-art-cerebro.md`

---

## 3. Questions To Answer

1. What current BUILD-1 seams should BUILD-2 extend instead of replacing?
2. Where should NeoFS live in the module tree?
3. Where should mkfs tooling live?
4. Which syscalls must change for BUILD-2, and which must remain stable?
5. What scenario names should xtask expose for BUILD-2?
6. What is the safest sub-commit sequence?
7. Which sub-commit is the first one that should touch persistent disk
   mutation?
8. Which sub-commit should introduce typed inode policy and FileCap generation
   checks?
9. Which implementation risks should block coding until Cerebro answers the
   format challenge?

---

## 4. Expected Sub-commit Shape

Use this as a starting hypothesis, not as a command to force-fit:

- **2A:** NeoFS format spec draft plus mkfs image skeleton, no kernel mount.
- **2B:** kernel read-only mount of mkfs image and superblock/root validation.
- **2C:** physical block allocator plus inode create/read/write for small
  regular files.
- **2D:** CoW transaction path, per-file `sys_fsync` intent-log semantics, and
  redundant superblock rotation with two-boot recovery tests.
- **2E:** typed inode discriminants, FileCap generation checks, and executable
  mapping refusal tests or stubs.

If you recommend a different split, explain why.

---

## 5. Deliverable

Create:

`Plans/OBSERVE-2026-05-12-phase-3-build-2-neofs-subcommit-plan-jarvis.md`

Required sections:

1. Current substrate map.
2. Extension seams and files likely touched.
3. Proposed sub-commit split.
4. Scenario/test matrix by sub-commit.
5. Blockers awaiting Cerebro or PM.
6. Anti-scope list.

Do not commit implementation. Do not start BUILD-2 code.

