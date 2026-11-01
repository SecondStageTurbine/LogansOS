<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# `zune-jpeg`: vendored etemesi254/zune-image (JPEG decoder)

This is the Rust `zune-jpeg` crate from [etemesi254/zune-image](https://github.com/etemesi254/zune-image),
vendored for Lane 2b (`docs/THINK-2026-10-31-lived-in-os-lanes.md` §4): the JPEG decoder behind the one `libneo`
image-decode API.

- **Upstream:** https://github.com/etemesi254/zune-image, branch `dev`, commit
  `2f9086b07cdf5a36621ce18f79d310f99b751de9` (2026-10-24, "feat(jpeg): start scanline reads lazily"),
  re-checked on 2026-11-01. Crate `crates/zune-jpeg`, version 0.5.16-rc3.
- **Licence:** the crate declares **MIT OR Apache-2.0 OR Zlib** in `Cargo.toml.upstream`; all three licence files
  are kept unchanged (`LICENSE-MIT`, `LICENSE-APACHE`, `LICENSE-ZLIB`). This vendored copy is used under
  **MIT**, which the triple licence permits.
- **Dependency:** `zune-jpeg` requires `zune-core`. It is vendored beside this crate at
  `third_party/zune-core`, at the same upstream commit, so upstream's relative `../zune-core` path dependency is
  unchanged. `zune-core` is `no_std` + `alloc`; `zune-jpeg` is used with `default-features = false` (no `std`,
  no `x86`/`neon` intrinsics) by `libneo`, which keeps the decoder alloc-only.
- **Manifest:** `Cargo.toml.upstream` is upstream's crate manifest, unchanged. `Cargo.toml` here differs only
  in `publish = false`, the removed (empty) `[dev-dependencies]` block, and `license = "MIT"` naming the option
  this copy is used under.
- **Source:** `src/` is upstream's, byte for byte, plus one first line per `.rs` file,
  `// SPDX-License-Identifier: MIT OR Apache-2.0 OR Zlib`, for LogansOS's SPDX close-gate. `README.md` and
  `Changelog.md` are upstream's. The directory is excluded from the LogansOS workspace.
- **Not vendored:** `tests/`, `fuzz/`, `Benches.md`, and the repository's other crates (`zune-image`,
  `zune-png`, `zune-bmp`, ...). Lane 2b's KATs live in `tools/image-decode-logic-tests` and use their own
  raw-fetched images.
- **Consumers:** `libneo`'s `image-decode` feature (host-only) and `tools/image-decode-logic-tests` (host).
  **The kernel never links it.**
- **Maintainer of this copy:** the Lane 2b (image decode) lane. Upstream is active; an update is a re-vendor of
  both `zune-jpeg` and `zune-core` at the same newer commit plus a re-run of the host KATs.

## Checking the base

```
git clone https://github.com/etemesi254/zune-image /tmp/zune-image
git -C /tmp/zune-image checkout 2f9086b07cdf5a36621ce18f79d310f99b751de9
for f in $(cd /tmp/zune-image && find crates/zune-jpeg/src -name '*.rs'); do
  tail -n +2 "third_party/zune-jpeg/${f#crates/zune-jpeg/}" | cmp - "/tmp/zune-image/$f" || echo "differs: $f"
done
for f in $(cd /tmp/zune-image && find crates/zune-core/src -name '*.rs'); do
  tail -n +2 "third_party/zune-core/${f#crates/zune-core/}" | cmp - "/tmp/zune-image/$f" || echo "differs: $f"
done
cmp third_party/zune-jpeg/Cargo.toml.upstream /tmp/zune-image/crates/zune-jpeg/Cargo.toml
cmp third_party/zune-core/Cargo.toml.upstream /tmp/zune-image/crates/zune-core/Cargo.toml
```

No patches beyond the one SPDX line per source file.
