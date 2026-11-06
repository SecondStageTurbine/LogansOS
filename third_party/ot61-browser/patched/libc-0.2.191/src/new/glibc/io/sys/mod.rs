// SPDX-License-Identifier: MIT OR Apache-2.0
//! Source directory: `io/sys/`
//!
//! <https://github.com/sailfishos-mirror/glibc/tree/master/io/sys>

#[cfg(target_os = "linux")]
pub(crate) mod statvfs;
