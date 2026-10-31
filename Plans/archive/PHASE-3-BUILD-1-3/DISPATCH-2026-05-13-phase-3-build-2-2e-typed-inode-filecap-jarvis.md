# DISPATCH - Phase 3 BUILD-2 sub-commit 2E - typed inode and FileCap generation

**Date:** 2026-05-13
**Owner:** Jarvis
**Status:** DISPATCHED
**Base:** `a2f1e49a36c08318a7e21fd41fe54fe6ca8ecafe` (`kernel/fs: harden NeoFS CoW recovery`)
**Authority:** `Plans/PM-DECISION-2026-05-13-phase-3-build-2-2d-consumption-and-2e-authorization.md`

---

## 1. Mission

Implement BUILD-2 sub-commit 2E only:

**Activate typed-inode policy and FileCap inode-generation stale-handle
rejection over the existing NeoFS v1 substrate.**

2E is the final BUILD-2 implementation lane. Do not file a close report and do
not start BUILD-3 work.

---

## 2. Required Implementation

### 2.1 Typed-inode executable policy

Add a narrow VFS/kernel policy helper that answers whether an opened file cap
may be used for an executable mapping/execution-like operation.

Required behavior:

- `Model` returns `TypeForbidden` for executable policy checks regardless of
  filename and regardless of rights.
- `Regular` returns `InsufficientRights` when the FileCap lacks the required
  execute/mmap-style right.
- `Regular` returns success when type allows and rights allow.
- `Directory`, `Symlink`, reserved, or unsupported inode types return
  `TypeForbidden` or fail inertly before rights grant access.

This helper may be kernel/test-harness only. Do not implement real file-backed
`mmap`, disk-backed ELF exec, or process spawning from NeoFS.

### 2.2 FileRights width and rights activation

Keep `FileRights` as a `u32`.

2E may activate a minimal execute/mmap-style bit using the already-reserved
rights space. Preserve the reserved high bits for future `GRANT`, `DERIVE`,
`RETYPE`, and `MOUNT` semantics. Do not collapse file rights back into the
legacy 8-bit `CapRights` pair.

If deterministic mkfs variants are needed to seed a Regular inode with execute
availability, add them under `tools/neofs-mkfs/**` and xtask scenario plumbing.
Do not add chmod/chown or a broad permission-edit syscall.

### 2.3 Type-first, rights-second dispatch

For executable policy checks, evaluate inode type before FileCap rights:

- `Model` with execute right -> `TypeForbidden`;
- `Model` without execute right -> `TypeForbidden`;
- `Regular` without execute right -> `InsufficientRights`;
- `Regular` with execute right -> success.

Read policy must continue to allow `Model` files to be read when READ rights
allow it. Write policy must continue to reject `Model` with `TypeForbidden`.

### 2.4 FileCap generation capture and stale rejection

Capture the current inode generation when a file handle is issued through
`sys_open` / create-open.

Before file operations that use an existing handle, compare the captured
generation with the current reachable inode generation. Stale handles must fail
with a distinct stale error. PM-approved default errno is `ESTALE = -116`.

The stale check must not merely compare against an old in-memory `NeoInode`
record, because that would let an old handle keep reading orphaned pre-CoW
content. It must validate against current NeoFS reachability.

Successful mutating operations through a handle may refresh that handle's
captured generation to the new inode generation. A second handle opened before
the mutation must remain stale.

---

## 3. Required Scenarios

Add and pass these Docker/QEMU scenarios:

1. `phase3_neofs_typed_inode_exec_refusal`
   - open the mkfs-created `model.neom`;
   - prove read still succeeds;
   - run the executable policy helper;
   - prove result is `TypeForbidden` even if the cap carries the execute bit.

2. `phase3_neofs_regular_exec_policy_stub`
   - open or seed a Regular inode with execute availability;
   - request a FileCap with the execute bit;
   - prove the executable policy helper succeeds;
   - prove no actual mmap/exec is performed.

3. `phase3_neofs_type_vs_rights_policy_errors`
   - prove the four-corner matrix:
     - Model + execute right => `TypeForbidden`;
     - Model + no execute right => `TypeForbidden`;
     - Regular + no execute right => `InsufficientRights`;
     - Regular + execute right => success.

4. `phase3_neofs_filecap_generation_stale`
   - open two handles to the same Regular inode;
   - mutate through handle A via the existing CoW write path;
   - prove A can continue after its generation refresh;
   - prove handle B fails with stale-cap errno;
   - serial evidence must include old cap generation and current inode
     generation.

