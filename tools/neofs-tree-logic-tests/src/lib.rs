// SPDX-License-Identifier: MPL-2.0
//! Host KATs for `kernel/src/fs/neofs/tree_logic.rs` (NFS-W1): a file's
//! pointer blocks against a hand-laid byte oracle, and every logical block
//! resolved by an independent walk that mirrors the kernel's reader. And for
//! `alloc_logic.rs`: the allocator, run on a model volume. The real modules
//! are mounted through `#[path]`.

#![allow(dead_code)]

#[path = "../../../kernel/src/fs/neofs/alloc_logic.rs"]
mod alloc_logic;
#[cfg(test)]
mod alloc_tests;
#[cfg(test)]
mod tests;
#[path = "../../../kernel/src/fs/neofs/tree_logic.rs"]
mod tree_logic;
