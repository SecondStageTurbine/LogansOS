<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# `zune-core`: vendored etemesi254/zune-image (core utilities)

This is the Rust `zune-core` crate from [etemesi254/zune-image](https://github.com/etemesi254/zune-image),
vendored because `third_party/zune-jpeg` declares it as a path dependency. It is part of the same Lane 2b
vendoring; the dispatch names `zune-jpeg`, and its required core crate is recorded here rather than pulled from
crates.io so the JPEG decoder is self-contained.

- **Upstream:** https://github.com/etemesi254/zune-image, branch `dev`, commit
  `2f9086b07cdf5a36621ce18f79d310f99b751de9` (2026-10-24), re-checked on 2026-11-01. Crate `crates/zune-core`,
  version 0.5.3 (satisfies `zune-jpeg`'s `version = "0.5.2"` requirement).
- **Licence:** **MIT OR Apache-2.0 OR Zlib** in `Cargo.toml.upstream`; all three licence files are kept
  unchanged. This copy is used under **MIT**.
- **Manifest:** `Cargo.toml.upstream` is upstream's, unchanged. `Cargo.toml` here differs only in
  `publish = false` and `license = "MIT"`.
- **Source:** `src/` is upstream's, byte for byte, plus one first line per `.rs` file,
  `// SPDX-License-Identifier: MIT OR Apache-2.0 OR Zlib`. `README.md` and `CHANGELOG.md` are upstream's. The
  directory is excluded from the LogansOS workspace.
- **Consumers:** only `third_party/zune-jpeg` (and therefore `libneo`'s host-only `image-decode` feature and
  `tools/image-decode-logic-tests`). **The kernel never links it.**
- **Maintainer of this copy:** the Lane 2b (image decode) lane; it moves with `third_party/zune-jpeg`.

## Checking the base

The `zune-jpeg` check in `third_party/zune-jpeg/VENDORED.md` covers both crates.
