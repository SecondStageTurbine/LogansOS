// SPDX-License-Identifier: MIT OR Apache-2.0
//! Source directory: `sys/`
//!
//! <https://github.com/sailfishos-mirror/glibc/tree/master/sys>

#[cfg(target_os = "linux")]
pub(crate) mod statvfs {
    pub use super::super::io::sys::statvfs::*;
}

#[cfg(target_os = "linux")]
pub(crate) mod socket {
    pub use super::super::socket::sys::socket::*;
}
