// SPDX-License-Identifier: MPL-2.0
//! Kernel-wide errno constants. Negative i64 values returned across the
//! syscall ABI to signal errors. Mirrored in libneo where userspace cares.
//!
//! These match the Linux/POSIX values where one exists, both for
//! recognisability and to ease future libc shim work; they are not
//! ABI-stable beyond Phase 1.x and may extend / re-curate when Phase 2
//! capability errors land.
//!
//! The constants are kept small and not type-wrapped (no `Errno(i64)`
//! newtype) because the syscall ABI itself is "negative i64 = error" —
//! adding a wrapper would force every callsite to `.0` it back. Phase 2
//! may revisit if richer error semantics arrive.

#![allow(dead_code)]

/// `-EPERM` — capability operation denied.
pub const EPERM: i64 = -1;
/// `-ENOENT` — no such file/process/module.
pub const ENOENT: i64 = -2;
/// `-ESRCH` — no such process. Returned by `sys_kill` when the resolved
/// `ProcessHandle` target is no longer registered (e.g. double-kill on
/// the same handle). Distinct from EBADF (handle index unallocated /
/// wrong variant) and EINVAL (target exists but is in an unkillable
/// state for Phase 1.5b — Blocked in IPC).
pub const ESRCH: i64 = -3;
/// `-EBADF` — bad handle index, wrong endpoint side, or stale fd-like ref.
pub const EBADF: i64 = -9;
/// `-E2BIG` - caller-provided output capacity is too small.
pub const E2BIG: i64 = -7;
/// `-EIO` — the storage device failed an operation (OT-67 W1: a replace the
/// device could not write or flush).
pub const EIO: i64 = -5;
/// `-EACCES` — requested file rights exceed the opened object's rights.
pub const EACCES: i64 = -13;
/// `-ENOMEM` — kernel could not satisfy a memory request (frame allocator
/// empty, or a per-Pmap cursor would cross `USER_HEAP_LIMIT`).
pub const ENOMEM: i64 = -12;
/// `-EAGAIN` — operation would block; used by `sys_wait(WNOHANG)` while
/// the target process is still running.
pub const EAGAIN: i64 = -11;
/// `-EFAULT` — bad pointer (translate failed, out of user half, or
/// arithmetic overflow that would land in kernel space).
pub const EFAULT: i64 = -14;
/// `-EBUSY` — resource is busy.
pub const EBUSY: i64 = -16;
/// `-EEXIST` — target already exists.
pub const EEXIST: i64 = -17;
/// `-EXDEV` — a rename between two volumes (F0, `SYS_RENAME_AT`).
pub const EXDEV: i64 = -18;
/// `-ENOTDIR` — non-directory inode used where a directory is required.
pub const ENOTDIR: i64 = -20;
/// `-EINVAL` — bounds / arg validation failed (NULL where forbidden,
/// length zero where forbidden, length too large, malformed UTF-8, etc.).
pub const EINVAL: i64 = -22;
/// `-EISDIR` — directory/non-regular inode used where a regular file is
/// required. BUILD-1 maps VFS TypeForbidden here so it is distinct from
/// insufficient file rights.
pub const EISDIR: i64 = -21;
/// `-ENOTEMPTY` — directory is not empty.
pub const ENOTEMPTY: i64 = -39;
/// `-EPIPE` — peer closed (process exit unblocked us, or a future
/// half-close primitive marked the peer endpoint dead).
pub const EPIPE: i64 = -32;
/// `-ETIMEDOUT` — a bounded wait reached its deadline (the atrium
/// present-fence wait; the bound is the I-14 containment property).
pub const ETIMEDOUT: i64 = -110;
/// `-ENOTCONN` - socket has no connected/default peer.
pub const ENOTCONN: i64 = -107;
/// `-ENOSYS` — unrecognised syscall number.
pub const ENOSYS: i64 = -38;
/// `-ESTALE` — stale file capability generation.
pub const ESTALE: i64 = -116;
/// `-EBADMSG` — B1b: a boot-module model's bytes do not match its pinned
/// digest. Nothing was loaded (fail closed); Arthur's "damaged" state.
#[allow(dead_code)] // live under model-boot-module
pub const EBADMSG: i64 = -74;
/// `-EALREADY` — the one-shot operation was already done (a DI-1a pointer
/// record forwarded or dropped twice).
#[allow(dead_code)] // live under atrium-pointer-forward
pub const EALREADY: i64 = -114;
/// `-ECANCELED` — an inference call whose process was torn down: B1c (AAG J3)
/// while it waited for, or held, the inference stack; CP-66 while its graph
/// request was parked in accelerator admission (withdrawn, never admitted). It
/// stops without running (or finishing) the forward and exits at the syscall
/// boundary, so user space does not see it.
pub const ECANCELED: i64 = -125;
