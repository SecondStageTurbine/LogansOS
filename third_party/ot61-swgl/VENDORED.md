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

## The crates (checked 2026-10-31)

| Crate | Licence | Upstream | Upstream commit (from `.cargo_vcs_info.json`) | Role |
|---|---|---|---|---|
| swgl 0.70.0 (**patched**, see above) | MPL-2.0 | github.com/servo/webrender `swgl/` | `2e341fa14234aa6f1dfb8d3309e7eae0f5d1d7f7` (the spike's pin; files identical to the git tree) | the software GL |
| glsl-to-cxx 0.70.0 | MPL-2.0 | same repository | `2e341fa1…` | build-time shader translator |
| webrender_build 0.70.0 | MPL-2.0 | same repository | `2e341fa1…` | build-time shader feature list |
| gleam 0.15.1 | MIT OR Apache-2.0 | github.com/servo/gleam | `e7b3f3296c70093f35357e644dc8711c1f79fc3d` (the spike's pin) | the GL trait WebRender calls |
| glsl 6.0.2 | BSD-3-Clause | github.com/phaazon/glsl | `a01a2ac9ce5f21b7c0824336d32b9eb7e6b6fe85` | GLSL parser (build time) |
| gl_generator 0.14.0 | Apache-2.0 | github.com/brendanzab/gl-rs | (crates.io) | gleam's build-time binding generator |
| khronos_api 3.1.0 | Apache-2.0 | github.com/brendanzab/gl-rs | (crates.io) | the Khronos GL registry XML (build time) |
| xml-rs 0.8.29 | MIT | github.com/kornelski/xml-rs | (crates.io) | XML parser (build time) |
| nom 7.1.3 | MIT | github.com/rust-bakery/nom | (crates.io) | parser combinators (glsl) |
| minimal-lexical 0.2.1 | MIT OR Apache-2.0 | github.com/Alexhuszagh/minimal-lexical | (crates.io) | nom's float parsing |
| memchr 2.8.3 | Unlicense OR MIT | github.com/BurntSushi/memchr | (crates.io) | nom |
| bitflags 2.13.2 | MIT OR Apache-2.0 | github.com/bitflags/bitflags | (crates.io) | webrender_build |
| lazy_static 1.5.1 | MIT OR Apache-2.0 | github.com/rust-lang-nursery/lazy-static.rs | (crates.io) | webrender_build |
| log 0.4.35 | MIT OR Apache-2.0 | github.com/rust-lang/log | (crates.io) | gl_generator |
| cc 1.2.63 | MIT OR Apache-2.0 | github.com/rust-lang/cc-rs | (crates.io) | C/C++ builds (also neo-crt's) |
| shlex 2.0.1, find-msvc-tools 0.1.15 | MIT OR Apache-2.0 | (cc's dependencies) | (crates.io) | cc |
| linked_list_allocator 0.10.6 | Apache-2.0 OR MIT | github.com/rust-osdev/linked-list-allocator | (crates.io) | neo-std-pal's heap |

All licences are on the standing list. Crates whose `.cargo_vcs_info.json` carries no commit are identified by the
published `.crate` SHA-256 in their `.cargo-checksum.json`.
