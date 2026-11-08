<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# File Viewer's pieces and their licences (appendix to `THINK-2026-11-08-file-viewer-prior-art.md`)

Subject repo: https://github.com/flyfish-dev/file-viewer @ `7e5e131c5340dbd421e47725c895554ecc75ff11` (2026-10-31T21:05:43+08:00). Collected 2026-11-08.

Method: licence fields read from the raw `package/package.json` inside each `npm pack <pkg>@<resolved>` tarball; LICENSE first lines read from the same tarball; deprecation from raw `npm view --json`; upstream HEAD from `git clone --depth 1 --filter=blob:none --no-checkout` + `git log -1 --format='%H %cI'`. No WebFetch/search used. Resolved versions are from `pnpm-lock.yaml` `importers`. The raw evidence (each package's `license` field, the first lines of its licence files, the upstream `git log -1` line) was saved per package during the survey and is summarized in the tables below. Archived status could not be read (no GitHub API access from the survey's container); each README was grepped for archived/deprecated/unmaintained notices and none applied to the project itself, so archived = unknown everywhere. 'alive' = last commit on or after 2024-11-08.

## 1. Third-party dependencies

Role: `dep`/`peer`/`optional` = declared by a published workspace package; `devDependency only` = declared only as a devDependency of a published renderer; these are shipped inside the renderer's `dist` rather than installed by consumers — verified for pdfjs-dist (`packages/renderers/pdf` build runs `scripts/stage-pdfjs-runtime.mjs` into `dist/vendor/pdfjs`), NOT verified for mermaid, epubjs, @cornerstonejs/*; `transitive` = pulled in by a listed dep (included because it carries compiled code or a notable licence); `apps-only` = only the private demo/docs apps.

