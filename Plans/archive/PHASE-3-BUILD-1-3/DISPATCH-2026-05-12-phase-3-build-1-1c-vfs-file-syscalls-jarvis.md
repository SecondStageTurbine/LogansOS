# Dispatch: Phase 3 BUILD-1 1C - Minimal VFS, Testfs, File Syscalls

Date: 2026-05-12
Recipient: Jarvis
Branch: `codex/phase-3-build-1-jarvis`
Base state: `b25c17d kernel/block: implement virtio-blk raw read write`
Priority: Resume BUILD-1 sub-commit 1C only

## PM Decision

BUILD-1 sub-commit 1B is accepted as PASS.

Evidence:

- Commit: `b25c17d3563def97702071c5c51fa4ee9619179b`
- Artifact: `Plans/PHASE-3-BUILD-1-SUB-COMMIT-1B-VERIFICATION-2026-05-12.md`
- `cargo k` PASS.
- `cargo k --no-default-features` PASS.
- Required virtio-blk Docker scenarios PASS.
- `NEO_SMP=4` raw read/write PASS.
- `NEO_SMP=4 docker compose run --rm neo cargo neo-test all` PASS after rerun, with the first miss isolated to the existing `irqsafe_mutex_bench` timing threshold.
- `git diff --check` PASS.
- `cargo fmt --check` remains host-blocked by missing pinned nightly rustfmt.

PM now authorizes BUILD-1 sub-commit 1C.

Do not proceed into 1D closure/integration until 1C lands, verifies, and reports back.

## 1C Scope

Implement the minimal VFS/testfs/file-syscall slice from ADR-021:

- `kernel/src/fs/` module tree if not already present;
- mount table sufficient for one testfs root;
- path lookup sufficient for smoke tests;
- inode object abstraction;
- file object/open-handle abstraction;
- file handles as capabilities;
- `sys_open`, `sys_read`, `sys_write`, `sys_close`;
- thin `libneo` wrappers;
- QEMU scenarios proving open/read/close and write/readback.

This is still BUILD-1. Keep it narrow.

## Explicit Non-Scope

Do not implement:

- NeoFS native format;
- disk-backed rootfs pivot;
- FAT32/ext4;
- mount namespaces;
- `sys_mount`/`sys_umount`;
- `sys_readdir`;
- `sys_fsync`;
- `mmap`/`execve` file semantics;
- typed-inode enforcement beyond structural forward hooks;
- hard links;
- symlinks;
- FileCap GRANT/DERIVE/REVOKE semantics for files.

Existing block/virtio 1B behavior must continue to pass but should not expand in 1C.

## Required ABI Shape

Append syscall numbers after the existing table:

- current last public syscall is `SYS_RECV_GRANT = 11`;
- add new numbers after that in both `kernel/src/arch/x86_64/syscall.rs` and `userland/libneo/src/lib.rs`;
- preserve the existing raw syscall calling convention.

Preferred temporary BUILD-1 open ABI:

```text
sys_open(path_ptr, path_len, flags, requested_rights_u32) -> file_handle_or_errno
```

This is a root-open shortcut for BUILD-1 smoke tests only. Label it temporary in code/comments and in the verification artifact. The target model remains capability-rooted open:

```text
sys_open(dir_cap, path, flags, requested_rights)
```

Do not design the kernel internals around permanent ambient absolute-path authority.

## File Handle / Rights Requirements

File handles must be capability-shaped.

Requirements:

- `FileRights` is a `u32`, not `u16` and not the existing `CapRights.semantic: u8`.
- At minimum define READ and WRITE now, with reserved room/comments for future LOOKUP, READDIR, EXECUTE, MMAP bits, GRANT, DERIVE, RETYPE, and MOUNT authority.
- If requested rights are not a subset of available rights, return an explicit insufficient-rights error. Do not silently attenuate.
- Keep `TypeForbidden` and `InsufficientRights` distinguishable at the VFS layer and in test evidence.
- If syscall errno values need to be extended to keep them distinguishable, mirror those constants in `libneo`.
- Do not broaden the existing IPC/channel/process capability semantics to make FileRights fit. If a common rights migration becomes necessary, STOP and file a blocker.

Suggested low-risk shape:

- add `Handle::File { file: Arc<OpenFile>, cap_id, rights: FileRights }`;
- add `HandleTable::remove(idx)` for `sys_close`;
- leave existing channel/process handle behavior and cap tests intact;
- keep FileCap grant/derive/revoke deferred unless it is trivial and does not perturb existing capability semantics.

