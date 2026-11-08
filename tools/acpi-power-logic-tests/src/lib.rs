// SPDX-License-Identifier: MPL-2.0
//! Host KATs for `kernel/src/acpi/power_logic.rs` (OT-90 P1): the FADT power
//! fields, the static `\_S5` decode, the PM1 and sleep-control values, and the
//! restart chain. The real module is mounted through `#[path]`, in a `no_std`
//! crate, so it is checked to build the way the kernel builds it.
//!
//! The oracles are independent of the module, and the corpus ones are
//! generated, never typed in:
//! - `fixtures/prototype-walk.txt` is the output of the THINK's Python
//!   prototype (`python3 -I docs/THINK-2026-10-27-ot90-s5-walk.py
//!   fixtures/acpi`, revision 5, identical to the THINK's Appendix A): each
//!   table's walk result and each set's decode;
//! - `fixtures/fadt-oracle.txt` is the FADT power fields of each set as ACPICA's
//!   disassembler reads them (`gen-fadt-oracle.py`, `iasl -d`, ACPICA
//!   `e85aa3be`), not our parser;
//! - the negatives in `fixtures/` were compiled with `iasl -f` from the `.asl`
//!   beside them; the probes iasl refuses, and one hand-laid FADT, are bytes
//!   written out in the tests.

#![no_std]

extern crate alloc;
#[cfg(test)]
extern crate std;

#[path = "../../../kernel/src/acpi/power_logic.rs"]
pub mod power_logic;

/// OT-90 P4: the `SYS_POWER` ABI (`power_abi_tests`).
#[path = "../../../kernel/src/acpi/power_abi.rs"]
pub mod power_abi;

#[cfg(test)]
mod power_abi_tests;

#[cfg(test)]
mod tests;
