// SPDX-License-Identifier: MIT OR Apache-2.0
//! LogansOS (`target_os = "neoplutarch"`): C type definitions only, as for Xous.
//! No functions are declared here, so this arm adds no syscall surface (the
//! A4 manifest is unchanged). The types match neo-crt's musl ABI on x86_64.

use crate::prelude::*;

pub type intmax_t = i64;
pub type uintmax_t = u64;

pub type size_t = usize;
pub type ptrdiff_t = isize;
pub type intptr_t = isize;
pub type uintptr_t = usize;
pub type ssize_t = isize;

pub type off_t = i64;
pub type wchar_t = i32; // musl x86_64: `int` (arch/x86_64/bits/alltypes.h.in)

#[deprecated(since = "0.2.190", note = "Use `c_int::MIN` instead.")]
pub const INT_MIN: c_int = c_int::MIN;
#[deprecated(since = "0.2.190", note = "Use `c_int::MAX` instead.")]
pub const INT_MAX: c_int = c_int::MAX;
