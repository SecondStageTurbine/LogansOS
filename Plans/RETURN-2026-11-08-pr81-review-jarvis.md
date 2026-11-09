<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: #81 Servo layout L0–L4 review (Jarvis)

Reviewed the dispatch-pinned head 5ed11f273 with git diff refs/pr/77...5ed11f27.

## Findings

- [ ] FAIL — vendored licences and sources. The VENDORED.md table records crate versions, licences, repositories, upstream commits, and crate hashes. But r-efi 6.0.0 is listed as MIT OR Apache-2.0 OR LGPL-2.1-or-later at third_party/ot61-browser/VENDORED.md:163 and again as a stub at :555. LGPL is outside the allowed list and the requirement explicitly excludes LGPL crates. Concrete offending entry: r-efi 6.0.0. The stub is not compiled for this target, but its presence and licence expression still fail the stated no GPL/LGPL/AGPL crate is present check.

- [ ] FAIL — patch/tree audit, by patch. The libc and stylo patches match their vendor/ → patched/ trees (ignoring generated timestamps and the documented SPDX/checksum exclusions); both patches explain their purpose. For the other nine patches, the corresponding pristine vendor/ directory is absent, so I could not establish that the patch reproduces the patched/ tree. The missing comparison inputs are concrete: for example, third_party/ot61-browser/vendor/servo-layout-0.7.0/ and vendor/parking_lot_core-0.9.12/ do not exist. Their patched/ trees and upstream provenance rows exist, but that is not a reproducible patch/tree match check.
  - libc-0.2.191-neoplutarch.patch — PASS; target C types (patch :8, :20).
  - parking_lot_core-0.9.12-neoplutarch.patch — FAIL; pristine source absent. Purpose documented in VENDORED.md:36-51.
  - servo-allocator-0.7.0-neoplutarch.patch — FAIL; pristine source absent; purpose comment at patch :32 (use neo-std-pal’s SyscallHeap).
  - servo-base-0.7.0-neoplutarch.patch — FAIL; pristine source absent; purpose comment at patch :8 (use std’s monotonic clock).
  - servo-embedder-traits-0.7.0-neoplutarch.patch — FAIL; pristine source absent; purpose comment at patch :8 (target webdriver types).
  - servo-fonts-0.7.0-neoplutarch.patch — FAIL; pristine source absent; purpose comments at patch :9, :32 (bundled-font backend).
  - servo-fonts-traits-0.7.0-neoplutarch.patch — FAIL; pristine source absent; purpose comment at patch :9 (font identifiers name bundled fonts).
  - servo-layout-0.7.0-neoplutarch.patch — FAIL; pristine source absent; purpose comment at patch :19 (option A replaces script/SpiderMonkey with neo-layout-dom).
  - servo-paint-api-0.7.0-neoplutarch.patch — FAIL; pristine source absent; purpose comment at patch :37 (no Surfman GL contexts on this target).
  - servo-url-0.7.0-neoplutarch.patch — FAIL; pristine source absent; purpose comment at patch :8 (no file paths on this target).
  - stylo-0.22.0-neoplutarch.patch — PASS; VENDORED.md:53-66 explains target thread-handle cfgs and keeping build output under OUT_DIR.
  - The patch inventory and upstream provenance are at third_party/ot61-browser/VENDORED.md:561-569; its layout description at :19-20 only identifies parking_lot_core and Stylo as patched crates with retained published copies.

- [x] PASS — no network fetch in the crate build. layout-smoke routes dependencies through local [patch.crates-io] trees in userland/layout-smoke/Cargo.toml; the closure CI check rejects registry-resolved packages at .github/workflows/ot61-closure.yml:427-435. The Python build step is Stylo’s bundled generator: the workflow documents its bundled Python at :417-424, and VENDORED.md:62-66 records the output-only change. No build.rs in the four new first-party crates runs Python or fetches data.

- [x] PASS — L2 oracle is independent and exact. userland/neo-static-page/src/oracle.rs:2-4, 9-27 derives the block boxes from CSS rules by hand; userland/layout-smoke/src/main.rs:203-221 compares each computed border box with exact expected values. The oracle page contains no text, so font metrics do not feed the expected geometry.

- [ ] FAIL — xtask does not require every L0–L4 success needle. The smoke emits checks for each glyph and font metric at userland/layout-smoke/src/main.rs:123-191, plus its L0/L2/L3/L4 results. But tools/xtask/src/main.rs:18874-18905 only requires a selected subset. For example, it never requires the L1 line "[ layout-smoke ] ok(10) 'H' is glyph 63, advance 1578/128 = 12.328125 px" (emitted by the glyph loop). A log missing that success needle while preserving the selected needles would pass xtask’s needle check. The kernel still checks the process exit, but this does not meet the dispatch requirement that the harness fail on any missing serial needle.

- [x] PASS — neo-crt change is additive. userland/neo-crt/src/lib.rs:218-243 adds only the four byte-order conversion exports (htonl, htons, ntohl, ntohs); it changes no existing signatures or behavior. Existing consumers remain webrender-smoke and cxx-runtime-smoke (userland/webrender-smoke/Cargo.toml:25, userland/cxx-runtime-smoke/Cargo.toml:33), while layout-smoke is the new consumer (userland/layout-smoke/Cargo.toml:24).

- [x] PASS — feature is off by default; the normal image stays on the existing default feature set. kernel/Cargo.toml:81 keeps the existing default list, while neo-test-layout-smoke is a separate opt-in feature at :498. The new scenario is selected only by xtask’s layout_smoke scenario entry (tools/xtask/src/main.rs:2330-2339); the default image does not enable it.

## Review boundary

The dispatch pins 5ed11f273; the fetched refs/pr/81 had advanced to 4000afd0f, so later changes on that ref are outside this review. No build or test was run; findings above are from the pinned source and harness inspection.
