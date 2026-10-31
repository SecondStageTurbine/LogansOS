# Dispatch - Phase 3 BUILD-4B Mount Table And Mount Syscalls

Date: 2026-05-20
Agent: Cerebro
Status: AUTHORIZED IMPLEMENTATION

Authority:

- `Plans/PM-DISPOSITION-2026-05-20-wake-token-state-if-invariant-correction-consumption.md`
- `Plans/PM-DISPOSITION-2026-05-16-phase-3-build-4-prereq-audit-consumption.md`
- `Plans/OBSERVE-2026-05-16-phase-3-build-4-prereq-audit-jarvis.md`
- `Plans/PHASE-3-BUILD-4A-CLOSE-EVIDENCE-2026-05-16.md`
- `docs/ADR-021-phase-3-design-substrate.md`

Substrate:

```text
0682d24 test/wake-token: wrap remaining WAKE_TOKEN_STATE writes in without_interrupts
```

## 1. Mission

Implement BUILD-4B:

```text
explicit mount table + directory-capability sys_mount/sys_umount
```

BUILD-4A proved that directory authority is a normal FileCap operation through
`SYS_READDIR = 16`. BUILD-4B extends that model to mount authority. Mount and
unmount are directory FileCap operations requiring `FileRights::MOUNT`; they are
not ambient absolute-path syscalls.

This lane is substrate only. It refactors the existing `/neofs` special case into
a real mount table and exposes mount/unmount over the syscall ABI. It does not
start FAT32 or ext4.

## 2. Scope

Authorized code scope:

- activate `FileRights::MOUNT` while preserving `MOUNT_RESERVED` as an alias;
- add `KNOWN_BUILD4B = KNOWN_BUILD4A | MOUNT` in kernel and libneo;
- reserve and implement:

```text
SYS_MOUNT  = 17
SYS_UMOUNT = 18
```

- replace the current `MountTable { root, neofs }` special case with a small
  explicit mount table;
- represent `/` as the root testfs mount and existing NeoFS mounts as normal
  child mount entries;
- preserve all existing internal NeoFS test helpers that mount device 0, but
  route them through the new mount table;
- add live-handle accounting so `sys_umount` returns `EBUSY` while any open
  `OpenFile` references the target mount;
- make `OpenFile` retain a mount-lifetime token or equivalent counted reference;
- implement directory-capability mount and unmount syscalls;
- add libneo wrappers and request structs/constants;
- add and run the required scenarios.

Anti-scope:

- no FAT32 parser;
- no ext4 parser;
- no `mtools`, `e2fsprogs`, or foreign image generation;
- no mount namespaces;
- no bind mounts;
- no symlink traversal;
- no hard links;
- no dentry cache;
- no rootfs pivot;
- no file-backed mmap/exec;
- no broad POSIX surface (`stat`, `lseek`, `unlink`, `rename`);
- no writable foreign filesystems;
- no default-noexec enforcement for foreign filesystems beyond reserving mount
  flag plumbing if it naturally falls out of the mount-table shape;
- no BUILD-3F CAS/dedup;
- no BUILD-5 AHCI/NVMe/bare-metal work.

## 3. ABI Requirements

Add:

```text
SYS_MOUNT  = 17
SYS_UMOUNT = 18
```

`sys_mount` ABI:

```text
sys_mount(dir_fd: u32, req_ptr: *const u8, req_len: usize, flags: u32) -> i64
```

`sys_umount` ABI:

```text
sys_umount(dir_fd: u32, name_ptr: *const u8, name_len: usize, flags: u32) -> i64
```

`dir_fd` is a directory FileCap. The operation must resolve the FileCap without
premature rights gating, then let the VFS mount operation enforce:

1. current-generation validation;
2. inode kind is `Directory`, else `ENOTDIR`;
3. rights contain `MOUNT`, else `EACCES`;
4. mount-table policy.

This mirrors BUILD-4A's type-first, rights-second operation discipline.

`MountRequestV1` is a versioned userspace byte buffer:

```text
offset  size  field
0       4     version: u32 little-endian, must be 1
4       4     fs_type: u32 little-endian
8       4     source_off: u32 little-endian
12      4     source_len: u32 little-endian
16      4     target_off: u32 little-endian
20      4     target_len: u32 little-endian
24      4     reserved0: u32, must be 0
28      4     reserved1: u32, must be 0
32      N     payload bytes containing source and target strings
```

