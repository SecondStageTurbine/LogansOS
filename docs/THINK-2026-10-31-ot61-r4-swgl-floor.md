<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK — OT-61 rung 4: the SWGL software-render floor, S1 to S3

**Seat:** D. **Date:** 2026-10-31. **Ladder:** `Plans/DISPATCH-2026-10-30-seat-d-browser.md` rung 4; ADR-078 §2.2;
`docs/RESEARCH-2026-07-22-ot61-swgl-browser-floor-spike-FINDINGS.md` (Janus), whose S0–S5 table this follows.
**Branch:** `claude/ot61-r4-swgl` (#262), stacked on C3a (#261), whose C/C++ runtime SWGL compiles against.

## 1. S0 + S1 (this branch)

- **The closure.** Upstream `swgl`, `glsl-to-cxx` and `webrender_build` 0.70.0 are published from exactly the spike's
  pin (`2e341fa1`; swgl's files are identical to the git tree), with gleam 0.15.1 (the spike's `e7b3f329`), glsl 6,
  gl_generator and their dependencies. All are vendored pristine under `third_party/ot61-swgl` (licences:
  MPL-2.0, MIT, Apache-2.0, BSD-3-Clause, Unlicense; `VENDORED.md`).
- **The build.** SWGL's `build.rs` translates every WebRender shader to C++ (`glsl-to-cxx`) and compiles them with
  `gl.cc` through the `cc` crate, using `cc`'s defaults for the target. xtask's `swgl_cc_env` replaces those
  defaults through `cc`'s per-target variables:
  - clang, with no default flags;
  - our target flags and neo-crt's headers (libc++, then musl, then clang's own);
  - `CXXSTDLIB` empty, since libc++ comes from neo-crt.

  Nothing in SWGL is patched. The first build runs on the CI runners (J.c.'s C2.3b ruling), and the CI job fails if
  any crate resolves to crates.io.
- **The check (S1, the spike's "first prototype milestone").** `swgl-smoke` hands SWGL a caller-owned buffer: 7x5
  pixels, stride 36 (padded), and 16-byte canaries on both sides. Through gleam it clears red, then a scissored
  3x2 block blue, then a 2-column strip green with alpha 0. After `get_color_buffer(0, flush = true)` it compares
  every byte with a hand-laid pattern:
  - BGRA memory order, the `tools/swgl-atrium-oracle` contract;
  - row y at `y * stride`, with no flip;
  - padding and canaries untouched.

  Every colour component is 0.0 or 1.0, so no rounding question arises. The pattern is asymmetric in both axes, so
  a transposed, flipped or tight-stride write fails.
- **Not in S1:** a draw call. SWGL runs only WebRender's own shader programs, and those arrive with WebRender in S2.
  Linking SWGL already proves that every translated shader compiles and links for the target.

## 2. S2: WebRender 0.70 on SWGL

**The closure grows to about 87 crates.** Measured by resolving `webrender = "=0.70.0"` with `sw_compositor`, no
default features. Licences: 45 "MIT OR Apache-2.0", 13 "MIT/Apache-2.0", 11 MPL-2.0, 9 MIT, a few Apache-2.0,
BSD-3-Clause, Unlicense-or-MIT, and unicode-ident's Unicode-3.0. `r-efi` is "MIT OR Apache-2.0 OR LGPL-2.1-or-later"
and is taken under MIT. `unicode-ident` adds the Unicode-3.0 data licence, which the standing list doesn't name; it
is already in C2.3b's closure under the same terms (a build-host proc-macro dependency, recorded in
`third_party/ot61-browser/VENDORED.md`). No GPL, LGPL-only or AGPL.

**More third-party build code (first run on CI):**
- WebRender's `build.rs` pulls in `glslopt` 0.1.14 (MIT), which compiles Mesa's GLSL optimiser (C++) for the build
  host, to optimise the *hardware-GL* shaders. The software path doesn't need its output, but the build-dependency
  is unconditional.
- Its proc macros (`peek-poke-derive`, `malloc_size_of_derive`, `serde_derive`, `derive_more`).

**The one real seam: fonts.** `wr_glyph_rasterizer` 0.70.1 picks its backend by `target_os`: Core Text on Apple,
DirectWrite on Windows, FreeType on `unix`/Android. `x86_64-unknown-neoplutarch` is none of these (no
`target_family`), so the crate's `platform` module is empty and WebRender does not compile. S2 therefore needs a
`neoplutarch` font backend. It is the only per-OS seam: `webrender_api` and `webrender` use only
"not Apple" / "not Windows" / "Linux-only" cfgs, so our target takes their generic paths (checked by reading every
`target_os` cfg in both crates at 0.70.0). The backend is exactly the "narrow capability-sensitive provider seam" ADR-078 §2.1 allows
patching (fonts are a resource provider; they arrive as handles, never an ambient font directory). It is a patched
crate, with its diff kept beside it, like C2.3b's `parking_lot_core`.

**Prior-art survey for the font backend** (liveness checked 2026-10-31, `git log -1` on each default branch):

| Candidate | What | Licence | Last commit | Fit | Take |
|---|---|---|---|---|---|
| **skrifa** (googlefonts/fontations) | Font parsing, outlines, hinting, metrics; Chrome's font stack | MIT OR Apache-2.0 | `745959f3` 2026-10-30 | Pure Rust, `no_std`-capable, maintained by Google Fonts; outlines as path commands | **Code**: outlines and metrics |
| **zeno** | Path rasterizer (anti-aliased coverage masks) | Apache-2.0 OR MIT | `334a74a7` 2026-10-27 | Small, pure Rust; turns skrifa's outlines into the A8 masks `wr_glyph_rasterizer` hands back | **Code**: rasterisation |
| swash | Shaping + scaling + rendering (on zeno) | Apache-2.0 OR MIT | `7773843d` 2026-07-17 | Higher level than needed; WebRender shapes nothing (Servo does) | Design only |
| fontdue | Simple rasterizer | MIT OR Apache-2.0 OR Zlib | `1a84e6d3` 2026-10-18 | Fast, but no hinting and limited format support | Fallback |
| ab_glyph | Outline + rasterizer | Apache-2.0 | `3eb21a59` 2026-08-30 | Fine, smaller feature set than skrifa | Nothing |
| tiny-skia | 2D rasterizer | BSD-3-Clause | `5d475477` 2026-08-20 | More than glyphs need | Nothing |
| rusttype | Outline + rasterizer | MIT/Apache-2.0 | `c1e820b4` 2023-03-08 | **Abandoned** (no commits since 2023) | Nothing |
| FreeType | C font engine (upstream's unix backend) | FTL or GPL-2.0 | `36cd3d56` 2026-10-30 | Alive, but FTL isn't on the standing licence list, and it's a second C library to port | Nothing, unless J.c. clears FTL |

**Plan:** `wr_glyph_rasterizer`'s `platform/neoplutarch` implements the same `FontContext` surface as the unix
backend, using skrifa for outlines and metrics and zeno for coverage. Font bytes come in as `FontTemplate::Raw`
(an in-memory blob the embedder supplies from a handle); there is no `FontTemplate::Native` path (no system font
directory to search).

**S2's check** is the spike's: one WebRender document whose display list holds solid, clipped, transformed, gradient,
image and text items, rendered through SWGL and compared with a checked-in pixel oracle. The oracle is rendered once
on a host from the same pinned sources and reviewed by eye before it's checked in. It must not be generated by the
target build it checks.

## 3. S3: `AtriumSwglRenderingContext`

Per the spike (§3.2, §4): framebuffer zero is the client's RW+NX mapping of one Atrium `SharedPixelBuffer`
(`init_default_framebuffer(0, 0, w, h, atrium_stride, mapping)`), flushed and committed as damage, and presented by
the compositor from its RO+NX mapping.
- The context validates width, height and stride before binding: Atrium accepts a stride of at least `width * 4`,
  aligned to 4, and SWGL does no check of its own. That is the spike's S1 "negative dimensions/stride fail without
  overwrite" item; it belongs to this context, not to SWGL.
- QEMU proves the pixels and the client direct-present refusal (`scanout_authorized()` → `EPERM`).

**S3 widens the PAL surface, so it is an A4 change** (`tools/ot61-pal-manifest.toml`).
- Today a bundle process reaches the kernel only through std's trampolines. The Atrium buffer calls live in
  no_std `libneo`, whose raw syscall stubs the A4 backstop would reject in a std ELF.
- S3 needs these calls from std:
  - `SYS_BUFFER_CREATE`, `SYS_BUFFER_MAP`, `SYS_BUFFER_QUERY`, `SYS_BUFFER_COMMIT_DAMAGE` and `SYS_BUFFER_UNMAP`;
  - the channel send/grant that hands the compositor its read-only view: `MAP_READ | QUERY | PRESENT`, the
    `neo-worker` pattern.
- Each one gets, in the same change:
  - a `[gate.provenance]` line saying which capability authorises it (the buffer handle the process created; the
    channel the launcher handed it);
  - its G1/G2 negative test in `tools/xtask/src/ot61_c05_tests.rs`;
  - a `std::os::neoplutarch` surface in the overlay, next to C2.1b's `handle_info`, rather than a raw call in the
    browser.
- `SYS_FB_PRESENT` is **not** on the list: the client never presents. The scenario proves the refusal anyway, by
  asking.
- This is a governed change, so it waits until S1 and S2 are green on CI.

## 4. Kill-switch and the default build

Every piece is opt-in: standalone crates built only for their scenarios (`swgl_smoke`, later `webrender_smoke`), and
test-only kernel features. No syscall. The default kernel and image are unchanged.

## 5. Decisions (none blocks S1)

1. **Fonts:** skrifa + zeno in a patched `wr_glyph_rasterizer` backend (recommended), or clear FreeType's FTL and
   port FreeType.
2. **The C/C++ compiler pin** (C3a's THINK §7) applies here too, since SWGL is the largest C++ in the bundle.