5. `phase3_neofs_reserved_type_refusal`
   - boot a deterministic image containing a reserved/future inode type in a
     reachable directory entry, or an equivalent mount candidate;
   - prove the reserved type is rejected or mounted inert per the existing
     format policy;
   - prove it cannot acquire executable access by filename or rights.

6. `phase3_neofs_filerights_u32_forward_bits`
   - prove `FileRights` is 32-bit wide in the kernel/libneo ABI;
   - prove high reserved bits such as `RETYPE` remain representable and are
     not truncated through open/policy validation.

If a scenario name needs a small adjustment to match xtask conventions, keep
the evidence and serial fields equivalent.

---

## 4. Regression Gates

After the new 2E scenarios pass, rerun:

- `phase3_neofs_fsync_noop_write_through`
- `phase3_neofs_cow_write_after_write`
- `phase3_neofs_crash_mid_overwrite_recovery`
- `phase3_neofs_superblock_rotation_recovery`
- `phase3_neofs_crash_mid_create_recovery`
- `phase3_neofs_create_small_file`
- `phase3_neofs_write_readback_same_boot`
- `phase3_neofs_allocator_reuse_guard`
- `phase3_neofs_reboot_persistence_small_file`
- `phase3_neofs_mount_readonly_root`
- `phase3_neofs_mount_selects_highest_generation`
- `phase3_neofs_mount_rejects_bad_superblock`
- `phase3_block_vfs_smoke`
- `phase3_virtio_blk_raw_rw_smoke`

Because 2E is the last implementation lane, also run the full non-explicit SMP
sweep if the Docker/QEMU host permits it:

```text
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test --no-meta
```

If that sweep cannot be run, the 2E artifact must say `2E PASS / BUILD-2 close
pending full sweep`, not claim closure.

Run `phase3_aarch64_virtio_barrier_lane` only if 2E touches virtio, barrier, or
block-driver files. Otherwise cite the unchanged substrate and previous
aarch64 evidence.

---

## 5. Build And Tooling Gates

Required:

```text
cargo check -p xtask
cargo check -p neofs-mkfs
cargo check -p libneo -p neofs-mkfs
cargo k
cargo k --no-default-features
```

Also run targeted kernel feature checks for every new 2E scenario.

Run scoped rustfmt over every Rust file changed by 2E. Do not use global
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

2E may modify:

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
- `Plans/PHASE-3-BUILD-2-SUB-COMMIT-2E-VERIFICATION-2026-05-13.md`

Do not edit phase docs, ADRs, `Plans/INDEX.md`, close reports, or BUILD-3
planning artifacts in this sub-commit.

---

## 7. Explicit Non-Scope

Do not implement:

- retype;
- FileCap grant/derive/revoke semantics for files;
- actual file-backed `mmap`;
- disk-backed ELF exec or spawning from NeoFS;
- chmod/chown/permission-edit syscall;
- truncate, unlink, rename, symlink following, hard links, or readdir syscall;
- Directory or Model creation;
- runtime mkfs;
- allocator reuse, free list, bitmap, free tree, or GC;
- intent log, journal, write-back cache, virtio FLUSH negotiation, or
  `CONFIG_WCE` negotiation;
- BUILD-3 CAS, dedup, content-address index, bloom filter, or mark-sweep.

---

## 8. Verification Artifact

Create:

`Plans/PHASE-3-BUILD-2-SUB-COMMIT-2E-VERIFICATION-2026-05-13.md`

The artifact must include:

- commit hash and branch;
- explicit statement that only 2E was implemented;
- serial PASS frames for every new 2E scenario;
- the type-first/rights-second four-corner matrix;
- stale-cap errno, old generation, and current generation evidence;
- proof that no real mmap/exec or retype was added;
- regression results from section 4;
- build/tooling results from section 5;
- scoped rustfmt result and any inherited drift caveat;
- explicit statement that no BUILD-3 or close-report work was started.

---

## 9. Stop And File A Blocker If

- executable policy cannot be tested without implementing real mmap/exec;
- stale generation checks break ordinary 2C/2D read/write behavior;
- stale detection cannot distinguish current reachable inode state from an
  orphaned old inode;
- `ESTALE` or equivalent stale-cap errno cannot be kept distinct;
- type and rights failures collapse into one error;
- any required regression fails and cannot be isolated;
- the implementation starts to require 2E-external mechanisms such as retype,
  file-cap grant/derive/revoke, GC, CAS, or write-back caching.

