# OBSERVE - Phase 3 BUILD-4 Prereq Audit

Date: 2026-05-16
Agent: Jarvis
Status: OBSERVE COMPLETE / PLAN RECOMMENDATION
Substrate:

```text
e6246f3 plans: open Phase 3 BUILD-4 prereq audit
```

Inputs:

- `Plans/PM-DECISION-2026-05-16-phase-3-build-4-opening.md`
- `Plans/DISPATCH-2026-05-16-phase-3-build-4-prereq-audit-jarvis.md`
- `docs/ADR-021-phase-3-design-substrate.md`
- Current source under `kernel/src/fs`, `kernel/src/arch/x86_64/syscall.rs`,
  `kernel/src/sched/process.rs`, `userland/libneo`, and `tools/xtask`.

RAG note: the local RAG endpoint at `127.0.0.1:8404` was unavailable during this
OBSERVE pass, so the audit used direct source reads and `rg` searches.

## 1. Current VFS / Syscall Substrate

The current VFS substrate is still the Phase 3 BUILD-1/2/3 shape:

- `kernel/src/fs/mod.rs` exposes `sys_open(path_ptr, path_len, flags,
  requested_rights_u32)` through `open_root_temporary()`. The comment already
  names the target model as `sys_open(dir_cap, path, flags, requested_rights)`.
- `FileRights` is a `u32`. `READ`, `WRITE`, and `EXECUTE` are active.
  `LOOKUP_RESERVED`, `READDIR_RESERVED`, and `MOUNT_RESERVED` already have
  stable bit positions but are not accepted by `FileRights::KNOWN_BUILD2`.
- `Handle::File { file: Arc<OpenFile>, cap_id, rights: FileRights }` is already
  in the normal process handle table. There is no separate fd table.
- `OpenFile::new()` accepts any `Arc<dyn Inode>`, but `open_root_temporary()`
  currently rejects non-`Regular` and non-`Model` inodes before a handle is
  installed.
- `OpenFile::read()` and `OpenFile::write()` already perform type gating after
  rights gating for their existing operations. `check_executable_policy()`
  performs type-first, rights-second enforcement for execute policy.
- `Inode` currently has `kind`, `available_rights`, `current_generation`,
  `fsync`, `read_at`, and `write_at`. It has no lookup/readdir operation.
- `MountTable` is not a real table yet: it is `root: Arc<TestFs>` plus optional
  `neofs: Option<Arc<NeoFs>>`, with `/neofs` hard-coded in
  `open_root_temporary()`.
- NeoFS has parsed directory entries and keeps the mounted root entries in
  `NeoFsState.entries: SwitchMutex<Vec<DirEntry>>`; it does not expose public
  VFS readdir.
- Syscall numbers stop at `SYS_FSYNC = 15`; `16+` are free in kernel and
  libneo.
- The syscall entry path currently marshals four arguments (`a0..a3`) into
  Rust dispatch. BUILD-4 syscalls should fit this limit or explicitly refactor
  the syscall ABI.

## 2. Audit Answers

### Q1 - Can `Handle::File` carry directory handles without a new variant?

Yes.

`Handle::File` is type-neutral: it stores an `Arc<OpenFile>` plus `FileRights`.
`OpenFile` stores an `Arc<dyn Inode>` and does not require the inode to be
regular at construction time. The current blocker is only
`open_root_temporary()`, which rejects directory inodes before `OpenFile::new()`.

Recommendation: keep directories as `Handle::File`. Directory-ness is an inode
operation namespace (`InodeKind::Directory`), not a handle-table variant.
Splitting a `DirHandle` variant would duplicate the capability table and fight
ADR-010's "new authority types are handle variants only when they are genuinely
different kernel object classes" pattern. Directory FileCaps are FileCaps with
directory operations.

### Q2 - Should BUILD-4 activate LOOKUP/READDIR/MOUNT rights?

Yes, but staged:

- BUILD-4A activates `LOOKUP` and `READDIR`.
- BUILD-4B activates `MOUNT`.
- Keep the current bit values. Add active names and preserve reserved aliases
  for source compatibility:

```rust
pub const LOOKUP: Self = Self(1 << 2);
pub const READDIR: Self = Self(1 << 3);
pub const MOUNT: Self = Self(1 << 27);
pub const LOOKUP_RESERVED: Self = Self::LOOKUP;
pub const READDIR_RESERVED: Self = Self::READDIR;
pub const MOUNT_RESERVED: Self = Self::MOUNT;
```

Directory inodes should advertise `LOOKUP | READDIR`; the root directory used
by mount tests may additionally advertise `MOUNT` once BUILD-4B lands. Regular
and Model inodes should not advertise directory rights.

