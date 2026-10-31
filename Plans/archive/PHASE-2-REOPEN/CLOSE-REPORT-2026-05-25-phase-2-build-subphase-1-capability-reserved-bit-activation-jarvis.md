# CLOSE REPORT: Phase 2 BUILD subphase 1 - capability reserved-bit activation

- Dispatch: `Plans/DISPATCH-2026-05-25-phase-2-build-subphase-1-capability-reserved-bit-activation-jarvis.md`
- Base commit before implementation: `e45406aa1b7de2d8b7be77e767c036494cb406a0`
- Implementation commit: `6587cd607222bb3ed9d2ac77d5d1dfa9940d69e3`
- Status: COMPLETE

## Files Changed

- `kernel/Cargo.toml`
- `kernel/src/arch/x86_64/syscall.rs`
- `kernel/src/errno.rs`
- `kernel/src/sched/process.rs`
- `kernel/src/test_harness.rs`
- `tools/xtask/src/main.rs`
- `userland/libneo/src/lib.rs`
- `userland/neo-init/Cargo.toml`
- `userland/neo-init/src/main.rs`
- `userland/neo-worker/Cargo.toml`
- `userland/neo-worker/src/main.rs`

## Syscall ABI

- `SYS_WAIT = 19`
  - Kernel ABI: `sys_wait(handle: u32, flags: u64) -> i64`
  - User ABI: `libneo::sys_wait(ProcessHandle, flags) -> Result<i32, i64>`
  - `WAIT_FLAG_WNOHANG = 1 << 0`
  - A still-running WNOHANG result returns `-EAGAIN`.
  - Exit success uses `WAIT_EXITED_TAG | (exit_code as u32)`, where `WAIT_EXITED_TAG = 1 << 32`.
  - The tagged success range is `[0x1_0000_0000, 0x1_ffff_ffff]`, so it cannot collide with any negative errno or any raw `i32` exit code. `libneo` decodes the tagged value back to `i32`.
  - Blocking wait parks the caller on a per-process preallocated wait queue; it does not busy-poll or allocate while parking.
  - Completion is first-consumer-wins: the first waiter/consumer receives the exit code, and later waiters for the same consumed completion see `-ESRCH`.

- `SYS_INSPECT = 20`
  - Kernel ABI: `sys_inspect(handle: u32, out: *mut ProcessSnapshotV1, out_len: usize) -> i64`
  - User ABI: `libneo::sys_inspect(ProcessHandle) -> Result<ProcessSnapshotV1, i64>`
  - `ProcessSnapshotV1` is `#[repr(C)]`, versioned with `version = 1`.
  - Snapshot fields are scalar only: state, pid, exit_code, thread_count, handle_count, channel_handle_count, bounded name length, and bounded name bytes.
  - No `Handle`, `CapId`, pointers, cap identities, or authority-bearing values are exposed.

## Rights And Errors

- `sys_kill` still requires process semantic `KILL` plus meta `USE`.
- `sys_wait` requires process semantic `WAIT` plus meta `USE`.
- `sys_inspect` requires process semantic `INSPECT` plus meta `USE`.
- Unallocated or non-process handle: `-EBADF`.
- Process handle missing meta `USE` or missing the operation-specific semantic right: `-EPERM`.
- Stale/absent target process or already-consumed completion: `-ESRCH`.
- `sys_wait` self-wait: `-EINVAL`.
- `sys_wait` invalid flags: `-EINVAL`.
- `sys_wait` WNOHANG while target has no completion yet: `-EAGAIN`.
- `sys_wait` wait queue full: `-EAGAIN`.
- `sys_inspect` null output pointer or wrong output length: `-EINVAL`.
- Existing `sys_close` file behavior is untouched. Process/channel/capability close and CLOSE-bit activation remain deferred for a later subphase.

## INV-CAP-11

Exact scenario added and wired:

- `cap_revoke_spawn_inheritance_descendants`

Coverage shape:

- Parent creates a control channel and a data root capability.
- Parent derives an inheritable/grantable source handle and passes it through `sys_spawn`.
- Worker derives a child capability from the inherited source handle.
- Parent revokes the original root capability.
- Worker verifies both the inherited source handle and its derived child handle are dead (`-EBADF`) after the revoke.

This covers the audit (b) property directly: revoke of a parent's root cap reaches a grandchild cap inherited through `sys_spawn` derived-copy inheritance.

## Verification

All commands were run through the repo standard container path.

- `docker compose run --rm neo cargo fmt`
  - PASS. Repo-wide formatter churn outside task files was restored.

- `docker compose run --rm neo cargo k`
  - PASS, exit 0.
  - Warning volume remained pre-existing; no compile errors.

- `docker compose run --rm -e NEO_SMP=4 neo cargo neo-test sys_wait_blocking sys_wait_wnohang sys_wait_already_exited sys_wait_multiple_waiters sys_inspect_running sys_inspect_exited sys_inspect_rights_gate cap_revoke_spawn_inheritance_descendants --no-meta`
  - PASS, exit 0.
  - Serial PASS markers:
    - `sys_wait_blocking`
    - `sys_wait_wnohang`
    - `sys_wait_already_exited`
    - `sys_wait_multiple_waiters`
    - `sys_inspect_running`
    - `sys_inspect_exited`
    - `sys_inspect_rights_gate`
    - `cap_revoke_spawn_inheritance_descendants`

- `docker compose run --rm -e NEO_SMP=4 neo cargo neo-test cap_mint_and_use cap_derive_subset cap_derive_rights_violation cap_derive_missing_meta cap_grant_basic cap_grant_missing_meta cap_revoke_local cap_revoke_cross_process cap_revoke_descendants_walk cap_revoke_spawn_inheritance_descendants cap_survive_exit --no-meta`
  - PASS, exit 0.
  - Serial PASS markers:
    - `cap_mint_and_use`
    - `cap_derive_subset`
    - `cap_derive_rights_violation`
    - `cap_derive_missing_meta`
    - `cap_grant_basic`
    - `cap_grant_missing_meta`
    - `cap_revoke_local`
    - `cap_revoke_cross_process`
    - `cap_revoke_descendants_walk`
    - `cap_revoke_spawn_inheritance_descendants`
    - `cap_survive_exit`

- Serial-log failure scan over all exercised scenarios:
  - PASS: no `event=FAIL`, `KERNEL PANIC`, `BUG:`, or frame-leak marker found.

- `git diff --check`
  - PASS, exit 0. Git emitted only local CRLF conversion warnings.

## Caveats

- No blockers.
- The exercised container builds still emit the repository's pre-existing warning volume.
- This subphase does not claim full FM-D14 structural retirement, PID recycling, stale handle revocation beyond wait/inspect behavior, or process/channel/capability `sys_close`.
