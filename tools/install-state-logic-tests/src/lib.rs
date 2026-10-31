// SPDX-License-Identifier: MPL-2.0
//! Host KATs for `kernel/src/drivers/block/install_logic.rs` (OT-87 R4.3):
//! the install state machine against a write-back-cache disk model that cuts
//! power at every submission and every flush (THINK §4.7). The real modules
//! are mounted through `#[path]`, side by side as in the kernel.

#![allow(dead_code)]

#[path = "../../../kernel/src/drivers/block/gpt_logic.rs"]
mod gpt_logic;
#[path = "../../../kernel/src/drivers/block/install_logic.rs"]
mod install_logic;
#[cfg(test)]
mod model;
#[cfg(test)]
mod tests;
