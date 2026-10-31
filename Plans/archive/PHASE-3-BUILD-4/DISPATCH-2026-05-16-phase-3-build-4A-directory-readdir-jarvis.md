# Dispatch - Phase 3 BUILD-4A Directory FileCaps And `sys_readdir`

Date: 2026-05-16
Agent: Jarvis
Status: AUTHORIZED IMPLEMENTATION
Authority:

- `Plans/PM-DISPOSITION-2026-05-16-phase-3-build-4-prereq-audit-consumption.md`
- `Plans/OBSERVE-2026-05-16-phase-3-build-4-prereq-audit-jarvis.md`
- `docs/ADR-021-phase-3-design-substrate.md`

Substrate:

```text
f2bea51 plans: record Phase 3 BUILD-4 prereq audit
```

## 1. Mission

Implement BUILD-4A:

```text
directory FileCaps + cursor sys_readdir over testfs and NeoFS
```

This is the first BUILD-4 code lane. It must prove directory authority is a
normal FileCap operation before mount syscalls or foreign filesystem parsers are
admitted.

## 2. Scope

Authorized code scope:

- activate `FileRights::LOOKUP` and `FileRights::READDIR`;
- keep reserved aliases for compatibility;
- allow directory inodes to be opened as `Handle::File`;
- add a VFS directory-entry operation surface;
- add `OpenFile::readdir_one()` with type-first, rights-second enforcement;
- add `SYS_READDIR = 16` in kernel and libneo;
- add libneo wrapper and public dirent constants;
- implement readdir for testfs root and `/dir`;
- implement readdir for NeoFS root, using existing root entries;
- preserve NeoFS stale FileCap generation behavior for directory handles;
- add and run the required scenarios.

Anti-scope:

- no `SYS_MOUNT`;
- no `SYS_UMOUNT`;
- no FAT32;
- no ext4;
- no mount-table refactor unless a tiny helper is absolutely required for
  directory readdir;
- no symlink traversal;
- no hard links;
- no dentry cache;
- no per-process mount namespace;
- no rootfs pivot;
- no file-backed mmap/exec;
- no broad POSIX surface (`stat`, `lseek`, `unlink`, `rename`);
- no BUILD-3F CAS/dedup.

## 3. ABI Requirements

Add:

```text
SYS_READDIR = 16
```

Kernel ABI:

```text
sys_readdir(fd: u32, out_ptr: *mut u8, out_len: usize, cursor_ptr: *mut u64) -> i64
```

Return values:

- `>0`: one entry written, byte count returned;
- `0`: EOF;
- `<0`: negative errno.

`cursor_ptr` points to a userspace `u64`. Caller initializes it to `0`. Kernel
updates it to `next_cursor` after a successful entry. Cursor value is opaque to
userspace; BUILD-4A may use simple entry indices internally.

Stable `NeoDirentV1` byte layout:

```text
offset  size  field
0       4     record_len: u32 little-endian
4       2     name_len: u16 little-endian
6       1     kind: u8
7       1     flags: u8
8       8     next_cursor: u64 little-endian
16      N     name bytes, UTF-8, not NUL-terminated
```

`record_len = 16 + name_len`.

Kind values:

```text
1 = Directory
2 = Regular
3 = Symlink
4 = Model
```

`flags` is zero for BUILD-4A.

## 4. Required Implementation Shape

### Step 1 - Rights and errno

Files:

- `kernel/src/fs/mod.rs`
- `kernel/src/errno.rs`
- `userland/libneo/src/lib.rs`

Changes:

- Add active `FileRights::LOOKUP` and `FileRights::READDIR`.
- Preserve `LOOKUP_RESERVED` and `READDIR_RESERVED` as aliases.
- Add `KNOWN_BUILD4A = KNOWN_BUILD2 | LOOKUP | READDIR`.
- Add `ENOTDIR = -20`.
- Add `EBUSY`, `EEXIST`, and `ENOTEMPTY` constants if cheap, but BUILD-4A only
  needs `ENOTDIR`.

### Step 2 - Directory operation surface

File:

- `kernel/src/fs/mod.rs`

Add an internal directory-entry type and an object-safe operation, for example:

```text
VfsDirEntry { name: String, kind: InodeKind, next_cursor: u64 }
Inode::readdir_at(cursor: u64) -> Result<Option<VfsDirEntry>, VfsError>
```

Default implementation returns `VfsError::NotDirectory`.

`OpenFile::readdir_one(cursor)` must enforce:

1. `validate_current()`;
2. `inode.kind() == Directory`, else `VfsError::NotDirectory`;
3. rights contain `READDIR`, else `VfsError::InsufficientRights`;
4. delegate to inode readdir.

This order is load-bearing. It preserves type-first, rights-second dispatch.

