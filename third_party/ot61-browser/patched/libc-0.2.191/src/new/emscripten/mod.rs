// SPDX-License-Identifier: MIT OR Apache-2.0
//! Emscripten libc.
//!
//! * Headers: <https://github.com/emscripten-core/emscripten/tree/main/system/lib/libc>

pub(crate) mod pthread;
pub(crate) mod sched;
pub(crate) mod unistd;
