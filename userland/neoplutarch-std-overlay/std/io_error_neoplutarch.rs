// SPDX-License-Identifier: MPL-2.0
//! OT-61 C0.5 — the `neoplutarch` arm of `std::sys::io::error`: the io-errno
//! table.
//!
//! Overlaid into `library/std/src/sys/io/error/neoplutarch.rs`. LogansOS
//! syscalls return a negative errno (`kernel/src/errno.rs`, mirrored in
//! libneo); std carries the positive value as the `RawOsError` of an
//! `io::Error::from_raw_os_error`. This module maps every one of those values
//! to an `io::ErrorKind` and a readable message. It replaces std's `generic`
//! arm, which decodes everything as `Uncategorized` and describes every error
//! as "operation successful".
//!
//! `ERRNO_TABLE` must stay exhaustive over `kernel/src/errno.rs`. The host KAT
//! `tools/xtask/src/ot61_c05_tests.rs` parses both files and fails if a kernel
//! errno is missing, renumbered, or mapped differently from its independent
//! hand-written expectation.
//!
//! Load-bearing rule: **no entry maps to `ErrorKind::Interrupted`.** std's
//! retry loops (`write_all`, `read_exact`, ...) retry on `Interrupted`;
//! LogansOS has no EINTR, so `is_interrupted` is constant `false` and a failing
//! syscall can never make std spin. Unknown codes decode to `Uncategorized`
//! with their number in the message (deterministic, never silent).

use crate::io::ErrorKind;

/// `(errno, name, kind, message)` for every errno in `kernel/src/errno.rs`.
/// One entry per line; the host KAT parses this format (hence `rustfmt::skip`).
#[rustfmt::skip]
const ERRNO_TABLE: &[(i32, &str, ErrorKind, &str)] = &[
    (1, "EPERM", ErrorKind::PermissionDenied, "operation not permitted"),
    (2, "ENOENT", ErrorKind::NotFound, "no such entry"),
    (3, "ESRCH", ErrorKind::NotFound, "no such process"),
    (5, "EIO", ErrorKind::Uncategorized, "input/output error"),
    (7, "E2BIG", ErrorKind::ArgumentListTooLong, "argument list too long"),
    (9, "EBADF", ErrorKind::InvalidInput, "bad handle"),
    (11, "EAGAIN", ErrorKind::WouldBlock, "resource temporarily unavailable"),
    (12, "ENOMEM", ErrorKind::OutOfMemory, "out of memory"),
    (13, "EACCES", ErrorKind::PermissionDenied, "permission denied"),
    (14, "EFAULT", ErrorKind::InvalidInput, "bad address"),
    (16, "EBUSY", ErrorKind::ResourceBusy, "resource busy"),
    (17, "EEXIST", ErrorKind::AlreadyExists, "already exists"),
    (18, "EXDEV", ErrorKind::CrossesDevices, "cross-device rename"),
    (20, "ENOTDIR", ErrorKind::NotADirectory, "not a directory"),
    (21, "EISDIR", ErrorKind::IsADirectory, "is a directory"),
    (22, "EINVAL", ErrorKind::InvalidInput, "invalid argument"),
    (32, "EPIPE", ErrorKind::BrokenPipe, "broken pipe"),
    (38, "ENOSYS", ErrorKind::Unsupported, "function not implemented"),
    (39, "ENOTEMPTY", ErrorKind::DirectoryNotEmpty, "directory not empty"),
    (74, "EBADMSG", ErrorKind::InvalidData, "bad message (digest mismatch)"),
    (107, "ENOTCONN", ErrorKind::NotConnected, "not connected"),
    (110, "ETIMEDOUT", ErrorKind::TimedOut, "timed out"),
    (114, "EALREADY", ErrorKind::AlreadyExists, "operation already done"),
    (116, "ESTALE", ErrorKind::StaleNetworkFileHandle, "stale handle"),
    (125, "ECANCELED", ErrorKind::Uncategorized, "operation canceled"),
];

fn lookup(code: i32) -> Option<&'static (i32, &'static str, ErrorKind, &'static str)> {
    ERRNO_TABLE.iter().find(|e| e.0 == code)
}

/// No thread-local errno on LogansOS: errors are syscall return values, so the
/// "last OS error" is never set. `io::Error::last_os_error()` is therefore
/// `from_raw_os_error(0)`, which decodes as "no error (errno 0)".
pub fn errno() -> i32 {
    0
}

/// Always `false`: LogansOS has no EINTR, and no table entry maps to
/// `Interrupted` (see the module docs).
pub fn is_interrupted(_code: i32) -> bool {
    false
}

pub fn decode_error_kind(code: i32) -> ErrorKind {
    match lookup(code) {
        Some(e) => e.2,
        None => ErrorKind::Uncategorized,
    }
}

pub fn error_string(code: i32) -> String {
    match lookup(code) {
        Some(e) => format!("{}: {} (LogansOS errno {})", e.1, e.3, code),
        None if code == 0 => "no error (LogansOS errno 0)".to_string(),
        None => format!("unknown LogansOS errno {code}"),
    }
}