### Step 3 - testfs directory handles

File:

- `kernel/src/fs/mod.rs`

Refactor testfs enough to distinguish root directory from `/dir`:

- root directory lists `hello.txt`, `scratch`, `dir`, `link`;
- `/dir` is an empty directory;
- both advertise `LOOKUP | READDIR`;
- regular files keep regular rights;
- symlink remains inert/unsupported.

`open_root_temporary()` must accept directories when requested rights are a
subset of directory available rights. It must continue rejecting symlinks and
invalid requested bits.

### Step 4 - NeoFS directory readdir

Files:

- `kernel/src/fs/neofs/mount.rs`
- `kernel/src/fs/neofs/inode.rs`

Implement directory available rights and readdir:

- NeoFS directory `available_rights()` returns `LOOKUP | READDIR`.
- Root readdir may use `NeoFsState.entries`.
- Non-root directory readdir may parse the inode's directory content if natural.
- Existing current-generation validation must remain anchored to the reachable
  inode generation.

The normal mkfs image root entries are:

```text
hello.txt -> Regular
model.neom -> Model
subdir -> Directory
```

### Step 5 - syscall and libneo

Files:

- `kernel/src/arch/x86_64/syscall.rs`
- `userland/libneo/src/lib.rs`

Add:

- `SYS_READDIR = 16`;
- dispatch arm;
- bounded user buffer validation;
- copy `cursor` in from userspace;
- resolve FileCap without premature rights gating;
- call `OpenFile::readdir_one()`;
- encode the little-endian header manually;
- copy record and updated cursor back through `with_user_access`;
- return byte count or `0` at EOF.

Do not add a new raw syscall ABI unless absolutely necessary; existing `raw4`
is enough.

### Step 6 - scenarios

Likely files:

- `tools/xtask/src/main.rs`
- `userland/neo-init/src/main.rs`
- `kernel/src/test_harness.rs`

Required new scenarios:

```text
phase3_vfs_readdir_testfs_root
phase3_vfs_readdir_requires_right
phase3_vfs_readdir_regular_is_enotdir
phase3_vfs_directory_read_is_eisdir
phase3_neofs_readdir_root
phase3_neofs_readdir_stale_generation
phase3_readdir_cursor_eof_stable
```

Userland-facing testfs scenarios should live in `neo-init` where practical.
NeoFS scenarios can live in `kernel/src/test_harness.rs`, matching the existing
NeoFS scenario pattern.

## 5. Verification Gates

Run, in order:

```text
cargo k
cargo k --no-default-features
cargo neo-test phase3_vfs_readdir_testfs_root
cargo neo-test phase3_vfs_readdir_requires_right
cargo neo-test phase3_vfs_readdir_regular_is_enotdir
cargo neo-test phase3_vfs_directory_read_is_eisdir
cargo neo-test phase3_neofs_readdir_root
cargo neo-test phase3_neofs_readdir_stale_generation
cargo neo-test phase3_readdir_cursor_eof_stable
```

Then run targeted anti-regressions:

```text
cargo neo-test phase3_vfs_testfs_open_read_close
cargo neo-test phase3_vfs_testfs_write_readback
cargo neo-test phase3_vfs_type_vs_rights_errors
cargo neo-test phase3_block_vfs_smoke
cargo neo-test phase3_neofs_mount_readonly_root
cargo neo-test phase3_neofs_filecap_generation_stale
cargo neo-test phase3_neofs_fsync_dirty_persists_after_crash
```

Final close gate:

```text
NEO_SMP=4 cargo neo-test --no-meta
```

Capture literal `NEO_TEST/1 event=PASS name=<scenario>` lines for every new
scenario and include them in the close artifact.

## 6. Stop Conditions

Stop and surface before committing if:

- directory handles appear to require a new handle variant;
- `sys_readdir` on a regular file returns `EACCES` instead of `ENOTDIR`;
- existing `sys_read` on a directory stops returning `EISDIR`;
- NeoFS stale generation behavior weakens or becomes path-name based instead of
  current reachable inode generation anchored;
- implementation wants to add mount syscalls or foreign FS code;
- ext4/FAT32 tooling enters the patch;
- any new source warning appears that is not mechanically justified.

## 7. Close Artifact

Create:

```text
Plans/PHASE-3-BUILD-4A-CLOSE-EVIDENCE-2026-05-16.md
```

Required content:

- commit hash;
- diff stat;
- build outputs summarized with exit status;
- literal PASS lines for all seven new scenarios;
- targeted anti-regression results;
- final `NEO_SMP=4 --no-meta` result;
- deviations/course corrections;
- explicit statement that no mount/FAT32/ext4 code landed.

Suggested implementation commit subject:

```text
build-4A: add directory FileCaps and sys_readdir
```
