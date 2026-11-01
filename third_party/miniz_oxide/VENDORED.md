<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# `miniz_oxide`: vendored Frommi/miniz_oxide

This is the Rust `miniz_oxide` crate from [Frommi/miniz_oxide](https://github.com/Frommi/miniz_oxide), vendored for
Lane 2a (`docs/THINK-2026-10-31-lived-in-os-lanes.md` §4): a `no_std` + `alloc` DEFLATE/zlib implementation to sit
under the OS's compression users (gzip over HTTP for the fetch broker, image codecs later).

- **Upstream:** https://github.com/Frommi/miniz_oxide, branch `master`, commit
  `7fc2dd45810e1722272ff77c4238a8f8e0d0da22` (2026-10-29, "Create AGENTS.md"). The Rust crate lives in the
  repository's `miniz_oxide/` subdirectory; the repository root is the separate `miniz_oxide_c_api` C wrapper, which
  is **not** vendored. Crate version 0.9.1.
- **Licence:** the crate declares **MIT OR Zlib OR Apache-2.0** in `Cargo.toml.upstream` (the survey row said
  "MIT"; re-checked here on 2026-10-31, the declaration is the triple). All three licence files are kept unchanged:
  `LICENSE` (the MIT text), `LICENSE-MIT.md`, `LICENSE-APACHE.md`, `LICENSE-ZLIB.md`. This vendored copy is used
  under **MIT**, which the triple permits, so the lane's licensing conclusion is unchanged.
- **Manifest:** `Cargo.toml.upstream` is upstream's `miniz_oxide/Cargo.toml`, unchanged. `Cargo.toml` here differs
  only in `publish = false`, the removed `[[bench]]` target (benches are not vendored), the removed
  `[dev-dependencies]` block (upstream's is commented out), and `license = "MIT"` naming the option this copy is
  used under.
- **Source:** `src/` is upstream's, byte for byte, plus one first line per file,
  `// SPDX-License-Identifier: MIT`, for LogansOS's SPDX close-gate. `Readme.md` is upstream's. The directory is
  excluded from the LogansOS workspace, so first-party fmt and clippy settings do not apply to it.
- **Not vendored:** `benches/`, `tests/` (upstream test data), `fuzz/`, the C API crate at the repository root, and
  the `miniz/` C sources. Lane 2a's KATs live in `tools/miniz-oxide-logic-tests` and use their own raw-fetched
  streams.
- **Consumers:** only `tools/miniz-oxide-logic-tests` (host). **The kernel never links it yet**: Lane 2a is the
  vendoring plus host KATs; wiring a codec into `libneo`/the fetch broker is a later build.
- **Maintainer of this copy:** the Lane 2a (miniz_oxide) lane. Upstream is active, so an update is a re-vendor at a
  newer commit plus a re-run of the host KATs.

## Checking the base

```
git clone https://github.com/Frommi/miniz_oxide /tmp/miniz_oxide
git -C /tmp/miniz_oxide checkout 7fc2dd45810e1722272ff77c4238a8f8e0d0da22
for f in /tmp/miniz_oxide/miniz_oxide/src/*.rs /tmp/miniz_oxide/miniz_oxide/src/*/*.rs; do
  rel="${f#/tmp/miniz_oxide/miniz_oxide/}"
  tail -n +2 "third_party/miniz_oxide/$rel" | cmp - "$f" || echo "differs: $rel"
done
cmp third_party/miniz_oxide/Cargo.toml.upstream /tmp/miniz_oxide/miniz_oxide/Cargo.toml
```

No patches beyond the one SPDX line per source file.