## `sys_write(fd=1)` Compatibility Gate

Existing userspace depends on:

```text
sys_write(1, buf, len) -> serial stdout
```

1C must preserve that behavior.

Because file handles are also integer handles, explicitly define and test the namespace rule. Acceptable approaches include reserving stdio pseudo-fds, preventing `sys_open` from returning `1`, or another equally clear compatibility rule. Do not leave `fd == 1` ambiguous between stdout and an open file.

The verification artifact must state the chosen rule.

## Path Lookup / Testfs Rules

BUILD-1 path lookup may be small:

- root mount only;
- UTF-8 paths;
- absolute test paths accepted by the temporary root-open ABI;
- `.` allowed if easy;
- `..` clamps at root if implemented;
- symlinks return `TypeForbidden` or equivalent unsupported-type error;
- no hard links;
- no directory enumeration.

Testfs may be in-memory or synthetic. It should include:

- at least one immutable known file for read tests;
- at least one mutable node or create/open flag path for write/readback tests;
- at least one directory/non-regular node if needed to prove `TypeForbidden`.

## Required Scenarios

Add or complete:

- `phase3_vfs_testfs_open_read_close`
- `phase3_vfs_testfs_write_readback`
- `phase3_vfs_type_vs_rights_errors`
- `phase3_vfs_stdout_compat`

Scenario expectations:

- open a known testfs file, read exact bytes, close it, and prove second close returns the expected error;
- write bytes through a writable handle, read them back through the same handle if seek exists or through a second handle/reopen if seek is deferred;
- prove a type-forbidden operation and an insufficient-rights operation return distinguishable evidence;
- prove `sys_write(1, ...)` still routes to serial stdout and does not collide with a file handle.

Existing cap, IPC, scheduler, and virtio-blk scenarios must remain PASS.

## Verification Gate

Run at minimum:

```powershell
cargo k
cargo k --no-default-features
docker compose run --rm neo cargo neo-test phase3_vfs_testfs_open_read_close
docker compose run --rm neo cargo neo-test phase3_vfs_testfs_write_readback
docker compose run --rm neo cargo neo-test phase3_vfs_type_vs_rights_errors
docker compose run --rm neo cargo neo-test phase3_vfs_stdout_compat
docker compose run --rm neo cargo neo-test phase3_virtio_blk_raw_rw_smoke
$env:NEO_SMP='4'; docker compose run --rm neo cargo neo-test all
git diff --check
cargo fmt --check
```

If `cargo fmt --check` remains blocked by missing `cargo-fmt.exe` for `nightly-2026-04-16-x86_64-pc-windows-msvc`, record it exactly as prior artifacts do.

## Reporting

On PASS:

- Create `Plans/PHASE-3-BUILD-1-SUB-COMMIT-1C-VERIFICATION-2026-05-12.md`.
- Include commit hash, files changed, commands/results, and serial-log paths.
- State the syscall numbers and libneo wrapper names.
- State the temporary root-open ABI and migration path to capability-rooted open.
- State the file-handle namespace rule for stdout compatibility.
- State the `FileRights` width and defined bits.
- Include evidence that `TypeForbidden` and `InsufficientRights` are distinguishable.
- Commit the code and verification artifact as the 1C sub-commit.

On FAIL:

- Stop.
- File `Plans/FAILURE-2026-05-12-phase-3-build-1-1c-jarvis.md`.
- Include failing scenario, serial log, suspected source area, and whether the failure is VFS lookup, handle-table integration, syscall ABI, user-copy, libneo, or testfs behavior.

## Stop Conditions

Stop immediately if:

- `FileRights` can only be represented by shrinking into `u16`/`u8`;
- file handles require a broad rewrite of existing IPC/process capability semantics;
- `sys_write(1, ...)` compatibility cannot be preserved cleanly;
- `sys_close` risks draining unrelated process handles;
- path lookup introduces permanent ambient authority assumptions;
- type and rights failures collapse into one undistinguishable error path;
- existing `docker compose run --rm neo cargo neo-test all` regresses.

## PM Posture

1C is the bridge between storage and the user-visible file surface. The win condition is not "a full filesystem"; it is a capability-shaped VFS skeleton that can survive BUILD-2 NeoFS without a migration fight.
