<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# `png`: vendored image-rs/image-png

This is the Rust `png` crate from [image-rs/image-png](https://github.com/image-rs/image-png), vendored for
Lane 2b (`docs/THINK-2026-10-31-lived-in-os-lanes.md` §4): the PNG decoder behind the one `libneo` image-decode
API.

- **Upstream:** https://github.com/image-rs/image-png, branch `master`, commit
  `b2f10c6a7e3127d4aaf231f3eae731a33e6eb0bc` (2026-09-23, "Remove import of miniz_oxide, unused since 0.17.12
  (#710)"), re-checked on 2026-11-01. Crate version 0.18.1.
- **Licence:** the crate declares **MIT OR Apache-2.0** in `Cargo.toml.upstream`; both licence files are kept
  unchanged (`LICENSE-MIT`, `LICENSE-APACHE`). This vendored copy is used under **MIT**, which the dual licence
  permits.
- **Manifest:** `Cargo.toml.upstream` is upstream's `Cargo.toml`, unchanged. `Cargo.toml` here differs only in
  `publish = false`, the removed `[[bench]]` targets and `[dev-dependencies]` block (benches and tests are not
  vendored), and `license = "MIT"` naming the option this copy is used under. The crate's own dependencies
  (`bitflags`, `crc32fast`, `fdeflate`, `flate2`) resolve from crates.io as upstream declares them.
- **Source:** `src/` is upstream's, byte for byte, plus one first line per `.rs` file,
  `// SPDX-License-Identifier: MIT OR Apache-2.0`, for LogansOS's SPDX close-gate. `README.md` and `CHANGES.md`
  are upstream's. The directory is excluded from the LogansOS workspace, so first-party fmt and clippy settings do
  not apply to it.
- **Not vendored:** `benches/`, `tests/`, `examples/`, `fuzz/`, `png-afl/`, and the CI/Docker files. Lane 2b's
  KATs live in `tools/image-decode-logic-tests` and use their own raw-fetched images.
- **Consumers:** `libneo`'s `image-decode` feature (host-only; see `userland/libneo/src/image_decode.rs`) and
  `tools/image-decode-logic-tests` (host). **The kernel never links it**: this lane is vendoring plus a host
  KAT, not a kernel codec. The OS userland target has no `std`, which `png` 0.18 requires, so the feature is
  off by default and is never enabled for the OS target.
- **Maintainer of this copy:** the Lane 2b (image decode) lane. Upstream is active, so an update is a re-vendor
  at a newer commit plus a re-run of the host KATs.

## Checking the base

```
git clone https://github.com/image-rs/image-png /tmp/image-png
git -C /tmp/image-png checkout b2f10c6a7e3127d4aaf231f3eae731a33e6eb0bc
for f in $(cd /tmp/image-png && find src -name '*.rs'); do
  tail -n +2 "third_party/png/$f" | cmp - "/tmp/image-png/$f" || echo "differs: $f"
done
cmp third_party/png/Cargo.toml.upstream /tmp/image-png/Cargo.toml
```

No patches beyond the one SPDX line per source file.