| package | version (range → resolved) | role | licence (package.json) | LICENSE file says | upstream repo | HEAD SHA | last commit | alive? |
|---|---|---|---|---|---|---|---|---|
| `@flyfish-dev/cad-viewer` | 0.8.3 → **0.8.3** | dep | `"AGPL-3.0-only"` | LICENSE: “GNU AFFERO GENERAL PUBLIC LICENSE” [AGPL-3.0]; NOTICE: “@flyfish-dev/cad-viewer” [unclassified] | github.com/flyfish-dev/cad-viewer | `0ce90d08bbf96aa780676ffeb81e0da12c575097` | 2026-10-11T09:58:36+08:00 | alive; archived: unknown |
| `dwf-viewer` | (transitive) → **0.6.7** | transitive | `"AGPL-3.0-only"` | LICENSE: “GNU AFFERO GENERAL PUBLIC LICENSE” [AGPL-3.0]; NOTICE: “DWF Viewer” [AGPL-3.0] | github.com/flyfish-dev/dwf-viewer | `72783571e2cd744862d8eb32f7d3455906c76226` | 2026-09-08T10:40:44+08:00 | alive; archived: unknown |
| `@mlightcad/libredwg-web` | (transitive) → **0.7.9** | transitive | `"GPL-3.0"` | no LICENSE in tarball; upstream COPYING = GPL v3 | github.com/mlightcad/libredwg-web | `82bf45777635942e5eb9b68c15a2ee75bbaf64ab` | 2026-11-08T08:43:54+08:00 | alive; archived: unknown |
| `@mlightcad/shx-parser` | (transitive) → **1.4.5** | transitive | `"MIT"` | LICENSE: “MIT License” [MIT-style] | github.com/mlightcad/shx-parser | `5100b5f29083d1f39d24ac57d68ec05cc702d647` | 2026-07-09T11:43:14+08:00 | alive; archived: unknown |
| `dxf-parser` | (transitive) → **1.1.2** | transitive | `"MIT"` | LICENSE: “The MIT License (MIT)” [MIT-style] | github.com/gdsestimating/dxf-parser | `0df7a37a4207a1f925b8d0bfffc270ff121446b4` | 2023-03-15T09:01:53-07:00 | **stale >2y** (last commit 2023-03-15); archived: unknown |
| `@file-viewer/docx` | 0.3.39 → **0.3.39** | dep | `"Apache-2.0"` | LICENSE: “Apache License” [Apache-2.0] | github.com/flyfish-dev/docxjs — clone failed (auth prompt ⇒ private/missing) | unverified | unverified | unverified (no reachable repo) |
| `@file-viewer/ppt` | 1.0.2 → **1.0.2** | dep | `"Apache-2.0"` | LICENSE: “Apache License” [Apache-2.0]; NOTICE.md: “# Source and distribution notice” [unclassified] | github.com/flyfish-dev/pptjs | `510613d0d5789be8ebd7c059503deb55a8af617e` | 2026-10-31T03:54:51+00:00 | alive; archived: unknown |
| `@fontsource-variable/noto-sans-sc` | 5.3.1 → **5.3.1** | dep | `"OFL-1.1"` | LICENSE: “Google Inc.” [MIT-style/OFL-1.1] — OFL-1.1 text | github.com/fontsource/font-files (`fonts/variable/noto-sans-sc`) | `5a72889057f592bb0659629e38337359f94d53b2` | 2026-11-03T18:49:36+09:00 | alive; archived: unknown |
| `@giro3d/giro3d` | 2.0.5 → **2.0.5** | peer | `"MIT"` | LICENSE: “MIT License” [MIT-style] | gitlab.com/giro3d/giro3d | `17f525457ad775efa2db6367c10553c2df530618` | 2026-11-03T12:13:03+01:00 | alive; archived: unknown |
| `@kenjiuno/msgreader` | ^1.29.0 → **1.29.0** | dep | `"Apache-2.0"` | LICENSE: “Apache License” [Apache-2.0] | github.com/HiraokaHyperTools/msgreader | `8f9b2a3aa53dcabc94ce20ea783cbba47613ec98` | 2026-10-27T18:47:23+09:00 | alive; archived: unknown |
| `@ljheee/xmind-parser` | ^1.1.3 → **1.1.3** | dep | `"MIT"` | no LICENSE in tarball AND none in upstream repo (package.json `files` lists LICENSE but it doesn't exist) | github.com/ljheee/xmind-parser | `60c3e0d941f4a9ac90fece9b5adb53bfd31b815a` | 2026-04-01T22:40:39+08:00 | alive; archived: unknown |
| `@myriaddreamin/typst-ts-renderer` | 0.7.0 → **0.7.0** | dep | `"Apache-2.0"` | no LICENSE in tarball; upstream root LICENSE: “Apache License” | github.com/Myriad-Dreamin/typst.ts | `52dd6cfb986208bcb8a88cd13a6070b9ba3aad13` | 2026-10-30T17:04:03+08:00 | alive; archived: unknown |
| `@myriaddreamin/typst-ts-web-compiler` | 0.7.0 → **0.7.0** | dep | `"Apache-2.0"` | no LICENSE in tarball; upstream root LICENSE: “Apache License” | github.com/Myriad-Dreamin/typst.ts | `52dd6cfb986208bcb8a88cd13a6070b9ba3aad13` | 2026-10-30T17:04:03+08:00 | alive; archived: unknown |
| `@myriaddreamin/typst.ts` | 0.7.0 → **0.7.0** | dep | `"Apache-2.0"` | LICENSE: “Apache License” [Apache-2.0] | github.com/Myriad-Dreamin/typst.ts | `52dd6cfb986208bcb8a88cd13a6070b9ba3aad13` | 2026-10-30T17:04:03+08:00 | alive; archived: unknown |
| `@paged-media/introspect-wasm` | 0.73.4 → **0.73.4** | dep | `"MPL-2.0 OR LicenseRef-PMEL"` | no LICENSE in tarball; no repository field; file-viewer ships `packages/build-support/licenses/introspect-wasm-0.73.4-LICENSE.md` (dual MPL-2.0 OR PMEL commercial) | unverified (no `repository` field) | unverified | unverified | unverified (no reachable repo) |
| `@panzoom/panzoom` | ^4.6.2 → **4.6.2** | dep | `"MIT"` | MIT-License.txt: “Copyright 2016-2019 Timmy Willison” [MIT-style] | github.com/timmywil/panzoom | `1c8de6bc5d644419f22319243cb013aa2dda9ef7` | 2026-11-02T10:22:42-05:00 | alive; archived: unknown |
| `@prettier/plugin-xml` | 3.4.2 → **3.4.2** | dep | `"MIT"` | LICENSE: “The MIT License (MIT)” [MIT-style] | github.com/prettier/plugin-xml | `40185b5f4dc0487bf3f1f1883e19fbcadbc8d04d` | 2026-11-05T00:03:01+00:00 | alive; archived: unknown |
| `@ssabrojs/hwpxjs` | 0.4.0 → **0.4.0** | dep | `"MIT"` | LICENSE: “MIT License” [MIT-style] | github.com/ssabro/hwpxjs | `58da65d8865d3500e60ba0ec5e901abd1b1b613c` | 2026-04-28T15:43:16+09:00 | alive; archived: unknown |
| `@thatopen/components` | 3.6.0 → **3.6.0** | peer | `"MIT"` | no LICENSE in tarball; upstream root LICENSE.md: “Permission is hereby granted, free of charge, to any person ” | github.com/ThatOpen/engine_components | `ad540db9c60362865362cbcdea2a1dfc623bc299` | 2026-10-22T21:32:10+02:00 | alive; archived: unknown |
| `@thatopen/fragments` | 3.6.0 → **3.6.0** | peer | `"MIT"` | no LICENSE in tarball; upstream root LICENSE.md: “Permission is hereby granted, free of charge, to any person ” | github.com/ThatOpen/engine_fragment (`packages/fragments`) | `a1e3aa3accbf276f5b78c099caf9e440f9fb3f76` | 2026-10-22T21:34:26+02:00 | alive; archived: unknown |
| `@tmcw/togeojson` | ^7.1.3 → **7.1.3** | dep | `"BSD-2-Clause"` | LICENSE: “Copyright (c) 2019 Tom MacWright, Mapbox All rights reserved” [BSD-2-style] | github.com/placemark/togeojson | `8348f05c996bc98931068d10fbf3667ae04f7db0` | 2026-10-09T08:56:21-04:00 | alive; archived: unknown |
| `@tonejs/midi` | ^2.0.28 → **2.0.28** | dep | `"MIT"` | LICENSE.md: “[The MIT License](http://opensource.org/licenses/MIT)” [MIT-style] | github.com/Tonejs/Midi | `1d3a1f539a2de55a448687285a393bc3aff7da1a` | 2023-05-31T10:03:44-07:00 | **stale >2y** (last commit 2023-05-31); archived: unknown |
| `@types/emscripten` | ^1.41.6 → **1.41.6** | dep | `"MIT"` | LICENSE: “MIT License” [MIT-style] | github.com/DefinitelyTyped/DefinitelyTyped (`types/emscripten`) | `163a20b93df6fd194aeb02404807816e2a81d303` | 2026-11-06T19:38:20+00:00 | alive; archived: unknown |
| `@types/pako` | ^2.0.4 → **2.0.4** | dep | `"MIT"` | LICENSE: “MIT License” [MIT-style] | github.com/DefinitelyTyped/DefinitelyTyped (`types/pako`) | `163a20b93df6fd194aeb02404807816e2a81d303` | 2026-11-06T19:38:20+00:00 | alive; archived: unknown |
| `@webtoon/psd` | 0.4.0 → **0.4.0** | dep | `"MIT"` | LICENSE: “Copyright 2021-present NAVER WEBTOON” [MIT-style] | github.com/webtoon/psd (inferred from homepage webtoon.github.io/psd; no repository field) | `d1a9a32ad38ff17c16f94562fc2dd47018338e78` | 2023-07-14T08:03:26+09:00 | **stale >2y** (last commit 2023-07-14); archived: unknown |
| `@xmldom/xmldom` | 0.9.12, ^0.9.12 → **0.9.12** | dep | `"MIT"` | LICENSE: “Copyright 2019 - present Christopher J. Brody and other cont” [MIT-style] | github.com/xmldom/xmldom | `325fa28b53c8bf7dddc967ace0cd6020047a2313` | 2026-10-11T21:11:40+02:00 | alive; archived: unknown |
| `ag-psd` | 31.0.3, ^31.0.3 → **31.0.3** | dep | `"MIT"` | LICENSE: “The MIT License (MIT)” [MIT-style] | github.com/Agamnentzar/ag-psd | `a74425db6584cc07b1cc135e370916cb4eccfb32` | 2026-10-28T23:06:33+01:00 | alive; archived: unknown |
| `avsc` | ^5.7.9 → **5.7.9** | dep | `"MIT"` | LICENSE: “Copyright (c) 2015-2017, Matthieu Monsch.” [MIT-style] | github.com/mtth/avsc | `91d653f72906102448a059cb81692177bb678f52` | 2025-04-09T21:13:49-07:00 | alive; archived: unknown |
| `billboard.js` | 4.1.3 → **4.1.3** | dep | `"MIT"` | LICENSE: “The MIT License (MIT)” [MIT-style] | github.com/naver/billboard.js | `3af24c97ddb860c199f69e4ce7b0ccea733d82c4` | 2026-10-25T22:42:51+09:00 | alive; archived: unknown |
| `bpmn-js` | 18.31.1 → **18.31.1** | peer | `"SEE LICENSE IN LICENSE"` | LICENSE: “Copyright (c) 2014-present Camunda Services GmbH” [MIT-style] — MIT text + watermark clause (see flags) — not plain MIT | github.com/bpmn-io/bpmn-js | `e7b7435ed60f662b3d80caaefe5dccba5d3a4c75` | 2026-10-21T15:49:48+02:00 | alive; archived: unknown |
| `buffer` | 6.0.3, ^6.0.3 → **6.0.3** | dep | `"MIT"` | LICENSE: “The MIT License (MIT)” [MIT-style] | github.com/feross/buffer | `d02bd0a30d9b0bbd0189d5a7c424b0ab165fe5ab` | 2025-10-28T11:33:50+11:00 | alive; archived: unknown |
| `cfb` | ^1.2.2 → **1.2.2** | dep | `"Apache-2.0"` | LICENSE: “Apache License” [Apache-2.0] | github.com/SheetJS/js-cfb | `b01260294e204f7c128ba6ae81ec4f4d3d61a298` | 2022-11-10T10:35:49+00:00 | **stale >2y** (last commit 2022-11-10); archived: unknown |
| `d3-format` | ^3.1.2 → **3.1.2** | dep | `"ISC"` | LICENSE: “Copyright 2010-2026 Mike Bostock” [ISC/0BSD-style] | github.com/d3/d3-format | `36eed1a337bac2e38b5b4d70cf7cb127f5de554f` | 2026-11-02T22:30:30-08:00 | alive; archived: unknown |
| `dicom-parser` | 1.8.21 → **1.8.21** | dep | `"MIT"` | LICENSE: “The MIT License (MIT)” [MIT-style] | github.com/cornerstonejs/dicomParser | `7d2084349bf2bdaffe74021e27b286a6c295ca66` | 2023-10-17T16:50:44-05:00 | **stale >2y** (last commit 2023-10-17); archived: unknown |
| `diff2html` | ^3.4.56 → **3.4.56** | dep | `"MIT"` | LICENSE.md: “Copyright 2014-2016 Rodrigo Fernandes https://rtfpessoa.gith” [MIT-style] | github.com/rtfpessoa/diff2html | `72331ff5d13a8d4d1f11256a7b554dd0d69d7014` | 2026-01-31T12:43:24+00:00 | alive; archived: unknown |
| `dingbat-to-unicode` | ^1.0.1 → **1.0.2** | dep | `"BSD-2-Clause"` | LICENSE: “Copyright (c) 2021, Michael Williamson” [BSD-2-style] | github.com/mwilliamson/dingbat-to-unicode | `9eb2d1d8b8463b7bd3cd125597eabcfa9df42128` | 2026-09-19T10:09:50+01:00 | alive; archived: unknown |
| `dompurify` | 3.4.17 → **3.4.17** | dep | `"(MPL-2.0 OR Apache-2.0)"` | LICENSE: “Apache License” [Apache-2.0]; LICENSE-MPL: “Mozilla Public License Version 2.0” [GPL/MPL-2.0] — two files: Apache-2.0 and MPL-2.0 texts (dual) | github.com/cure53/DOMPurify | `ac3c6644e377dfebf202fe37d5b79aca092fc03f` | 2026-11-07T12:05:56+01:00 | alive; archived: unknown |
| `e-virt-table` | 1.4.17 → **1.4.17** | dep | `"MIT"` | LICENSE: “MIT License” [MIT-style] | github.com/laichuangwen/e-virt-table | `633496b6980aeba888230dfdcb5e8813d8373b93` | 2026-11-03T17:41:57+08:00 | alive; archived: unknown |
| `epsg-index` | 2.0.0 → **2.0.0** | dep | `"ISC"` | license.md: “Copyright (c) 2023, Jannis R” [ISC/0BSD-style] | github.com/derhuerst/epsg-index | `29ecfcad5a54a3552a71d3946cc825151bd33c7a` | 2025-10-21T17:57:50+02:00 | alive; archived: unknown |
| `events` | 3.3.0, ^3.3.0 → **3.3.0** | dep | `"MIT"` | LICENSE: “MIT” [MIT-style] | github.com/Gozala/events | `48e3d18659caf72d94d319871106f089bb40002d` | 2021-02-27T18:06:12+01:00 | **stale >2y** (last commit 2021-02-27); archived: unknown |
| `fzstd` | 0.1.1 → **0.1.1** | dep | `"MIT"` | LICENSE: “MIT License” [MIT-style] | github.com/101arrowz/fzstd | `4527daeb1fd8e5c3cff89262fc0d779593e5f2fd` | 2024-03-15T02:44:22-04:00 | **stale >2y** (last commit 2024-03-15); archived: unknown |
| `heic2any` | ^0.0.4 → **0.0.4** | dep | `"MIT"` | LICENSE.md: “MIT License” [MIT-style] | github.com/alexcorvi/heic2any | `3428539e643e112323a5b8a2c77c6402cb1372f6` | 2023-03-29T11:35:40+03:00 | **stale >2y** (last commit 2023-03-29); archived: unknown |
| `highlight.js` | ^11.11.1 → **11.12.0** | dep | `"BSD-3-Clause"` | LICENSE: “BSD 3-Clause License” [BSD-3-style] | github.com/highlightjs/highlight.js | `60e092057673595fa542db0bc2b13a99df5d0551` | 2026-11-01T01:07:41-07:00 | alive; archived: unknown |
| `hls.js` | ^1.7.2 → **1.7.3** | dep | `"Apache-2.0"` | LICENSE: “Copyright (c) 2017 Dailymotion (http://www.dailymotion.com)” [Apache-2.0] | github.com/video-dev/hls.js | `15ded1909feda7a414998ba022d3b88e647592f7` | 2026-11-05T12:20:25+00:00 | alive; archived: unknown |
| `hyparquet` | ^1.31.5 → **1.31.5** | dep | `"MIT"` | LICENSE: “The MIT License (MIT)” [MIT-style] | github.com/hyparam/hyparquet | `8f4096afaa8da5d0bbd76a0225b44c71ac4fcd83` | 2026-11-07T19:17:48-08:00 | alive; archived: unknown |
| `illustrator-pgf` | 0.1.0 → **0.1.0** | dep | `"MIT"` | LICENSE: “MIT License” [MIT-style]; THIRD_PARTY_NOTICES.md: “# Third-Party Notices” [unclassified] | github.com/flyfish-dev/illustrator-pgf | `29edb189807cc6a0e86cfdaad0fdb844e79bcb91` | 2026-08-31T20:57:19+08:00 | alive; archived: unknown |
| `jszip` | 3.10.2 → **3.10.2** | dep/peer | `"(MIT OR GPL-3.0-or-later)"` | LICENSE.markdown: “JSZip is dual licensed. At your choice you may use it under ” [AGPL-3.0/MIT-style] — dual: MIT *or* GPLv3 (LICENSE.markdown line 1) | github.com/Stuk/jszip | `609d95f4098a11507160cd101e0b181cfad6a582` | 2026-09-09T19:13:04+02:00 | alive; archived: unknown |
| `keynote-archives` | 2.0.1 → **2.0.1** | dep | `"MIT"` | LICENSE: “MIT license” [MIT-style] | unverified (no `repository` in package.json; README links psobot/keynote-parser) | unverified | unverified | unverified (no reachable repo) |
| `libarchive.js` | ^2.0.2 → **2.0.2** | dep | `"MIT"` | LICENSE: “MIT License” [MIT-style] | github.com/nika-begiashvili/libarchivejs | `0989c1a6db20d030d793b1763e20d880068091bd` | 2024-01-27T21:35:29+04:00 | **stale >2y** (last commit 2024-01-27); archived: unknown |
| `maplibre-gl` | 6.16.0 → **6.16.0** | dep | `"BSD-3-Clause"` | LICENSE.txt: “Copyright (c) 2023, MapLibre contributors” [MIT-style/BSD-3-style] — BSD-3 + MIT-style sections (multi-notice file) | github.com/maplibre/maplibre-gl-js | `839df0849182abd7558a6727df16d8bdc4e4d15e` | 2026-11-07T22:53:50+00:00 | alive; archived: unknown |
| `marked` | ^18.1.2 → **18.1.2** | dep | `"MIT"` | LICENSE: “# License information” [MIT-style/BSD-3-style] — MIT + BSD-3 (markdown.pl) sections | github.com/markedjs/marked | `ba50aa95636ac8f97f46446bbfeb04592ed22216` | 2026-11-02T17:59:13+00:00 | alive; archived: unknown |
| `modern-screenshot` | ^4.7.0 → **4.7.0** | dep | `"MIT"` | LICENSE: “The MIT License (MIT)” [MIT-style] | github.com/qq15725/modern-screenshot | `792d6db7411839c62940a6e930161f8e376e817f` | 2026-04-16T15:25:10+08:00 | alive; archived: unknown |
| `occt-import-js` | 0.0.23 → **0.0.23** | dep | `"LGPL-2.1"` | LICENSE.md: “GNU LESSER GENERAL PUBLIC LICENSE” [LGPL] — LGPL-2.1 text | github.com/kovacsv/occt-import-js | `41e470890ae0f9dc69ac50ffd5fc73e03576f4eb` | 2024-12-03T08:01:25+01:00 | alive; archived: unknown |
| `pako` | 2.2.0, ^2.2.0 → **2.2.0** | dep | `"(MIT AND Zlib)"` | LICENSE: “(The MIT License)” [MIT-style] — LICENSE has MIT text only; Zlib part of `MIT AND Zlib` not in the LICENSE file | github.com/nodeca/pako | `d0101a9cf08e5260d962e47d1a095f530231e61d` | 2026-10-26T22:44:51+02:00 | alive; archived: unknown |
| `pdf-lib` | 1.17.1 → **1.17.1** | peer | `"MIT"` | LICENSE.md: “MIT License” [MIT-style] | github.com/Hopding/pdf-lib | `93dd36e85aa659a3bca09867d2d8fac172501fbe` | 2021-11-11T18:13:45-06:00 | **stale >2y** (last commit 2021-11-11); archived: unknown |
| `plantuml-encoder` | ^1.4.0 → **1.4.0** | dep | `"MIT"` | LICENSE: “Copyright (C) 2015 by Markus Hedvall” [MIT-style] | github.com/markushedvall/plantuml-encoder | `bf14043ad90dc6ba00a196b6e6cfce450085f780` | 2019-08-09T09:08:46+02:00 | **stale >2y** (last commit 2019-08-09); archived: unknown |
| `postal-mime` | 4.1.4 → **4.1.4** | dep | `"MIT-0"` | LICENSE.txt: “Copyright (c) 2021-2025 Andris Reinman” [MIT-style] — MIT-0 text (no attribution clause) — agrees | github.com/postalsys/postal-mime | `939a220bd518f54d98a088c315d7227e6215cae3` | 2026-10-29T14:24:57+02:00 | alive; archived: unknown |
| `prettier` | 3.9.10 → **3.9.10** | dep | `"MIT"` | LICENSE: “Copyright © James Long and contributors” [MIT-style]; THIRD-PARTY-NOTICES.md: “# Licenses of bundled dependencies” [Apache-2.0/MIT-style/BSD-3-style/ISC/0BSD-style] | github.com/prettier/prettier | `b09dd90022c875f3089f19f37724da6377e52b7f` | 2026-11-06T15:01:21+08:00 | alive; archived: unknown |
| `proj4` | ^2.22.0 → **2.22.0** | dep | `"MIT"` | LICENSE.md: “## Proj4js -- Javascript reprojection library.” [MIT-style] | github.com/proj4js/proj4js | `7b94f6be407e25ea4b8b5ae80f0fece279a79ea4` | 2026-10-13T12:54:56+02:00 | alive; archived: unknown |
| `roughjs` | ^4.6.6 → **4.6.6** | dep | `"MIT"` | LICENSE: “MIT License” [MIT-style] | github.com/pshihn/rough | `56a2762171b1294d643501e8d14f120db6b27bd7` | 2023-11-19T20:30:18-08:00 | **stale >2y** (last commit 2023-11-19); archived: unknown |
| `rtf.js` | ^3.0.9 → **3.0.9** | dep | `"MIT"` | LICENSE: “The MIT License (MIT)” [MIT-style] | github.com/tbluemel/rtf.js | `85fddf55b2f262bfd450769120c18c9ccef1f21c` | 2022-10-09T15:01:28+02:00 | **stale >2y** (last commit 2022-10-09); archived: unknown |
| `saxes` | 6.0.0 → **6.0.0** | dep | `"ISC"` | no LICENSE in tarball; upstream root LICENSE: “The ISC License” | github.com/lddubeau/saxes | `4968bd09b5fd0270a989c69913614b0e640dae1b` | 2025-10-14T05:01:37+00:00 | alive; archived: unknown |
| `shpjs` | ^6.2.0 → **6.2.0** | dep | `"MIT"` | LICENSE.md: “The MIT License (MIT)” [MIT-style] | github.com/calvinmetcalf/shapefile-js | `b5427363f1629abfcf9d43db9ad9f09d0be2f308` | 2025-10-01T10:22:20-04:00 | alive; archived: unknown |
| `sql.js` | 1.15.2 → **1.15.2** | dep | `"MIT"` | LICENSE: “MIT license” [MIT-style] | github.com/sql-js/sql.js | `4f1c22d0cf7d91b78e5feedab62212e8127751f4` | 2026-10-26T12:10:14+01:00 | alive; archived: unknown |
| `styled-exceljs` | 0.21.6 → **0.21.6** | dep | `"Apache-2.0"` | LICENSE: “Apache License” [Apache-2.0] | github.com/flyfish-dev/styled-exceljs | `501ea6d68602a84380be54bd1eb866f40942b534` | 2026-10-11T14:19:17+08:00 | alive; archived: unknown |
| `three` | ^0.186.0 → **0.186.1** | dep | `"MIT"` | LICENSE: “The MIT License” [MIT-style] | github.com/mrdoob/three.js | `f2b6a71bdd088d9da387fcc62802323341099b92` | 2026-11-07T10:16:06+01:00 | alive; archived: unknown |
| `tinycolor2` | ^1.6.0 → **1.6.0** | dep | `"MIT"` | LICENSE: “Copyright (c), Brian Grinstead, http://briangrinstead.com” [MIT-style] | github.com/bgrins/TinyColor | `b49018c9f2dbca313d80d7a4dad25e26143cfe01` | 2023-02-03T15:44:50-08:00 | **stale >2y** (last commit 2023-02-03); archived: unknown |
| `tslib` | ^2.8.1 → **2.8.1** | dep | `"0BSD"` | CopyrightNotice.txt: “/***********************************************************” [ISC/0BSD-style]; LICENSE.txt: “Copyright (c) Microsoft Corporation.” [ISC/0BSD-style] — 0BSD text | github.com/Microsoft/tslib | `a842f37ad47b6026cf4b7f9ef4f74bdfd4afd37f` | 2026-11-05T17:01:45-08:00 | alive; archived: unknown |
| `utif` | ^3.1.0 → **3.1.0** | dep | `"MIT"` | LICENSE: “MIT License” [MIT-style] | github.com/photopea/UTIF.js | `e7f41d49e5e203c5139994eb12e1400bcae2b009` | 2026-11-07T19:53:34+01:00 | alive; archived: unknown |
| `web-ifc` | 0.0.77 → **0.0.77** | peer | `"MPL-2.0"` | LICENSE.md: “Mozilla Public License Version 2.0” [GPL/MPL-2.0] — MPL-2.0 text | github.com/ThatOpen/engine_web-ifc | `bbb34120e2dd2659836d849f84cf97d5f47e6587` | 2026-11-05T19:00:14+00:00 | alive; archived: unknown |
| `xmllint-wasm` | 5.3.0 → **5.3.0** | peer | `"MIT"` | COPYING: “This project is MIT licensed, just like libxml which it is a” [MIT-style] | github.com/noppa/xmllint-wasm | `83b814cdc39625af5c34370c627599d973d28b07` | 2026-08-05T22:03:54+03:00 | alive; archived: unknown |
| `xslt-polyfill` | 1.0.33 → **1.0.33** | peer | `"BSD-3-Clause"` | LICENSE: “BSD 3-Clause License” [BSD-3-style] | github.com/mfreed7/xslt_polyfill | `8b17cff917223f40c22063e82e04a91f0cb6cf25` | 2026-11-06T13:16:09-08:00 | alive; archived: unknown |
| `pdfjs-dist` | 5.4.624 → **5.4.624** | devDependency only | `"Apache-2.0"` | LICENSE: “Apache License” [Apache-2.0] | github.com/mozilla/pdf.js | `a4800c8193ac77ce19a3dbdaf6e92ce677534b7f` | 2026-11-07T16:42:22+01:00 | alive; archived: unknown |
| `mermaid` | 12.2.0 → **12.2.0** | devDependency only | `"MIT"` | LICENSE: “The MIT License (MIT)” [MIT-style] | github.com/mermaid-js/mermaid | `a0fd6b8cee272d47d39c0b6bfdf6d2283efa9a49` | 2026-11-06T11:51:56+00:00 | alive; archived: unknown |
| `epubjs` | 0.3.93 → **0.3.93** | devDependency only | `"BSD-2-Clause"` | license: “Copyright (c) 2013, FuturePress” [BSD-2-style] | github.com/futurepress/epub.js | `eee359d0790002115a1156a9833c54f4bcd44c1d` | 2026-03-23T21:19:57-07:00 | alive; archived: unknown |
| `@cornerstonejs/core` | 5.13.2 → **5.13.2** | devDependency only | `"MIT"` | no LICENSE in tarball; upstream root LICENSE: “MIT License” | github.com/cornerstonejs/cornerstone3D | `08d6211982efbf1c76267e569ecae74d1824993e` | 2026-11-03T19:33:36+00:00 | alive; archived: unknown |
| `@cornerstonejs/dicom-image-loader` | 5.13.2 → **5.13.2** | devDependency only | `"MIT"` | LICENSE: “The MIT License (MIT)” [MIT-style] | github.com/cornerstonejs/cornerstone3D | `08d6211982efbf1c76267e569ecae74d1824993e` | 2026-11-03T19:33:36+00:00 | alive; archived: unknown |
| `@cornerstonejs/metadata` | 5.13.2 → **5.13.2** | devDependency only | `"MIT"` | no LICENSE in tarball; upstream root LICENSE: “MIT License” | github.com/cornerstonejs/cornerstone3D | `08d6211982efbf1c76267e569ecae74d1824993e` | 2026-11-03T19:33:36+00:00 | alive; archived: unknown |
| `@cornerstonejs/utils` | (transitive) → **5.13.2** | transitive | `"MIT"` | no LICENSE in tarball; upstream root LICENSE: “MIT License” | github.com/cornerstonejs/cornerstone3D | `08d6211982efbf1c76267e569ecae74d1824993e` | 2026-11-03T19:33:36+00:00 | alive; archived: unknown |
| `@cornerstonejs/codec-charls` | (transitive) → **1.2.7** | transitive | `"MIT"` | LICENSE: “MIT License” [MIT-style] | github.com/cornerstonejs/codecs (`packages/charls`) | `d9eee38611d20558299d5b02cd3137aabdc565e1` | 2026-10-14T12:16:41+00:00 | alive; archived: unknown |
| `@cornerstonejs/codec-libjpeg-turbo-8bit` | (transitive) → **1.2.7** | transitive | `"ISC"` | no LICENSE in tarball; upstream root LICENSE: “MIT License” | github.com/cornerstonejs/codecs (`packages/libjpeg-turbo-8bit`) | `d9eee38611d20558299d5b02cd3137aabdc565e1` | 2026-10-14T12:16:41+00:00 | alive; archived: unknown |
| `@cornerstonejs/codec-libjxl` | (transitive) → **1.1.1** | transitive | `"BSD-3-Clause"` | no LICENSE in tarball; upstream root LICENSE: “MIT License” | github.com/cornerstonejs/codecs (`packages/libjxl`) | `d9eee38611d20558299d5b02cd3137aabdc565e1` | 2026-10-14T12:16:41+00:00 | alive; archived: unknown |
| `@cornerstonejs/codec-openjpeg` | (transitive) → **1.3.6** | transitive | `"MIT"` | no LICENSE in tarball; upstream root LICENSE: “MIT License” | github.com/cornerstonejs/codecs (`packages/openjpeg`) | `d9eee38611d20558299d5b02cd3137aabdc565e1` | 2026-10-14T12:16:41+00:00 | alive; archived: unknown |
| `@cornerstonejs/codec-openjph` | (transitive) → **2.4.11** | transitive | `"MIT"` | LICENSE: “MIT License” [MIT-style] | github.com/cornerstonejs/codecs (`packages/openjphjs`) | `d9eee38611d20558299d5b02cd3137aabdc565e1` | 2026-10-14T12:16:41+00:00 | alive; archived: unknown |
| `@cornerstonejs/jpeg-lossless-decoder-js` | (transitive) → **2.2.1** | transitive | `"MIT"` | LICENSE: “Copyright (c) 2015 RII-UTHSCSA” [MIT-style] | github.com/cornerstonejs/JPEGLosslessDecoderJS | `cbe7e276c3ea5f0d2eda85a5e3788acdca6ef261` | 2026-09-11T14:05:02+00:00 | alive; archived: unknown |
| `@kitware/vtk.js` | (transitive) → **36.4.1** | transitive | `"BSD-3-Clause"` | LICENSE: “Copyright (c) 2016, Kitware Inc.” [BSD-3-style] | github.com/Kitware/vtk-js | `5d46987847429e1556b3c182d21bcb92f226d9e2` | 2026-11-05T16:43:18+00:00 | alive; archived: unknown |
| `dcmjs` | (transitive) → **0.52.0** | transitive | `"MIT"` | License.txt: “The MIT License (MIT)” [MIT-style] | github.com/dcmjs-org/dcmjs | `aee06e435df085394798bb36c7e8ca584a262040` | 2026-10-19T11:57:41-04:00 | alive; archived: unknown |
| `comlink` | (transitive) → **4.4.2** | transitive | `"Apache-2.0"` | LICENSE: “Apache License” [Apache-2.0] | github.com/GoogleChromeLabs/comlink | `114a4a6448a855a613f1cb9a7c89290606c003cf` | 2025-06-17T17:01:59-07:00 | alive; archived: unknown |
| `react` | >=16.8 <18, >=17 <20, ^17.0.2, ^19.2.7, ^19.2.8 → **19.3.0** | peer | `"MIT"` | LICENSE: “MIT License” [MIT-style] | github.com/react/react (`packages/react`) | `d71b537241d26861df496413db2142c7ba192c25` | 2026-11-06T14:43:46-05:00 | alive; archived: unknown |
| `react-dom` | ^19.2.7, ^19.2.8 → **19.3.0** | apps-only (private app) | `"MIT"` | LICENSE: “MIT License” [MIT-style] | github.com/react/react (`packages/react-dom`) | `d71b537241d26861df496413db2142c7ba192c25` | 2026-11-06T14:43:46-05:00 | alive; archived: unknown |
| `vue` | 2.6.14, 3.5.42, >=2.6 <2.7, >=2.7 <3, >=3.3 <4, ^2.6.14, ^2.7.16, ^3.5.42 → **3.5.42** | peer | `"MIT"` | LICENSE: “The MIT License (MIT)” [MIT-style] | github.com/vuejs/core | `8b479a89445868d474f17e2d837765e7c9ece5e4` | 2026-11-05T17:10:59+08:00 | alive; archived: unknown |
| `jquery` | >=3 <5, ^4.0.0 → **4.0.0** | peer | `"MIT"` | LICENSE.txt: “Copyright OpenJS Foundation and other contributors, https://” [MIT-style] | github.com/jquery/jquery | `feaa9fcdb68cdeadf013d97d8fd40848d4618578` | 2026-11-02T19:01:29+01:00 | alive; archived: unknown |
| `svelte` | >=3.59 <6, ^5.57.3 → **5.57.3** | peer | `"MIT"` | LICENSE.md: “Copyright (c) 2016-2025 [Svelte Contributors](https://github” [MIT-style] | github.com/sveltejs/svelte (`packages/svelte`) | `7775f73b7cbe32de3bb566f3515113208887e961` | 2026-11-05T04:40:54+08:00 | alive; archived: unknown |
| `vite` | 8.3.5, >=5 <9 → **8.3.5** | peer | `"MIT"` | LICENSE.md: “# Vite core license” [MIT-style/BSD-2-style/ISC/0BSD-style] | github.com/vitejs/vite (`packages/vite`) | `a579e352a66fbdce6a51d4e1f67006ccf4f91343` | 2026-11-06T20:40:00+09:00 | alive; archived: unknown |
| `esbuild` | 0.28.2, ^0.28.1, ^0.28.2 → **0.28.2** | peer | `"MIT"` | LICENSE.md: “MIT License” [MIT-style] | github.com/evanw/esbuild | `f6058f8364fe7ab91ca57a83e02577ed74c9cae4` | 2026-08-09T13:17:12-04:00 | alive; archived: unknown |
| `next` | 16.4.3 → **16.4.3** | apps-only (private app) | `"MIT"` | license.md: “The MIT License (MIT)” [MIT-style] | github.com/vercel/next.js | `ec8ddcd7a52fb7b3508fa1cf467abc85dea3a0b8` | 2026-11-08T00:21:18+00:00 | alive; archived: unknown |
| `fumadocs-core` | 16.17.8 → **16.17.8** | apps-only (private app) | `"MIT"` | LICENSE: “MIT License” [MIT-style] | github.com/fuma-nama/fumadocs | `dd9ba6f1f45c639102c80e2d5c50c5ae995442ac` | 2026-11-08T13:09:18+08:00 | alive; archived: unknown |
| `fumadocs-mdx` | 15.4.10 → **15.4.10** | apps-only (private app) | `"MIT"` | LICENSE: “MIT License” [MIT-style] | github.com/fuma-nama/fumadocs | `dd9ba6f1f45c639102c80e2d5c50c5ae995442ac` | 2026-11-08T13:09:18+08:00 | alive; archived: unknown |
| `@fumadocs/base-ui` | (transitive) → **16.15.8** | transitive | `"MIT"` | LICENSE: “MIT License” [MIT-style] | github.com/fuma-nama/fumadocs | `dd9ba6f1f45c639102c80e2d5c50c5ae995442ac` | 2026-11-08T13:09:18+08:00 | alive; archived: unknown |
| `@fumadocs/language` | 0.2.16 → **0.2.16** | apps-only (private app) | `"MIT"` | LICENSE: “MIT License” [MIT-style] | github.com/fuma-nama/fumadocs | `dd9ba6f1f45c639102c80e2d5c50c5ae995442ac` | 2026-11-08T13:09:18+08:00 | alive; archived: unknown |
| `@tabler/icons-react` | 3.49.0 → **3.49.0** | apps-only (private app) | `"MIT"` | LICENSE: “MIT License” [MIT-style] | github.com/tabler/tabler-icons (`packages/icons-react`) | `a218e471be9fd559a32113bb05bad34ca0018c1d` | 2026-11-07T23:34:48+01:00 | alive; archived: unknown |
| `@lucide/vue` | ^1.61.0 → **1.61.0** | apps-only (private app) | `"ISC"` | LICENSE: “ISC License” [MIT-style/ISC/0BSD-style] — ISC + MIT (Feather) sections | github.com/lucide-icons/lucide (`packages/vue`) | `53fab88ff9d1b48f4704b5b5b560d6b4d560bf4c` | 2026-11-06T13:25:23+00:00 | alive; archived: unknown |
| `cnfast` | ^0.2.0 → **0.2.0** | apps-only (private app) | `"MIT"` | LICENSE: “MIT License” [MIT-style] | github.com/aidenybai/cnfast (`packages/cnfast`) | `06263fb062aa755959ee06d9fb1b8d021d8052b8` | 2026-09-01T16:41:35-07:00 | alive; archived: unknown |
| `flexsearch` | 0.8.212 → **0.8.212** | apps-only (private app) | `"Apache-2.0"` | LICENSE: “Apache License” [Apache-2.0] | github.com/nextapps-de/flexsearch | `f7ed963096a0792da7b2fd63bb7114b3fbac55ed` | 2026-05-29T10:32:35+02:00 | alive; archived: unknown |
| `diff` | ^9.0.0 → **9.0.0** | apps-only (private app) | `"BSD-3-Clause"` | LICENSE: “BSD 3-Clause License” [BSD-3-style] | github.com/kpdecker/jsdiff | `8d706c8f2c17283d1061308113c95146a5118aa6` | 2026-10-27T11:45:40+00:00 | alive; archived: unknown |
| `qs` | ^6.16.1 → **6.16.1** | apps-only (private app) | `"BSD-3-Clause"` | LICENSE.md: “BSD 3-Clause License” [BSD-3-style] | github.com/ljharb/qs | `43baea4655cb9b543fb1c5cc0db95cde42852140` | 2026-10-28T16:37:24-07:00 | alive; archived: unknown |

## 2. Workspace packages (pnpm-workspace.yaml globs)

Root `LICENSE` first lines: `Apache License` / `Version 2.0, January 2004` / `http://www.apache.org/licenses/`.

| dir | name | version | license (package.json) | LICENSE file in tree (first line) |
|---|---|---|---|---|
| packages/core | `@file-viewer/core` | 3.3.3 | Apache-2.0 | none (not committed) |
| packages/thumbnail | `@file-viewer/thumbnail` | 3.3.3 | Apache-2.0 | LICENSE: “Apache License 2.0” |
| packages/capabilities/drawio-official | `@file-viewer/capability-drawio-official` | 3.3.3 | Apache-2.0 | none (not committed) |
| packages/capabilities/mermaid | `@file-viewer/capability-mermaid` | 3.3.3 | Apache-2.0 | LICENSE: “Apache License” |
| packages/capabilities/midi | `@file-viewer/capability-midi` | 3.3.3 | Apache-2.0 | none (not committed) |
| packages/capabilities/pdf-identity-repair | `@file-viewer/capability-pdf-identity-repair` | 3.3.3 | Apache-2.0 | LICENSE: “Apache License” |
| packages/capabilities/pptx-charts | `@file-viewer/capability-pptx-charts` | 3.3.3 | Apache-2.0 | none (not committed) |
| packages/capabilities/rtf | `@file-viewer/capability-rtf` | 3.3.3 | Apache-2.0 | none (not committed) |
| packages/capabilities/streaming-media | `@file-viewer/capability-streaming-media` | 3.3.3 | Apache-2.0 | none (not committed) |
| packages/capabilities/text-tools | `@file-viewer/capability-text-tools` | 3.3.3 | Apache-2.0 | none (not committed) |
| packages/renderers/3d | `@file-viewer/renderer-3d` | 3.3.3 | Apache-2.0 | LICENSE: “Apache License” |
| packages/renderers/archive | `@file-viewer/renderer-archive` | 3.3.3 | Apache-2.0 | LICENSE: “Apache License” |
| packages/renderers/binary | `@file-viewer/renderer-binary` | 3.3.3 | Apache-2.0 | LICENSE: “Apache License” |
| packages/renderers/cad | `@file-viewer/renderer-cad` | 3.3.3 | Apache-2.0 | LICENSE: “Apache License” |
| packages/renderers/chm | `@file-viewer/renderer-chm` | 3.3.3 | Apache-2.0 | LICENSE: “Apache License” |
| packages/renderers/data | `@file-viewer/renderer-data` | 3.3.3 | Apache-2.0 | LICENSE: “Apache License” |
| packages/renderers/design | `@file-viewer/renderer-design` | 3.3.3 | Apache-2.0 | LICENSE: “Apache License” |
| packages/renderers/dicom | `@file-viewer/renderer-dicom` | 3.3.3 | Apache-2.0 | LICENSE: “Apache License” |
| packages/renderers/doc | `@file-viewer/doc` | 3.3.3 | MIT | LICENSE: “MIT License” |
| packages/renderers/drawing | `@file-viewer/renderer-drawing` | 3.3.3 | Apache-2.0 | LICENSE: “Apache License” |
| packages/renderers/ebook | `@file-viewer/renderer-epub` | 3.3.3 | Apache-2.0 | LICENSE: “Apache License” |
| packages/renderers/eda | `@file-viewer/renderer-eda` | 3.3.3 | Apache-2.0 | LICENSE: “Apache License” |
| packages/renderers/eda-layout | `@file-viewer/eda-layout` | 3.3.3 | Apache-2.0 | LICENSE: “Apache License” |
| packages/renderers/eda-orcad | `@file-viewer/eda-orcad` | 3.3.3 | Apache-2.0 | LICENSE: “Apache License” |
| packages/renderers/email | `@file-viewer/renderer-email` | 3.3.3 | Apache-2.0 | LICENSE: “Apache License” |
| packages/renderers/geo | `@file-viewer/renderer-geo` | 3.3.3 | Apache-2.0 | LICENSE: “Apache License” |
| packages/renderers/geometry-engine | `@file-viewer/geometry-engine` | 3.3.3 | Apache-2.0 | LICENSE: “Apache License” |
| packages/renderers/hangul | `@file-viewer/renderer-hangul` | 3.3.3 | Apache-2.0 | LICENSE: “Apache License 2.0” |
| packages/renderers/image | `@file-viewer/renderer-image` | 3.3.3 | Apache-2.0 | LICENSE: “Apache License” |
| packages/renderers/iwork | `@file-viewer/renderer-iwork` | 3.3.3 | Apache-2.0 | LICENSE: “Apache License 2.0” |
| packages/renderers/media | `@file-viewer/renderer-media` | 3.3.3 | Apache-2.0 | LICENSE: “Apache License” |
| packages/renderers/mindmap | `@file-viewer/renderer-mindmap` | 3.3.3 | Apache-2.0 | LICENSE: “Apache License” |
| packages/renderers/ofd | `@file-viewer/renderer-ofd` | 3.3.3 | Apache-2.0 | LICENSE: “Apache License” |
| packages/renderers/pdf | `@file-viewer/renderer-pdf` | 3.3.3 | Apache-2.0 | LICENSE: “Apache License” |
| packages/renderers/pptx | `@file-viewer/pptx` | 3.3.3 | Apache-2.0 | LICENSE: “Apache License” |
| packages/renderers/presentation | `@file-viewer/renderer-presentation` | 3.3.3 | Apache-2.0 | LICENSE: “Apache License” |
| packages/renderers/presentation-ppt | `@file-viewer/renderer-ppt` | 3.3.3 | Apache-2.0 | LICENSE: “Apache License” |
| packages/renderers/presentation-pptx | `@file-viewer/renderer-pptx` | 3.3.3 | Apache-2.0 | LICENSE: “Apache License” |
| packages/renderers/signature | `@file-viewer/renderer-signature` | 3.3.3 | Apache-2.0 | LICENSE: “Apache License” |
| packages/renderers/spreadsheet | `@file-viewer/renderer-spreadsheet` | 3.3.3 | Apache-2.0 | LICENSE: “Apache License” |
| packages/renderers/text | `@file-viewer/renderer-text` | 3.3.3 | Apache-2.0 | LICENSE: “Apache License” |
| packages/renderers/typst | `@file-viewer/renderer-typst` | 3.3.3 | Apache-2.0 | LICENSE: “Apache License” |
| packages/renderers/word | `@file-viewer/renderer-word` | 3.3.3 | Apache-2.0 | LICENSE: “Apache License” |
| packages/renderers/wordperfect | `@file-viewer/renderer-wordperfect` | 3.3.3 | Apache-2.0 | LICENSE: “Apache License 2.0” |
| packages/presets/all | `@file-viewer/preset-all` | 3.3.3 | Apache-2.0 | LICENSE: “Apache License” |
| packages/presets/engineering | `@file-viewer/preset-engineering` | 3.3.3 | Apache-2.0 | LICENSE: “Apache License” |
| packages/presets/lite | `@file-viewer/preset-lite` | 3.3.3 | Apache-2.0 | LICENSE: “Apache License” |
| packages/presets/office | `@file-viewer/preset-office` | 3.3.3 | Apache-2.0 | LICENSE: “Apache License” |
| packages/presets/standard | `@file-viewer/preset-standard` | 3.3.3 | Apache-2.0 | LICENSE: “Apache License” |
| packages/presets/vite-plugin | `@file-viewer/vite-plugin` | 3.3.3 | Apache-2.0 | LICENSE: “Apache-2.0” |
| packages/tools/asset-installer | `@file-viewer/asset-installer` | 3.3.3 | Apache-2.0 | none (not committed) |
| packages/tools/assets-cad | `@file-viewer/assets-cad` | 3.3.3 | Apache-2.0 | none (not committed) |
| packages/tools/assets-chm | `@file-viewer/assets-chm` | 3.3.3 | Apache-2.0 | LICENSE: “Apache License” |
| packages/tools/assets-data | `@file-viewer/assets-data` | 3.3.3 | Apache-2.0 | none (not committed) |
| packages/tools/assets-design | `@file-viewer/assets-design` | 3.3.3 | Apache-2.0 | LICENSE: “Apache License” |
| packages/tools/assets-drawing | `@file-viewer/assets-drawing` | 3.3.3 | Apache-2.0 | none (not committed) |
| packages/tools/assets-hangul | `@file-viewer/assets-hangul` | 3.3.3 | Apache-2.0 | none (not committed) |
| packages/tools/assets-iwork | `@file-viewer/assets-iwork` | 3.3.3 | Apache-2.0 | none (not committed) |
| packages/tools/assets-model | `@file-viewer/assets-model` | 3.3.3 | Apache-2.0 | none (not committed) |
| packages/tools/assets-ppt | `@file-viewer/assets-ppt` | 3.3.3 | Apache-2.0 | none (not committed) |
| packages/tools/assets-standard | `@file-viewer/assets-standard` | 3.3.3 | Apache-2.0 | none (not committed) |
| packages/tools/assets-typst | `@file-viewer/assets-typst` | 3.3.3 | Apache-2.0 | none (not committed) |
| packages/tools/assets-wordperfect | `@file-viewer/assets-wordperfect` | 3.3.3 | Apache-2.0 | none (not committed) |
| packages/tools/cli | `@file-viewer/cli` | 3.3.3 | Apache-2.0 | LICENSE: “Apache License” |
| packages/tools/copy-assets | `file-viewer-copy-assets` | 3.3.3 | Apache-2.0 | LICENSE: “Apache License” |
| packages/components/jquery | `@file-viewer/jquery` | 3.3.3 | Apache-2.0 | none (not committed) |
| packages/components/jquery-full | `@file-viewer/jquery-full` | 3.3.3 | Apache-2.0 | none (not committed) |
| packages/components/react | `@file-viewer/react` | 3.3.3 | Apache-2.0 | none (not committed) |
| packages/components/react-full | `@file-viewer/react-full` | 3.3.3 | Apache-2.0 | none (not committed) |
| packages/components/react-legacy | `@file-viewer/react-legacy` | 3.3.3 | Apache-2.0 | none (not committed) |
| packages/components/react-legacy-full | `@file-viewer/react-legacy-full` | 3.3.3 | Apache-2.0 | none (not committed) |
| packages/components/svelte | `@file-viewer/svelte` | 3.3.3 | Apache-2.0 | none (not committed) |
| packages/components/svelte-full | `@file-viewer/svelte-full` | 3.3.3 | Apache-2.0 | none (not committed) |
| packages/components/vue2.6 | `@file-viewer/vue2.6` | 3.3.3 | Apache-2.0 | none (not committed) |
| packages/components/vue2.6-full | `@file-viewer/vue2.6-full` | 3.3.3 | Apache-2.0 | none (not committed) |
| packages/components/vue2.7 | `@file-viewer/vue2.7` | 3.3.3 | Apache-2.0 | none (not committed) |
| packages/components/vue2.7-full | `@file-viewer/vue2.7-full` | 3.3.3 | Apache-2.0 | none (not committed) |
| packages/components/vue3 | `@file-viewer/vue3` | 3.3.3 | Apache-2.0 | LICENSE: “Apache License” |
| packages/components/vue3-full | `@file-viewer/vue3-full` | 3.3.3 | Apache-2.0 | none (not committed) |
| packages/components/web | `@file-viewer/web` | 3.3.3 | Apache-2.0 | LICENSE: “Apache License” |
| packages/components/web-full | `@file-viewer/web-full` | 3.3.3 | Apache-2.0 | none (not committed) |
| packages/compat/create-file-viewer | `create-file-viewer` | 3.3.3 | Apache-2.0 | LICENSE: “Apache License” |
| packages/compat/file-viewer-cli | `file-viewer-cli` | 3.3.3 | Apache-2.0 | LICENSE: “Apache License” |
| packages/compat/msdoc-viewer | `msdoc-viewer` | 0.2.15 | MIT | LICENSE: “MIT License” |
| packages/compat/react | `@flyfish-group/file-viewer-react` | 3.3.3 | Apache-2.0 | none (not committed) |
| packages/compat/vue2.7 | `@flyfish-group/file-viewer` | 3.3.3 | Apache-2.0 | none (not committed) |
| packages/compat/vue3-scoped | `@flyfish-group/file-viewer3` | 3.3.3 | Apache-2.0 | LICENSE: “Apache License” |
| packages/compat/vue3-unscoped | `file-viewer3` | 3.3.3 | Apache-2.0 | none (not committed) |
| packages/compat/web | `@flyfish-group/file-viewer-web` | 3.3.3 | Apache-2.0 | none (not committed) |
| apps/component-demo | `@flyfish-group/file-viewer-component-demo` | 3.3.3 | (none — private) | none (not committed) |
| apps/docs-site | `@flyfish-group/file-viewer-docs` | 3.3.3 | (none — private) | none (not committed) |
| apps/official-site | `@flyfish-group/file-viewer-site` | 3.3.3 | (none — private) | none (not committed) |
| apps/viewer-demo | `@flyfish-group/file-viewer-demo` | 3.3.3 | (none — private) | none (not committed) |
| apps/web-demo | `@flyfish-group/file-viewer-web-demo` | 3.3.3 | (none — private) | none (not committed) |
| examples/vanilla-vite | `@file-viewer/example-vanilla-vite` | 0.1.0 | (none — private) | none (not committed) |
| examples/react-vite | `@file-viewer/example-react-vite` | 0.1.0 | (none — private) | none (not committed) |
| examples/vue3-vite | `@file-viewer/example-vue3-vite` | 0.1.0 | (none — private) | none (not committed) |

### 1a. `@flyfish-dev/cad-viewer` and `dwf-viewer` (not workspace packages: npm deps)

Neither is a workspace package. `@flyfish-dev/cad-viewer@0.8.3` is a `dependency` of the workspace package `@file-viewer/renderer-cad` (`packages/renderers/cad`, itself `Apache-2.0`). `dwf-viewer@0.6.7` is a transitive dep of cad-viewer (pnpm-lock), and is also pinned in `packages/build-support/cad-source-snapshots/3.3.3/package.json`, the corresponding-source snapshot that ships `LICENSES/AGPL-3.0.txt` and `LICENSES/GPL-3.0.txt`.

- `@flyfish-dev/cad-viewer@0.8.3`: tarball `package.json` `"license": "AGPL-3.0-only"`. Tarball `LICENSE`: `GNU AFFERO GENERAL PUBLIC LICENSE` / `Version 3, 19 November 2007` / `Copyright (C) 2007 Free Software Foundation, Inc. <http://fsf.org/>`. Tarball `NOTICE`: "@mlightcad/libredwg-web: DWG parsing in the browser through LibreDWG WebAssembly. License: GPL-3.0-only. … dwf-viewer 0.6.x … License: AGPL-3.0-only. The combined package is distributed under AGPL-3.0-only."
- `dwf-viewer@0.6.7`: tarball `package.json` `"license": "AGPL-3.0-only"`. Tarball `LICENSE`: `GNU AFFERO GENERAL PUBLIC LICENSE` / `Version 3, 19 November 2007`. Tarball `NOTICE`: "This project is licensed under the GNU Affero General Public License, version 3 only."
- Both match the expected AGPL-3.0-only.

## 1b. Native (C/C++) libraries compiled into WASM/asm.js inside dependencies

| host package | bundled library @ version (how identified) | licence (raw file) | upstream SHA / date |
|---|---|---|---|
| occt-import-js 0.0.23 | **OpenCascade (OCCT) 7.6.1**: git submodule `occt` → `d2abb6d844231cb8f29be6894440874a4700e4a5`, the same commit at the HEAD `41e4708` and at the npm `gitHead` `c2148e5`. That commit's message is "Update version up to 7.6.1". The submodule URL is git.dev.opencascade.org, read through the GitHub mirror Open-Cascade-SAS/OCCT. | `LICENSE_LGPL_21.txt`: "GNU LESSER GENERAL PUBLIC LICENSE / Version 2.1, February 1999"; plus `OCCT_LGPL_EXCEPTION.txt`: "Open CASCADE exception (version 1.0) to GNU LGPL version 2.1." (it allows object code that uses OCCT header material under terms of your choice, with a prominent notice). occt-import-js's own `LICENSE.md` is LGPL-2.1. | `d2abb6d8…` 2022-02-01 (mirror HEAD `53706f78` 2026-10-20) |
| heic2any 0.0.4 | **libheif**: emscripten build committed as `src/libheif.js` and inlined in `dist/heic2any.js` (the dist mentions `libheif.HeifDecoder`). No version string and no licence notice in the bundle. heic2any's README and LICENSE never mention libheif. | upstream strukturag/libheif `COPYING` at HEAD: "The library `libheif` is distributed under the terms of the GNU Lesser General Public License." The text that follows is LGPL **Version 3, 29 June 2007**. The bundled version is **unverified**, so the LGPL version that applied to it is unverified too. | libheif HEAD `12a259a5` 2026-10-26 (v1.23.7) |
| libarchive.js 2.0.2 | `lib/tools/docker/Dockerfile` at the HEAD `0989c1a6` (tag v2.0.2 is the same commit, and so is the npm gitHead) statically links **libarchive 3.7.2, zlib 1.3, xz/liblzma 5.2.11, bzip2 1.0.8 and OpenSSL 1.0.2s** into `dist/libarchive.wasm`. | libarchive v3.7.2 `COPYING`: "Except as listed below, all C sources … " use the BSD-2 licence, with listed public-domain files and some files under CC0/OpenSSL/Apache-2.0. zlib v1.3 `LICENSE`: the zlib licence ("provided 'as-is'"). xz v5.2.11 `COPYING`: "liblzma is in the public domain." bzip2-1.0.8 `LICENSE`: the bzip2 BSD-style licence (© Julian R Seward). OpenSSL_1_0_2s `LICENSE`: "double license … OpenSSL License and the original SSLeay license". libarchive.js's own LICENSE (MIT) mentions none of these. | libarchive `6468cd1f` 2023-09-12; zlib `09155eaa` 2023-08-18; xz `3e206e5c` 2023-03-18; bzip2 `6a8690fc` 2019-07-13 (gitlab.com/bzip2/bzip2; sourceware returned proxy 403); openssl `cf9143f9` 2019-05-28 |
| xmllint-wasm 5.3.0 | **libxml2 v2.13.8**: submodule `libxml2` → `eb0a2f2b91566384f378d870ffcd69c0a24162cc`, at the repo HEAD, which equals tag v5.3.0. That commit is "Release v2.13.8". | `Copyright`: MIT-style ("Permission is hereby granted, free of charge"; © 1998-2012 Daniel Veillard). xmllint-wasm `COPYING`: "This project is MIT licensed, just like libxml". | `eb0a2f2b` 2025-04-17 |
| sql.js 1.15.2 | **SQLite 3.54.0**: `Makefile` at HEAD `4f1c22d0`, which equals tag v1.15.2 and the npm gitHead, has `SQLITE_AMALGAMATION = sqlite-amalgamation-3540000`. | sqlite/sqlite `LICENSE.md`: "SQLite Is Public Domain". sql.js's own licence is MIT. | sqlite mirror HEAD `85814474` 2026-11-07 |
| @mlightcad/libredwg-web 0.7.9 (via cad-viewer) | **LibreDWG** fork. The repo mlightcad/libredwg-web is a LibreDWG tree with `bindings/javascript/wasm`. | repo `COPYING`: "GNU GENERAL PUBLIC LICENSE / Version 3, 29 June 2007". LibreDWG/libredwg `COPYING` is the same GPL v3. The tarball has no licence file; its package.json says `GPL-3.0`. | libredwg-web `82bf4577` 2026-11-08; LibreDWG mirror `2f68d32d` 2026-11-07 |
| @cornerstonejs/codec-* | cornerstonejs/codecs `.gitmodules`: CharLS, libjpeg-turbo, OpenJPH and openjpeg come from cornerstonejs forks; libjxl comes from libjxl/libjxl. Submodule pins were **not** resolved; the licences were read at the forks' HEADs. | CharLS: `LICENSE.md` "BSD 3-Clause License". libjpeg-turbo: `LICENSE.md` "three compatible BSD-style … IJG …". openjpeg: `LICENSE` "2-clauses BSD License". OpenJPH: `LICENSE` "BSD 2-Clause License". libjxl: `LICENSE` is BSD-3-style. The codecs repo's own LICENSE is MIT. | charls `38d95d00` 2023-01-05; libjpeg-turbo `dc4a93fa` 2022-12-07; openjpeg `2d606701` 2022-10-19; openjph `0da149a8` 2023-11-07; libjxl `1e812edf` 2026-11-06 |
| web-ifc 0.0.77 | Its own C++ engine (MPL-2.0). It also uses third-party C++ deps through `src/cpp/CMakeLists.txt` FetchContent at the HEAD: fast_float, tinynurbs, TinyCppTest, glm, earcut.hpp, CDT, spdlog, stduuid and unordered_dense. | **unverified**: their licences were not fetched. | web-ifc HEAD `bbb34120` 2026-11-05 |
| @myriaddreamin/typst-ts-* 0.7.0 | Rust code compiled to WASM (the typst compiler and renderer). | typst.ts repo `LICENSE`: Apache-2.0. The Rust crate dependency licences are **unverified**. | `52dd6cfb` 2026-10-30 |
| @paged-media/introspect-wasm 0.73.4 | Rust or C compiled to WASM (`paged_introspect_wasm_bg.wasm`). It has no repository. | Only `"MPL-2.0 OR LicenseRef-PMEL"` in package.json. The file-viewer copy `packages/build-support/licenses/introspect-wasm-0.73.4-LICENSE.md` says "**paged** … is **dual-licensed** … Mozilla Public License, version 2.0 … or the Paged Media Enterprise License (PMEL) — a commercial option available from And The Next GmbH". The upstream source is **unverified**. | unverified |

## 1c. AOSP PacketVideo MPEG-4 Part 2 decoder (vendored in file-viewer)

- **Location:** `packages/renderers/media/vendor/aosp-mp4v/`:
  - build inputs: `AOSP-NOTICE.txt`, `EMSCRIPTEN-LICENSE.txt`, `README.md`, `SOURCE.json`, `wrapper.cpp`, `log/log.h`;
  - built runtime: `packages/renderers/media/src/vendor/mp4v/` (`mp4v-decoder.wasm`, `mp4v-decoder.mjs`, `AOSP-NOTICE.txt`, `EMSCRIPTEN-LICENSE.txt`, `TOOLCHAIN_THIRD_PARTY_LICENSES.json`).
- **What is not vendored:** the decoder C sources themselves. `README.md` says the build "downloads one checksum-pinned AOSP subdirectory archive".
- **Last file-viewer commit touching it:** `3e8e8b0cda18e76ffb76ff4aa6af1bb0beaac0c9`, 2026-10-27.
- **`AOSP-NOTICE.txt` header lines:**
  ```
     Copyright (c) 2005-2008, The Android Open Source Project

     Licensed under the Apache License, Version 2.0 (the "License");
     you may not use this file except in compliance with the License.
  ```
  The full Apache-2.0 text follows.
- **`SOURCE.json`:** `"upstream": "https://android.googlesource.com/platform/frameworks/av"`, `"commit": "e2f098935447ca4945946de5cb69db843fe3f003"`, `"path": "media/module/codecs/m4v_h263/dec"`, `"license": "Apache-2.0"`, `"emscriptenVersion": "6.0.6"`, `"toolchainLicense": "MIT OR NCSA"`, `"forbiddenLicenses": ["LGPL","GPL","AGPL"]`.
- **The two file-viewer files:** `wrapper.cpp` and `log/log.h` start with `// SPDX-License-Identifier: Apache-2.0`.
- **Upstream is unverified:** `git fetch` of android.googlesource.com got "CONNECT tunnel failed, response 403" from the proxy.

## 3. Patches (`patches/`, wired via `pnpm-workspace.yaml` `patchedDependencies`)

| patch file | package@version | what it patches | licence statement in patch? |
|---|---|---|---|
| `patches/illustrator-pgf@0.1.0.patch` (321 lines) | `illustrator-pgf@0.1.0` (MIT, flyfish-dev's own package) | `dist/src/{ast,container,lexer,semantic}.js`. It treats PostScript's executable `[` mark inside Illustrator `Bd`/`BD` gradient resource blocks as a name rather than an array start. `parseCompressionMarker` takes candidate offsets (it finds `%AI…_CompressedData` / `%AI24_ZStandard_Data` markers beyond offset 0). It also keeps the RGB alternate of AI5+ custom colours. | No: no licence, SPDX or copyright line. |
| `patches/pdfjs-dist@5.4.624.patch` (123 lines) | `pdfjs-dist@5.4.624` (Apache-2.0) | `legacy/build/pdf.worker.mjs`, two "File Viewer modification" comments. (1) It keeps composite CMap decoding when a CID font has no descriptor (`if (isType3Font \|\| composite)`). (2) A cancelled worker initialisation returns quietly instead of throwing `ensureNotTerminated()` / rejecting detached promises; `pdfManagerReady` is rewritten as async with `terminated` checks. | No licence line. It does carry "File Viewer modification:" comments, which serve as Apache-2.0 §4(b) change notices. |

## 4. Flags

**Copyleft / non-OSI / no-licence**
- **AGPL-3.0-only:**
  - `@flyfish-dev/cad-viewer@0.8.3` is a direct dependency of `@file-viewer/renderer-cad`, which is Apache-2.0. Its NOTICE says the combined package is AGPL-3.0-only.
  - `dwf-viewer@0.6.7` comes in transitively through cad-viewer.
  - The repo ships an AGPL/GPL corresponding-source snapshot in `packages/build-support/cad-source-snapshots/3.3.3/`.
- **GPL-3.0:** `@mlightcad/libredwg-web@0.7.9` (transitive through cad-viewer) is a LibreDWG WASM build:
  - the tarball has no LICENSE file;
  - package.json says `"GPL-3.0"`, a deprecated SPDX id with no -only/-or-later;
  - the upstream `COPYING` is GPL v3.
- **LGPL-2.1:** `occt-import-js@0.0.23`, with its LICENSE.md. It bundles **OCCT 7.6.1** under **LGPL-2.1 + Open CASCADE exception v1.0**.
- **LGPL, version unverified:** `heic2any@0.0.4` (package.json MIT, LICENSE MIT) embeds an emscripten build of **libheif**, which upstream licenses as LGPL (LGPL v3 text at the current HEAD).
  - The bundle has no notice for it.
  - The bundled libheif version could not be identified.
  - Package.json and LICENSE say MIT, but the binary inside is LGPL code.
- **Dual with a GPL option:** `jszip@3.10.2`, `"(MIT OR GPL-3.0-or-later)"`. MIT can be chosen.
- **Dual with a commercial option (non-OSI LicenseRef):** `@paged-media/introspect-wasm@0.73.4`, `"MPL-2.0 OR LicenseRef-PMEL"`.
  - PMEL is a commercial licence from And The Next GmbH.
  - There is no LICENSE in the tarball and no repository field.
- **Non-standard MIT, watermark clause:** `bpmn-js@18.31.1`. Its package.json says `"SEE LICENSE IN LICENSE"`. LICENSE adds, verbatim:
  > The source code responsible for displaying the bpmn.io project watermark that
  > links back to https://bpmn.io as part of rendered diagrams MUST NOT be
  > removed or changed. When this software is being used in a website or application,
  > the watermark must stay fully visible and not visually overlapped by other elements.
- **MPL-2.0 (file-level copyleft):** `web-ifc@0.0.77`. `dompurify@3.4.17` is `(MPL-2.0 OR Apache-2.0)`, so Apache-2.0 can be chosen.
- **OFL-1.1 (font):** `@fontsource-variable/noto-sans-sc@5.3.1`.
- **OpenSSL/SSLeay dual licence, and end-of-life:** **OpenSSL 1.0.2s** (May 2019) is statically linked into `libarchive.js@2.0.2`'s WASM, per its Dockerfile.
- **No licence file anywhere:** `@ljheee/xmind-parser@1.1.3`.
  - Package.json says `"MIT"` and its `files` lists `LICENSE`.
  - No LICENSE is in the tarball or at the upstream HEAD `60c3e0d9`.
  - The upstream repo also commits `node_modules/` and `.DS_Store`.
- **No LICENSE in the tarball, upstream licence found and matching:**
  - `@myriaddreamin/typst-ts-renderer` and `typst-ts-web-compiler` (Apache-2.0);
  - `@thatopen/components` and `@thatopen/fragments` (MIT, upstream `LICENSE.md`);
  - `saxes` (ISC);
  - `@cornerstonejs/core`, `metadata`, `utils` and `codec-openjpeg` (MIT, cornerstone3D/codecs repo LICENSE).

**package.json vs LICENSE / upstream disagreements**
- `pako@2.2.0`: package.json says `"(MIT AND Zlib)"`, but the tarball LICENSE has only the MIT text (21 lines). The Zlib terms are not in the LICENSE file.
- `bpmn-js@18.31.1`: package.json `"SEE LICENSE IN LICENSE"` versus MIT plus the watermark clause (above). It is not an SPDX id.
- `heic2any@0.0.4`: MIT everywhere, but it embeds LGPL libheif (above).
- `libarchive.js@2.0.2`: the LICENSE is MIT only and doesn't mention the bundled libarchive (BSD-2), zlib, liblzma (public domain), bzip2 or OpenSSL (OpenSSL/SSLeay).
- `@cornerstonejs/codec-libjpeg-turbo-8bit@1.2.7`:
  - package.json says `"ISC"`;
  - the tarball has no LICENSE;
  - the codecs repo LICENSE is MIT;
  - the libjpeg-turbo inside is IJG/BSD-3/zlib-style.
- `@cornerstonejs/codec-libjxl@1.1.1`: package.json says `"BSD-3-Clause"` (which matches libjxl's licence), the tarball has no LICENSE, and the codecs repo LICENSE is MIT.
- `@cornerstonejs/codec-charls@1.2.7`: package.json and LICENSE say MIT, but the CharLS inside is BSD-3-Clause. Whether the tarball carries a CharLS notice was not checked.
- `@mlightcad/libredwg-web@0.7.9`: `"GPL-3.0"` (a deprecated SPDX id) with no LICENSE file in the tarball.
- In-repo: 44 of the 97 workspace package dirs have no committed LICENSE file. 36 of those declare `Apache-2.0` or `MIT`; the other 8 are the private apps and examples (see section 2). The files may be staged at publish; this wasn't checked.

**Deprecated / abandoned**
- **npm `deprecated`:** none. Raw `npm view --json` showed no `deprecated` field for any resolved version.
- **Archived:** unknown for every repo (no GitHub API access). No README declares the project archived or unmaintained.
- **Last commit more than 2 years ago (before 2024-11-08):**
  - `plantuml-encoder` 1.4.0 (2019-08-09)
  - `events` 3.3.0 (2021-02-27)
  - `pdf-lib` 1.17.1 (2021-11-11)
  - `rtf.js` 3.0.9 (2022-10-09)
  - `cfb` 1.2.2 (2022-11-10)
  - `tinycolor2` 1.6.0 (2023-02-03)
  - `dxf-parser` 1.1.2 (2023-03-15)
  - `heic2any` 0.0.4 (2023-03-29)
  - `@tonejs/midi` 2.0.28 (2023-05-31)
  - `@webtoon/psd` 0.4.0 (2023-07-14)
  - `dicom-parser` 1.8.21 (2023-10-17)
  - `roughjs` 4.6.6 (2023-11-19)
  - `libarchive.js` 2.0.2 (2024-01-27)
  - `fzstd` 0.1.1 (2024-03-15)
- **Borderline:** `occt-import-js` (last commit 2024-12-03, about 23 months).
- **Old bundled native code:**
  - OCCT 7.6.1 (2022-02);
  - OpenSSL 1.0.2s (2019, end-of-life line);
  - bzip2 1.0.8 (2019);
  - xz 5.2.11 (2023-03, the 5.2 line);
  - the cornerstone codec forks (2022-2023 HEADs);
  - the libheif in heic2any (version unknown, package last published 2023-03).

**Unverified rows**
- `@file-viewer/docx@0.3.39`: upstream `github.com/flyfish-dev/docxjs` clone failed with an auth prompt (private or missing). The SHA, date and liveness are unverified; the licence (Apache-2.0, LICENSE Apache) comes from the tarball.
- `keynote-archives@2.0.1`: no `repository` field, so the upstream, SHA and date are unverified. The licence is MIT (package.json and LICENSE).
- `@paged-media/introspect-wasm@0.73.4`: no repository and no LICENSE in the tarball. The upstream, SHA and date are unverified.
- `@webtoon/psd@0.4.0`: the repo URL was inferred from the `homepage` (webtoon.github.io/psd → github.com/webtoon/psd), because package.json has no `repository`.
- AOSP decoder upstream (android.googlesource.com, proxy 403). The bundled libheif version. web-ifc's C++ FetchContent deps. Rust crate deps of typst-ts. Cornerstone codec submodule pins: licences were read at the fork HEADs, not at the pinned commits.
