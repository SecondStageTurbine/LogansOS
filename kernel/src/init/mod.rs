// SPDX-License-Identifier: MPL-2.0
//! One-shot boot-time code.
//!
//! Per ADR-002 + Bible Article 17 (amended). The `init/` module is for
//! code that runs once during bring-up and never again — boot frame
//! rendering, kernel banner, future hardware probes.
//!
//! What does NOT live here: long-lived subsystems (those have their own
//! top-level dirs), per-process initialization (that lives in `sched/`),
//! per-syscall initialization (none exists; syscalls are stateless from
//! the kernel's perspective).

#[cfg(not(feature = "boot-eth0"))]
pub mod boot_frame;
#[cfg(feature = "boot-eth0")]
#[path = "boot_frame_eth0.rs"]
pub mod boot_frame;
mod boot_logo;
