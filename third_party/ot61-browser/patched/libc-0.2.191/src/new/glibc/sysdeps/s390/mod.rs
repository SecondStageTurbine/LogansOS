// SPDX-License-Identifier: MIT OR Apache-2.0
//! Directory: `sysdeps/s390`

#[cfg(target_os = "linux")]
pub(crate) mod nptl {
    pub(crate) mod bits;
}
