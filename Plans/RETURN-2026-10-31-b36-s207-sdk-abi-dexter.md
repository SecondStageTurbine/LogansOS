<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — #259 SDK-ABI doc errors (Section 207)

**Seat:** Dexter (DeepSeek family) · **Dispatch:** `Plans/DISPATCH-2026-10-31-b36-bounces.md`, Section 207
**Branch:** `dexter/sdk-abi-refresh` · **Base:** `origin/main` = `924c189c` (rebased; no `site/` path in the diff)
**Old head:** `fc663850` → **rebased head before this RETURN:** `30771307` → **head:** this commit
**Scope:** docs only (`docs/SDK-ABI.md`, this RETURN). No kernel, userland, tools or `site/` change; no QEMU gate is
required. `git diff --stat origin/main...HEAD` shows no `site/` path.

## The three named FAILs

| # | Item | Fix |
|---|------|-----|
| 1 | `SYS_THREAD_JOIN` return convention | Row 72 now says **0** on success, with the exit code written through `exit_out` (a null pointer discards it), matching `syscall.rs:7595-7621` (`sys_thread_join_impl` returns 0 after `write_volatile(exit_out, code)`). Errors are `EFAULT` (bad `exit_out`), `EINVAL` (self/detached/already-joined/bad tag) and `ESRCH` (`process.rs:4873-4971`). |
| 2 | `SYS_GETRANDOM` error list | Row 400 now lists `EINVAL` (`len > MAX_GETRANDOM_LEN`), `EFAULT` (null/out-of-user-half/overflow), `ENOMEM` (DRBG fill, `rng::fill_random`), and `ENOSYS` (feature off), from `syscall.rs:5511-5548`. |
| 3 | `SYS_FUTEX_WAIT` missing `EFAULT` | Row 68 now lists `EINVAL` (misaligned `uaddr`) and `EFAULT` (a `uaddr` that does not translate), from `syscall.rs:7251-7273` (`validate_user_buf` + `pmap.translate` → `EFAULT`). `EAGAIN`/`ETIMEDOUT` stay in the Returns cell. |

## Item 4 — the other rows

I read every non-test handler body (and the helpers it delegates to: `process::{thread_join,thread_detach,
thread_get_info,thread_set_name}_for_current`, `derive_handle`, `revoke_handle`, `wait_for_exit`, `close_handle`,
`tensor_errno`, `socket_errno`, `tcp_errno`, `dns_errno`, `dial_errno`, `vfs_errno`) and rebuilt each row's
**Errors** cell from the code. **92 row cells changed** in `docs/SDK-ABI.md`. Beyond the three named FAILs, these
were outright wrong, not just incomplete:

