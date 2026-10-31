# PM Disposition - Phase 3 BUILD-4 Prereq Audit Consumption

Date: 2026-05-16
From: PM
Status: ACCEPTED WITH ONE SHAPING DECISION / BUILD-4A AUTHORIZED
Consumed artifact:

```text
Plans/OBSERVE-2026-05-16-phase-3-build-4-prereq-audit-jarvis.md
f2bea51 plans: record Phase 3 BUILD-4 prereq audit
```

## 1. Disposition

The BUILD-4 prereq audit is accepted.

The audit ratifies the original PM split and adds one important shaping
decision: mount/umount should be directory-capability operations, not ambient
absolute-path operations. That is accepted. It keeps BUILD-4 aligned with the
capability model instead of reintroducing Unix-style ambient mount authority.

Implementation is authorized for BUILD-4A only:

```text
BUILD-4A = directory FileCaps + cursor sys_readdir over testfs and NeoFS.
```

No FAT32, ext4, sys_mount, or sys_umount code is authorized yet.

## 2. Decisions

### D1 - Directory handles

Accepted: directory handles are normal `Handle::File` values.

There will be no `DirHandle` table and no parallel fd namespace. Directory-ness
is an `InodeKind` operation namespace checked by VFS dispatch.

### D2 - Rights activation

Accepted:

- BUILD-4A activates `LOOKUP` and `READDIR`.
- BUILD-4B activates `MOUNT`.
- Existing reserved bit positions remain stable.
- Reserved aliases stay temporarily for source compatibility.

### D3 - Syscall slots

Accepted reservation:

```text
SYS_READDIR = 16
SYS_MOUNT   = 17
SYS_UMOUNT  = 18
```

BUILD-4A implements only `SYS_READDIR = 16`.

### D4 - `sys_readdir` ABI

Accepted:

```text
sys_readdir(fd: u32, out_ptr: *mut u8, out_len: usize, cursor_ptr: *mut u64) -> i64
```

Return value:

- positive: bytes written for one directory entry;
- `0`: EOF;
- negative: errno.

Record format is stable for BUILD-4A:

```text
NeoDirentV1:
  u32 record_len
  u16 name_len
  u8  kind
  u8  flags
  u64 next_cursor
  u8  name[name_len]
```

All integer fields are little-endian. `record_len` includes the 16-byte header.
Names are UTF-8 bytes with no NUL terminator. One call returns at most one
entry.

Kind values:

```text
1 = Directory
2 = Regular
3 = Symlink
4 = Model
```

### D5 - Error discipline

Accepted additions:

```text
EBUSY     = -16
EEXIST    = -17
ENOTDIR   = -20
ENOTEMPTY = -39
```

BUILD-4A must use `ENOTDIR` for `sys_readdir` on a non-directory handle.
`sys_read` on a directory remains `EISDIR`. Rights failure remains `EACCES`.

Important implementation note: the existing `file_lookup(fd, required)` helper
checks rights before the operation can check type. BUILD-4A must not use that
helper in a way that turns "readdir regular file" into `EACCES`. Add a lookup
path that resolves the FileCap first, then let `OpenFile::readdir_one()` enforce
type-first, rights-second.

### D6 - NeoFS readdir

Accepted: NeoFS readdir does not require disk-format widening.

Root readdir may use `NeoFsState.entries`. Non-root directory readdir may parse
directory content from the inode record if needed, but BUILD-4A tests only need
root plus the empty `subdir` if implementation naturally supports it.

The stale FileCap generation invariant remains current reachable inode
generation anchored. An old directory FileCap must return `ESTALE` after root
directory mutation, same as regular FileCaps.

### D7 - Mount shape for BUILD-4B

Accepted in principle, not authorized for code in BUILD-4A:

```text
sys_mount(dir_fd, req_ptr, req_len, flags)
sys_umount(dir_fd, name_ptr, name_len, flags)
```

Both require `MOUNT` on the directory FileCap. This consumes the audit's
directory-capability operation recommendation and prevents ambient absolute-path
mount authority from becoming the ABI.

### D8 - Unmount rule

Accepted for BUILD-4B:

Unmount returns `EBUSY` while any live handle references the mount. BUILD-4B
must add mount lifetime/open-count accounting before admitting `sys_umount`.

### D9 - Foreign-FS order and tooling

Accepted:

- FAT32 before ext4.
- FAT32 may use existing `mtools`.
- ext4 requires either `e2fsprogs` in the container or a controlled Rust image
  generator before implementation.
- No large binary filesystem fixtures are allowed in the repo.

### D10 - noexec policy

Accepted for BUILD-4C/4D:

Noexec is mount policy. VFS open computes effective rights by combining inode
rights with the mount's rights mask. `OpenFile::check_executable_policy()` stays
the final type-first, rights-second executable gate.

## 3. Authorized Next Lane

Authorized now:

```text
Plans/DISPATCH-2026-05-16-phase-3-build-4A-directory-readdir-jarvis.md
```

Scope:

- directory FileCaps;
- `LOOKUP` / `READDIR` rights;
- `SYS_READDIR = 16`;
- testfs root/dir readdir;
- NeoFS root readdir;
- stale directory FileCap regression;
- docs/verification artifact for BUILD-4A.

Not authorized:

- `SYS_MOUNT`;
- `SYS_UMOUNT`;
- FAT32;
- ext4;
- mount-table refactor beyond what is strictly necessary for readdir;
- symlink traversal;
- broad POSIX expansion;
- BUILD-3F CAS/dedup;
- BUILD-5 storage.

## 4. PM Close Condition For BUILD-4A

BUILD-4A closes only when:

- all new readdir scenarios PASS;
- existing Phase 3 VFS and NeoFS scenarios remain green;
- `cargo k` and `cargo k --no-default-features` pass;
- final `NEO_SMP=4 cargo neo-test --no-meta` passes or any retry behavior is
  disclosed with serial evidence;
- the close artifact records literal `NEO_TEST/1 event=PASS` lines for each new
  scenario.
