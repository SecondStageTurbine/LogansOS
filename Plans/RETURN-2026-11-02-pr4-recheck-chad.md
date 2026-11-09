<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# PR #4 re-check: Stylo closure

Reviewed head: `a7295b1b6558d8bbb70192bc6af8a9e0f03f3a14`.

- **PASS — `windows-sys` stub.** Its manifest records what the stub is, why it is lockfile-resolved, its `cfg(windows)` boundary, failure behavior, licence and upstream repository (`third_party/ot61-browser/stubs/windows-sys-0.61.2/Cargo.toml:1-21`). `VENDORED.md` documents the stub and its limits (`third_party/ot61-browser/VENDORED.md:67-85`); the generated crate table identifies its upstream repository and marks it as a no-code stand-in (`third_party/ot61-browser/VENDORED.md:238`).
- **PASS — `properties/build.py` output boundary.** The patched generator now writes `css-properties.json` and `css-properties.html` only under `OUT_DIR`; the copy to `OUT_DIR/../../../../doc/stylo` is removed (`third_party/ot61-browser/patched/stylo-0.22.0/properties/build.py:70-74`).
- **New findings:** None.
