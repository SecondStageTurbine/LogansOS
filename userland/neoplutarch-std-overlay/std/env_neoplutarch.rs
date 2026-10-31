// SPDX-License-Identifier: MPL-2.0
//! OT-61 C2.1b — the `neoplutarch` arm of `std::sys::env`: an empty,
//! immutable environment.
//!
//! Overlaid into `library/std/src/sys/env/neoplutarch.rs`. A LogansOS process
//! has no ambient environment: whatever it may use arrives as capabilities at
//! spawn. So `std::env::vars()` is empty and `var(..)` is `NotPresent`, rather
//! than std's `unsupported` arm, whose `env()` panics (an abort, 134, for any
//! crate that merely lists the environment). Setting a variable stays an error:
//! there is nothing a later spawn would inherit it into.

use crate::ffi::{OsStr, OsString};
use crate::{fmt, io};

/// The (always empty) environment.
pub struct Env(());

impl fmt::Debug for Env {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().finish()
    }
}

impl Iterator for Env {
    type Item = (OsString, OsString);
    fn next(&mut self) -> Option<(OsString, OsString)> {
        None
    }
}

pub fn env() -> Env {
    Env(())
}

pub fn getenv(_: &OsStr) -> Option<OsString> {
    None
}

pub unsafe fn setenv(_: &OsStr, _: &OsStr) -> io::Result<()> {
    Err(io::const_error!(io::ErrorKind::Unsupported, "LogansOS has no environment to set"))
}

pub unsafe fn unsetenv(_: &OsStr) -> io::Result<()> {
    Err(io::const_error!(io::ErrorKind::Unsupported, "LogansOS has no environment to unset"))
}
