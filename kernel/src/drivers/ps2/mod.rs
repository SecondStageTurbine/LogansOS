// SPDX-License-Identifier: MPL-2.0
//! PS/2 input through the legacy i8042 controller: the keyboard on port 1.
//! The AUX (mouse) port is disabled and never used.

pub mod i8042;

#[path = "i8042_logic.rs"]
pub mod i8042_logic;
