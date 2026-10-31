// SPDX-License-Identifier: MPL-2.0
//! Device drivers, organized by class (Bible Article 9.1).
//!
//! Each driver class lives in its own submodule. As of Phase 1.4.1:
//!
//!   - `console` — serial console (`16550 UART`); future: PS/2 keyboard,
//!     VT100-style multiplexer
//!   - `fb`      — Limine-provided linear framebuffer + 8x16 bitmap font
//!
//! Future phases add `accel/` (Phase 6, AI accelerators per ANSPEC-001 §9),
//! `net/` (Phase 4).

pub mod accel;
pub mod block;
pub mod console;
/// Phase 7.5 BUILD-0 (ADR-045): the fenced DMA arena — the I-14(c1) guard.
pub mod dma_arena;
pub mod fb;
/// OT-63 GPU P1 Slice 5 (ADR-077 §Slice-5 + amendment 5): the named GRAPHICS
/// DMA arena — the contiguous region the graphics VT-d domain maps (all
/// GPU-reachable allocations, and nothing else = I-14(c2) for the iGPU).
/// Always compiled; its narrowing kill-switch defaults OFF (≡ Slice-4).
pub mod graphics_arena;
/// The Zenbook S14 touchpad: HID over I2C on Intel Serial-IO I2C, mouse mode
/// (rung 1). Compiled out by default (the kill-switch); `i2c-hid-touchpad`
/// turns it on.
#[cfg(feature = "i2c-hid-touchpad")]
pub mod i2c_hid;
/// Phase 7.5 BUILD-V: Intel VT-d / IOMMU — the I-14(c2) hardware DMA fence.
/// Compiled out by default (the kill-switch); `vtd` turns it on.
#[cfg(feature = "vtd")]
pub mod iommu;
pub mod net;
pub mod pci;
/// The i8042 (PS/2) keyboard. Compiled out by default (the kill-switch);
/// `ps2-keyboard` turns it on.
#[cfg(feature = "ps2-keyboard")]
pub mod ps2;
/// OT-90 P3b-2: K6, the shutdown's device quiesce.
#[cfg(feature = "power-quiesce")]
pub mod quiesce;
/// Phase 7.5 BUILD-A (ADR-046): the USB host stack. Compiled out by default
/// (the kill-switch); `usb-host` turns it on.
#[cfg(feature = "usb-host")]
pub mod usb;
