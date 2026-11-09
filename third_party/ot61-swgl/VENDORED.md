<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# `ot61-swgl`: the vendored SWGL closure (OT-61 rung 4)

Upstream **SWGL**, WebRender's software OpenGL, and everything it needs to build, pinned here as sources the
way C2.3b's closure is (`third_party/ot61-browser/VENDORED.md`). The design is
`docs/RESEARCH-2026-07-22-ot61-swgl-browser-floor-spike-FINDINGS.md` (Janus) and ADR-078 §2.2.

## Layout and rules

- `vendor/<crate>-<version>/`: each crate exactly as published on crates.io, unpacked by `cargo vendor`. The
  `.cargo-checksum.json` beside each holds the SHA-256 of the published `.crate` and of every file.
- Only `userland/swgl-smoke` uses this tree, through `[patch.crates-io]` path entries. The CI job
  (`.github/workflows/ot61-closure.yml`, `swgl-build`) fails if any crate resolves to crates.io instead.
- **The first build ran on the CI runners**, per J.c.'s C2.3b ruling on third-party build code. Two build
  scripts here run at build time: SWGL's translates WebRender's GLSL shaders to C++ (through `glsl-to-cxx`) and
  compiles `gl.cc`; gleam's generates GL bindings (`gl_generator`).
- SWGL's C++ is compiled for the target against neo-crt (C3a, #261): xtask sets `cc`'s per-target variables
  (`swgl_cc_env`).