`fs_type` values for BUILD-4B:

```text
1 = NeoFS device0 readonly
2 = NeoFS device0 readwrite
```

`source` is reserved for future multi-device support. For BUILD-4B it must be
either empty or `device0`; any other value returns `EINVAL`.

`target` is a single child name relative to `dir_fd`. It must be UTF-8, non-empty,
not contain `/`, and not be `.` or `..`. BUILD-4B does not implement nested
mount-target paths inside the request. To mount at `/neofs`, callers pass a root
directory FileCap and target `neofs`.

`flags` is accepted but limited:

```text
0 = default
```

Any nonzero flag returns `EINVAL` in BUILD-4B unless the implementation needs a
single internal test-only flag; surface that before commit if so.

## 4. Required Implementation Shape

### Step 1 - Rights and syscall constants

Files:

- `kernel/src/fs/mod.rs`
- `kernel/src/arch/x86_64/syscall.rs`
- `userland/libneo/src/lib.rs`

Changes:

- add active `FileRights::MOUNT = 1 << 27`;
- keep `MOUNT_RESERVED = MOUNT`;
- add `KNOWN_BUILD4B`;
- keep existing BUILD-4A rights stable;
- add `SYS_MOUNT = 17` and `SYS_UMOUNT = 18`;
- add libneo wrappers and request constants.

### Step 2 - Mount table substrate

File:

- `kernel/src/fs/mod.rs`

Replace the current special case:

```text
MountTable { root: Arc<TestFs>, neofs: Option<Arc<NeoFs>> }
```

with an explicit table. Suggested shape:

```text
MountId(u64)
MountFlags(u32)
MountEntry {
  id,
  parent_id,
  name,
  fs: Arc<dyn MountedFs>,
  flags,
  live_handles,
}
MountedFs {
  root() -> Arc<dyn Inode>
  lookup(relative_path) -> Result<Arc<dyn Inode>, VfsError>
}
```

This is a suggested shape, not a mandatory type spelling. The load-bearing
requirements are:

- longest-prefix or equivalent deterministic mount selection;
- root testfs represented as a mount entry;
- NeoFS represented as a mount entry, not a side field;
- opening a file/directory increments the selected mount's live-handle count;
- dropping/closing the resulting `OpenFile` decrements it;
- `sys_umount` can answer `EBUSY` precisely from that count.

Do not remove an entry from the namespace before proving it is not busy.

### Step 3 - Preserve existing internal mount helpers

Files:

- `kernel/src/fs/mod.rs`
- callers in `kernel/src/test_harness.rs` only if required by signature changes

Existing helpers:

```text
mount_neofs_from_device0_readonly()
mount_neofs_from_device0_readwrite()
```

must keep their public behavior for existing tests. They should now install or
replace the `/neofs` entry through the mount table. Existing NeoFS mount/read/
write/fsync scenarios must not know that the mount-table refactor happened.

If helper replacement semantics conflict with live-handle accounting, surface it
before commit. Do not silently force-unmount a busy mount.

### Step 4 - `sys_mount`

Files:

- `kernel/src/arch/x86_64/syscall.rs`
- `kernel/src/fs/mod.rs`
- `userland/libneo/src/lib.rs`

Implementation requirements:

- validate and copy the request buffer with normal user-access discipline;
- parse `MountRequestV1` with checked offsets and lengths;
- reject malformed buffers with `EINVAL`;
- resolve `dir_fd` without premature rights gating;
- enforce directory type before `MOUNT` rights;
- reject target collision with `EEXIST`;
- mount NeoFS device0 readonly/readwrite according to `fs_type`;
- return 0 on success.

### Step 5 - `sys_umount`

Files:

- `kernel/src/arch/x86_64/syscall.rs`
- `kernel/src/fs/mod.rs`
- `userland/libneo/src/lib.rs`

Implementation requirements:

- validate and copy the child-name buffer;
- resolve `dir_fd` without premature rights gating;
- enforce directory type before `MOUNT` rights;
- reject malformed names with `EINVAL`;
- return `ENOENT`/`NotFound` for absent mount target;
- return `EBUSY` while live handles reference the target mount;
- remove the mount entry only after the busy check passes;
- return 0 on success.

