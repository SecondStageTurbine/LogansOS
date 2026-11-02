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
  `// SPDX-License-Identifier: MIT`, for LogansOS's SPDX close-gate, **except the one patch below**. `rustfmt.toml` is upstream's plus an SPDX
  line. The directory is excluded from the LogansOS workspace, so first-party fmt and clippy settings don't apply.
- **Consumers:**
  - `tools/xtask` (host), for the ESP templates.
  - Since L3a-2, the kernel, behind its `fat-fs` feature, for **read-only** FAT12/16/32 mounts
    (`kernel/src/fs/fat.rs`; `docs/THINK-2026-10-31-l3a-fat-readonly.md` §5). The kernel builds it with
    `default-features = false, features = ["alloc", "lfn", "unicode"]`: no `std`, no `chrono`, and `log` with no
    logger installed. Its I/O adapter refuses every write, so OT-87's rule still holds: the kernel never formats
    or rewrites FAT through this crate (the installer writes the template's bytes whole, then sectors inside the
    slot extents and the slot files' directory-entry size fields). L3b, read-write, revisits that.
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

## Patches

One, for L3a-2 (Chad's #18 review): **a cluster chain that loops is an error**. Upstream follows the FAT with no
visited set and no bound, so a corrupt chain (`A -> B -> A`) makes a file read return the same clusters again, up to
the file's size, and a directory listing (no size to stop at) walk forever.

- `src/table.rs`: `CycleGuard`, Brent's cycle detection in O(1) state; `ClusterIterator` steps it on every next
  cluster and, on a repeat, ends with `Error::CorruptedFileSystem`.
- `src/file.rs`: before a file's (or directory's) first read returns any data, its whole chain is walked once,
  at most `total_clusters + 2` steps with the cycle guard, and a loop or an over-long chain fails the read with
  `Error::CorruptedFileSystem`. A file's size can end a read before a loop shows (`A -> B -> A` over a
  three-cluster file reads A, B, A), which is why the walk doesn't wait for the read to reach the loop. `File` also
  keeps a `CycleGuard` across reads and writes (each steps one cluster with a fresh iterator), reset on seek.
- `src/fs.rs`: `total_clusters_bound()`, a crate-private accessor for that bound.

A chain that doesn't loop walks exactly as before, so the ESP templates xtask builds are unchanged. The discriminators:
host KAT `a_looping_chain_is_corruption` (`tools/fat-logic-tests`: a looping file reads back 6144 bytes of revisited
clusters without the patch) and the gate scenario `phase3_fat32_loop_chain`. Worth offering upstream.

The base check below reports `table.rs`, `file.rs` and `fs.rs` as differing; `git log -- third_party/fatfs/src` shows the patch.
