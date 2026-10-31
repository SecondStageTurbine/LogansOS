# Phase 3 BUILD-1 Sub-Commit 1C Verification

Date: 2026-05-12
Branch: `codex/phase-3-build-1-jarvis`
Dispatch: `Plans/DISPATCH-2026-05-12-phase-3-build-1-1c-vfs-file-syscalls-jarvis.md`
Commit: containing commit. This artifact is committed with the code, so the
exact hash is self-referential; use `git log -1 --oneline` after commit.

## Verdict

PASS for BUILD-1 sub-commit 1C implementation and runtime gates. No 1D work was
started.

The only non-PASS static gate is `cargo fmt --check`, which is environment
blocked because `rustfmt` is not installed for the pinned nightly on either the
Windows host or the Docker dev container.

## Files Changed

- `kernel/src/fs/mod.rs`
- `kernel/src/main.rs`
- `kernel/src/errno.rs`
- `kernel/src/arch/x86_64/syscall.rs`
- `kernel/src/sched/process.rs`
- `kernel/src/test_harness.rs`
- `kernel/Cargo.toml`
- `tools/xtask/src/main.rs`
- `userland/libneo/src/lib.rs`
- `userland/neo-init/src/main.rs`
- `userland/neo-init/Cargo.toml`
- `userland/neo-worker/Cargo.toml`
- `Plans/PHASE-3-BUILD-1-SUB-COMMIT-1C-VERIFICATION-2026-05-12.md`

## ABI And Scope

- `SYS_WRITE = 1` remains the existing raw write ABI.
- `SYS_OPEN = 12`, `SYS_READ = 13`, and `SYS_CLOSE = 14` were appended after
  `SYS_RECV_GRANT = 11`.
- `libneo` now exposes `FileHandle`, `FileRights`, `sys_open`, `sys_read`, and
  `sys_close`; the existing fd-oriented `sys_write` wrapper is retained for
  stdout and file writes.
- Temporary 1C open ABI:
  `sys_open(path_ptr, path_len, flags, requested_rights_u32)`, with `flags = 0`.
- Intended future ABI:
  `sys_open(dir_cap, path, flags, requested_rights)`, rooted in a directory
  capability rather than the temporary root namespace.

## Capability And Rights Rules

- `FileRights` is a `u32`.
- Active BUILD-1 bits: `READ = 0x00000001`, `WRITE = 0x00000002`.
- Reserved forward bits: `LOOKUP`, `READDIR`, `EXECUTE`, `MMAP`, `GRANT`,
  `DERIVE`, `RETYPE`, and `MOUNT`.
- File handles are represented as `Handle::File` and carry `FileRights` rather
  than the legacy `CapRights` byte pair.
- Generic FileCap grant/derive/revoke semantics remain out of scope. File
  handles expose no legacy meta-rights through `Handle::rights()`, so the
  existing derive/grant/revoke paths reject them instead of minting file
  descendants.

## Stdout Namespace Rule

BUILD-1 reserves fd 1 permanently for serial stdout compatibility:

- `sys_open` never returns handle index 1.
- `sys_write(1, ...)` routes directly to serial and never resolves the process
  handle table.
- `sys_read(FileHandle(1), ...)` and `sys_close(FileHandle(1))` return `EBADF`.
- This rule is specific to the `sys_write(1)` compatibility namespace; existing
  IPC/process handle indices are not migrated in 1C.

## Error Evidence

VFS distinguishes type and rights failures:

- `VfsError::TypeForbidden` maps to `EISDIR (-21)`.
- `VfsError::InsufficientRights` maps to `EACCES (-13)`.

Serial evidence:

`target/neo-test/phase3_vfs_type_vs_rights_errors.serial.log`

```text
[ neo-init ] vfs errors type=EISDIR(-21) rights=EACCES(-13)
NEO_TEST/1 event=PASS name=phase3_vfs_type_vs_rights_errors vfs_user_scenario=ok exit_code=0
```

## Serial Logs

- `target/neo-test/phase3_vfs_testfs_open_read_close.serial.log`
- `target/neo-test/phase3_vfs_testfs_write_readback.serial.log`
- `target/neo-test/phase3_vfs_type_vs_rights_errors.serial.log`
- `target/neo-test/phase3_vfs_stdout_compat.serial.log`
- `target/neo-test/phase3_virtio_blk_raw_rw_smoke.serial.log`
- `target/neo-test/irqsafe_mutex_bench.serial.log`

VFS evidence lines:

```text
[ neo-init ] vfs open/read/close ok second_close=-9
[ neo-init ] vfs write/readback ok reopened=true
[ neo-init ] vfs errors type=EISDIR(-21) rights=EACCES(-13)
NEO_STDOUT_COMPAT/1 fd=1 route=serial
[ neo-init ] vfs stdout compat ok sys_open_skips_fd1=true
```

Virtio retained-gate evidence:

```text
[ kernel ] virtio-blk: feature-policy ... F_FLUSH_offered=true F_FLUSH_negotiated=false F_CONFIG_WCE_negotiated=false ...
NEO_TEST/1 event=PASS name=phase3_virtio_blk_raw_rw_smoke ... descriptor_publish_barriers=2 notification_suppression_barriers=2 F_FLUSH_offered=true F_FLUSH_negotiated=false F_CONFIG_WCE_negotiated=false
```

## Commands

| Command | Result | Notes |
| --- | --- | --- |
| `cargo k` | PASS | Warnings only. |
| `cargo k --no-default-features` | PASS | Warnings only. |
| `docker compose run --rm neo cargo neo-test phase3_vfs_testfs_open_read_close` | PASS | Open/read/close and second-close `EBADF`. |
| `docker compose run --rm neo cargo neo-test phase3_vfs_testfs_write_readback` | PASS | `/scratch` write then reopen/readback. |
| `docker compose run --rm neo cargo neo-test phase3_vfs_type_vs_rights_errors` | PASS | `EISDIR` and `EACCES` distinguished. |
| `docker compose run --rm neo cargo neo-test phase3_vfs_stdout_compat` | PASS | fd 1 serial route preserved; `sys_open` skips fd 1. |
| `docker compose run --rm neo cargo neo-test phase3_virtio_blk_raw_rw_smoke` | PASS | F_FLUSH offered/not negotiated; CONFIG_WCE not negotiated; descriptor and notification barriers counted separately. |
| `$env:NEO_SMP='4'; docker compose run --rm neo cargo neo-test all` | FAIL then isolated | Stopped at existing `irqsafe_mutex_bench` timing threshold (`overhead_ns=13579 threshold_ns=6000`). |
| `docker compose run --rm -e NEO_SMP=4 neo cargo neo-test irqsafe_mutex_bench` | PASS after xtask retry | Current serial: `overhead_ns=5985 threshold_ns=6000`. |
| `docker compose run --rm -e NEO_SMP=4 neo cargo neo-test all` | PASS | Full SMP=4 suite passed after isolating the benchmark timing miss. |
| `git diff --check` | PASS | Git emitted CRLF working-copy warnings only. |
| `cargo fmt --check` | BLOCKED | Host: `cargo-fmt.exe` missing for `nightly-2026-04-16-x86_64-pc-windows-msvc`. |
| `docker compose run --rm neo cargo fmt --check` | BLOCKED | Container: `cargo-fmt` missing for `nightly-2026-04-16-x86_64-unknown-linux-gnu`. |

## Non-Scope Confirmed

No NeoFS, fsync, mount namespaces, readdir, mmap/exec, symlinks, hard links, or
FileCap grant/derive/revoke semantics were implemented in 1C.
