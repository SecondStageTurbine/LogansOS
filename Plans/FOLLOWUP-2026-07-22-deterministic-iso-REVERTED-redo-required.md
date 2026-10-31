<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# OWED VERIFICATION: deterministic-ISO two-rebuild SHA proof

**Date:** 2026-07-22
**From:** PM (Claude)
**Status:** **REVERTED (`4565a4a`, 2026-07-23) — the change was BROKEN, not merely unproven.** `89a1fc6` added a `-date` xorriso option that **the CI's xorriso version rejects**, so the ISO build failed *before QEMU in every shard* of the next 4-shard gate (Chad's, run `29970581363`) — blocking every kernel-gated merge. Reverted revert-first per the escaped-defect contract (fix-forward was not viable: the owner has no xorriso to verify a fix). Superseded — see the re-do requirements below.

## Root cause + why it escaped
- `-date` (and possibly `--modification-date`) is **not valid xorriso syntax** in the CI image — likely mkisofs/genisoimage-ism. The correct xorriso option for pinning the volume timestamp is **`-volume_date`** (e.g. `-volume_date uuid <YYYYMMDDhhmmsscc>` or `-volume_date c/m ...`). The change was never run against a real xorriso, so this was invisible until CI built the ISO.
- **It escaped because push-CI does not build the ISO** — the ISO is only assembled during the 4-shard QEMU gate (and real ISO builds). So a broken xorriso arg passes push-CI green and only reddens at the next gated build. (The PM compounded it by merging with the proof "owed" despite having no xorriso — the proof would have caught this at once. Lesson: **do not merge a tooling change you cannot run onto the path that runs it; get the tool-host verification FIRST.**)

## Re-do requirements (Prometheus, on an xorriso host — do NOT merge without these)
1. Use the **correct xorriso option** (`-volume_date`, verified against the CI image's `xorriso` `--version`), not `-date`.
2. **Build the ISO locally with xorriso** (the change must actually assemble an ISO — CI green on push is NOT sufficient; push-CI never builds the ISO).
3. Run the **two-rebuild-identical-SHA proof** (below) and paste both SHAs — this simultaneously proves the option works AND that volume-timestamp was the sole variance.
4. Because the fix touches the ISO-assembly path used by the QEMU gate, the branch must pass a **4-shard `workflow_dispatch` gate** (which builds the ISO) — not just push-CI.

## Promotion (escaped-defect contract §4)
The escape class = "an xorriso/ISO-assembly change passes push-CI but breaks the QEMU-gate ISO build." Promote a cheap guard so this can't escape twice: either a **push-CI ISO-assembly smoke** (invoke the xorriso command with a tiny fixture so a bad option fails on push, not only at the gate), or a standing rule that **any `tools/xtask` ISO-assembly change requires a 4-shard `workflow_dispatch` gate before merge** (since push-CI doesn't cover it). PM to pick + record.

## What landed
`tools/xtask` ISO assembly now pins the xorriso volume timestamps: `SOURCE_DATE_EPOCH` → `format_iso_date()` (ECMA-119), else a fixed `198001010000.00` (1980-01-01); both `-date` and `--modification-date` set to it. Targets the diagnosed non-determinism source (xorriso volume-timestamp metadata).

## What is VERIFIED (PM, at merge)
- `format_iso_date()` is Howard Hinnant's canonical `civil_from_days` — hand-checked: `epoch=0 → "197001010000.00"` (1970-01-01 00:00:00). Correct + deterministic.
- The unset-`SOURCE_DATE_EPOCH` fallback is a fixed constant — deterministic by construction.
- Host-only, metadata-only (payload untouched) → **zero boot/behavior risk**; CI green (fmt/clippy/test/SPDX/identity).

## What is NOT yet proven (the OWED item)
The **empirical two-rebuild-identical-SHA proof** — the dispatch's actual deliverable — was **not run** (no xorriso on the submitter's host or the PM container). It is the *only* thing that confirms the diagnosis was **complete** (i.e. volume timestamps were the *sole* variance, not one of several). Pinning the timestamp is necessary; the two-rebuild proves it is *sufficient*.

## The proof (run at the next ISO build, on any xorriso host — PL build machine or a worker tree)
```
cargo xtask iso && sha256sum boot/neoplutarch.iso   # rebuild 1
cargo xtask iso && sha256sum boot/neoplutarch.iso   # rebuild 2
# PASS iff the two SHA-256s are IDENTICAL.
```
- **PASS** → determinism confirmed; update this note to CLOSED and record "tested-bytes == shipped-bytes" as proven.
- **FAIL (SHAs differ)** → the diagnosis was incomplete (another variance source, e.g. file mtimes, ordering, or a build-embedded timestamp). That is a **follow-up fix**, NOT a revert — the timestamp pinning is still correct and needed; the new source gets pinned too, then re-prove.

The OptiPlex JOINT sessions build/stage ISOs — fold this two-line proof into that run (or any worker-tree ISO build) so it rides an ISO build that happens anyway.
