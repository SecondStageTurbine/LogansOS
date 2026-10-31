# DISPATCH - Phase 3 BUILD-2 sub-commit 2A: NeoFS format spec + mkfs skeleton

**Date:** 2026-05-13
**To:** Jarvis
**From:** PM
**Status:** READY
**Branch:** create/switch to `codex/phase-3-build-2-2a-neofs-format-mkfs`

---

## 1. Mission

Implement BUILD-2 sub-commit 2A only:

**NeoFS format spec + host-side mkfs image skeleton.**

No kernel NeoFS mount. No runtime disk mutation. No BUILD-2 2B/2C/2D/2E work.

---

## 2. Inputs

Read first:

- `Plans/PM-DECISION-2026-05-13-phase-3-build-2-plan-consumption-and-2a-authorization.md`
- `Plans/RESEARCH-2026-05-12-phase-3-build-2-neofs-format-cerebro.md`
- `Plans/OBSERVE-2026-05-12-phase-3-build-2-neofs-subcommit-plan-jarvis.md`
- `Plans/PREP-2026-05-12-phase-3-build-2-evidence-matrix-prometheus.md`
- `docs/ADR-021-phase-3-design-substrate.md`
- `docs/phases/PHASE-3.md`

PM's 2026-05-13 decision overrides stale or conflicting details in the earlier
planning artifacts.

---

## 3. Required Work

### 3.1 Format spec

Create:

`docs/specs/neofs-format.md`

The spec is implementation-binding for BUILD-2 2A. It must define:

- superblock layout, exactly 512 bytes;
- inode layout, exactly 256 bytes;
- directory entry layout, exactly 64 bytes;
- little-endian integer encoding;
- fixed byte-array magic values, not endian-sensitive `u64` magic;
- BLAKE3 checksum ranges;
- 4 KiB BUILD-2 filesystem block size;
- superblock copy offsets: `0`, `1 MiB`, `64 MiB` where device size permits;
- mount-time copy selection rule: highest generation with valid checksum;
- bump allocator rule: `next_free_block` only, no free list;
- file size cap: 8 direct blocks × 4 KiB = 32 KiB;
- indirect block reserved as zero in BUILD-2;
- inode generation from `superblock.next_generation`;
- typed inode discriminant values including `Regular`, `Directory`, `Symlink`,
  `Model`, and reserved/future behavior;
- `content_hash` and `reserved_retype_audit[16]` forward-compat fields;
- explicit BUILD-2 anti-scope: no CAS, dedup, GC, write-back cache, kthreads,
  AHCI/NVMe, foreign FS, or rootfs pivot.

Use the normalized superblock table from the PM decision.

### 3.2 Host mkfs tool

Add:

```text
tools/neofs-mkfs/
  Cargo.toml
  src/main.rs
  src/format.rs
```

Add it to the workspace in root `Cargo.toml`.

The tool must be host-side only. It must be able to produce a deterministic
sparse NeoFS v1 image with:

- a root directory;
- `hello.txt` as a Regular file with deterministic contents;
- `model.neom` as a Model inode;
- `subdir` as an empty Directory inode;
- valid superblock copies;
- valid root inode checksum/hash;
- valid inode checksums/content hashes;
- zeroed reserved fields.

Default image size should be large enough to exercise all three superblock copy
locations, using sparse file allocation so the host cost stays small.

Mkfs must reject:

- non-4096 filesystem block sizes in BUILD-2;
- images too small for the required superblock copy policy selected by the
  invocation;
- unsupported inode type IDs;
- filenames longer than the 47-byte payload plus NUL terminator.

### 3.3 Host-side verification path

Add a host-side sanity verification path. Prefer a direct `cargo` or `xtask`
command that does not boot QEMU.

Required evidence:

- image is deterministic across two invocations;
- output hash is identical across two invocations with the same inputs;
- superblock copy offsets contain valid checksummed superblocks;
- root inode block resolves to a Directory inode;
- `hello.txt`, `model.neom`, and `subdir` entries are present and typed;
- `model.neom` inode has `Model` discriminant;
- reserved indirect pointer is zero;
- reserved fields are zero.

Suggested scenario name if integrated into xtask:

`phase3_neofs_mkfs_image_sanity`

This scenario should remain host-side for 2A. It must not require Limine or
QEMU.

---

## 4. Allowed Files

Allowed create/update set:

- `docs/specs/neofs-format.md`
- `tools/neofs-mkfs/**`
- root `Cargo.toml`
- `Cargo.lock`
- `tools/xtask/src/main.rs` for host-side mkfs sanity orchestration only
- `Plans/PHASE-3-BUILD-2-SUB-COMMIT-2A-VERIFICATION-2026-05-13.md`

Do not add unrelated untracked `Plans/` material.

---

## 5. Required Verification

Run and record exact commands/results:

- `cargo check -p neofs-mkfs`
- host-side mkfs image sanity command, twice, proving deterministic output
- `cargo check -p xtask` if `xtask` was edited
- `git diff --check`
- scoped rustfmt for new/edited Rust files

Also run, unless blocked by environment:

- `cargo k`
- `cargo k --no-default-features`

Do not run a QEMU NeoFS mount scenario for 2A; there is no kernel mount yet.

---

## 6. Commit

If verification passes, commit only the allowed files.

Suggested commit message:

`tools/fs: add NeoFS v1 mkfs skeleton`

After committing, report:

- commit SHA;
- files changed;
- exact verification results;
- image size and deterministic hash;
- any dependency caveat;
- confirmation that no kernel mount or runtime disk mutation was started.

---

## 7. Stop Conditions

Stop and file a blocker/deviation instead of committing if:

- BLAKE3 cannot be used without substituting a different hash;
- deterministic image generation fails;
- the normalized 512-byte superblock table cannot fit or conflicts with code;
- the mkfs tool requires kernel mount code to validate its output;
- a dependency fetch or toolchain issue blocks `cargo check -p neofs-mkfs`;
- the implementation needs to modify kernel filesystem, syscall, virtio, or
  userland runtime code for 2A;
- any anti-scope item appears: CAS, dedup, GC, write-back cache, kthreads,
  AHCI/NVMe, foreign FS, rootfs pivot, broad POSIX expansion.

