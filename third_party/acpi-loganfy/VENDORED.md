<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# `acpi-loganfy`: vendored rust-osdev `acpi` 6.1.1

This is the crates.io release `acpi` 6.1.1 (rust-osdev/acpi, by Isaac Woods and contributors), vendored for
OT-86 (Loganfy) so the patches LogansOS carries are a reviewable diff against the stock crate.

- **Licence:** upstream's dual MIT / Apache-2.0, unchanged: `LICENCE-MIT`, `LICENCE-APACHE`. The patches in this
  directory are contributed under the same terms.
- **Upstream manifest:** `Cargo.toml.upstream` is the tarball's `Cargo.toml.orig`, unchanged. `Cargo.toml` here
  differs only in the package name (`acpi-loganfy`, so the grading harness can link it beside the stock
  crate), `publish = false`, and dropping upstream's tool workspace. The library is still named `acpi`.
- **Formatting:** upstream's `rustfmt.toml`, plus an SPDX comment line. The directory is excluded from the
  LogansOS workspace, so first-party fmt/clippy settings do not apply to it.
- **Consumers:** only `tools/acpi-aml-grade` (host-only). The kernel does not depend on it.

## Checking the base

The base is the crates.io tarball, which Cargo downloads for the stock grade (`acpi_stock = "=6.1.1"`). Diff the
vendored `src/` against that registry copy:

```
diff -ru ~/.cargo/registry/src/*/acpi-6.1.1/src third_party/acpi-loganfy/src
```

The only differences are the patches below and the SPDX line. (Fix 1 was squash-merged as #102, so no commit on
`main` holds the unpatched copy; the registry tarball is the reference.)

Each `.rs` file gained one first line, `// SPDX-License-Identifier: MIT OR Apache-2.0` (upstream's licence, for
LogansOS's SPDX close-gate).

## The patches

Each change carries a `LOGANFY(Fix 1, <item>)` or `LOGANFY(Fix 2)` comment naming its item and, where one applies,
the ACPI section it follows.

- **Fix 1 (L3):** the value-kind fixes, the PCI_Config base, the context unwind. Items, KATs and numbers:
  `docs/RESEARCH-2026-10-23-acpi-aml-fix1-FINDINGS.md`.
- **Fix 2:** every AML-reachable panic in `src/aml` becomes an error. `PANIC-CENSUS.md` lists every explicit
  panic site before and after; the findings are in `docs/RESEARCH-2026-10-23-acpi-aml-fix2-FINDINGS.md`.
