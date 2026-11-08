<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: opening a file, the prior-art survey (File Viewer first)

**Date:** 2026-11-08 · **Author:** Seat F · **Asked by:** J.c. (handoff `Plans/HANDOFF-2026-11-03-pause.md`, "File
Viewer", added 2026-11-07), relayed by the PM on resume · **Status:** design only, no code.
**Read at:** `origin/main` @ `13eed2c5`.

**The question.** When a user opens a file in LogansOS, what shows it? This is the survey the CLAUDE.md rule asks for
before an unfamiliar feature is built ("Unfamiliar features: survey open source before writing from scratch"). J.c.
asked for File Viewer (`github.com/flyfish-dev/file-viewer`) to be in it, **with the licence of each piece checked**.

**Bottom line.**
1. **File Viewer is a design reference, not code we can run or vendor now.** It's a browser JS/TS library: it needs
   script, a DOM, WASM and Workers, and our browser doesn't run script yet (post-C3c). Its own code is Apache-2.0,
   which we could vendor later, but nearly every format it shows comes from a third-party engine (§3.3), and two of
   those pieces are AGPL-3.0-only (the CAD runtime: never copy).
2. **What to take from it is its shape** (§3.2): a format registry with an honest support level per format, a
   bounded content check before any parser runs, one lazily loaded engine per format, and "never execute" rules
   (macros, scripts).
3. **Our build stays Rust, one viewer per process**, routed by a small `open` step that reads the file's first
   bytes (§5). The Rust crates to borrow from are in §4. Most are MIT or Apache-2.0; a few are `no_std` already,
   which matters for our userland.
4. **The first rungs reuse what's planned:** text through the editor (OT-67 E2) read-only, images through `neo-view`
   (L2d), then a PDF viewer and an archive lister (§6).

## 1. What main has today

| Piece | State | Where |
|---|---|---|
| Files (the file manager) | F0/F1 rungs designed, Files is `MODULE_FILES = 5` | `docs/THINK-2026-10-29-ot88-app-model.md` §4 |
| Text | the editor (OT-67) is designed; E2 "open a file from Files" | `docs/THINK-2026-10-29-ot67-editor.md` §7 |
| Images | `neo-image-decode` (PNG, JPEG) is host-only std; `neo-view`, a one-image viewer client, is designed | `docs/THINK-2026-11-01-l2d-images-in-the-desktop.md` §1.1, §3.4 |
| Text engine | neo-text T0: TrueType, layout, wrap; no BiDi or complex shaping (T2) | `userland/neo-text/src/lib.rs` |
| Browser | parses and lays out HTML/CSS; no script until C3c | the OT-61 THINKs |
| NeoFS | open by path, read, readdir; no `stat`, no timestamps | app-model THINK §4.1 |

So "open a file" has two viewers designed (text, image) and nothing that picks between them.

## 2. Scope

**In:** read-only viewing of a file the user opened; how the viewer is picked; which formats come first; what we can
borrow. **Out:** editing (the editor THINK), thumbnails, search across files, and anything that needs the network.

## 3. File Viewer (`flyfish-dev/file-viewer`)

Checked at `7e5e131c5340dbd421e47725c895554ecc75ff11` (2026-10-31, v3.3.3; still the tip on 2026-11-08), cloned with
git. Every licence below was read from the raw `LICENSE` file or the raw `package.json` (`git show`, `npm pack`), never
a summarizing fetch.

### 3.1 What it is

- 275 registered extensions (225 stable, 50 experimental) over 47 preview pipelines: Word, Excel, PowerPoint, PDF,
  OFD, DWG/DXF, STEP/IFC, archives, email, images, media, code, DICOM (`README.en.md:39-44`).
- It runs in the browser, with no conversion server. Heavy engines (PDF, CAD, Office, 3D) are loaded per format, and
  their Workers, WASM and fonts are served next to the app.
- One API for Web Components, Vue, React, Svelte and jQuery.
- Its README says it's "a viewer, not an editor", and that an extension in its matrix doesn't promise the same
  fidelity for every file of that type (`README.en.md:98`).

### 3.2 What to take from its design

1. **A generated format registry** (`packages/core/src/registry/formats.generated.ts`). Each renderer has an id, its
   extensions, a support level (`high-fidelity` 12, `structured` 30, `basic` 1, `experimental` 4), a status
   (`stable`/`experimental`), container versions, known limits ("VBA is never executed") and its capabilities (print,
   zoom, search). **Take:** our registry as data, with the support level shown to the user, so a partial view never
   passes as a faithful one.
2. **A bounded precheck before any parser** (`packages/core/src/source/precheck.ts`). The extension picks a renderer,
   then a structural check runs: ZIP and OLE signatures, the parts an OpenXML file must have (`word/document.xml`),
   and a cap on the ZIP central directory read (8 MiB). The result is `ready`, `invalid` or `unsupported`, with a
   reason such as `signature-mismatch`. **Take:** the check, but with content first (§5.1): we don't trust the
   extension to pick the parser.
3. **One engine per format, loaded only when needed.** **Take:** one viewer process per format family, started only
   when a file of that family is opened.
4. **Licence bookkeeping as code** (`packages/build-support/`: bundle notices, the CAD corresponding-source offer).
   **Take:** the same idea as Raven's `LICENSES.json` (same handoff): a manifest of every vendored piece, checked in CI.

### 3.3 Its pieces and their licences

The full table, one row per dependency (106 rows: package, resolved version from `pnpm-lock.yaml`, the licence in
its raw `package.json`, what its `LICENSE` file says, upstream repo, head SHA, last commit, alive or not), is the
appendix `docs/THINK-2026-11-08-file-viewer-prior-art-licences.md`. That appendix also covers the 97 workspace
packages, the native C/C++ libraries compiled into WASM inside the dependencies, the vendored AOSP MPEG-4 decoder and
the two patches. Licences come from `npm pack` tarballs and shallow git clones. Spot-checked here: `cad-viewer`,
`occt-import-js` (re-packed independently: `"license": "LGPL-2.1"`), `heic2any`, `bpmn-js`.

**File Viewer's own code.** 87 of the 97 workspace packages are Apache-2.0 and 2 are MIT (`@file-viewer/doc`,
`msdoc-viewer`). The other 8 are private apps and examples with no licence field. The root `LICENSE` is Apache-2.0.
44 package folders have no committed `LICENSE` file (36 of them declare Apache-2.0 or MIT), so a vendored package would
need the root `LICENSE` copied in.