| Row | Was | Now (kernel evidence) |
|-----|-----|-----------------------|
| 19 `SYS_WAIT` | Returns `0` for a running child with `WNOHANG` | **`EAGAIN`** for a still-running child with `WNOHANG` (`process.rs:3992-3994`); the tagged exit value otherwise. Errors `EINVAL`/`EBADF`/`EPERM`/`ESRCH`/`EAGAIN`. |
| 77 `SYS_THREAD_GET_INFO` | `EPIPE` (gone) | **`ESRCH`** (no such tid) — `process.rs:5005-5022`; plus `EFAULT`. |
| 274 `SYS_MODEL_DROP` | `EAGAIN` (in flight), `ENOSYS` | **`EBADF`** (unallocated/not a model) and `EINVAL` — `syscall.rs:3167-3178` is `close_handle` only; it has no in-flight or feature path. |
| 65 `SYS_VM_UNMAP` | `—` | `EINVAL` (size/not mapped), `EFAULT` (non-canonical `va`) — `syscall.rs:7131-7156`. |
| 66 `SYS_VM_PROTECT` | `EINVAL` (`W\|X`) | `EINVAL` (`W\|X`, size/not mapped) **and `EFAULT`** (non-canonical `va`) — `syscall.rs:7161-7188`. |
| 69 `SYS_FUTEX_WAKE` | `—` | `EINVAL` (misaligned), `EFAULT` (untranslatable `uaddr`) — `syscall.rs:7278-7296`. |
| 79 `SYS_THREAD_SET_TLS` | `—` | `EPERM` (no thread cap), `EFAULT` (`base >= USER_HALF_LIMIT`) — `syscall.rs:7577-7586`. |
| 3 `SYS_CHAN_CREATE` | `—` | `EINVAL`; `ENOMEM` under `obj-quota` — `syscall.rs:2077-2119`. |
| 14 `SYS_CLOSE` | `—` | `EBADF` (unallocated, wrong kind, or stdio fd) — `syscall.rs:6743-6771`. |
| 8/9/10/11 `SYS_DERIVE`/`REVOKE`/`GRANT`/`RECV_GRANT` | `—`/`ENOSYS` only | `EINVAL`, `EBADF`, `EPERM` (and `EPIPE` on grant); derive/receive-grant also `ENOMEM`/`ENOSYS` per the code. |
| 259/258 `SYS_TENSOR_DROP`/`SHARE` | `—` | `EBADF`; share adds `EPERM`/`EINVAL` — `syscall.rs:2393-2426`. |
| 7/20 `SYS_KILL`/`SYS_INSPECT` | `EINVAL` only | `EBADF` (bad/non-Process handle) and `EPERM` (right) too — `syscall.rs:7706-7791`. |
| 21/22/81/83 `SYS_SYSSTAT`/`OBSERVE`/`HANDLE_INFO`/`MEM_INFO` | size error only | `EFAULT` (all), `EBADF` (handle-info) too. |
| 6/1/13 `SYS_MAP_REGION`/`WRITE`/`READ` | `ENOMEM`/`EACCES` only | `EINVAL` (size / `MAX_FILE_IO`) and `EBADF` too. |
| 384/385/388/390 `SYS_SOCKET_CREATE`/`BIND`/`CLOSE`/`SENDTO` | `—` | `EINVAL` (create/bind/sendto), `EBADF` (close), plus the `socket_errno` set (`EBUSY`/`EAGAIN`/`EPERM`). |
| 398 `SYS_NET_TCP_ENABLE_TLS` | `ENOMEM` only | also `EINVAL` (key length), `EFAULT`, `EBADF`, `EPERM` — `syscall.rs:5468-5490`. |
| 399 `SYS_NET_DIAL_CAP_DERIVE` | `EACCES`, `ENOSYS` | also `EINVAL`/`EFAULT`/`EBADF`/`EPERM`/`ENOMEM` — `process.rs:3314-3367`. |
| Atrium rows 416-437 | mostly one code or `—` | completed from each handler: `EINVAL`/`EFAULT`/`EBADF`/`EPERM` as applicable; `SYS_INPUT_SAK_TAKE` gains `EINVAL`/`EBUSY`/`ESRCH` (`syscall.rs:9065-9085`); `SYS_TRY_RECV` gains `EBADF`/`EPERM`/`EINVAL`/`EFAULT`; `SYS_WAIT_ANY` gains `EBADF`/`EPERM`/`EFAULT`. |
| 448/449 `SYS_INSTALL_*` | `EPERM` only | `EINVAL` (bad `start`/null out) and `EFAULT` too. |

The remaining changed rows were incomplete rather than wrong — `SYS_SPAWN`, `SYS_OPEN`, `SYS_MOUNT`, `SYS_UMOUNT`,
`SYS_READDIR`, `SYS_FTRUNCATE`, `SYS_FILE_REPLACE`, `SYS_FSYNC`, `SYS_TENSOR_CREATE`/`QUERY`/`WRITE`,
`SYS_MODEL_LOAD`/`QUERY`, `SYS_INFER_RUN`, the three `SYS_GENERATE*`, the rest of the network rows, and the atrium
input/buffer rows. Each now carries the full handler set; VFS/DNS/TCP-delegated cases stay marked "VFS errors" (etc.)
rather than inventing codes the body can return. No row was removed or renumbered, so the drift guard is unaffected.

The legend now states that the **Errors** cell is the complete handler set (the old "call-specific; the common ones
still apply" wording let these gaps read as intentional), and the common-errno paragraph is scoped to the
pointer/handle-taking calls instead of claiming to apply to every call.

## Evidence

- Drift guard: `cargo test --manifest-path tools/xtask/Cargo.toml sdk_abi_doc_covers_every_kernel_syscall -- --nocapture`
  → **1 passed** (158 filtered out). The edited tables parse and every non-test `SYS_*` row still matches the kernel.
- `git diff --stat origin/main...HEAD` → `docs/SDK-ABI.md` (+ `Plans/RETURN-2026-10-31-b36-s207-sdk-abi-dexter.md`
  with this commit); **no `site/` path**, no `kernel/`/`userland/`/`tools/` change.
- CI: a docs-only push is paths-ignored by the workflow (`docs/**`, `Plans/**`, `**.md`), so no run is expected and
  none is claimed. No QEMU gate: nothing executable changed.
- Encoding: the file stays UTF-8, no BOM, CRLF preserved.

## Notes / repo obstacles

- §1–§3's line numbers were useful; the defect in §1 was the doc's Returns cell only — the kernel comment and
  handler already agreed.
- `SYS_THREAD_JOIN`'s `exit_out` is optional (a null pointer is legal and discards the status); that is now in the
  row rather than only in the kernel comment.
- Nothing in this dispatch required a design change; no other issues found in the SDK-ABI surface.
