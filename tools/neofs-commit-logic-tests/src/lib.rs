// SPDX-License-Identifier: MPL-2.0
//! Host KATs for `kernel/src/fs/neofs/commit_logic.rs`, the NeoFS commit
//! order on a disk with a volatile write cache. The real module is mounted
//! through `#[path]`. `model.rs` is a write-back disk and a small
//! copy-on-write volume shaped like NeoFS; `tests.rs` cuts power at every
//! write and flush of each commit, with every subset of the unflushed writes
//! landing, and checks that a generation always mounts.

#![allow(dead_code)]

extern crate alloc;

#[path = "../../../kernel/src/fs/neofs/commit_logic.rs"]
pub mod commit_logic;

pub mod model;

#[cfg(test)]
mod tests;
