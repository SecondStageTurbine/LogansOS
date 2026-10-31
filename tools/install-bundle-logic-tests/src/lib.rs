// SPDX-License-Identifier: MPL-2.0
//! Host KATs for `kernel/src/install_bundle_logic.rs` (OT-87 R4.2): the
//! `NEOINST1` bundle and its slot maps, against hand-laid bytes. The real
//! module is mounted through `#[path]`.

#![allow(dead_code)]

#[path = "../../../kernel/src/install_bundle_logic.rs"]
mod install_bundle_logic;
#[cfg(test)]
mod tests;