**Never copy (copyleft):**

| Piece | Licence (raw) | Where it comes from |
|---|---|---|
| `@flyfish-dev/cad-viewer` 0.8.3 | **AGPL-3.0-only** (LICENSE and NOTICE: "The combined package is distributed under AGPL-3.0-only") | a direct dependency of `@file-viewer/renderer-cad`, which is itself Apache-2.0 |
| `dwf-viewer` 0.6.7 | **AGPL-3.0-only** | through `cad-viewer` |
| `@mlightcad/libredwg-web` 0.7.9 (LibreDWG in WASM) | **GPL-3.0** (package.json `"GPL-3.0"`, no licence file in the tarball; upstream `COPYING` is GPL v3) | through `cad-viewer` |
| `occt-import-js` 0.0.23 (STEP/IGES) | **LGPL-2.1** (its `LICENSE.md`). It compiles **OpenCascade 7.6.1** in (LGPL-2.1 plus the Open CASCADE exception 1.0) | a dependency. Last commit 2024-12-03: nearly two years |
| `heic2any` 0.0.4 | says **MIT**, but embeds an emscripten build of **libheif**, which is **LGPL** (version unverified: the bundle has no version string and no notice) | a dependency. Last commit 2023-03-29 |

So DWG, DWF and STEP/IGES are the formats File Viewer can only show through copyleft code, and HEIC goes through LGPL
code that its own package doesn't declare.

