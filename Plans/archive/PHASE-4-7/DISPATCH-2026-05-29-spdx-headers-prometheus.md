# DISPATCH: SPDX-License-Identifier header rollout (ADR-026)

**Date:** 2026-05-29
**From:** PM (Cerebro)
**To:** Prometheus
**Branch:** `prometheus/spdx-headers` (from origin/main HEAD after the license commit lands)
**Type:** Mechanical documentation/cleanup pass (Prometheus lane). No logic changes.
**Authority:** ADR-026 (ratified 2026-05-29).

---

## Goal

Add SPDX-License-Identifier headers to every first-party file per ADR-026:
- Source code -> `SPDX-License-Identifier: MPL-2.0`
- Documentation -> `SPDX-License-Identifier: CC-BY-4.0`

This makes per-file licensing machine-checkable (REUSE-style) and is the execution step of ADR-026.

## Read first
- `docs/ADR-026-license-mpl-2.0.md` (the decision + scope)
- `./LICENSE` (MPL-2.0), `./NOTICE` (licensing summary)

## Scope: CODE -> MPL-2.0 (per-file header, REQUIRED)

Apply to all first-party source under: `kernel/`, `userland/`, `tools/`, `tests/`.

Header by file type, as the FIRST line(s) of the file:
- `.rs`            -> `// SPDX-License-Identifier: MPL-2.0`
- `.toml`          -> `# SPDX-License-Identifier: MPL-2.0`
- shell/`.sh`/`.ps1` build scripts -> `# SPDX-License-Identifier: MPL-2.0`
- assembly `.s`/`.asm` -> use the file's comment syntax (`;` or `//` as appropriate)

Rules:
- For `.rs` files, the SPDX line goes ABOVE any existing `//!` module doc-comment and above `#![...]` inner attributes is NOT allowed — place the SPDX `//` line as the very first line; module attributes/doc-comments follow. (A leading `//` line comment before `#![...]` is valid Rust.)
- Idempotent: if a file already contains an `SPDX-License-Identifier` line, SKIP it (do not duplicate).
- Preserve all existing content, shebangs, and encoding (UTF-8 no BOM). If a script starts with `#!`, put the SPDX line on the SECOND line (immediately after the shebang).

## Scope: DOCUMENTATION -> CC-BY-4.0 (targeted, not blanket)

Do NOT churn hundreds of Plans/ files. Apply a doc SPDX header ONLY to the primary, durable governance documents:
- `docs/` — the Bible, ANSPEC specs, and every `ADR-*.md` and `PHASE-*.md`
Header: a top-of-file HTML comment `<!-- SPDX-License-Identifier: CC-BY-4.0 -->`
- For `Plans/` working artifacts (dispatch/return/disposition/ratification): do NOT add per-file headers; they are covered by `NOTICE` directory-level declaration. (Avoids massive churn on transient artifacts.)
- Idempotent: skip any doc already carrying an SPDX line.

## Exclusions (DO NOT TOUCH)
- `.cargo-home/` and any dependency/registry cache (third-party; retains upstream license).
- `Cargo.lock` (generated).
- `boot/` vendored Limine artifacts or any other vendored third-party blobs — leave their licensing untouched; if unsure whether a file is first-party, SKIP and list it in the RETURN for PM review.
- Binary files (fonts, images, firmware blobs).

## Verification
- Report counts: # of .rs, .toml, script, and doc files modified; # skipped-already-had-SPDX; # skipped-uncertain (list those).
- Build still passes: `cargo check` (headers are comments; must not change build). Run host `cargo check` for kernel + userland.
- Spot-check 3 files of each type in the RETURN (paste the first 3 lines).
- Confirm no file content changed other than the inserted header line (diff stat: every changed file should be +1 line, except shebang/doc-comment edge cases which you note explicitly).

## Deliverable
`Plans/RETURN-2026-05-29-spdx-headers-prometheus.md` with the counts, the uncertain-file list, the cargo check result, and commit hashes. No logic changes; headers only.