# DISPATCH - Phase 3 BUILD-2 sub-commit 2D - NeoFS CoW/fsync/recovery

**Date:** 2026-05-13
**Owner:** Jarvis
**Status:** DISPATCHED
**Base:** `d2c8810` (`kernel/fs: add NeoFS create-write path`)
**Authority:** `Plans/PM-DECISION-2026-05-13-phase-3-build-2-2c-consumption-and-2d-authorization.md`

---

## 1. Mission

Implement BUILD-2 sub-commit 2D only:

**Make NeoFS create/write/overwrite recovery-safe under BUILD-1
write-through block I/O, add BUILD-2 `sys_fsync` as a documented no-op, and
prove recovery through deterministic QEMU scenarios.**

This is not an intent-log lane. BUILD-2 has no write-back cache. Completed
block writes are already durable because `F_FLUSH` is offered while `F_FLUSH`
and `CONFIG_WCE` are not negotiated.

---

## 2. Required Implementation

### 2.1 CoW ordering for mutation

For create and overwrite paths, order persistent writes so an interrupted
sequence can remount either the old complete state or the new complete state:

1. allocate fresh blocks with the existing monotone bump allocator;
2. write new data blocks first;
3. write new file inode/root directory state only after referenced data exists;
4. update root hash/root pointer metadata only after the new root is valid;
5. publish redundant superblock copies last.

Do not introduce a free-space manager. Old/orphaned blocks remain allocated.

### 2.2 Mount-time recovery selection

Mount must not select a superblock solely because its own checksum is valid.
A candidate superblock is selectable only if the reachable root inode, root
hash, root directory content, directory entries, child inodes, and file content
hashes validate.

Select the highest-generation fully mountable candidate. If the highest
checksum-valid copy points at torn metadata, reject it and fall back to the
highest older fully mountable copy.

### 2.3 `sys_fsync`

If absent, add:

- kernel syscall number `SYS_FSYNC = 15`;
- libneo wrapper `sys_fsync(handle)`;
- kernel implementation returning `0` for a valid file handle.

BUILD-2 `sys_fsync` is a no-op over write-through synchronous I/O. It must not
write an intent log, flush a global transaction, negotiate virtio FLUSH, or bump
NeoFS generation merely because fsync was called. Invalid handles should follow
existing handle error behavior.

### 2.4 Crash injection

Add deterministic test-only crash/power-cut injection as needed for the 2D
QEMU scenarios. Keep it gated to explicit `neo-test` scenario code and avoid
normal runtime behavior changes.

Acceptable approaches include a scenario-only marker plus xtask-controlled
QEMU termination, or an existing test-exit/reset path if one is already present.
The evidence must show which mutation phase was interrupted and which state was
recovered after reboot.

---

## 3. Required Scenarios

Add and pass these QEMU Docker scenarios:

1. `phase3_neofs_fsync_noop_write_through`
   - create/write a Regular file;
   - call `sys_fsync`;
   - prove fsync returns success;
   - prove fsync itself does not mutate generation/superblock state;
   - prove data remains readable and durable.

2. `phase3_neofs_cow_write_after_write`
   - create a Regular file;
   - write version A;
   - overwrite the same logical range with version B;
   - prove readback returns B;
   - prove old and new physical data blocks are distinct.

3. `phase3_neofs_crash_mid_overwrite_recovery`
   - start from a committed file containing version A;
   - interrupt an overwrite at a deterministic pre-commit point;
   - reboot using the same disk image;
   - prove mount recovers a complete valid file state, preferably A for a
     pre-superblock-commit injection;
   - prove no torn metadata or content-hash mismatch is accepted.

4. `phase3_neofs_superblock_rotation_recovery`
   - create a state where a newer superblock copy is corrupt or points at
     invalid root metadata;
   - reboot/remount;
   - prove the invalid candidate is rejected;
   - prove the highest older fully mountable generation is selected.

5. `phase3_neofs_crash_mid_create_recovery`
   - interrupt file creation before the new root is fully committed;
   - reboot/remount;
   - prove the root directory is not torn;
   - prove the file is either absent or fully present, with serial evidence
     naming which injection point produced which result.

