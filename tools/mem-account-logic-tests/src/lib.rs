// SPDX-License-Identifier: MPL-2.0
//! Host KATs for OT-61 C2.2 (A2, I-18): the memory account (hierarchical
//! charge with rollback, release, the per-owner ledger), one case per path
//! that charges or releases, a cross-thread hammer, and the `SYS_MEM_INFO`
//! byte oracle (kernel encoder and libneo decoder against hand-laid bytes).

extern crate alloc;

#[path = "../../../kernel/src/mm/mem_account_logic.rs"]
pub mod kernel;

#[path = "../../../userland/libneo/src/mem_info.rs"]
pub mod libneo;

#[cfg(test)]
mod tests;
