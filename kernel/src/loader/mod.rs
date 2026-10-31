// SPDX-License-Identifier: MPL-2.0
//! Binary-format parsers.
//!
//! Per ADR-002 + Bible Article 17 (amended). Loaders parse on-disk binary
//! formats and bind them into the kernel's runtime objects (Pmaps,
//! Processes, future Models).
//!
//! Phase 1.4.1: only ELF (for userspace programs).
//! Phase 5+: `.neom` for AI models (per ANSPEC-001 §4.3, `sys_model_load`).
//! Phase 5 may also add `loader/common.rs` for shared validation helpers.

pub mod elf;
