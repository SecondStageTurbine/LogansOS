# RETURN: SPDX-License-Identifier header rollout (ADR-026)

**Date:** 2026-05-29
**From:** Prometheus
**To:** PM (Cerebro)
**Branch:** `prometheus/spdx-headers`
**Dispatch:** `Plans/DISPATCH-2026-05-29-spdx-headers-prometheus.md`
**Status:** COMPLETE

---

## Summary

Added SPDX-License-Identifier headers to all first-party files per ADR-026. Zero logic changes — every file is +1 line (the header comment).

## Counts

| Category | License Header | Files Modified |
|----------|---------------|----------------|
| `.rs` source | `// SPDX-License-Identifier: MPL-2.0` | 83 |
| `.toml` config | `# SPDX-License-Identifier: MPL-2.0` | 14 |
| `.ld` linker scripts | `/* SPDX-License-Identifier: MPL-2.0 */` | 4 |
| `.md` documentation | `<!-- SPDX-License-Identifier: CC-BY-4.0 -->` | 38 |
| **TOTAL** | | **139** |

### Skipped — already had SPDX
- 0 files (none pre-existed)

### Skipped — uncertain (listed for PM review)
- None. All files were clearly first-party or clearly excluded.

## Exclusions (correctly left untouched)
- `boot/altimit-neo.iso`, `boot/neoplutarch.iso` — binary blobs
- `boot/limine.conf` — vendored Limine config (not first-party source)
- `Plans/*.ps1` — working artifacts, covered by NOTICE directory-level declaration per dispatch instructions
- `Plans/` dispatch/return/disposition/ratification artifacts — excluded per dispatch ("do NOT churn hundreds of Plans/ files")
- No `.cargo-home/`, `Cargo.lock`, or third-party cache files touched

## Verification

### Diff stat
```
139 files changed, 139 insertions(+)
```
Every changed file is exactly +1 line. No content modifications beyond the inserted header.

### Build verification
- `cargo check --package neoplutarch-kernel` — **PASS** (126 pre-existing warnings, 0 errors)
- `cargo check --package neo-init --package neo-worker --package libneo` — **PASS** (0 warnings, 0 errors)
- `cargo check --package xtask --package neo-extract-log --package neofs-mkfs` — **PASS** (0 warnings, 0 errors)

### Spot-check (first 3 lines of representative files)

**Rust source (`kernel/src/main.rs`):**
```
// SPDX-License-Identifier: MPL-2.0
//! NeoPlutarch Kernel — entry point.
//!
```

**Rust library (`userland/libneo/src/lib.rs`):**
```
// SPDX-License-Identifier: MPL-2.0
//! NeoPlutarch userland library.
//!
```

**Tool source (`tools/xtask/src/main.rs`):**
```
// SPDX-License-Identifier: MPL-2.0
//! Build orchestration for NeoPlutarch.
//!
```

**TOML config (`kernel/Cargo.toml`):**
```
# SPDX-License-Identifier: MPL-2.0
[package]
name = "neoplutarch-kernel"
```

**Linker script (`kernel/linker.ld`):**
```
/* SPDX-License-Identifier: MPL-2.0 */
/* NeoPlutarch Kernel Linker Script
 *
```

**Bible (`docs/bible.md`):**
```
<!-- SPDX-License-Identifier: CC-BY-4.0 -->
The NeoPlutarch Bible
A Foundational Charter for an AI-Native Operating System
```

**ADR (`docs/ADR-026-license-mpl-2.0.md`):**
```
<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# ADR-026: Project license — MPL-2.0 for code, CC-BY-4.0 for documentation

```

**Phase doc (`docs/phases/PHASE-3.md`):**
```
<!-- SPDX-License-Identifier: CC-BY-4.0 -->
```

**Spec (`docs/specs/neofs-format.md`):**
```
<!-- SPDX-License-Identifier: CC-BY-4.0 -->
```

## Line endings
All files preserved original LF line endings. No CRLF introduced.

## Commit hashes
(Pending — branch `prometheus/spdx-headers` not yet pushed. PM to merge after review.)

## Notes
- The ADR-026 file itself initially triggered a false-positive skip (its body text references `SPDX-License-Identifier` as a term). This was corrected manually; the header is now present and verified.
- No files were touched outside the scope. `boot/`, `Plans/` working artifacts, and all third-party content remain untouched.
