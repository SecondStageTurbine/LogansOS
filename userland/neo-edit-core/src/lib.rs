// SPDX-License-Identifier: MPL-2.0
//! neo-edit-core: the text editor's core (OT-67 rung E1).
//!
//! A gap buffer of UTF-8 ([`gap`]), the line index ([`lines`]), undo and
//! redo ([`history`]), grapheme-safe cursor moves ([`cursor`]), the file
//! format and text test ([`format`]), and soft wrap over a measuring trait
//! ([`wrap`]), tied together by [`Document`].
//!
//! Pure `no_std` + `alloc`, no syscalls and no drawing, so every part is a
//! host KAT. The editor app (rung E2) draws with neo-text. Userland only:
//! the kernel never links it. Design: docs/THINK-2026-10-29-ot67-editor.md.

#![cfg_attr(not(test), no_std)]

extern crate alloc;

pub mod cursor;
pub mod document;
pub mod format;
pub mod gap;
pub mod history;
pub mod lines;
pub mod wrap;

#[cfg(test)]
mod tests;

pub use cursor::Selection;
pub use document::{Document, LineChange, Motion};
pub use format::{Format, LineEnding, MAX_FILE_BYTES, NotText};
pub use history::{HISTORY_BYTES, History};
pub use wrap::{Measure, WrapCache};
