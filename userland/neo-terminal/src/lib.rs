// SPDX-License-Identifier: MPL-2.0
//! neo-terminal: the terminal window (Lane 1, rung L1a.3).
//!
//! Design: `docs/THINK-2026-10-31-l1a-terminal-window.md` §2-§8. This crate is
//! the part of the app that has no syscalls, so every piece is a host test:
//!
//! - [`session`]: one child's session: keys through the encoder and the line
//!   discipline into the child's stdin (never blocking: a full pipe drops the
//!   key and rings the bell, §3.3, §4), the child's output into the screen,
//!   its mode requests, End process (Ctrl+Shift+C), and its exit.
//! - [`render`]: the screen's cells drawn into the window's pixels with IBM
//!   Plex Mono, only the rows that changed.
//! - [`dump`]: the grid as text, the scenarios' read-back.
//!
//! `neo-worker`'s `terminal-app` role runs the WM client and the pump around
//! it (`userland/neo-worker/src/terminal_app.rs`).

#![cfg_attr(not(test), no_std)]

extern crate alloc;

pub mod dump;
pub mod render;
pub mod session;
