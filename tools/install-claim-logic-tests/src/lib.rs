// SPDX-License-Identifier: MPL-2.0
//! Host KATs for `kernel/src/drivers/block/claim_logic.rs` (OT-87 R4.1, THINK
//! §2a): the identity parsers against hand-laid bytes, and the §2a test table
//! against a model registry and a model device whose medium can be swapped,
//! killed, or bumped mid-request; and `nvme_event_logic.rs` (namespace
//! notices, resets) on a model controller.

#![allow(dead_code)]

#[path = "../../../kernel/src/drivers/block/claim_logic.rs"]
mod claim_logic;
#[path = "../../../kernel/src/drivers/block/nvme_event_logic.rs"]
mod nvme_event_logic;

#[cfg(test)]
mod tests;
// Muse's R4.1 review, item 4: a model NVMe controller, driven the way
// `nvme.rs` drives it, under the claim.
#[cfg(test)]
mod nvme_tests;