### Q3 - What syscall numbers and ABI shapes should BUILD-4 use?

Reserve the next three syscall numbers:

```text
SYS_READDIR = 16
SYS_MOUNT   = 17
SYS_UMOUNT  = 18
```

Recommended `sys_readdir` ABI:

```text
sys_readdir(fd: u32, out_ptr: *mut u8, out_len: usize, cursor_ptr: *mut u64) -> i64
```

Semantics:

- `cursor_ptr` points to a userspace `u64`. Caller initializes it to `0`.
- Each successful call returns one entry, writes a packed record to `out_ptr`,
  and updates `*cursor_ptr` to the next cursor.
- Return `0` at EOF.
- Return negative errno on failure.
- The cursor is opaque to userspace but can be a simple entry index for BUILD-4A.

Recommended record format:

```text
NeoDirentV1:
  u32 record_len
  u16 name_len
  u8  kind
  u8  flags
  u64 next_cursor
  u8  name[name_len]
```

This is one-entry-at-a-time with a variable-length name tail. It avoids a fixed
name ceiling in the syscall ABI, avoids batch cursor complexity, and still gives
tests deterministic byte-level records.

Recommended mount ABI for BUILD-4B:

```text
sys_mount(dir_fd: u32, req_ptr: *const u8, req_len: usize, flags: u32) -> i64
sys_umount(dir_fd: u32, name_ptr: *const u8, name_len: usize, flags: u32) -> i64
```

`sys_mount` should consume a versioned `MountRequestV1` buffer containing
`fs_type`, source string offset/length, and target child-name offset/length.
The operation requires `FileRights::MOUNT` on `dir_fd`. This fits the current
four-argument syscall path and avoids an ambient absolute-path mount syscall.

### Q4 - What errno additions are needed?

Add POSIX-shaped constants to both `kernel/src/errno.rs` and
`userland/libneo/src/lib.rs`:

```text
EBUSY     = -16
EEXIST    = -17
ENOTDIR   = -20
ENOTEMPTY = -39
```

Recommended mappings:

- `ENOTDIR`: readdir on a non-directory handle, lookup through a non-directory
  component, or mount target parent that is not a directory.
- `EBUSY`: unmount while any live handle references the mounted filesystem.
- `EEXIST`: mount target already occupied, create target already exists.
- `ENOTEMPTY`: not needed by BUILD-4A, but reserve now for future directory
  removal or stricter mount-point replacement policy.

Keep current mappings:

- `EISDIR = -21` remains "directory used where regular file is required"
  (`read`, `write`, execute policy on non-regular).
- `EACCES = -13` remains rights attenuation failure.

### Q5 - What minimum directory-entry ABI should `sys_readdir` use?

Use the one-entry-at-a-time variable-tail record described in Q3.

Rejected alternatives:

- Fixed `name[56]` or `name[256]` records: simpler, but bakes a name ceiling
  into the ABI and either truncates future ext4/VFAT names or wastes user
  buffer space.
- Batch packed records: efficient, but cursor and partial-buffer semantics are
  larger than BUILD-4A needs.

The one-entry ABI is boring in the best way: easy to fuzz, easy to test, and
does not require kernel-side persistent cursor objects.

### Q6 - Can NeoFS root readdir use existing data without format widening?

Yes.

NeoFS already parses on-disk directory entries (`parse_directory_entries`) and
keeps root entries in `NeoFsState.entries`. The normal mkfs image has exactly:

```text
hello.txt: Regular
model.neom: Model
subdir: Directory
```

BUILD-4A can expose readdir without changing the NeoFS disk format. For root,
the simplest implementation reads from `NeoFsState.entries` under its lock. For
non-root directories, the current inode content reader can parse directory
content from the inode record. Nested path lookup is not required for BUILD-4A.

Important invariant: NeoFS directory FileCaps should keep the current reachable
inode generation check. If directory contents mutate after a directory handle is
opened, `sys_readdir` should return `ESTALE`, matching the existing stale
FileCap discipline.

### Q7 - What is the minimum mount-table refactor?

Replace the special-case struct:

```text
root: Arc<TestFs>
neofs: Option<Arc<NeoFs>>
```

with a small explicit table:

```text
Mount {
  id: MountId,
  path: String,
  fs: Arc<dyn MountedFs>,
  flags: MountFlags,
  open_count: AtomicU32
}
```

For BUILD-4, path lookup can remain absolute and select the longest matching
mount prefix. This supports `/`, `/neofs`, `/fat`, and `/ext4` without dentry
caches, bind mounts, symlinks, or mount namespaces.

`MountedFs` should be a narrow trait:

```text
lookup(relative_path) -> Arc<dyn Inode>
root() -> Arc<dyn Inode>
```

