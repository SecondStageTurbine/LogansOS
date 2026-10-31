// SPDX-License-Identifier: MPL-2.0
//! Host KATs for the Zenbook touchpad lane's pure logic (rung 1):
//! `kernel/src/drivers/i2c_hid/{quirk_logic,dw_logic,hid_logic,pci_pm_logic}.rs`.
//!
//! The modules are mounted through `#[path]`, so these tests run the exact
//! code the kernel runs. Every expected byte and value is laid out by hand
//! in the comment beside it, from the Microsoft HID-over-I2C spec v1.0, HID
//! 1.11, ACPI 6.5 and the DesignWare register map, never computed from the
//! code under test. The quirk tests also read the rung-0 ACPI corpus
//! (`fixtures/acpi/*/dsdt.dat`) when it is in the tree.

#![allow(dead_code)]

#[path = "../../../kernel/src/drivers/i2c_hid/dw_logic.rs"]
pub mod dw_logic;
#[path = "../../../kernel/src/drivers/i2c_hid/hid_logic.rs"]
pub mod hid_logic;
#[path = "../../../kernel/src/drivers/i2c_hid/pci_pm_logic.rs"]
pub mod pci_pm_logic;
#[path = "../../../kernel/src/drivers/i2c_hid/quirk_logic.rs"]
pub mod quirk_logic;

#[cfg(test)]
mod model;

#[cfg(test)]
mod hid_tests;

#[cfg(test)]
mod dw_tests;

#[cfg(test)]
mod quirk_tests;

#[cfg(test)]
mod pci_pm_tests;
