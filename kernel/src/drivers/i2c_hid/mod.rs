// SPDX-License-Identifier: MPL-2.0
//! HID over I2C on Intel Serial-IO (LPSS DesignWare) I2C: the Zenbook S14's
//! built-in touchpad in mouse mode (rung 1,
//! `Plans/DISPATCH-2026-10-23-zenbook-i2c-hid-touchpad.md`).
//!
//! - `quirk_logic`: the static platform values and the machine match (pure).
//! - `dw_logic`: the LPSS private block and the DesignWare master, polled (pure).
//! - `hid_logic`: the HID-over-I2C protocol and the mouse report (pure).
//! - `pci_pm_logic`: BAR0 across the D3hot -> D0 transition (pure).
//! - `touchpad`: the driver (PCI, MMIO, the GSI, the service thread, the pump).

pub mod dw_logic;
pub mod hid_logic;
pub mod pci_pm_logic;
pub mod quirk_logic;
pub mod touchpad;