### Step 6 - Scenarios

Likely files:

- `tools/xtask/src/main.rs`
- `userland/neo-init/src/main.rs`
- `kernel/src/test_harness.rs`

Required new scenarios:

```text
phase3_mount_neofs_device0_readonly
phase3_mount_requires_mount_right
phase3_mount_regular_fd_is_enotdir
phase3_mount_target_exists_is_eexist
phase3_umount_busy_is_ebusy
phase3_umount_removes_namespace
phase3_mount_request_validation
```

Scenario expectations:

- `phase3_mount_neofs_device0_readonly`: root directory FileCap with `MOUNT`
  mounts NeoFS device0 at a child name, opens a known file through that mount,
  and reads expected content.
- `phase3_mount_requires_mount_right`: directory FileCap without `MOUNT` returns
  `EACCES`.
- `phase3_mount_regular_fd_is_enotdir`: regular-file fd passed as `dir_fd`
  returns `ENOTDIR`, proving type-first mount dispatch.
- `phase3_mount_target_exists_is_eexist`: mounting over an occupied child
  returns `EEXIST`.
- `phase3_umount_busy_is_ebusy`: open a file under the mounted filesystem, then
  unmount its mountpoint and expect `EBUSY`.
- `phase3_umount_removes_namespace`: close all handles, unmount successfully,
  then opening through the removed mountpoint returns not found.
- `phase3_mount_request_validation`: bad version, bad offsets, slash in target,
  and unsupported source all return `EINVAL`.

Do not add speculative FAT32/ext4/noexec scenarios in BUILD-4B.

## 5. Verification Gates

Run, in order:

```text
cargo k
cargo k --no-default-features
cargo neo-test phase3_mount_neofs_device0_readonly
cargo neo-test phase3_mount_requires_mount_right
cargo neo-test phase3_mount_regular_fd_is_enotdir
cargo neo-test phase3_mount_target_exists_is_eexist
cargo neo-test phase3_umount_busy_is_ebusy
cargo neo-test phase3_umount_removes_namespace
cargo neo-test phase3_mount_request_validation
```

Then run targeted anti-regressions:

```text
cargo neo-test phase3_vfs_readdir_testfs_root
cargo neo-test phase3_neofs_readdir_root
cargo neo-test phase3_neofs_filecap_generation_stale
cargo neo-test phase3_neofs_fsync_filecap_stale_regression
cargo neo-test phase3_sched_wake_token_abandon
cargo neo-test ipc_heap_kill_smoke
cargo neo-test cap_revoke_local
```

Final close gate:

```text
NEO_SMP=4 cargo neo-test --no-meta
```

Capture literal `NEO_TEST/1 event=PASS name=<scenario>` lines for every new
scenario and include them in the close artifact.

## 6. Stop Conditions

Stop and surface before committing if:

- mount/umount wants to become ambient absolute-path authority;
- `sys_mount` or `sys_umount` cannot preserve type-first, rights-second errors;
- live-handle accounting cannot answer `EBUSY` precisely;
- existing internal NeoFS mount helpers need force-unmount semantics;
- `/neofs` existing tests break in a way that requires changing scenario
  meaning rather than preserving behavior;
- FAT32/ext4 code enters the patch;
- any implementation tries to add binary filesystem images to the repo;
- a new `SwitchMutex` site is introduced without an explicit interrupt-state
  discipline;
- any new warning appears that is not mechanically justified.

## 7. Close Artifact

Create:

```text
Plans/PHASE-3-BUILD-4B-CLOSE-EVIDENCE-2026-05-20.md
```

Required content:

- commit hash;
- diff stat;
- build outputs summarized with exit status and warning count;
- literal PASS lines for all seven new scenarios;
- targeted anti-regression results;
- final `NEO_SMP=4 --no-meta` result;
- live-handle accounting explanation;
- explicit statement that no FAT32/ext4 code landed;
- explicit statement that mount authority is directory-capability rooted, not
  ambient absolute-path authority;
- any deviations/course corrections.

Suggested implementation commit subject:

```text
build-4B: add mount table and directory-capability mount syscalls
```
