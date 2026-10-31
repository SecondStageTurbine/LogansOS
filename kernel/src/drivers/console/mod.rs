// SPDX-License-Identifier: MPL-2.0
//! Console drivers — kernel-side text input/output.
//!
//! Phase 1.4.1: only serial. Future: PS/2 keyboard for the kernel debug
//! shell, VT100-style console multiplexing once a window system arrives.

pub mod serial;
