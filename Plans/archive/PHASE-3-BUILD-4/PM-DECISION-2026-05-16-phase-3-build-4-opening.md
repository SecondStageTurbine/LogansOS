# PM Decision - Phase 3 BUILD-4 Opening

Date: 2026-05-16
From: PM
Status: BUILD-4 THINK / PREREQ AUDIT OPEN

## 1. Substrate

`main` is published at:

```text
1eb41c5c782805265024def5c28bfe5b96b5e93d docs: update Phase 3 after BUILD-3 closure
```

Phase 3 BUILD-1, BUILD-2, and BUILD-3 are closed. BUILD-3F CAS/dedup remains
unadmitted and measurement-gated. BUILD-4 is the next Phase 3 lane, but it is
not authorized for implementation until the prereq audit and structural
challenge are consumed.

## 2. ADR-021 BUILD-4 Scope

ADR-021 defines BUILD-4 as:

- FAT32 read-only;
- ext4 read-only;
- default `noexec` on all foreign-FS mounts, with explicit operator opt-in for
  `exec`;
- ext4 xattrs ignored for type import;
- `sys_mount` / `sys_umount`;
- directory enumeration;
- cursor-based `sys_readdir`;
- broader POSIX-ish syscall surface.

This scope is broad enough to sprawl. PM narrows the opening rule: BUILD-4 must
land mount and directory capability substrate before foreign filesystem code.
FAT32/ext4 drivers must not invent their own path, mount, directory cursor, or
noexec policy mechanisms.

## 3. Current Substrate Observations

Observed current code state on `main`:

- `kernel/src/fs/mod.rs` still exposes temporary root-path open:
  `sys_open(path_ptr, path_len, flags, requested_rights_u32)`.
- The target model is already documented in comments:
  `sys_open(dir_cap, path, flags, requested_rights)`.
- `FileRights` reserves `LOOKUP`, `READDIR`, and `MOUNT` bits but only
  READ/WRITE/EXECUTE are active.
- `MountTable` is hard-coded to an in-memory testfs root plus optional NeoFS
  mounted at `/neofs`.
- `mount_neofs_from_device0_readonly()` and `mount_neofs_from_device0_readwrite()`
  are kernel/test paths, not user-visible mount syscalls.
- `Inode` has `kind`, `read_at`, `write_at`, `fsync`, and generation hooks, but
  no directory lookup/readdir trait surface.
- NeoFS has directory entries internally and root directory parsing, but no
  public cursor-based directory enumeration.
- `OpenFile` only supports Regular/Model read and Regular write; directories
  are rejected with `TypeForbidden`.
- `sys_read`, `sys_write`, `sys_close`, and `sys_fsync` operate on file handles;
  there is no `sys_readdir`, `sys_mount`, or `sys_umount`.
- `userland/libneo` mirrors the same temporary file ABI.

## 4. Load-Bearing BUILD-4 Questions

### Q1 - Directory Capability ABI

Recommendation: activate directory handles as normal `Handle::File` values
whose inode kind is `Directory`, gated by `FileRights::LOOKUP_RESERVED` and
`FileRights::READDIR_RESERVED`, but do not expose a separate `DirHandle` type
yet.

Rationale: The handle table already treats files as capabilities. Splitting
directory handles into a parallel table would violate the ADR-010 pattern.
Keeping one file-handle namespace also preserves syscall simplicity. The type
distinction lives in `InodeKind` and the operation gate.

Open point for audit: whether to rename `LOOKUP_RESERVED` / `READDIR_RESERVED`
to active `LOOKUP` / `READDIR` in BUILD-4A, and how libneo should represent
those bits without breaking existing callers.

### Q2 - Mount Table Shape

Recommendation: replace the special-case `root + optional /neofs` mount table
with an explicit small mount table keyed by mount path for BUILD-4, not a full
namespace tree.

Rationale: BUILD-4 needs `/neofs`, `/fat`, and `/ext4` style test mounts, not
per-process namespaces or bind mounts. A small mount table keeps the path
resolver auditable while still forcing FAT32/ext4 through the same VFS path.

Rejected for BUILD-4: per-process mount namespaces, VFS dentry cache, bind
mounts, writable foreign mounts, symlink traversal, hard links.

### Q3 - Syscall Sequencing

Recommendation:

1. `sys_readdir(fd, out_ptr, out_len, cursor_ptr)` lands before `sys_mount`.
2. `sys_mount(source, target, fs_type, flags)` lands as a privileged/test-root
   syscall after mount-table refactor.
3. `sys_umount(target)` lands only after mounted-FS lifetime and open-handle
   behavior are specified.

Rationale: readdir can be proven over testfs and NeoFS before foreign drivers
exist. Mount can then compose with a real directory enumeration surface. Umount
is the risky operation because open handles may outlive a mount-table entry;
the audit must decide whether BUILD-4 allows unmount with open handles or
returns `EBUSY`.

