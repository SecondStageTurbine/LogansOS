// SPDX-License-Identifier: MPL-2.0
//! files-core: the Files app's core (OT-88 rung F1a).
//!
//! - [`fs`]: the directory trait, shaped like F0's calls (#106), and F0's
//!   error set.
//! - [`name`]: the name check, the listing's sort order, numbered names.
//! - [`files`]: the listing, navigation and the operations (new folder,
//!   rename, delete, small copy) as a state machine, with the stale-folder
//!   fallback Muse's F0 observation calls for.
//! - [`open_with`]: the MVP's fixed "open with" mapping.
//! - [`view`]: the render model, as plain data in the session's language.
//!
//! Pure `no_std` + `alloc`, no syscalls and no window manager: every part is
//! a host KAT against an in-memory volume. **Linked into no image**: F1b's
//! `neo-files` module wires it to libneo and the WM, and that module's
//! feature is the kill-switch. Design: `docs/THINK-2026-10-29-ot88-app-model.md`
//! §4.2-4.3; `Plans/DISPATCH-2026-11-12-seat-c-f1a-files-core.md`.

#![cfg_attr(not(test), no_std)]

extern crate alloc;

pub mod files;
pub mod fs;
pub mod name;
pub mod open_with;
pub mod view;

#[cfg(test)]
mod fake;
#[cfg(test)]
mod tests;

pub use files::{Effect, Files, Input, Mode, Note, Row};
pub use fs::{DirEntry, Fs, FsError, Kind, Rights, Stat};
pub use open_with::{OpenRequest, OpenWith};
pub use view::{Dialog, RowView, View, view};
