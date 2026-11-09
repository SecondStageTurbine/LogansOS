<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: #76 File Viewer survey review (Janus)

**Reviewed head:** `6be22efa59c84ff163a8fc14efb23dbcdb2770de`  
**Review branch:** `codex/pr76-review-janus`  
**Dispatch:** Section 321, 2026-11-08

- [x] **PASS — Ten dependency rows spot-checked against raw package tarballs and upstream commit records.** I fetched the exact npm versions with `curl` from the registry tarballs and read each `package/package.json` plus available raw licence files. I checked the cited upstream commit dates from the commit records. All ten versions, licence fields and dates agree with the appendix:

  | Package | Raw version and licence | Raw licence evidence | Cited upstream commit; date |
  |---|---|---|---|
  | `@flyfish-dev/cad-viewer` | `0.8.3`, `AGPL-3.0-only` | `LICENSE`: GNU AGPL, version 3 | `0ce90d08bbf96aa780676ffeb81e0da12c575097`; 2026-10-11 |
  | `dwf-viewer` | `0.6.7`, `AGPL-3.0-only` | `LICENSE`: GNU AGPL, version 3 | `72783571e2cd744862d8eb32f7d3455906c76226`; 2026-09-08 |
  | `@mlightcad/libredwg-web` | `0.7.9`, `GPL-3.0` | No tarball licence file, as disclosed; package field agrees | `82bf45777635942e5eb9b68c15a2ee75bbaf64ab`; 2026-11-08 |
  | `bpmn-js` | `18.31.1`, `SEE LICENSE IN LICENSE` | `LICENSE` confirms the MIT text and watermark condition | `e7b7435ed60f662b3d80caaefe5dccba5d3a4c75`; 2026-10-21 |
  | `heic2any` | `0.0.4`, `MIT` | `LICENSE.md`: MIT | `3428539e643e112323a5b8a2c77c6402cb1372f6`; 2023-03-29 |
  | `occt-import-js` | `0.0.23`, `LGPL-2.1` | `LICENSE.md`: LGPL version 2.1 | `41e470890ae0f9dc69ac50ffd5fc73e03576f4eb`; 2024-12-03 |
  | `sql.js` | `1.15.2`, `MIT` | `LICENSE`: MIT | `4f1c22d0cf7d91b78e5feedab62212e8127751f4`; 2026-10-26 |
  | `pdfjs-dist` | `5.4.624`, `Apache-2.0` | `LICENSE` is present in the package | `a4800c8193ac77ce19a3dbdaf6e92ce677534b7f`; 2026-11-07 |
  | `mermaid` | `12.2.0`, `MIT` | `LICENSE`: MIT | `a0fd6b8cee272d47d39c0b6bfdf6d2283efa9a49`; 2026-11-06 |
  | `epubjs` | `0.3.93`, `BSD-2-Clause` | `license`: BSD-2-style text | `eee359d0790002115a1156a9833c54f4bcd44c1d`; 2026-03-24 |

  The appendix states the method and presents these rows at [the dependency table](../docs/THINK-2026-11-08-file-viewer-prior-art-licences.md:12); the associated version/commit/date values are at lines 14–16, 43, 55, 66, 77, 86–88. The focused CAD and native-library evidence is at lines 225–238.

- [x] **PASS — The AGPL CAD runtime is clearly never-copy.** Both `@flyfish-dev/cad-viewer@0.8.3` and transitive `dwf-viewer@0.6.7` have raw AGPL-3.0-only package fields and AGPLv3 LICENSE files in the downloaded tarballs. The combined-package NOTICE supports the stated restriction. The table and dependency path are explicit ([THINK lines 91–102](../docs/THINK-2026-11-08-file-viewer-prior-art.md:91), [appendix lines 225–231](../docs/THINK-2026-11-08-file-viewer-prior-art-licences.md:225)); the recommendation treats File Viewer as design reference only ([THINK lines 12–16](../docs/THINK-2026-11-08-file-viewer-prior-art.md:12)).

- [x] **PASS — `occt-import-js` licence is resolved with evidence.** The exact `0.0.23` npm tarball reports `LGPL-2.1` and contains the LGPL-2.1 text in `LICENSE.md`. The cited OCCT submodule commit `d2abb6d844231cb8f29be6894440874a4700e4a5` identifies OCCT 7.6.1; I fetched it with Git and confirmed `LICENSE_LGPL_21.txt` and `OCCT_LGPL_EXCEPTION.txt` at that commit. This supports the appendix’s LGPL-2.1 plus Open CASCADE exception v1.0 classification ([appendix line 237](../docs/THINK-2026-11-08-file-viewer-prior-art-licences.md:237)). The package’s 2024-12-03 last commit is correctly described as borderline freshness, and the survey does not recommend copying it ([appendix lines 345–348](../docs/THINK-2026-11-08-file-viewer-prior-art-licences.md:345)).

- [x] **PASS — Rust candidates are surveyed under the repository rule.** The survey includes a comparison table of Rust crates covering PDF, images, archives, Office, markup, audio and other formats, with approach, licence, version, commit/date, liveness, `no_std` fit and a borrow/design decision ([THINK §4](../docs/THINK-2026-11-08-file-viewer-prior-art.md:161)). It flags the GPL EPUB crate as never-copy and marks compatible candidates or later work accordingly (lines 192–204). This meets the relevant `CLAUDE.md` survey requirements for candidate comparison and licensing/freshness ([CLAUDE.md lines 903–925](../CLAUDE.md:903)).

- [x] **PASS — Recommendation follows from the evidence.** Keeping File Viewer as a design reference fits its browser/DOM/Worker/WASM requirements and its AGPL CAD path. The staged Rust recommendation uses the existing image path and compares `zune-png` for V1, then ZIP/tar and `hayro` for later rungs; the plan is explicitly read-only, bounded, and avoids running embedded scripts or macros ([THINK lines 12–16](../docs/THINK-2026-11-08-file-viewer-prior-art.md:12), [lines 195–204](../docs/THINK-2026-11-08-file-viewer-prior-art.md:195), [lines 206–235](../docs/THINK-2026-11-08-file-viewer-prior-art.md:206)).

**New findings:** None in the checked items. The survey’s stated unknowns (including archived status and selected embedded native dependency versions) remain clearly marked as unknown in the appendix; they do not change the recommendation.