PM lean: return `EBUSY` if any live handle references the mounted filesystem.

### Q4 - Foreign FS Order

Recommendation: FAT32 first, ext4 second.

Rationale: FAT32 read-only is smaller and exercises the mount/read/readdir path
without extents, block groups, journaling flags, or xattr policy. ext4 is the
security-sensitive one because ADR-021 requires xattrs ignored for type import.
The project should not debug mount-table and ext4 parser complexity at the same
time.

### Q5 - noexec and Type Import

Recommendation: all foreign-FS inodes import as `Regular` or `Directory` only,
with mount-level `noexec` default. `EXECUTE` may be requested only if the mount
was created with explicit `exec` flag, and ext4 xattrs are ignored for NeoFS type
import in all cases.

Rationale: this directly satisfies ADR-021 Q9. Foreign filesystems cannot
manufacture `Model`, `Tensor`, or future typed authority from untrusted metadata.

## 5. Proposed BUILD-4 Sub-Commit Split

### BUILD-4A - Directory handles + cursor readdir over existing filesystems

Scope:

- Activate LOOKUP/READDIR file rights.
- Extend `Inode` or add a directory operation trait for lookup/readdir.
- Permit opening directories with READDIR/LOOKUP rights.
- Add `sys_readdir` and libneo wrapper.
- Prove cursor semantics over testfs and NeoFS root.
- Preserve existing `sys_open` compatibility for current tests.

Anti-scope:

- No `sys_mount` / `sys_umount`.
- No FAT32/ext4 parser.
- No symlink traversal.
- No POSIX `stat`, `lseek`, `unlink`, or `rename`.

### BUILD-4B - Mount table refactor + sys_mount skeleton

Scope:

- Replace hard-coded `/neofs` special case with an explicit small mount table.
- Add mount target resolution for `/neofs`, `/fat`, `/ext4` style mount points.
- Add `sys_mount` for kernel-recognized test devices/images and `fs_type`.
- Enforce mount authority through a temporary privileged/test-root rule or
  active `MOUNT` right, to be decided in the audit.
- Ensure existing NeoFS tests pass unchanged or through a compatibility shim.

Anti-scope:

- No foreign FS parser yet.
- No unmount unless B1 audit proves open-handle lifetime semantics.

### BUILD-4C - FAT32 read-only

Scope:

- Minimal FAT32 read-only parser sufficient for mkfs-created test images.
- Root directory lookup and cursor readdir.
- Regular-file read.
- Default noexec.
- Tests for mount, read, readdir, no write, no exec.

Anti-scope:

- No long-filename write support.
- No FAT32 write support.
- No VFAT mutation.
- No boot root pivot.

### BUILD-4D - ext4 read-only

Scope:

- Minimal ext4 read-only parser for controlled ext4 images.
- Regular-file read and directory readdir.
- Default noexec.
- xattrs ignored for type import.
- Tests proving xattr distrust and no Model import.

Anti-scope:

- No journal replay.
- No ext4 write support.
- No extents beyond the minimum required by test images unless the audit admits
  them explicitly.
- No hard links as authority-bearing aliases.

### BUILD-4E - Integration close

Scope:

- Full `NEO_SMP=4 --no-meta` sweep.
- aarch64 runtime barrier lane remains green.
- Phase docs and close report.
- Explicit caveats and forward markers.

## 6. Required Next Artifact

Next artifact should be:

```text
Plans/OBSERVE-2026-05-16-phase-3-build-4-prereq-audit-jarvis.md
```

Required audit questions:

- Does current `Handle::File` support directory handles without variant split?
- What exact syscall numbers are free after `SYS_FSYNC = 15`?
- What errno mapping is needed for `ENOTDIR`, `EBUSY`, `ENOTEMPTY`, and
  `EEXIST` if BUILD-4 activates mount/readdir?
- What minimum directory-entry ABI should `sys_readdir` return: fixed record,
  packed name records, or one-entry-at-a-time?
- Can NeoFS root readdir be implemented without widening on-disk format?
- What host tooling exists in the dev container to create FAT32/ext4 images?
- Can FAT32/ext4 tests be deterministic without adding large binary fixtures?
- Where should noexec be enforced: mount object, OpenFile policy, or executable
  policy helper?
- What is the unmount/open-handle rule?

## 7. Authorization Boundary

Authorized now:

- BUILD-4 prereq audit and design planning.
- Source reading, grep/RAG, and artifact creation.
- No code changes except docs/plans artifacts explicitly recording the audit.

Not authorized yet:

- BUILD-4 implementation code.
- `sys_readdir`, `sys_mount`, `sys_umount` code.
- FAT32/ext4 parser code.
- BUILD-3F CAS/dedup.
- BUILD-5 AHCI/NVMe.
- Rootfs pivot, file-backed mmap/exec, broad POSIX expansion.
