# PM DECISION - Phase 3 BUILD-2 2D consumption and 2E authorization

**Date:** 2026-05-13
**Status:** RATIFIED; 2E AUTHORIZED
**Consumes:**
- `Plans/PHASE-3-BUILD-2-SUB-COMMIT-2D-VERIFICATION-2026-05-13.md`
- Commit `a2f1e49a36c08318a7e21fd41fe54fe6ca8ecafe` (`kernel/fs: harden NeoFS CoW recovery`)
- Prior PM decision `Plans/PM-DECISION-2026-05-13-phase-3-build-2-2c-consumption-and-2d-authorization.md`

---

## 1. Decision

BUILD-2 sub-commit 2D is accepted as complete.

Jarvis is authorized to proceed to BUILD-2 sub-commit 2E only:

**Typed-inode policy activation and FileCap inode-generation stale-handle
rejection.**

No retype implementation, no real `mmap`/exec subsystem, no broader POSIX
surface, no BUILD-3 CAS, no write-back cache, and no allocator/GC work are
authorized by this decision.

---

## 2. 2D Evidence Accepted

PM accepts the following 2D evidence:

- CoW write path writes new data/inode/root before publishing superblocks.
- Mount selection validates the reachable root and child metadata before
  accepting a candidate superblock.
- Newer invalid superblock candidates are rejected in favor of the older fully
  mountable generation.
- `SYS_FSYNC = 15` is wired as a BUILD-2 no-op over synchronous write-through
  I/O.
- No intent log, journal, write-back cache, or virtio FLUSH negotiation was
  introduced.
- All new 2D Docker/QEMU scenarios passed.
- Required 2B/2C/block/virtio regression matrix passed.
- Build/tooling and targeted feature checks passed.

The optional SMP sweep was not run in 2D and is not a 2D acceptance blocker.
Because 2E is the final BUILD-2 implementation lane, 2E should pick up the
full non-explicit SMP sweep if it aims to become the close-candidate commit.

The scoped rustfmt caveat is accepted as pre-existing shared-file drift. Global
formatting remains outside the BUILD-2 correctness lane.

---

## 3. 2E Required Shape

2E must activate the fields and policy hooks already reserved by 2A-2D:

1. `FileRights` remains a `u32`.
2. Typed inode policy is checked by inode type first and rights second.
3. `Model` inodes are readable but cannot pass an executable mapping/execution
   policy check, regardless of filename or requested rights.
4. `Regular` inodes can pass the same executable policy check only when the
   FileCap carries the required execute/mmap-style right.
5. `TypeForbidden` and `InsufficientRights` remain distinct and test-visible.
6. File capabilities capture the inode generation at issue time.
7. File operations reject stale FileCaps when the current reachable inode
   generation no longer matches the captured generation.

2E may add a narrow kernel/VFS executable-policy helper or test stub. It must
not implement actual file-backed `mmap`, ELF loading from NeoFS, or process
execution from disk.

---

## 4. Generation Semantics

The stale-cap check must compare a handle's captured generation against the
current reachable inode for that path/object, not merely against a stale
in-memory `NeoInode` snapshot.

The desired two-handle behavior is:

- open handle A and handle B to the same Regular inode;
- mutate through A using the existing CoW write path;
- A may refresh its own captured generation after its successful mutating
  operation;
- B must remain stale and fail subsequent file operations with a distinct stale
  error.

PM ratifies `ESTALE = -116` as the syscall errno for stale FileCaps unless
Jarvis finds an existing, better local errno convention and files the deviation
in the 2E artifact.

---

## 5. 2E Authorization Boundary

2E may update:

- `kernel/src/fs/mod.rs`
- `kernel/src/fs/neofs/**`
- `kernel/src/arch/x86_64/syscall.rs`
- `kernel/src/errno.rs`
- `kernel/src/sched/process.rs`
- `userland/libneo/src/lib.rs`
- `kernel/src/test_harness.rs`
- `tools/xtask/src/main.rs`
- `tools/neofs-mkfs/**` only for deterministic typed-policy image variants
- `kernel/Cargo.toml` / `Cargo.lock` only if strictly required
- one verification artifact under `Plans/`

2E must not edit phase docs, ADRs, `Plans/INDEX.md`, close reports, or BUILD-3
planning files.

---

## 6. Stop Conditions

Jarvis must stop and file a blocker if:

- typed policy requires actual file-backed `mmap` or exec;
- stale-cap detection cannot distinguish current reachable inode generation
  from stale in-memory inode snapshots;
- adding generation checks breaks the 2C/2D writer's own handle after a
  successful write and cannot be fixed with a narrow generation refresh;
- the implementation requires retype;
- `TypeForbidden` and `InsufficientRights` collapse to the same error;
- `FileRights` width pressure reintroduces a 16-bit rights word;
- 2E needs an on-disk format migration;
- any required 2B/2C/2D/BUILD-1 regression fails and cannot be isolated as a
  known unrelated flake;
- scope pressure appears toward BUILD-3 CAS, dedup, write-back, FLUSH, GC, or
  broader POSIX work.

---

## 7. External Status

Phase 3 BUILD-2 state after this decision:

- 2A: accepted.
- 2B: accepted.
- 2C: accepted.
- 2D: accepted.
- 2E: authorized only.
- BUILD-2 close report: not authorized yet.

