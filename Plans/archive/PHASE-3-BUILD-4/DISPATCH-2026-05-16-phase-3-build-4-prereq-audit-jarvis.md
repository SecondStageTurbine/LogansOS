# Dispatch - Phase 3 BUILD-4 Prereq Audit

Date: 2026-05-16
Agent: Jarvis
Status: AUTHORIZED OBSERVE / PLAN ONLY
Authority:
- `Plans/PM-DECISION-2026-05-16-phase-3-build-4-opening.md`
- `docs/ADR-021-phase-3-design-substrate.md`
- `docs/phases/PHASE-3.md`

## 1. Mission

Run the BUILD-4 prereq audit before any implementation code lands.

BUILD-4 is foreign filesystem read-only plus VFS/userspace polish. The scope is
large enough to sprawl, so this audit must narrow the first implementation
slice and surface the load-bearing decisions before code.

No kernel/source implementation is authorized in this dispatch.

## 2. Current Substrate

Start from `main`:

```text
1eb41c5c782805265024def5c28bfe5b96b5e93d docs: update Phase 3 after BUILD-3 closure
```

BUILD-1, BUILD-2, and BUILD-3 are closed and pushed to `origin/main`.
BUILD-4 has not started. BUILD-3F CAS/dedup remains unadmitted and
measurement-gated.

Known current VFS state:

- `kernel/src/fs/mod.rs` still exposes temporary root-path open:
  `sys_open(path_ptr, path_len, flags, requested_rights_u32)`.
- The target model is already documented in comments:
  `sys_open(dir_cap, path, flags, requested_rights)`.
- `FileRights` reserves `LOOKUP`, `READDIR`, and `MOUNT` bits but only
  READ/WRITE/EXECUTE are active.
- `MountTable` is hard-coded to testfs root plus optional NeoFS at `/neofs`.
- NeoFS has directory entries internally but no public readdir surface.
- There is no `sys_readdir`, `sys_mount`, or `sys_umount`.

## 3. Audit Questions

Answer these concretely against the current code:

1. Can `Handle::File` carry directory handles without a new handle-table variant?
2. Should BUILD-4 activate `LOOKUP_RESERVED`, `READDIR_RESERVED`, and
   `MOUNT_RESERVED` as stable rights bits?
3. What syscall numbers are free after `SYS_FSYNC = 15`, and what ABI shape
   should `sys_readdir`, `sys_mount`, and `sys_umount` use?
4. What errno additions are needed for directory/mount semantics? Evaluate at
   least `ENOTDIR`, `EBUSY`, `ENOTEMPTY`, and `EEXIST`.
5. What minimum directory-entry ABI should `sys_readdir` use: fixed record,
   packed variable names, or one-entry-at-a-time?
6. Can NeoFS root readdir be implemented from existing in-memory directory
   entries without widening the on-disk format?
7. What is the minimum mount-table refactor that removes the hard-coded `/neofs`
   special case without introducing full mount namespaces?
8. Where should foreign-FS noexec policy live: mount object, `OpenFile`, or
   executable-policy helper?
9. What is the unmount/open-handle rule? PM lean is `EBUSY` while any live
   handle references the mount.
10. What deterministic image tooling exists in the dev container for FAT32 and
    ext4 tests? If missing, identify whether host-side mkfs helpers must be
    added.
11. Can FAT32 and ext4 read-only tests avoid large binary fixtures?
12. What is the smallest safe BUILD-4A implementation slice?

## 4. PM Lean To Challenge Or Ratify

PM lean before audit:

- BUILD-4A should be directory handles + cursor `sys_readdir` over testfs and
  NeoFS only.
- BUILD-4B should refactor the mount table and add `sys_mount` skeleton.
- BUILD-4C should add FAT32 read-only.
- BUILD-4D should add ext4 read-only.
- BUILD-4E should close BUILD-4.
- FAT32 comes before ext4.
- All foreign-FS mounts default `noexec`.
- ext4 xattrs are ignored for type import.
- No symlink traversal, hard links, writable foreign FS, rootfs pivot,
  file-backed mmap/exec, or broad POSIX expansion in BUILD-4A.

Jarvis may challenge any lean with code-grounded evidence. Do not silently
accept a PM lean if current code makes it unsafe.

## 5. Required Output

Create:

```text
Plans/OBSERVE-2026-05-16-phase-3-build-4-prereq-audit-jarvis.md
```

Required sections:

- Current VFS/syscall substrate summary.
- Answers to the 12 audit questions.
- Recommended BUILD-4 sub-commit split.
- BUILD-4A detailed implementation outline.
- Test scenario matrix for BUILD-4A.
- Risks and stop conditions.
- Anti-scope list.

Do not commit source code. A docs/plans-only commit is allowed for the audit
artifact after `git diff --check` and `git diff --cached --check`.

Suggested commit message:

```text
plans: open Phase 3 BUILD-4 prereq audit
```

## 6. Verification

For this dispatch:

```text
git diff --check
git diff --cached --check
```

No `cargo k`, `cargo neo-test`, or runtime boot is required because this is
observe/plan only.

## 7. Boundaries

Authorized:

- Read code/docs.
- Use grep/RAG for discovery.
- Create the prereq audit artifact.
- Commit only the audit artifact if clean.

Not authorized:

- Kernel/source implementation.
- Syscall number changes.
- FAT32/ext4 parser work.
- BUILD-3F CAS/dedup.
- BUILD-5 AHCI/NVMe.
- Rootfs pivot.
- File-backed mmap/exec.
- Broad POSIX expansion.
- Merge or push after the audit commit unless PM explicitly authorizes.