If a scenario name needs a small adjustment to match xtask conventions, keep
the purpose and evidence unchanged.

---

## 4. Regression Gates

After the new 2D scenarios pass, rerun:

- `phase3_neofs_create_small_file`
- `phase3_neofs_write_readback_same_boot`
- `phase3_neofs_allocator_reuse_guard`
- `phase3_neofs_reboot_persistence_small_file`
- `phase3_neofs_mount_readonly_root`
- `phase3_neofs_mount_selects_highest_generation`
- `phase3_neofs_mount_rejects_bad_superblock`
- `phase3_block_vfs_smoke`
- `phase3_virtio_blk_raw_rw_smoke`

Run `phase3_aarch64_virtio_barrier_lane` if any virtio, barrier, or block-driver
file changes. Otherwise note that 2D did not touch that substrate.

If runtime budget allows, run the non-explicit SMP sweep:

```text
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test --no-meta
```

Known timing flakes must be isolated with a targeted rerun and documented.

---

## 5. Build And Tooling Gates

Required:

```text
cargo check -p xtask
cargo check -p neofs-mkfs
cargo k
cargo k --no-default-features
```

Also run targeted kernel feature checks for each new explicit scenario if the
feature-gating pattern supports it.

Run scoped rustfmt over every Rust file changed by 2D. Do not use global
`cargo fmt --check` as a blocking gate unless the pre-existing repository-wide
format drift has already been fixed.

Run:

```text
git diff --check
git diff --cached --check
```

Use the cached check only after staging for commit.

---

## 6. Allowed Files

2D may modify:

- `kernel/src/fs/mod.rs`
- `kernel/src/fs/neofs/**`
- `kernel/src/arch/x86_64/syscall.rs`
- `kernel/src/errno.rs`
- `userland/libneo/src/lib.rs`
- `kernel/src/test_harness.rs`
- `tools/xtask/src/main.rs`
- `tools/neofs-mkfs/**` only for deterministic test-image variants
- `kernel/Cargo.toml` / `Cargo.lock` only if strictly required
- `Plans/PHASE-3-BUILD-2-SUB-COMMIT-2D-VERIFICATION-2026-05-13.md`

Do not edit phase docs, ADRs, close reports, or index files in this sub-commit.

---

## 7. Explicit Non-Scope

Do not implement:

- BUILD-2 2E typed-inode exec policy;
- FileCap stale-generation rejection;
- retype;
- intent log, journal, or per-file log records;
- write-back cache;
- virtio FLUSH negotiation or `CONFIG_WCE` negotiation;
- allocator reuse, free list, bitmap, free tree, or GC;
- truncate, unlink, rename, hard links, symlink following, or readdir syscall;
- Directory or Model creation;
- runtime mkfs;
- BUILD-3 CAS, dedup, content-address index, bloom filter, or mark-sweep.

---

## 8. Verification Artifact

Create:

`Plans/PHASE-3-BUILD-2-SUB-COMMIT-2D-VERIFICATION-2026-05-13.md`

The artifact must include:

- commit hash and branch;
- explicit statement that only 2D was implemented;
- serial PASS frames for every new 2D scenario;
- crash injection point and recovery outcome for each crash scenario;
- proof that `sys_fsync` is a no-op in BUILD-2;
- proof that mount selection rejects checksum-valid but root-invalid candidates;
- regression results from section 4;
- build/tooling results from section 5;
- any wrapper-timeout caveat, with serial PASS and container cleanup evidence;
- explicit statement that no 2E, BUILD-3, or close-report work was started.

---

## 9. Stop And File A Blocker If

- recovery requires an intent log or journal;
- a checksum-valid but root-invalid superblock cannot be rejected cleanly;
- overwrite cannot be made CoW without a NeoFS format change;
- deterministic crash injection is too flaky to trust;
- `sys_fsync` pressures a global flush or metadata mutation;
- any required regression fails and cannot be isolated;
- the work begins to require 2E typed/FileCap enforcement or BUILD-3 storage
  mechanisms.

