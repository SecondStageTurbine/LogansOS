// SPDX-License-Identifier: MPL-2.0
//! Graders for `acpi` 6.1.1's interpreter (`acpi::aml::Interpreter`), the
//! successor to the standalone `aml` crate: once for the stock crates.io
//! copy, once for the vendored copy carrying the OT-86 Fix 1 patches
//! (`third_party/acpi-loganfy`).
//!
//! Driven only through the crate's public API: `Interpreter::new` (with the
//! FADT-derived `FixedRegisters` and the FACS it requires),
//! `Interpreter::load_table` per DSDT/SSDT body, the public `namespace`
//! for the walk, `Interpreter::evaluate` per `_STA`/`_INI`, and
//! `Interpreter::initialize_namespace` (the crate's own init) on a fresh
//! interpreter. Both graders run the same body (`acpi6_body.rs`) with the
//! same handler and judge; only the crate copy differs.

/// The stock crate, unmodified.
pub mod stock {
    use ::acpi_stock as acpi;

    pub const CRATE: &str = "acpi";
    pub const VERSION: &str = "6.1.1";

    include!("acpi6_body.rs");
}

/// The vendored crate with the Fix 1 patches.
pub mod fix1 {
    use ::acpi_fix1 as acpi;

    pub const CRATE: &str = "acpi+fix1";
    pub const VERSION: &str = "6.1.1";

    include!("acpi6_body.rs");
}
