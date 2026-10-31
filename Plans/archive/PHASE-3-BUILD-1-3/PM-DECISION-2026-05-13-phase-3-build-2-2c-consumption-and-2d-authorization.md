# PM DECISION - Phase 3 BUILD-2 2C consumption and 2D authorization

**Date:** 2026-05-13
**Status:** RATIFIED; 2D AUTHORIZED
**Consumes:**
- `Plans/PHASE-3-BUILD-2-SUB-COMMIT-2C-VERIFICATION-2026-05-13.md`
- Commit `d2c8810` (`kernel/fs: add NeoFS create-write path`)
- Prior PM decision `Plans/PM-DECISION-2026-05-13-phase-3-build-2-plan-consumption-and-2a-authorization.md`

---

## 1. Decision

BUILD-2 sub-commit 2C is accepted as complete.

Jarvis is authorized to proceed to BUILD-2 sub-commit 2D only:

**NeoFS CoW ordering, crash/recovery selection, and BUILD-2 `sys_fsync`
documented no-op semantics.**

No BUILD-2 2E typed/FileCap-generation work, no BUILD-3 intent log, no
write-back cache, no FLUSH negotiation, and no free-space manager are
authorized by this decision.

---

## 2. 2C Evidence Accepted

PM accepts the following 2C evidence:

- `OPEN_CREATE_REGULAR = 1 << 0` added without changing the existing
  `sys_open(path, flags, rights)` ABI.
- `/neofs/<name>` Regular-file creation works and rejects broader create scope.
- NeoFS read-write mount is used only for 2C mutation scenarios while 2B
  read-only mount behavior remains intact.
- Regular-file write/readback works up to the 8-direct-block BUILD-2 cap.
- File data, inode hash, root directory hash, root inode hash, and superblock
  copies are persisted.
- Monotone bump allocation uses `next_free_block` / `next_generation`.
- Two-boot persistence for a newly created small file passes.
- 2B mount regressions and BUILD-1 block/VFS regressions emit serial PASS.

The two wrapper-timeout caveats in the 2C artifact are accepted because the
serial stream had already emitted PASS and no leftover test container remained.
2D should still prefer wrapper exit-code PASS for any new required scenario.

---

## 3. Scope Correction For 2D

The older observe/evidence-matrix wording that mentions BUILD-2 intent-log
flushes is superseded by the 2026-05-13 PM decision and
`docs/specs/neofs-format.md`:

- BUILD-2 uses BUILD-1 write-through virtio-blk posture.
- `F_FLUSH` must be offered, but `F_FLUSH` and `CONFIG_WCE` remain
  un-negotiated.
- Completed writes are already durable.
- `sys_fsync` in BUILD-2 is a documented no-op on a valid file handle.
- Intent-log semantics move to BUILD-3, when write-back caching exists.

2D should therefore not build an intent log. Its durability work is ordering
and recovery selection for already-synchronous block writes.

---

## 4. 2D Required Shape

2D must make completed NeoFS mutations recover cleanly after interrupted update
sequences:

1. Write new data blocks before publishing metadata that references them.
2. Write new file inode/root-directory state before publishing a superblock that
   points at it.
3. Publish superblock copies last.
4. On mount, reject any superblock whose checksum is valid but whose root inode,
   root hash, directory entries, child inode, or content hash cannot be
   validated.
5. Choose the highest-generation fully mountable superblock copy, not merely the
   highest-generation checksum-valid superblock copy.
6. Preserve the previous committed state until the new committed state is fully
   reachable.

This is still a BUILD-2 bump-allocator design. Orphaned CoW blocks may
accumulate. They are not reclaimed until BUILD-3 mark-sweep GC.

---

## 5. 2D Authorization Boundary

2D may update:

- `kernel/src/fs/mod.rs`
- `kernel/src/fs/neofs/**`
- `kernel/src/arch/x86_64/syscall.rs`
- `kernel/src/errno.rs`
- `userland/libneo/src/lib.rs`
- `kernel/src/test_harness.rs`
- `tools/xtask/src/main.rs`
- `tools/neofs-mkfs/**` only for deterministic test-image variants
- `kernel/Cargo.toml` / `Cargo.lock` only if strictly required
- one verification artifact under `Plans/`

2D may add `SYS_FSYNC = 15` if it is still absent. The syscall must return
success for a valid file handle and must not perform a global filesystem flush
or mutate NeoFS metadata merely because fsync was called.

2D must not add:

- an intent log or journal;
- write-back cache;
- virtio `F_FLUSH` negotiation or explicit FLUSH commands;
- block free list, bitmap, free tree, allocator reuse, or GC;
- runtime mkfs;
- truncate, unlink, rename, symlink, hard link, or readdir syscall;
- Directory or Model creation;
- typed-inode exec policy or FileCap stale-generation enforcement beyond what
  existing code already carries;
- BUILD-3 CAS, dedup, hash-as-address lookup, or content-hash index.

---

## 6. Stop Conditions

Jarvis must stop and file a blocker if:

- crash/recovery evidence requires an intent log, journal, or write-back cache;
- the recovery path cannot fall back from a checksum-valid but root-invalid
  superblock to an older fully valid copy;
- the overwrite path must overwrite old live data blocks before the new state is
  committed;
- `sys_fsync` cannot be implemented as a no-op without changing the syscall ABI;
- deterministic crash injection cannot be made reliable enough for QEMU tests;
- the work pressures a NeoFS format migration;
- any 2B/2C/BUILD-1 storage regression fails and cannot be isolated as a known
  unrelated flake;
- scope pressure appears toward 2E typed enforcement or BUILD-3 mechanisms.

---

## 7. External Status

Phase 3 BUILD-2 state after this decision:

- 2A: accepted.
- 2B: accepted.
- 2C: accepted.
- 2D: authorized only.
- 2E: not authorized yet.
- BUILD-2 close report: not authorized yet.

