// SPDX-License-Identifier: MPL-2.0
//! Host KATs for F0, the NeoFS directory operations.
//!
//! - `kernel/src/fs/neofs/dir_ops_logic.rs` (the rules: names, depth, pinned
//!   entries, rename into itself, `StatAtV1`) and libneo's independent
//!   `StatAtV1` decoder are mounted through `#[path]` and checked against
//!   hand-laid bytes and hand-worked cases (`logic_tests.rs`).
//! - `model.rs` is a write-back disk and a copy-on-write volume with nested
//!   directories, shaped like NeoFS, whose four operations follow the
//!   kernel's commit (`inode/dir_ops.rs`): the directory chains, the
//!   bottom-up rewrite, one publish, the barrier (`commit_logic.rs`, also
//!   mounted here). `cut_tests.rs` cuts power before every write and flush
//!   of each operation, with every subset of the unflushed writes landing,
//!   and checks the volume mounts at the tree before or after, never
//!   between.

#![allow(dead_code)]

extern crate alloc;

#[path = "../../../kernel/src/fs/neofs/commit_logic.rs"]
pub mod commit_logic;

#[path = "../../../kernel/src/fs/neofs/dir_ops_logic.rs"]
pub mod dir_ops_logic;

#[path = "../../../userland/libneo/src/stat_at.rs"]
pub mod libneo_stat_at;

pub mod model;

#[cfg(test)]
mod cut_tests;
#[cfg(test)]
mod logic_tests;
