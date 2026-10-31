<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# `fatfs`: vendored rafalh/rust-fatfs

This is `rafalh/rust-fatfs` (by Rafał Harabień and contributors), vendored for OT-87 R4.2: the host ESP-template
builder in xtask formats the installer's two ESP templates with it, replacing the `mformat` call
(`docs/THINK-2026-10-27-ot87-installer.md` §4.3, §11.2).

- **Upstream:** https://github.com/rafalh/rust-fatfs, branch `master`, commit
  `38b3a7950bf0e765e34286d1dfe76c519ee347e2` (2026-10-16). The crate version is 0.4.0, which is unreleased; the last
  tag is v0.3.6. `master` is pinned because 0.4's `format_volume` options (`bytes_per_sector`, `volume_id`) are what
  the templates use.
- **Licence:** MIT, unchanged: `LICENSE.txt`.
- **Manifest:** `Cargo.toml.upstream` is upstream's `Cargo.toml`, unchanged. `Cargo.toml` here differs only in
  `publish = false`, the dropped dev-dependencies (upstream's `tests/` and `resources/` aren't vendored), and
  `chrono` dropped from the default features. xtask passes a fixed time provider (1980-01-01 00:00), so a
  template build is reproducible.
- **Source:** `src/` is upstream's, byte for byte, plus one first line per file,
  `// SPDX-License-Identifier: MIT`, for LogansOS's SPDX close-gate. `rustfmt.toml` is upstream's plus an SPDX
  line. The directory is excluded from the LogansOS workspace, so first-party fmt and clippy settings don't apply.
- **Consumers:** only `tools/xtask` (host). **The kernel never links it**: the kernel never formats or rewrites
  FAT (§4.3, §7.1). It writes the template's bytes whole, then sectors inside the slot extents and the slot files'
  directory-entry size fields.
- **Maintainer of this copy:** the OT-87 installer lane. Upstream is active, so an update is a re-vendor at a
  newer commit, which changes the ESP templates and therefore the bundle's BLAKE3 pin.

## Checking the base

```
git clone https://github.com/rafalh/rust-fatfs /tmp/rust-fatfs
git -C /tmp/rust-fatfs checkout 38b3a7950bf0e765e34286d1dfe76c519ee347e2
for f in /tmp/rust-fatfs/src/*.rs; do
  tail -n +2 third_party/fatfs/src/$(basename $f) | cmp - $f || echo "differs: $f"
done
```

No patches.