**Usable with care:**
- **`bpmn-js` 18.31.1:** MIT plus a watermark clause: "The source code responsible for displaying the bpmn.io project
  watermark that links back to https://bpmn.io as part of rendered diagrams MUST NOT be removed or changed." Its
  package.json says `"SEE LICENSE IN LICENSE"`, not an SPDX id.
- **MPL-2.0 (our licence):** `web-ifc` 0.0.77 (its C++ dependencies' licences are unverified). `dompurify`
  `(MPL-2.0 OR Apache-2.0)`.
- **A choice we'd make:** `jszip` `(MIT OR GPL-3.0-or-later)`: take MIT.
- **Not usable as is:** `@paged-media/introspect-wasm` 0.73.4, `"MPL-2.0 OR LicenseRef-PMEL"` (PMEL is a commercial
  licence), with no licence file and no repository.
- **No licence file anywhere:** `@ljheee/xmind-parser` 1.1.3 says MIT but ships no LICENSE, upstream neither. By the
  CLAUDE.md rule that's all rights reserved until it has one.
- **Old native code inside:** `libarchive.js` 2.0.2 links **OpenSSL 1.0.2s** (2019, an end-of-life line) plus
  libarchive 3.7.2, zlib 1.3, xz 5.2.11 and bzip2 1.0.8, none of them named in its MIT `LICENSE`.
- **package.json and LICENSE disagree:** `pako` (`(MIT AND Zlib)`, LICENSE has MIT only), the cornerstone codecs (ISC
  or BSD-3 or MIT across package.json, the repo and the C library inside).

**The pieces J.c. named, in brief** (the full rows are in the appendix):

| Piece | Licence | Note |
|---|---|---|
| `pdfjs-dist` 5.4.624 | Apache-2.0 | patched by File Viewer (`patches/`, two marked changes in `pdf.worker.mjs`) |
| `three` | MIT | |
| `mermaid` | MIT | |
| `libarchive.js` 2.0.2 | MIT (its own); BSD-2, zlib, public domain, bzip2 and OpenSSL/SSLeay inside | last commit 2024-01-27 |
| `web-ifc` 0.0.77 | MPL-2.0 | |
| `@thatopen/*` | MIT (upstream `LICENSE.md`; no licence file in the tarballs) | |
| `@cornerstonejs/*` | MIT (core, metadata, utils); the codecs as above | |
| `sql.js` | MIT; SQLite 3.54.0 inside is public domain | |
| `xmllint-wasm` 5.3.0 | MIT; libxml2 2.13.8 inside is MIT-style | |
| `epubjs` | BSD-2-Clause | |
| `rtf.js` 3.0.9 | MIT | last commit 2022-10-09 |
| `postal-mime` | MIT-0 | |
| `@kenjiuno/msgreader` | Apache-2.0 | |
| `heic2any` 0.0.4 | MIT, with LGPL libheif inside | see above |
| `utif` | MIT | |
| `@webtoon/psd` 0.4.0 / `ag-psd` | MIT / MIT | `@webtoon/psd` last commit 2023-07-14 |
| `maplibre-gl` | BSD-3-Clause | |
| `proj4` | MIT | |
| `@myriaddreamin/typst*` 0.7.0 | Apache-2.0 (upstream; no licence file in the tarballs; the Rust crates inside unverified) | |
| `hyparquet` | MIT | |
| `bpmn-js` 18.31.1 | MIT plus the watermark clause | see above |
| `styled-exceljs` 0.21.6 | Apache-2.0 | |
| `cfb` 1.2.2 | Apache-2.0 | last commit 2022-11-10 |
| `occt-import-js` 0.0.23 | LGPL-2.1, OpenCascade 7.6.1 inside | see above |
| AOSP PacketVideo MPEG-4 decoder | Apache-2.0 (`AOSP-NOTICE.txt`: "Copyright (c) 2005-2008, The Android Open Source Project / Licensed under the Apache License, Version 2.0"); pinned to `frameworks/av` `e2f09893`, `media/module/codecs/m4v_h263/dec` | vendored at `packages/renderers/media/vendor/aosp-mp4v/`; its `SOURCE.json` lists `"forbiddenLicenses": ["LGPL","GPL","AGPL"]`. The upstream itself couldn't be fetched (proxy 403) |
| `patches/` | `illustrator-pgf@0.1.0` and `pdfjs-dist@5.4.624`; neither patch file states a licence | the pdf.js patch marks its changes ("File Viewer modification:"), as Apache-2.0 §4(b) asks |

**Alive or abandoned.** No resolved version is marked `deprecated` on npm. 14 dependencies have no commit since
2024-11-08 (the appendix's list, e.g. `pdf-lib` 2021, `rtf.js` 2022, `heic2any` 2023, `libarchive.js` 2024-01).
Archived status is unknown for every repo: no GitHub API access from the survey's container, and no README says
archived.

**Not verified** (each says so in the appendix): the upstream of `@file-viewer/docx` (its repo asks for credentials),
of `keynote-archives` and of `@paged-media/introspect-wasm` (no repository field); the AOSP upstream; the libheif
version in `heic2any`; `web-ifc`'s C++ dependencies; the typst-ts Rust crates; the cornerstone codec licences at
their pinned submodule commits (read at the forks' heads).

## 4. Rust candidates

One row per candidate. **Licence** is the crate's `license` field in its raw `Cargo.toml` and the head of its raw
`LICENSE` file(s), both read with git; the version is crates.io's latest (`cargo info`); the commit is the default
branch's tip, cloned on 2026-11-08. **`no_std`** means the crate's `src/lib.rs` declares `#![no_std]` (or
`cfg_attr(..., no_std)`) at that commit; nothing else is claimed.

| Crate (repo) | Does | Licence | Version | Commit (date) | Alive? | `no_std` | Take |
|---|---|---|---|---|---|---|---|
| `hayro` (LaurenzV/hayro) | PDF parse and render | Apache-2.0 OR MIT | 0.9.0 | `072819a4` (2026-11-07) | yes | `hayro-syntax`, `-ccitt`, `-cmap`, `-jbig2`, `-jpeg2000`, `-postscript` yes; `hayro`, `-interpret`, `-font` no | **code** (V3) |
| `pdf` (pdf-rs/pdf) | PDF parse | MIT | 0.10.0 | `689f744f` (2026-11-07) | yes | no | the alternative to hayro's parser |
| `lopdf` (J-F-Liu/lopdf) | PDF read and modify | MIT | 0.47.0 | `4eaec16b` (2026-11-04) | yes | no | reference only |
| `zune-image` and its crates (etemesi254/zune-image) | PNG, JPEG, JPEG XL, QOI, PSD, BMP, PPM, HEIC (with its own HEVC decoder) | MIT OR Apache-2.0 OR Zlib | 0.5.0 (`zune-heic` 0.5.0) | `8d8c6659` (2026-11-07) | yes | `zune-png`, `-jpeg`, `-jpegxl`, `-qoi`, `-psd`, `-bmp`, `-ppm`, `-heic`, `-inflate`, `-core` yes; `zune-image`, `-gif`, `-hdr` no | **code** (V1; HEIC later) |
| `image` (image-rs/image) | the common decoders behind one API | MIT OR Apache-2.0 | 0.25.10 | `075e65d6` (2026-09-15) | yes | no | host tools only |
| `jxl-oxide` (tirr-c/jxl-oxide) | JPEG XL | MIT OR Apache-2.0 | 0.12.6 | `c0cc4c7e` (2026-08-15) | yes | no | later |
| `resvg` (linebender/resvg) | SVG render | Apache-2.0 OR MIT | 0.49.0 | `847abd69` (2026-11-05) | yes | no | later (SVG) |
| `zip` (zip-rs/zip2) | ZIP read/write | MIT | 9.4.2 | `ce7aad08` (2026-11-08) | yes | no | **code** (V2) |
| `tar` (alexcrichton/tar-rs, now composefs/tar-rs) | tar read/write | MIT OR Apache-2.0 | 0.4.46 | `2c19ee2f` (2026-07-08; the same tip under both names) | yes | no | **code** (V2) |
| `sevenz-rust2` (hasenbanck/sevenz-rust) | 7z | Apache-2.0 | 0.25.0 | `3a56b95e` (2026-11-06) | yes | no | later. The original `sevenz-rust` (0.6.1, dyz1990/sevenz-rust) no longer answers a git clone: treat as abandoned |
| `calamine` (tafia/calamine) | xlsx, xls, ods read | MIT | 0.38.0 | `57011cf6` (2026-11-05) | yes | no | later (Office) |
| `docx-rs` (bokuweb/docx-rs) | docx read/write | MIT | 0.4.22 | `ad7fede6` (2026-10-17) | yes | no | later (Office) |
| `cfb` (mdsteele/rust-cfb) | OLE compound files (.doc, .xls, .msg) | MIT | 0.16.0 | `70992603` (2026-10-31) | yes | no | later (Office, email) |
| `quick-xml` (tafia/quick-xml) | XML parse | MIT | 0.42.0 | `250824b3` (2026-11-02) | yes | no | with the Office crates |
| `pulldown-cmark` (pulldown-cmark/pulldown-cmark) | Markdown | MIT | crates.io 0.13.6 (the repo's `Cargo.toml` says 0.13.1 at this commit) | `f7c6deaa` (2026-11-06) | yes | yes | **code** (V4) |
| `syntect` (trishume/syntect) | syntax highlighting | MIT | 5.3.0 | `4efbec17` (2026-10-31) | yes | no | V4, or a smaller highlighter |
| `mail-parser` (stalwartlabs/mail-parser) | MIME email (.eml) | Apache-2.0 OR MIT | 1.0.1 | `a56d5f1d` (2026-11-02) | yes | no | later (email) |
| `symphonia` (pdeljanov/Symphonia) | audio decode | MPL-2.0 (our licence) | 0.6.1 | `5f879df0` (2026-11-08) | yes | no | later (audio) |
| `typst` (typst/typst) | Typst documents | Apache-2.0 | 0.15.1 | `84cca406` (2026-10-28) | yes | no | later; heavy |
| `gltf` (gltf-rs/gltf) | glTF 3D models | MIT OR Apache-2.0 | 1.4.1 | `50d65229` (2026-05-11) | yes (slower) | no | later (3D) |
| `parquet` (apache/arrow-rs) | Parquet | Apache-2.0 | 60.1.0 | `f2d96279` (2026-11-08) | yes | no | later; heavy |
| `turso` (tursodatabase/turso) | SQLite-compatible database | MIT (with a `licenses/` folder of third-party notices to read before any use) | 0.9.0-pre.2 (a pre-release) | `9b71df9a` (2026-11-07) | yes | no | later (.sqlite) |
| **`epub` (danigm/epub-rs)** | EPUB | **GPL-3.0-or-later** | 2.1.6 | `09b50223` (2026-11-01) | yes | no | **never copy.** EPUB is a ZIP of XHTML: our browser plus `zip` covers it |
| `yazi` (sxyazi/yazi) | a terminal file manager with previews | MIT (icons MIT) | n/a (an app) | `6882e1ac` (2026-11-05) | yes | n/a | **design**: it picks a previewer by MIME type, fetched first, from an ordered rule list (`yazi-config/preset/yazi-default.toml:152-164`): the content-first routing of §5.1 |

**What the table says.**
- Every row but one is usable under our rules (MIT, Apache-2.0, Zlib, MPL-2.0). **`epub` is GPL: never copy.**
- Every candidate is alive (a commit since 2026-05). The only dead thing found is the original `sevenz-rust`.
- **`no_std` today:** `zune-png`/`zune-jpeg`, `pulldown-cmark`, and hayro's syntax and codec crates. The rest wait for
  OT-61's std target, or a viewer that runs as a std process when that exists.
- **`zune-jpeg` and `zune-core` are already vendored** (`third_party/zune-jpeg`, `third_party/zune-core`, root
  `Cargo.toml:113-120`, used by `neo-image-decode`). L2d's gap is the std-only `png` 0.18
  (`docs/THINK-2026-11-01-l2d-images-in-the-desktop.md` §1.1, which proposes writing a `no_std` PNG backend).
  `zune-png` from the same project is `no_std` and has the same licence. It's worth comparing with writing a new
  backend before V1.

## 5. The design this points to

### 5.1 Picking a viewer: the first bytes, then the name

An `open` step (in Files, or a small router it calls) reads the file's first bytes and picks a viewer from a registry
like File Viewer's:
- **The content decides** (`%PDF-`, `\x89PNG`, `PK\x03\x04`, the OLE header, UTF-8 that decodes). The extension only
  breaks ties between formats that share a container (a `.docx` and a `.zip` are both ZIP).
- **A mismatch is shown, never guessed past:** a `.png` that isn't a PNG opens as "not a PNG", not in the hex viewer.
- **A bounded precheck**, as File Viewer's: the ZIP central directory and the OpenXML parts are read with a cap.

### 5.2 One viewer per process

Each viewer is its own client (Design B, the app-model THINK §2.3), holding a read cap on the one file it was opened
with, and nothing else. A parser bug in, say, the PDF viewer ends that client alone, the same per-client fault rule
A1a already has (#15). That matters more for viewers than for most apps: they parse untrusted bytes.

### 5.3 Nothing runs

No macros, no embedded scripts, no PDF JavaScript, no external fetches (fonts, images, remote templates). File
Viewer says the same for VBA. A viewer that needs one of these shows what it skipped.

## 6. Rungs (each one kill-switchable, the feature off ≡ main)

| Rung | What | Depends on | Borrow |
|---|---|---|---|
| V0 | The registry and the `open` step: content sniffing, the mismatch message, routing to the text and image viewers | F1, E2 (read-only), L2d's `neo-view` | File Viewer's registry and precheck design |
| V1 | `no_std` PNG for `neo-view` (JPEG's `zune-jpeg` is already vendored and alloc-only) | L2d | `zune-png` (`no_std` at the commit in §4), compared with L2d's own-backend plan |
| V2 | Archive lister: ZIP and tar entries, read-only, with size caps | V0 | `zip`, `tar` (std: needs OT-61's std target, or a small `no_std` reader) |
| V3 | PDF viewer | V0, the text engine | `hayro` (its `hayro-syntax` and codec crates are `no_std`; the renderer isn't) |
| V4 | Markdown (and code with highlighting) in the text viewer | V0 | `pulldown-cmark` (`no_std`); `syntect` (std) |
| Later | Office (`calamine`, `docx-rs`, `cfb`), SVG (`resvg`), audio (`symphonia`), HEIC (`zune-heic`) | V0-V3 | §4 |

Each viewer rung's gate is a QEMU scenario that opens a fixture file and digests the window against an independent
oracle (L2d's frame-hash shape), plus a fixture whose content doesn't match its name.

## 7. Decisions for J.c.

1. **Content first, then the name** (§5.1), or the extension first as File Viewer does?
2. **The first two new viewers after text and image:** archives and PDF (as in §6), or another pair?
3. **The licence manifest** (§3.2 item 4): adopt it for `third_party/` now, or when the first viewer vendors code?
