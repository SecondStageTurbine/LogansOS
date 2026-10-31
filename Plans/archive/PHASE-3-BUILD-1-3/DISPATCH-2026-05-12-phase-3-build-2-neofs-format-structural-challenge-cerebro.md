# DISPATCH - Phase 3 BUILD-2 NeoFS format structural challenge

**Date:** 2026-05-12
**To:** Cerebro
**From:** PM
**Status:** READY
**Mode:** research / structural challenge only

---

## 1. Mission

Stress-test the exact BUILD-2 NeoFS native v1 format before Jarvis implements
it.

The live gate is the sharpened one:

**Should we be asking a different question entirely?**

If yes, say so first and name the corrected question.

---

## 2. Inputs

Read:

- `docs/ADR-021-phase-3-design-substrate.md`
- `docs/phases/PHASE-3.md`
- `Plans/PHASE-3-THINK-V2-2026-05-11.md`
- `Plans/RESEARCH-2026-05-11-phase-3-neofs-prior-art-cerebro.md`
- `Plans/RESEARCH-2026-05-12-phase-3-vfs-typed-inode-capability-cerebro.md`
- `Plans/CLOSE-REPORT-2026-05-12-phase-3-build-1.md`

You may inspect current source as needed, especially:

- `kernel/src/fs/mod.rs`
- `kernel/src/drivers/block/mod.rs`
- `kernel/src/drivers/block/virtio_blk.rs`
- `tools/xtask/src/main.rs`

---

## 3. Questions To Answer

Answer these directly:

1. **On-disk format minimum:** What is the smallest NeoFS v1 binary layout that
   can satisfy BUILD-2 without painting BUILD-3 into a corner?
   Include superblock fields, endianness, versioning, checksums, root inode,
   free-space representation, inode table/tree representation, content-hash
   reservation fields, generation counters, and typed-inode discriminants.

2. **Crash consistency:** Is ADR-021's BUILD-2 crash model coherent on the
   BUILD-1 write-through virtio path without negotiating FLUSH yet?
   Distinguish ordered write-through durability from later BUILD-3 explicit
   FLUSH negotiation. Name the exact write ordering required for create,
   overwrite, truncate if in scope, fsync, and superblock rotation.

3. **Intent-log scope:** ADR-021 says `sys_fsync` is per-file intent-log flush,
   not global CoW transaction flush. What minimal intent-log record set is
   needed for BUILD-2, and what can be deferred?

4. **Allocator shape:** Should BUILD-2 use a bitmap, extent list, free-tree, or
   other allocator for physical blocks? Optimize for correctness and testability
   over performance.

5. **Inode/tree shape:** Should BUILD-2 start with fixed direct blocks,
   extents, a tiny B-tree, or another structure? Identify the migration cost
   for BUILD-3 CAS and larger files.

6. **FileCap generation:** What exact invariant should connect inode
   generation counters, FileCap generation checks, stale handle detection, and
   future retype?

7. **Typed inode enforcement:** What BUILD-2 tests prove `Regular`,
   `Directory`, `Model`, and reserved future types are not merely filename
   conventions? Include executable-mapping refusal even if the full mmap/exec
   surface is stubbed.

8. **mkfs and mount tests:** What image-generation and QEMU scenarios are
   sufficient to prove mount, create/read/write, fsync, reboot recovery, and
   corruption detection?

9. **Anti-scope guardrails:** List anything that must remain out of BUILD-2:
   CAS, dedup, buffer-cache write-back, kthreads, background GC, AHCI/NVMe,
   foreign FS, rootfs pivot, or broad POSIX syscall expansion.

---

## 4. Deliverable

Create:

`Plans/RESEARCH-2026-05-12-phase-3-build-2-neofs-format-cerebro.md`

Required sections:

1. Verdict.
2. Corrected question if applicable.
3. Proposed NeoFS v1 on-disk layout.
4. Crash-consistency and write-ordering model.
5. FileCap/generation/type invariants.
6. Required BUILD-2 scenarios.
7. Implementation risks.
8. Stop conditions for Jarvis implementation.

No code. No normative docs edits.

