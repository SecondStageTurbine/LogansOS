// SPDX-License-Identifier: MPL-2.0
//! ACPI/AML rung 0: grade the stock rust-osdev AML interpreters against a
//! provenance-tracked corpus of real ACPI table sets.
//!
//! Host only. The interpreters run against a recording handler that logs
//! every OpRegion access and answers from a scripted model; nothing touches
//! real hardware. See `docs/RESEARCH-2026-10-22-acpi-aml-corpus-FINDINGS.md`.

pub mod classify;
pub mod corpus;
pub mod fuzz;
pub mod gates;
pub mod grade_acpi6;
pub mod grade_aml016;
pub mod probe;
pub mod ratchet;
pub mod recorder;
pub mod report;
pub mod sha256;
pub mod summary;