Directory enumeration belongs on the inode/directory operation surface, not on
the mount table itself. The mount table only selects the filesystem and enforces
mount policy.

### Q8 - Where should foreign-FS noexec policy live?

The mount object should own noexec policy. VFS open should compute effective
available rights as:

```text
effective_rights = inode.available_rights() & mount.rights_mask()
```

`OpenFile::check_executable_policy()` should remain the final type-first,
rights-second executable check, but it should not know which filesystem the file
came from. Noexec is a mount policy; execute authorization is a FileCap right.

For BUILD-4:

- FAT32 and ext4 mounts default to `noexec`.
- Explicit `exec` mount flag may allow `EXECUTE` on regular files only.
- Foreign filesystems import only `Regular` and `Directory` inode kinds.
- ext4 xattrs are ignored for type import in all cases.

### Q9 - What is the unmount/open-handle rule?

Return `EBUSY` while any live handle references the mounted filesystem.

This is the least surprising correctness rule and avoids dangling semantic
questions about whether an unmounted `Arc<OpenFile>` should remain usable. It
also gives BUILD-4B a concrete implementation requirement: `OpenFile` needs a
mount-lifetime token or mount id reference so open handles can increment and
decrement the mount's live-handle count.

Do not implement `sys_umount` until that accounting exists. A mount-table entry
with only an `Arc` reference is not enough to answer `EBUSY` precisely.

### Q10 - What deterministic FAT32/ext4 image tooling exists?

Observed inside the dev container:

```text
mformat=/usr/bin/mformat
mcopy=/usr/bin/mcopy
mdir=/usr/bin/mdir
mkfs.vfat=<missing>
mkfs.fat=<missing>
mkfs.ext4=<missing>
truncate=/usr/bin/truncate
dd=/usr/bin/dd
qemu-img=/usr/bin/qemu-img
debugfs=<missing>
```

FAT32 can use existing `mtools` (`mformat -F -i image`, `mcopy`, `mdir`) without
installing `dosfstools`.

ext4 tooling is missing. BUILD-4D should either:

- add `e2fsprogs` to the dev container and generate ext4 images in `target/`
  with deterministic mkfs flags, or
- write a tiny controlled ext4 image generator in Rust.

PM recommendation: add `e2fsprogs` when ext4 starts. Do not hand-author ext4
binary fixtures.

### Q11 - Can FAT32/ext4 tests avoid large binary fixtures?

Yes.

Follow the existing NeoFS pattern: generate storage images into `target/` from
xtask before QEMU boots, attach them as the virtio block device, and keep only
small source-side fixture manifests or staging directories in the repo.

For FAT32, xtask can create an empty raw image, format it with `mformat`, and
copy deterministic files with `mcopy`.

For ext4, xtask should generate a staging directory and run `mkfs.ext4` with
fixed UUID/label/lazy-init settings once `e2fsprogs` exists. The tests need
deterministic contents and behavior; they do not need a committed binary image.

### Q12 - What is the smallest safe BUILD-4A slice?

BUILD-4A should be:

- activate `LOOKUP` and `READDIR` rights;
- allow opening directory inodes as `Handle::File`;
- add directory operation surface to `Inode` or an adjacent `DirectoryOps`
  helper;
- add `OpenFile::readdir_one(cursor)` with type-first, rights-second gating;
- add `SYS_READDIR = 16` and libneo wrapper;
- implement readdir over testfs root/dir and NeoFS root;
- preserve existing `sys_open` compatibility and all existing VFS tests.

Do not add `sys_mount`, `sys_umount`, FAT32, ext4, symlinks, nested path walk,
`stat`, `lseek`, `unlink`, `rename`, or rootfs pivot in BUILD-4A.

## 3. Recommended BUILD-4 Split

### BUILD-4A - Directory handles and `sys_readdir`

Land directory FileCaps and cursor readdir over testfs and NeoFS. This is the
substrate proof that directory authority is a normal FileCap operation.

### BUILD-4B - Mount table refactor and mount syscalls

Replace `/neofs` special-casing with a small explicit mount table. Add
`sys_mount` / `sys_umount` using directory FileCaps and `MOUNT` rights. Enforce
`EBUSY` with live-handle accounting before admitting `sys_umount`.

### BUILD-4C - FAT32 read-only

Add a minimal FAT32 read-only driver using generated mtools images. Exercise
mount, lookup, readdir, read, no write, and default noexec.

### BUILD-4D - ext4 read-only

Add a minimal controlled ext4 read-only driver after container tooling is
settled. Exercise xattr distrust and type import as Regular/Directory only.

### BUILD-4E - Integration close

