// SPDX-License-Identifier: MPL-2.0
//! neo-term-core: the terminal window's core (Lane 1, rung L1a.2).
//!
//! Design: `docs/THINK-2026-10-31-l1a-terminal-window.md` (merged as #12).
//! Every part is pure `no_std` + `alloc` and a host KAT against that THINK's
//! hand-laid bytes:
//!
//! - [`page`]: the 4 KiB term page shared with the child (§3.2): its layout,
//!   the seqlock'd size and the mode request/ack, with the THINK's orderings.
//! - [`keys`]: HID usage + modifiers → the bytes xterm sends (§5.3).
//! - [`discipline`]: cooked and raw mode (§4).
//! - [`answer`]: the three answerbacks the terminal gives, and nothing else
//!   (§5.4).
//! - [`paste`]: the sanitizer and bracketing for the later clipboard rung
//!   (§6.1).
//!
//! The VT parser and screen (vendored `vt100`) join in a later commit.
//! Userland only: the kernel never links it.

#![cfg_attr(not(test), no_std)]

extern crate alloc;

pub mod answer;
pub mod discipline;
pub mod keys;
pub mod page;
pub mod paste;
