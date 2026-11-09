<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — Relay 298: #66 review (Jarvis)

Reviewed PR head: `81c291cad08342c80b9fb867c8f9386723a2e7cb`

1. **PASS** — `third_party/ot61-swgl/VENDORED.md:49` records upstream repositories, the SWGL commit, MPL-2.0, and the local delayed-clear patch at lines 30–45. The MPL text is present in the repository `LICENSE`; the other closure license files are vendored with their crates.
2. **FAIL** — `third_party/ot61-swgl/patched/swgl-0.70.0/src/gl.cc:2564` forwards dimensions and buffer to `set_tex_storage` without validating them. Concrete input: `init_default_framebuffer(0, 0, 0, 0, 0, buf)` is not refused; `set_buffer`'s stride assertion at `src/gl.cc:497` accepts zero stride for zero width. There is also no check that `buf` is aligned. The 7x5 smoke at `userland/swgl-smoke/src/main.rs:72` checks canaries and padded rows for its single valid buffer but does not establish refusal of zero-sized or misaligned buffers.
3. **PASS** — `userland/swgl-smoke/src/main.rs:30` defines an asymmetric per-pixel expected pattern; lines 105–125 compare every framebuffer, padding, and canary byte against it.
4. **PASS** — `userland/swgl-smoke/Cargo.toml:1` is a standalone crate excluded from the root workspace and only this crate depends on SWGL. `swgl_cc_env` sets the target compiler and flags at `tools/xtask/src/main.rs:7769`; the CI job pins clang 19 and invokes the build explicitly in `.github/workflows/ot61-closure.yml:234` and `:258`.

Result: **FAIL** (item 2).