Run full `NEO_SMP=4 --no-meta`, aarch64 runtime barrier lane, docs, caveats,
and close report.

## 4. BUILD-4A Implementation Outline

1. Rename/activate file rights:
   - add active `LOOKUP`, `READDIR`;
   - keep reserved aliases;
   - add `KNOWN_BUILD4A = KNOWN_BUILD2 | LOOKUP | READDIR`.
2. Add directory entry types:
   - `DirEntryKind` can reuse `InodeKind` encoded as ABI `u8`;
   - add an internal `VfsDirEntry { name: String, kind: InodeKind }`.
3. Add VFS operation:
   - either `Inode::readdir_at(cursor: u64)` with default `Err(NotDirectory)`;
   - or a separate helper trait if object safety gets awkward.
4. Permit directory open:
   - `open_root_temporary()` accepts `Directory` if requested rights are within
     directory available rights;
   - regular/model behavior remains unchanged.
5. Add `OpenFile::readdir_one()`:
   - `validate_current()`;
   - assert `kind == Directory` before rights check;
   - require `READDIR`;
   - return one entry plus next cursor, or EOF.
6. Add syscall:
   - `SYS_READDIR = 16`;
   - user buffer validation for output and cursor;
   - bounded copy through `with_user_access`;
   - no allocation inside user-copy AC window.
7. Add libneo wrapper:
   - expose `FileRights::LOOKUP`, `READDIR`;
   - expose `DirEntryKind`, `DirentHeader`, and `sys_readdir`.
8. Add scenarios and xtask wiring.

## 5. BUILD-4A Test Scenario Matrix

Required new scenarios:

| Scenario | Purpose |
| --- | --- |
| `phase3_vfs_readdir_testfs_root` | Open `/` with `READDIR`, iterate entries, assert `hello.txt`, `scratch`, `dir`, and `link` with expected kinds. |
| `phase3_vfs_readdir_requires_right` | Open directory without `READDIR`; `sys_readdir` returns `EACCES`. |
| `phase3_vfs_readdir_regular_is_enotdir` | Call `sys_readdir` on a regular file handle; returns `ENOTDIR`, proving type-first directory dispatch. |
| `phase3_vfs_directory_read_is_eisdir` | `sys_read` on a directory handle still returns `EISDIR`; readdir did not weaken regular-file read gates. |
| `phase3_neofs_readdir_root` | Mount NeoFS readonly, open `/neofs`, enumerate `hello.txt`, `model.neom`, `subdir` with Regular/Model/Directory kinds. |
| `phase3_neofs_readdir_stale_generation` | Open `/neofs`, mutate root through existing create path, then readdir the old handle and expect `ESTALE`. |
| `phase3_readdir_cursor_eof_stable` | Readdir to EOF, call again with EOF cursor, assert `0` and no duplicate terminal frame behavior. |

Regression scenarios to keep green:

- all existing Phase 3 VFS/testfs scenarios;
- NeoFS mount/read/write/fsync scenarios;
- `phase3_neofs_filecap_generation_stale`;
- full default `cargo neo-test --no-meta` before close.

## 6. Risks And Stop Conditions

Stop and surface if any of these happen:

- Directory handles require a new handle-table variant. That would contradict
  current source evidence and needs PM review.
- `sys_readdir` cannot preserve type-first, rights-second errors without a
  larger `file_lookup` refactor.
- NeoFS stale-generation behavior for directory handles diverges from regular
  FileCaps.
- BUILD-4A needs mount-table surgery before readdir can be tested. That means
  the slice is too large and should be split again.
- ext4 work wants to start before FAT32 and mount substrate are green.
- Any implementation tries to add binary filesystem images to the repo.

## 7. Anti-Scope

Not in BUILD-4A:

- `sys_mount`;
- `sys_umount`;
- FAT32 parser;
- ext4 parser;
- symlink traversal;
- hard links;
- dentry cache;
- per-process mount namespaces;
- rootfs pivot;
- file-backed mmap/exec;
- broad POSIX surface (`stat`, `lseek`, `unlink`, `rename`);
- writable foreign filesystems;
- ext4 xattr import;
- CAS/dedup BUILD-3F.

## 8. PM Disposition Recommendation

PM leans are mostly ratified, with one adjustment:

- Keep directory handles as `Handle::File`.
- Activate `LOOKUP` / `READDIR` in BUILD-4A.
- Defer `MOUNT` activation to BUILD-4B, but keep its bit stable now.
- Use `SYS_READDIR = 16`, `SYS_MOUNT = 17`, `SYS_UMOUNT = 18`.
- Shape mount/umount as directory-capability operations, not ambient
  absolute-path operations.
- Proceed to BUILD-4A after PM consumes this audit.