- `patched/<crate>-<version>/`: a crate we change. It is checked like first-party code, so each of its `.rs` and
  `.toml` files starts with an SPDX line naming the upstream licence (C2.3b's precedent). Those lines aren't in
  the `.patch` files (`diff -I SPDX-License-Identifier`), so each `.patch` is the change alone and applies
  upstream.

## The patch: SWGL's delayed clear (`patched/swgl-0.70.0`)

**An upstream bug, found by S1's hand-laid oracle on its first on-target run.** A full-surface clear is
delayed: SWGL records the value and a per-row "cleared" bitmask. A later scissored clear calls `force_clear` with a
skip rectangle. Upstream applies the skip's **x-range to every not-yet-cleared row in the same 32-row mask word**,
then marks the whole word cleared. Any row outside the skip's y-range keeps a hole where the skip's columns are, and
nothing ever fills it. If the scissor is full-width, those rows aren't cleared at all.
- In S1, a red full clear, then a blue 3x2 scissored clear at rows 1–2: rows 0, 3 and 4 kept their fill bytes at
  x 1–3. GL requires red there.
- The code is the same in Firefox's tree today: `gfx/wr/swgl/src/gl.cc` `force_clear` at `9fcf3a16`, 2026-10-30.

**The fix** (`swgl-0.70.0-force-clear.patch`, `src/gl.cc` only): each run of uncleared rows is split at the skip's
y-range. Rows before and after it are cleared full-width, and only rows inside it skip the scissored columns. The
bookkeeping (`rows[i] = ~0U`, `delay_clear`) is unchanged. With the patch, S1 matches every byte. Without it, the
same check fails, which is the discriminator. It should go upstream (Bugzilla, Core :: Graphics: WebRender); the
patch is written to apply there.

## The patch: WebRender's frame timestamp (`patched/webrender-0.70.0`)

`FrameStamp::first`/`advance` call `SystemTime::now()`. The browser bundle has no wall clock, and std's `SystemTime`
aborts there (C3a THINK §6 item 3) rather than invent a time. Nothing reads the value back: `FrameStamp::time` has
no caller in 0.70.0, and it only has to differ from `UNIX_EPOCH` for a debug check. On
`target_os = "neoplutarch"`, `frame_time()` records `UNIX_EPOCH` plus the monotonic time since the first frame,
plus 1 ns. Other targets are unchanged. Diff: `webrender-0.70.0-frame-time.patch`.

## Patched and stubbed crates

- **`patched/wr_glyph_rasterizer-0.70.1`** (rung 4 S2): a `target_os = "neoplutarch"` font backend,
  `src/platform/neoplutarch/font.rs`, on skrifa (outlines, metrics) and zeno (coverage), plus its two `cfg` lines
  in `src/lib.rs` and two target dependencies in `Cargo.toml`. Upstream's backends are FreeType (unix), Core Text
  and DirectWrite; our target is none of them, so without this the crate doesn't compile. The geometry follows the
  unix backend step for step (see the file's header). The full diff against the published crate is
  `wr_glyph_rasterizer-0.70.1-neoplutarch.patch`. The new file carries upstream's MPL-2.0 header.
- **`stubs/winapi-0.3.9`**: an empty crate with winapi's real name, version, licence and feature table. winapi is
  reached only through WebRender's Windows paths and `cfg(windows)` dependencies, never for our target or build
  host. The real crate pulls 106 MB of Windows import libraries (`winapi-*-pc-windows-gnu`).

`gen-patch-table.py` (from C2.4's `third_party/ot61-browser`) prints a smoke's `[patch.crates-io]` table from this
tree.

## The crates (checked 2026-10-31)

Generated from each crate's own `Cargo.toml` and `.cargo_vcs_info.json`. The S1 closure (`swgl-smoke`) is the first
18; `webrender-smoke` uses all of them. Every licence is MPL-2.0, MIT, Apache-2.0, BSD-3-Clause, Zlib or
Unlicense-or-MIT, with three notes: `r-efi` is "MIT OR Apache-2.0 OR LGPL-2.1-or-later" and is taken under MIT;
`unicode-ident` adds the Unicode-3.0 data licence (as in C2.3b's closure; a build-host proc-macro dependency); and
none is GPL-only.

| Crate | Licence | Repository | Upstream commit |
|---|---|---|---|
| allocator-api2 0.2.21 | MIT OR Apache-2.0 | https://github.com/zakarumych/allocator-api2 | `63cd7fcc2f88` |
| app_units 0.7.8 | MPL-2.0 | https://github.com/servo/app_units | `2e4e0e0ad8b4` |
| autocfg 1.5.1 | Apache-2.0 OR MIT | https://github.com/cuviper/autocfg | `2799b09c24e6` |
| bincode 1.3.3 | MIT | https://github.com/servo/bincode | `c44b5e364e70` |
| bitflags 1.3.2 | MIT/Apache-2.0 | https://github.com/bitflags/bitflags | `ed185cfb1c44` |
| bitflags 2.13.2 | MIT OR Apache-2.0 | https://github.com/bitflags/bitflags | `80ce9b545acb` |
| build-parallel 0.1.2 | MIT/Apache-2.0 | https://github.com/jrmuizel/build-parallel | `bdab82360381` |
| bytemuck 1.25.2 | Zlib OR Apache-2.0 OR MIT | https://github.com/Lokathor/bytemuck | `f363643e951a` |
| bytemuck_derive 1.12.1 | Zlib OR Apache-2.0 OR MIT | https://github.com/Lokathor/bytemuck | `839ee557dd9b` |
| byteorder 1.5.0 | Unlicense OR MIT | https://github.com/BurntSushi/byteorder | `ec068eefa042` |
| cc 1.2.63 | MIT OR Apache-2.0 | https://github.com/rust-lang/cc-rs | `b49589a6a1d2` |
| cfg-if 1.0.5 | MIT OR Apache-2.0 | https://github.com/rust-lang/cfg-if | `8b0a2b0fc1fa` |
| core-foundation 0.9.4 | MIT OR Apache-2.0 | https://github.com/servo/core-foundation-rs | `795398aa3775` |
| core-foundation-sys 0.8.7 | MIT OR Apache-2.0 | https://github.com/servo/core-foundation-rs | `652bab0a62d8` |
| core-graphics 0.23.2 | MIT OR Apache-2.0 | https://github.com/servo/core-foundation-rs | `07be553c3f1d` |
| core-graphics-types 0.1.3 | MIT OR Apache-2.0 | https://github.com/servo/core-foundation-rs | `d4ce710182f1` |
| core-text 20.1.0 | MIT OR Apache-2.0 | https://github.com/servo/core-foundation-rs | `5be8e2cd8f6c` |
| crossbeam-channel 0.5.17 | MIT OR Apache-2.0 | https://github.com/crossbeam-rs/crossbeam | `2920c9842902` |
| crossbeam-deque 0.8.8 | MIT OR Apache-2.0 | https://github.com/crossbeam-rs/crossbeam | `2920c9842902` |
| crossbeam-epoch 0.9.21 | MIT OR Apache-2.0 | https://github.com/crossbeam-rs/crossbeam | `2920c9842902` |
| crossbeam-utils 0.8.23 | MIT OR Apache-2.0 | https://github.com/crossbeam-rs/crossbeam | `2920c9842902` |
| derive_more 2.1.1 | MIT | https://github.com/JelteF/derive_more | `f7bb41ac054c` |
| derive_more-impl 2.1.1 | MIT | https://github.com/JelteF/derive_more | `f7bb41ac054c` |
| dwrote 0.11.5 | MPL-2.0 | https://github.com/servo/dwrote-rs | `5bbe910f7156` |
| either 1.19.0 | MIT OR Apache-2.0 | https://github.com/rayon-rs/either | `1e4326df5972` |
| etagere 0.2.15 | MIT/Apache-2.0 | https://github.com/nical/etagere | `f093428295b0` |
| euclid 0.22.14 | MIT OR Apache-2.0 | https://github.com/servo/euclid | `60f2bd96deec` |
| find-msvc-tools 0.1.15 | MIT OR Apache-2.0 | https://github.com/rust-lang/cc-rs | `ffbb10da608f` |
| foldhash 0.2.0 | Zlib | https://github.com/orlp/foldhash | `8f878c636fda` |
| font-types 0.12.7 | MIT OR Apache-2.0 | https://github.com/googlefonts/fontations | `ef666662f598` |
| foreign-types 0.5.0 | MIT/Apache-2.0 | https://github.com/sfackler/foreign-types | `c5a90ed3f39d` |
| foreign-types-macros 0.2.4 | MIT/Apache-2.0 | https://github.com/sfackler/foreign-types | `2d5077646c44` |
| foreign-types-shared 0.3.1 | MIT/Apache-2.0 | https://github.com/sfackler/foreign-types | `bdce800c23ce` |
| freetype 0.8.0 | MIT OR Apache-2.0 | https://github.com/servo/rust-freetype | `cdd3cd616be8` |
| getrandom 0.4.3 | MIT OR Apache-2.0 | https://github.com/rust-random/getrandom | `5e7cd5733536` |
| gl_generator 0.14.0 | Apache-2.0 | https://github.com/brendanzab/gl-rs/ | `ea503e8d5fb6` |
| gleam 0.15.1 | MIT OR Apache-2.0 | https://github.com/servo/gleam | `e7b3f3296c70` |
| glsl 6.0.2 | BSD-3-Clause | https://github.com/phaazon/glsl | `a01a2ac9ce5f` |
| glsl-to-cxx 0.70.0 | MPL-2.0 |  | `2e341fa14234` |
| glslopt 0.1.14 | MIT | https://github.com/jamienicol/glslopt-rs | `68019f02a143` |
| hermit-abi 0.5.3 | MIT OR Apache-2.0 | https://github.com/hermit-os/hermit-rs | `c0b97d12a2f1` |
| jobserver 0.1.35 | MIT OR Apache-2.0 | https://github.com/rust-lang/jobserver-rs | `85ba04e3cf89` |
| khronos_api 3.1.0 | Apache-2.0 | https://github.com/brendanzab/gl-rs/ | `f150967b1c44` |
| lazy_static 1.5.1 | MIT OR Apache-2.0 | https://github.com/rust-lang-nursery/lazy-static.rs | `4c1b9a170c15` |
| libc 0.2.191 | MIT OR Apache-2.0 | https://github.com/rust-lang/libc | `ccc2ff4aaab9` |
| linked_list_allocator 0.10.6 | Apache-2.0/MIT | https://github.com/phil-opp/linked-list-allocator | `a1d90498ff82` |
| log 0.4.35 | MIT OR Apache-2.0 | https://github.com/rust-lang/log | `a46450ba7632` |
| malloc_buf 0.0.6 | MIT | https://github.com/SSheldon/malloc_buf | (crates.io checksum) |
| malloc_size_of_derive 0.1.3 | MIT OR Apache-2.0 | https://github.com/bholley/malloc_size_of_derive | `943920641982` |
| memchr 2.8.3 | Unlicense OR MIT | https://github.com/BurntSushi/memchr | `5fdb40c054e1` |
| minimal-lexical 0.2.1 | MIT/Apache-2.0 | https://github.com/Alexhuszagh/minimal-lexical | `e997c46656eb` |
| nom 7.1.3 | MIT | https://github.com/Geal/nom | `869f8972a438` |
| num-traits 0.2.19 | MIT OR Apache-2.0 | https://github.com/rust-num/num-traits | `7ec3d41d39b2` |
| num_cpus 1.17.0 | MIT OR Apache-2.0 | https://github.com/seanmonstar/num_cpus | `342af76b4863` |
| objc 0.2.7 | MIT | http://github.com/SSheldon/rust-objc | `858f92f7c11d` |
| once_cell 1.21.4 | MIT OR Apache-2.0 | https://github.com/matklad/once_cell | `80fe900b21f6` |
| peek-poke 0.3.0 | MIT/Apache-2.0 | https://github.com/servo/webrender | `5cc92bce8e5a` |
| peek-poke-derive 0.3.0 | MIT OR Apache-2.0 | https://github.com/servo/webrender | `5cc92bce8e5a` |
| plane-split 0.18.0 | MPL-2.0 | https://github.com/servo/plane-split | `8b20a38ddbf0` |
| proc-macro2 1.0.107 | MIT OR Apache-2.0 | https://github.com/dtolnay/proc-macro2 | `ed8a5497669c` |
| quote 1.0.47 | MIT OR Apache-2.0 | https://github.com/dtolnay/quote | `723dcb47d3f0` |
| r-efi 6.0.0 | MIT OR Apache-2.0 OR LGPL-2.1-or-later | https://github.com/r-efi/r-efi | `7e1b0322d31d` |
| rayon 1.12.0 | MIT OR Apache-2.0 | https://github.com/rayon-rs/rayon | `c9ced185ae35` |
| rayon-core 1.13.0 | MIT OR Apache-2.0 | https://github.com/rayon-rs/rayon | `7af20d7692d5` |
| read-fonts 0.46.0 | MIT OR Apache-2.0 | https://github.com/googlefonts/fontations | `47a301615fd9` |
| rustc-hash 2.1.3 | Apache-2.0 OR MIT | https://github.com/rust-lang/rustc-hash | `c13e7ccca705` |
| rustc_version 0.4.1 | MIT OR Apache-2.0 | https://github.com/djc/rustc-version-rs | `eeca449cca83` |
| semver 1.0.28 | MIT OR Apache-2.0 | https://github.com/dtolnay/semver | `7625c7aa3f0e` |
| serde 1.0.229 | MIT OR Apache-2.0 | https://github.com/serde-rs/serde | `7fc3b4c30c94` |
| serde_bytes 0.11.19 | MIT OR Apache-2.0 | https://github.com/serde-rs/bytes | `34f3c7d44271` |
| serde_core 1.0.229 | MIT OR Apache-2.0 | https://github.com/serde-rs/serde | `7fc3b4c30c94` |
| serde_derive 1.0.229 | MIT OR Apache-2.0 | https://github.com/serde-rs/serde | `7fc3b4c30c94` |
| shlex 2.0.1 | MIT OR Apache-2.0 | https://github.com/comex/rust-shlex | `e82b1411beb7` |
| skrifa 0.49.0 | MIT OR Apache-2.0 | https://github.com/googlefonts/fontations | `47a301615fd9` |
| smallvec 1.16.4 | MIT OR Apache-2.0 | https://github.com/servo/rust-smallvec | `eab77462eda2` |
| stable_deref_trait 1.2.1 | MIT OR Apache-2.0 | https://github.com/storyyeller/stable_deref_trait | `30002b4228f7` |
| svg_fmt 0.4.5 | MIT/Apache-2.0 | https://github.com/nical/rust_debug | `93f414cd572b` |
| swgl 0.70.0 **patched** | MPL-2.0 |  | `2e341fa14234` |
| syn 2.0.119 | MIT OR Apache-2.0 | https://github.com/dtolnay/syn | `3295f9e98417` |
| syn 3.0.9 | MIT OR Apache-2.0 | https://github.com/dtolnay/syn | `7c44a5565b44` |
| synstructure 0.13.2 | MIT | https://github.com/mystor/synstructure | `91ead072fa43` |
| synstructure 0.14.0 | MIT | https://github.com/mystor/synstructure | `30cfafe36e42` |
| topological-sort 0.1.0 | MIT OR Apache-2.0 | https://github.com/gifnksm/topological-sort-rs | (crates.io checksum) |
| tracy-rs 0.1.2 | MPL-2.0 |  | `8066aa2e452e` |
| unicode-ident 1.0.27 | (MIT OR Apache-2.0) AND Unicode-3.0 | https://github.com/dtolnay/unicode-ident | `c1ac3afb721f` |
| webrender 0.70.0 | MPL-2.0 | https://github.com/servo/webrender | `2e341fa14234` |
| webrender_api 0.70.0 | MPL-2.0 | https://github.com/servo/webrender | `2e341fa14234` |
| webrender_build 0.70.0 | MPL-2.0 | https://github.com/servo/webrender | `2e341fa14234` |
| winapi 0.3.9 **stub** | MIT/Apache-2.0 |  | (crates.io checksum) |
| wio 0.2.2 | MIT/Apache-2.0 | https://github.com/retep998/wio-rs | (crates.io checksum) |
| wr_glyph_rasterizer 0.70.1 **patched** | MPL-2.0 |  | `d399b904fdce` |
| wr_malloc_size_of 0.2.2 | MIT OR Apache-2.0 |  | `bdbf23ea2a34` |
| xml-rs 0.8.29 | MIT | https://github.com/kornelski/xml-rs | `33cdf417b112` |
| yoke 0.8.4 | Unicode-3.0 | https://github.com/unicode-org/icu4x | `031d9e89f6a3` |
| yoke-derive 0.8.4 | Unicode-3.0 | https://github.com/unicode-org/icu4x | `a59ab860d4bd` |
| zeitstempel 0.2.2 | MPL-2.0 | https://github.com/badboy/zeitstempel | `ba1513d78e2d` |
| zeno 0.3.3 | Apache-2.0 OR MIT | https://github.com/dfrg/zeno | `698399439f80` |
| zerofrom 0.1.8 | Unicode-3.0 | https://github.com/unicode-org/icu4x | `e9575f7e4b5a` |
| zerofrom-derive 0.1.8 | Unicode-3.0 | https://github.com/unicode-org/icu4x | `ce235d80cf7c` |

## Fonts (`fonts/`)

| Font | Licence | Upstream | Commit | SHA-256 |
|---|---|---|---|---|
| Roboto Slab (variable, `wght`), `fonts/robotoslab/RobotoSlab-wght.ttf` | Apache-2.0 (`fonts/robotoslab/LICENSE.txt`; `METADATA.pb` says `APACHE2`) | github.com/google/fonts `apache/robotoslab/RobotoSlab[wght].ttf` | `6c1dbe27dcf7d817d4b62cec033ba77fa8e2a5ec` (2026-10-30) | `786ae192477447d33c6672c3055fba7cbfe45184c9a79e77a14f15716ca05b16` |

Fetched with `git` (sparse checkout), bytes unchanged; only the file name drops the brackets. Most google/fonts
families are OFL, which the standing list doesn't name, so the test font comes from its `apache/` directory.
`webrender-smoke` uses it for the S2 text item; the smoke's text oracle uses values read from its `head`, `hhea`,
`hmtx` and `cmap` tables by a separate parser, not by skrifa.
