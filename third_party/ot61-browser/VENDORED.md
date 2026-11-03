<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# `ot61-browser`: the vendored html5ever + selectors closure (OT-61 C2.3b), and Stylo's (C2.4)

The browser's first third-party crates: upstream **html5ever** (the HTML parser) and **selectors** (Stylo's
selector matching, published from `servo/stylo`), with **cssparser** and everything they depend on. Pinned here
as sources, per J.c.'s decision on the C2.3b spike (#200, ledger 2026-10-29): *vendor the sources*, and *the first
build runs on the CI runners* (`.github/workflows/ot61-closure.yml`). The closure and its reasoning are in
`docs/RESEARCH-2026-10-29-ot61-c2.3b-html5ever-stylo-closure-SPIKE.md`.

C2.4 adds **Stylo** 0.22.0 (Servo's CSS engine) and its closure: 95 crates beyond C2.3b's, plus a `windows-sys`
stub. Their survey (licences, last commits, what links and what only builds) is
`docs/RESEARCH-2026-10-30-ot61-c2.4-stylo-closure-SURVEY.md`.

## Layout

- `vendor/<crate>-<version>/` — each crate exactly as published on crates.io, unpacked by `cargo vendor`. The
  `.cargo-checksum.json` beside each holds the SHA-256 of the published `.crate` (`package`) and of every file,
  so the copy can be checked against crates.io at any time.
- `patched/parking_lot_core-0.9.12/` and `patched/stylo-0.22.0/` — the two patched crates. Each one's diff
  against the published crate is `<crate>-<version>-neoplutarch.patch`; the published copy stays in `vendor/`.
- `stubs/windows-sys-0.61.2/` — an empty crate with `windows-sys`'s name, version, licence and feature table.
  It is in the C2.4 lockfile only behind `cfg(windows)` dependencies, and the published crate is 60 MB of
  Windows bindings that never build for our target.
- `stubs/<crate>-<version>/` (43 more, OT-61 L0) — empty stand-ins for crates `userland/layout-smoke`'s lockfile
  names only behind other platforms' `cfg`s, optional features this target leaves off, or edges option A
  switches off (`servo-script`, `surfman`):
  "The stubs for other platforms", below.
- `licenses/` — licence texts a vendored crate was published without (`void` 1.0.2, below).
- `gen-vendored-table.py` prints this file's crate table from the crates' own files; `gen-patch-table.py` prints a
  smoke's `[patch.crates-io]` table.
- `userland/servo-closure-smoke` (C2.3b) and `userland/stylo-smoke` (C2.4) use this tree, through
  `[patch.crates-io]` path entries in their `Cargo.toml`. (Not `[source]` replacement: `-Z build-std` resolves std's own dependencies through the same
  sources, and std's are not vendored.) The CI job fails if any closure crate resolves to crates.io instead.
- No other build reads it. The default kernel, the product image and every other userland crate are unchanged.

## The patch: `parking_lot_core` parks on the futex

Upstream `parking_lot_core` picks its thread parker by `target_os`/`target_family`, and
`x86_64-unknown-neoplutarch` (no `target_family`) falls to `generic.rs`, whose `park()` is a pure
`spin_loop()`. `string_cache`'s atom table is `parking_lot::Mutex` buckets, so contended interning would spin;
with no timer tick on an AP (CP-86) a spinning waiter can starve the holder on its CPU forever.

The patch adds `src/thread_parker/neoplutarch.rs`: a parker on std's `Mutex<bool>` + `Condvar`, which this
target's std implements on `SYS_FUTEX_WAIT`/`SYS_FUTEX_WAKE` (C1.3). It is `unix.rs`'s shape in std terms:
`unpark_lock` takes the mutex and `unpark` clears the flag, signals, then unlocks, so a parked thread cannot
return (and free its `ThreadData`) between the two. It adds no syscall site of its own (the A4 manifest is
unchanged; xtask's syscall-site check runs on the smoke ELF). It uses only std, so it is upstreamable as is.

The copy's `Cargo.toml` also drops the `redox`, `unix` and `windows` parker dependencies (`redox_syscall`,
`libc`, `windows-link`): this copy is built only for our target, and dropping them keeps 7 MB of `libc` sources
out of the tree. That trim is local, not part of what would go upstream.

## The patch: Stylo's thread handle, and its build script's output

Stylo's `global_style_data.rs` names a platform thread handle for its style pool: `RawPthread` on `unix`,
`RawHandle` on `windows`, and a no-op `DummyThreadHandle` on `wasm32`. `x86_64-unknown-neoplutarch` is none of
those (no `target_family`), so `PlatformThreadHandle` was undefined and the crate didn't build (CI run
41254489164). The patch extends the `wasm32` no-op branch to `target_os = "neoplutarch"`: four `cfg`s. The handles are only read by Gecko's embedding (`get_thread_handles`); Servo, and our smoke, never ask for
them. The style pool's threads themselves are ordinary `std::thread`s on this target's C1.3 threads. Upstreamable
as is.

The patch also stops `properties/build.py` writing outside `OUT_DIR`. Upstream copies `css-properties.html` and
`css-properties.json` to `OUT_DIR/../../../../doc/stylo` (the `target/doc` tree) as well as to `OUT_DIR`; a build
script must write only under `OUT_DIR`, so the copy outside it is dropped (`properties/build.py`, the
`doc_servo` lines). The same two files are still written to `OUT_DIR`, and nothing reads the `doc/stylo` copy.
Upstreamable as an opt-out.

Each source file of the copy also starts with its SPDX line (`MPL-2.0`, the crate's licence and the notice every
file already carries), as `parking_lot_core`'s do: a patched crate is checked like first-party code.

## The stub: `windows-sys`

`stubs/windows-sys-0.61.2/` is **not** the published crate. It is an empty crate (its `src/lib.rs` is one doc line)
with the published `windows-sys` 0.61.2's name, version, licence and `[features]` table, which is all Cargo needs
to resolve the lockfile.

- **Why it's in the lockfile.** Stylo's build script uses `walkdir`. `walkdir` 2.5.0 and `same-file` 1.0.6 depend
  on `winapi-util` 0.1.11 under `[target."cfg(windows)".dependencies]`, and `winapi-util` depends on
  `windows-sys`. Cargo resolves every target's dependencies into the lockfile, so the crate must exist, even
  though nothing on our build hosts or target compiles it.
- **Why it's safe.** No LogansOS build host or target is Windows, so `cfg(windows)` is never true and the stub is
  never compiled or linked. If a Windows host ever built this closure, `winapi-util` would fail to compile against
  the empty stub: a loud build failure, never a silently wrong build.
- **Licence.** `MIT OR Apache-2.0`, the upstream crate's (github.com/microsoft/windows-rs). The only upstream text
  in the stub is the feature table, copied from the published 0.61.2 `Cargo.toml`; both licence texts sit beside
  its manifest. The manifest's `repository` names the project it stands in for, and its header comment repeats
  this section.
- **Why not vendor the real crate.** The published `windows-sys` 0.61.2 is about 60 MB of Windows API bindings that
  never build for our target.

## The stubs for other platforms (OT-61 L0)

`userland/layout-smoke` (L0's closure probe) names 43 more stubs in its `[patch.crates-io]` table, in
`stubs/<crate>-<version>/`. Each is built the same way as the `windows-sys` stub above: the published crate's
name, version, licence, `[lib]` name and `[features]` table, and an empty `#![no_std]` `src/lib.rs`. No upstream
code is in them. Feature entries that point at dependencies (`dep:x`, `x/feature`) are dropped, since a stub has
no dependencies; optional dependencies that were implicit features are kept as empty features, so every feature
a dependent asks for still exists. The manifests are generated from each crate's published `Cargo.toml`, never
typed by hand.

- **Why they're in the lockfile.** Cargo resolves every platform's dependencies, and every optional feature any
  platform turns on, into one lockfile. Each stub below is reached only through one of:
  - a dependency edge whose `cfg` is false for our target and for the build host's build scripts (macOS,
    Windows, wasm, Android, hermit, UEFI);
  - an optional feature our target's build leaves off: image's AVIF and EXR codecs, serde_with's chrono, hex,
    indexmap 1, jiff, schemars and bs58 support, postcard's embedded-io, flate2's zlib-rs backend,
    rustls-webpki's ring, HarfBuzz's FreeType, derive_more's rustc_version;
  - an edge option A switched off for our target: `servo-layout`'s `script` dependency and `servo-paint-api`'s
    `surfman`, both under `cfg(not(target_os = "neoplutarch"))`.
- **Why it's safe: checked, not assumed.** `cargo build -Zunstable-options --unit-graph` for
  `x86_64-unknown-neoplutarch` gives the same units over the same packages, with the same features, with and
  without the stubs (host build-script units included), and the build stops at the same point. None of the 43 is
  compiled. If one ever were, its dependent would fail to compile against the empty crate: a loud failure, never
  a silently wrong build. With `neo-crt` added (what #55 needs to link), the same check holds (510 units over 387
  packages) and the binary links.
- **What they save.** Without stubs the lockfile has 668 packages, including mozjs (Servo's SpiderMonkey
  bindings), `aws-lc-sys`, `bindgen`, rav1e, `freetype-sys`, ring and the Apple/Windows/wasm binding crates. With
  them it has 424, of which **363 come from crates.io: exactly the crates.io crates the target build compiles**
  (the unit graph names the same 363). 142 of those are already vendored here or in `third_party/ot61-swgl`,
  so L0 vendors the other 221.
- **The `cfg(unix)` / `cfg(target_os = "linux")` edges.** Some edges below (`num_threads`, `mio`, `tempfile`,
  `freetype`, `fontconfig-parser`, `yeslogic-fontconfig-sys`) are true on a Linux host. They are safe only because
  no crate behind them is built for the host here; the unit graph above is the check. The stubs apply to
  `layout-smoke`'s workspace only: `neo-layout-dom` and `neo-static-page` are separate workspaces with no
  `[patch]` table, so their host tests resolve the real crates.
- **Licences.** Each stub keeps its upstream licence expression, in its manifest's `license` field and its SPDX
  line, and the upstream licence files where the published crate ships them. Seven published crates ship none
  (`dwrote`, the four `objc2` crates, `r-efi`, `servo-script`); their stubs carry a `LICENSE-NOTE.md` saying so.
  `winapi` 0.3.9 and `bs58` 0.5.1 declare the old `MIT/Apache-2.0` form; their stubs write it as
  `MIT OR Apache-2.0`. #25 carries its own `winapi` 0.3.9 stub (under `ot61-swgl`); when #55 rebases onto it,
  the two converge on one copy.
- **`servo-script`.** Its stub holds the published 0.7.0's name, version, `[lib] name = "script"` and features.
  Option A (`Plans/DECISION-2026-11-02-ot61-first-page-option-a.md`) makes `servo-layout` not depend on it for our
  target, so the closure behind it (mozjs and its build tooling, about 420 MB of sources) is never vendored.

| Stub | Licence | Reached only from (dependent, under) |
|---|---|---|
| `bs58` 0.5.1 | MIT OR Apache-2.0 | serde_with 3.24.0, all (an optional feature this target leaves off) |
| `chrono` 0.4.45 | MIT OR Apache-2.0 | serde_with 3.24.0, all (an optional feature this target leaves off) |
| `core-foundation` 0.9.4 | MIT OR Apache-2.0 | wr_glyph_rasterizer 0.70.1, `cfg(any(target_os = "macos", target_os = "ios"))` |
| `core-graphics` 0.23.2 | MIT OR Apache-2.0 | wr_glyph_rasterizer 0.70.1, `cfg(any(target_os = "macos", target_os = "ios"))` |
| `core-text` 20.1.0 | MIT OR Apache-2.0 | wr_glyph_rasterizer 0.70.1, `cfg(any(target_os = "macos", target_os = "ios"))` |
| `dwrote` 0.11.5 | MPL-2.0 | servo-fonts 0.7.0, `cfg(target_os = "windows")`; servo-fonts-traits 0.7.0, `cfg(target_os = "windows")`; wr_glyph_rasterizer 0.70.1, `cfg(target_os = "windows")` |
| `embedded-io` 0.4.0 | MIT OR Apache-2.0 | postcard 1.1.3, all (an optional feature this target leaves off) |
| `embedded-io` 0.6.1 | MIT OR Apache-2.0 | postcard 1.1.3, all (an optional feature this target leaves off) |
| `exr` 1.74.2 | BSD-3-Clause | image 0.25.10, all (an optional feature this target leaves off) |
| `fontconfig-parser` 0.5.8 | MIT | fontdb 0.24.0, `cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))` |
| `freetype` 0.8.0 | MIT OR Apache-2.0 | wr_glyph_rasterizer 0.70.1, `cfg(any(target_os = "android", all(unix, not(any(target_os = "macos", target_os = "ios")))))` |
| `freetype-sys` 0.23.0 | MIT | harfbuzz-sys 0.8.0, all (an optional feature this target leaves off); servo-fonts 0.7.0, `cfg(any(target_os = "android", target_env = "ohos"))`; servo-fonts 0.7.0, `cfg(any(target_os = "linux", target_os = "android", target_os = "freebsd"))` |
| `hermit-abi` 0.5.3 | MIT OR Apache-2.0 | num_cpus 1.17.0, `cfg(target_os = "hermit")` |
| `hex` 0.4.3 | MIT OR Apache-2.0 | serde_with 3.24.0, all (an optional feature this target leaves off) |
| `indexmap` 1.9.3 | Apache-2.0 OR MIT | serde_with 3.24.0, all (an optional feature this target leaves off) |
| `jiff` 0.2.39 | Unlicense OR MIT | serde_with 3.24.0, all (an optional feature this target leaves off) |
| `js-sys` 0.3.107 | MIT OR Apache-2.0 | glow 0.17.0, `cfg(target_arch = "wasm32")`; uuid 1.29.0, `cfg(all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none"), target_feature = "atomics"))` |
| `mach2` 0.7.0 | BSD-2-Clause OR MIT OR Apache-2.0 | servo-base 0.7.0, `cfg(any(target_os = "macos", target_os = "ios"))` |
| `mio` 1.2.4 | MIT | ipc-channel 0.23.1, `cfg(any(target_os = "linux", target_os = "openbsd", target_os = "freebsd", target_os = "illumos"))` |
| `num_threads` 0.1.8 | MIT OR Apache-2.0 | time 0.3.55, `cfg(target_family = "unix")` |
| `objc` 0.2.7 | MIT | wr_glyph_rasterizer 0.70.1, `cfg(any(target_os = "macos", target_os = "ios"))` |
| `objc2` 0.6.5 | MIT | servo-fonts 0.7.0, `cfg(target_os = "macos")` |
| `objc2-core-foundation` 0.3.2 | Zlib OR Apache-2.0 OR MIT | servo-fonts 0.7.0, `cfg(target_os = "macos")` |
| `objc2-core-graphics` 0.3.2 | Zlib OR Apache-2.0 OR MIT | harfbuzz-sys 0.8.0, `cfg(target_vendor = "apple")`; servo-fonts 0.7.0, `cfg(target_os = "macos")` |
| `objc2-core-text` 0.3.2 | Zlib OR Apache-2.0 OR MIT | harfbuzz-sys 0.8.0, `cfg(target_vendor = "apple")`; servo-fonts 0.7.0, `cfg(target_os = "macos")` |
| `r-efi` 6.0.0 | MIT OR Apache-2.0 OR LGPL-2.1-or-later | getrandom 0.4.3, `cfg(all(target_os = "uefi", getrandom_backend = "efi_rng"))` |
| `ravif` 0.13.0 | BSD-3-Clause | image 0.25.10, all (an optional feature this target leaves off) |
| `ring` 0.17.14 | Apache-2.0 AND ISC | rustls-webpki 0.103.16, all (an optional feature this target leaves off) |
| `rustc_version` 0.4.1 | MIT OR Apache-2.0 | derive_more-impl 2.1.1, all (an optional feature this target leaves off) |
| `schemars` 0.9.0 | MIT | serde_with 3.24.0, all (an optional feature this target leaves off) |
| `schemars` 1.2.3 | MIT | serde_with 3.24.0, all (an optional feature this target leaves off) |
| `servo-script` 0.7.0 | MPL-2.0 | servo-layout 0.7.0, `cfg(not(target_os = "neoplutarch"))` |
| `surfman` 0.14.0 | MIT OR Apache-2.0 OR MPL-2.0 | servo-paint-api 0.7.0, `cfg(not(target_os = "neoplutarch"))` |
| `tempfile` 3.27.0 | MIT OR Apache-2.0 | ipc-channel 0.23.1, `cfg(any(target_os = "linux", target_os = "openbsd", target_os = "freebsd", target_os = "illumos"))` |
| `wasi` 0.11.1+wasi-snapshot-preview1 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | getrandom 0.2.17, `cfg(target_os = "wasi")` |
| `wasm-bindgen` 0.2.130 | MIT OR Apache-2.0 | glow 0.17.0, `cfg(target_arch = "wasm32")`; uuid 1.29.0, `cfg(all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none")))` |
| `web-sys` 0.3.107 | MIT OR Apache-2.0 | glow 0.17.0, `cfg(target_arch = "wasm32")` |
| `winapi` 0.3.9 | MIT OR Apache-2.0 | servo-fonts 0.7.0, `cfg(target_os = "windows")`; zeitstempel 0.2.2, `cfg(windows)` |
| `winapi-util` 0.1.11 | Unlicense OR MIT | same-file 1.0.6, `cfg(windows)`; walkdir 2.5.0, `cfg(windows)` |
| `windows` 0.62.2 | MIT OR Apache-2.0 | harfbuzz-sys 0.8.0, `cfg(target_family = "windows")`; ipc-channel 0.23.1, `cfg(target_os = "windows")` |
| `xml` 1.4.0 | MIT | servo-fonts 0.7.0, `cfg(target_os = "android")` |
| `yeslogic-fontconfig-sys` 6.0.1 | MIT | servo-fonts 0.7.0, `cfg(all(any(target_os = "linux", target_os = "freebsd"), not(target_env = "ohos")))` |
| `zlib-rs` 0.6.8 | Zlib | flate2 1.1.10, all (an optional feature this target leaves off) |

## The crates

Generated by `gen-vendored-table.py`, never typed by hand: licences from each crate's `Cargo.toml`, "Upstream
commit" from `.cargo_vcs_info.json` (the commit the release was published from), the SHA-256 of the published
`.crate` from `.cargo-checksum.json`. Release and last-commit dates are in the C2.3b spike's §2 and the C2.4
survey's §4. Every licence is MPL-2.0, MIT, Apache-2.0, Unlicense-or-MIT, BSD-3-Clause or Unicode-3.0 (the data
licence of `icu4x` and `unicode-ident`; on CLAUDE.md's vendoring list by J.c.'s decision of 2026-10-31, `69550a3c`).
OT-61 L0's 195 crates (`userland/layout-smoke`, #55) add ISC, Zlib, BSD-2-Clause and 0BSD (each as the crate's
only licence or one choice of several), all on CLAUDE.md's list. No GPL, LGPL or AGPL.

| Crate | Version | Licence | Repository | Upstream commit | crates.io `.crate` SHA-256 |
|---|---|---|---|---|---|
| accesskit | 0.24.1 | MIT OR Apache-2.0 | github.com/AccessKit/accesskit | `a55d3e1a18bb9ef0e4bccc9083fb13c3e0ad8969` | `d3b7f7f85a7e5f68090000ed7622545829afd484d210358702ae4cb97dd0c320` |
| adler2 | 2.0.1 | 0BSD OR MIT OR Apache-2.0 | github.com/oyvindln/adler2 | `89a031a0f42eeff31c70dc598b398cbf31f1680f` | `320119579fcad9c21884f5c4861d16174d0e06250625266f50fe6898340abefa` |
| aho-corasick | 1.1.5 | Unlicense OR MIT | github.com/BurntSushi/aho-corasick | `5178060ce73d91938f8582d0360e3be031380440` | `c982642fa9e8606056828ee9a8505737230110bb1099153c79efe865c59d12ba` |
| alloc-no-stdlib | 2.0.4 | BSD-3-Clause | github.com/dropbox/rust-alloc-no-stdlib | `6032b6a9b20e03737135c55a0270ccffcc1438ef` | `cc7bb162ec39d46ab1ca8c77bf72e890535becd1751bb45f64c597edb4c8c6b3` |
| app_units | 0.7.8 | MPL-2.0 | github.com/servo/app_units | `2e4e0e0ad8b43bf49a1d3d84e3ca4c55367314a7` | `467b60e4ee6761cd6fd4e03ea58acefc8eec0d1b1def995c1b3b783fa7be8a60` |
| arrayref | 0.3.9 | BSD-2-Clause | github.com/droundy/arrayref | `f8d0299d863922db6c409d08098941e833b70d69` | `76a2e8124351fda1ef8aaaa3bbd7ebbcb486bbcd4225aca0aa0d84bb2db8fecb` |
| arrayvec | 0.7.8 | MIT OR Apache-2.0 | github.com/bluss/arrayvec | `0cb664cf505844348538230479b0040b44f3faf1` | `d3fb67a6e08acf24fdeccbac2cb6ac4305825bd1f117462e0e6f2f193345ad56` |
| atomic_refcell | 0.1.14 | Apache-2.0 OR MIT | github.com/mozilla/atomic_refcell | `216bba408cf6df886dca5b0fb4b1d410d46db03c` | `21e4227379beff4205943696e6c3e0cd809bacdf3f0edd6e3dd153e2269571a4` |
| autocfg | 1.5.1 | Apache-2.0 OR MIT | github.com/cuviper/autocfg | `2799b09c24e6632f8e653c5cd8fc303e85a906ba` | `f2032f911046de80f0a198e0901378627c33f59ea0ac00e363d481118bd70a53` |
| base64 | 0.22.1 | MIT OR Apache-2.0 | github.com/marshallpierce/rust-base64 | `e14400697453bcc85997119b874bc03d9601d0af` | `72b3254f16251a8381aa12e40e3c4d2f0199f8c6508fbecb9d91f575e0fbb8c6` |
| base64 | 0.23.1 | MIT OR Apache-2.0 | github.com/marshallpierce/rust-base64 | `069bf7067b949f5c0a92b6ceb82492920502f2c2` | `ac07cdecf99051d9a5238b80f35af32cdeba5b336e55d957b318b50137e18da5` |
| bitflags | 2.13.2 | MIT OR Apache-2.0 | github.com/bitflags/bitflags | `80ce9b545acb0bd42150695fc351889cac1d8eb4` | `3ded4057c258ba199e2d26386d3af3780957ecaee6c4ef4041c6b4b8b97c0b06` |
| block-buffer | 0.10.4 | MIT OR Apache-2.0 | github.com/RustCrypto/utils | `6d35952d3d3b124bc1049ad6fb406b42b1ce4bfe` | `3078c7629b62d3f0439517fa394996acacc5cbc91c5a20d8c658e77abd503a71` |
| brotli-decompressor | 5.0.3 | BSD-3-Clause/MIT | github.com/dropbox/rust-brotli-decompressor | `b56177b8eb3d2440536281397e5ffe06eedf1a5c` | `3a32acac15fe1967bc3986b2a6347dffc965602354ea6f450ad07e8bfd253583` |
| byte-slice-cast | 1.2.3 | MIT | github.com/sdroege/bytes-num-slice-cast | `a7f61c2b4dd74064b4cfd6b0f5cb5bf50f86e64b` | `7575182f7272186991736b70173b0ea045398f984bf5ebbb3804736ce1330c9d` |
| byteorder | 1.5.0 | Unlicense OR MIT | github.com/BurntSushi/byteorder | `ec068eefa042d494475db125c4b034bd8e9e34dd` | `1fd0f2584146f6f2ef48085050886acf353beff7305ebd1ae69500e27c67f64b` |
| byteorder-lite | 0.1.0 | Unlicense OR MIT | github.com/image-rs/byteorder-lite | `7d44da06a48fe10d7e23efd9a932d18a7a2def63` | `8f1fe948ff07f4bd06c30984e69f5b4899c516a3ef74f34df92a2df2ab535495` |
| bytes | 1.12.1 | MIT | github.com/tokio-rs/bytes | `76c0fbb54ed4336caf9d2311658a2f4a5627c21d` | `fc652a48c352aef3ea3aed32080501cf3ef6ed5da78602a020c991775b0aff04` |
| cc | 1.11.0 | MIT OR Apache-2.0 | github.com/rust-lang/cc-rs | `67e9f6d1824a2b1d5b3a21f71e23f5510c669a41` | `375989decec1a0fa424442bed520fa12ede43a48475ce8c7417d2c69b32b5f6b` |
| cfg-if | 1.0.5 | MIT OR Apache-2.0 | github.com/rust-lang/cfg-if | `8b0a2b0fc1fa3b3741b9e72895f390cf499fe36f` | `4e7648175b45a9a48536d676f68d918270699102aa8dab5496df06904c914600` |
| chacha20 | 0.10.2 | MIT OR Apache-2.0 | github.com/RustCrypto/stream-ciphers | `6b236b758a0279f64d777797514813b2cb572c8b` | `65c35e4b699c7e15ccbe7ee35c005e4fc0a278d22238a2857e6ce2dadeda1b06` |
| cobs | 0.3.0 | MIT OR Apache-2.0 | github.com/jamesmunns/cobs.rs | `b86d5c1ace3d8c311d1db143f05f8083743700d1` | `0fa961b519f0b462e3a3b4a34b64d119eeaca1d59af726fe450bbba07a9fc0a1` |
| color | 0.3.3 | Apache-2.0 OR MIT | github.com/linebender/color | `598fae3faca4d17bbde51ec12c0d051102ef4541` | `2ec7c5eb7a16992b1904d76c517d170ab353b0e0b3d5a0c81a8a0cd1037893cf` |
| color_quant | 1.1.0 | MIT | github.com/image-rs/color_quant | `6c302d2d1540496584476d513dd75b815f14e7a1` | `3d7b894f5411737b7867f4827955924d7c254fc9f4d91a6aad6b097804b1018b` |
| content-security-policy | 0.9.0 | MIT OR Apache-2.0 | github.com/rust-ammonia/rust-content-security-policy | `a75b255dcb668982b7586f08edb3f5293b0622ea` | `9dd26eb56761db2c5288e97cfb8593fb411d3a7907ced6b6679512207ea8759c` |
| cookie | 0.16.2 | MIT OR Apache-2.0 | github.com/SergioBenitez/cookie-rs | `d5ce0569d41d69f300d6c06955613983236391bb` | `e859cd57d0710d9e06c381b550c06e76992472a8c6d527aecd2fc673dcc231fb` |
| cookie | 0.18.2 | MIT OR Apache-2.0 | github.com/SergioBenitez/cookie-rs | `1ec8fded5654679482a91c784e7ff2874b0fdeff` | `1a373e3602691c3cdea496d2f0ee5935151e6168fe87739483c463db1b2f2f87` |
| core_detect | 1.0.0 | MIT/Apache-2.0 | github.com/thomcc/core_detect | `a0f957776563806e7d20978471676e0db72892ba` | `7f8f80099a98041a3d1622845c271458a2d73e688351bf3cb999266764b81d48` |
| core_maths | 0.1.1 | MIT | github.com/robertbastian/core_maths | `d8ea96acac3893ad9a7e16cd7526936fbfc51021` | `77745e017f5edba1a9c1d854f6f3a52dac8a12dd5af5d2f54aecf61e43d80d30` |
| cpufeatures | 0.2.17 | MIT OR Apache-2.0 | github.com/RustCrypto/utils | `9d92d5e95ab4c07c5d8bfd024bf2a17e96d20feb` | `59ed5838eebb26a2bb2e58f6d5b5316989ae9d08bab10e0e6d103e656d1b0280` |
| cpufeatures | 0.3.1 | MIT OR Apache-2.0 | github.com/RustCrypto/utils | `e3ac92bfd33051e146025baa2d0ce79891b7fc73` | `5ca28b0ae3115b884660db4118d803791fd6756b6e88f39c0f3f7859060d7566` |
| crc32fast | 1.5.4 | MIT OR Apache-2.0 | github.com/srijs/rust-crc32fast | `118796a864d40beed113e887522531d5474b8abc` | `5f97d3416d5337d15c4b0d78c8c91df922ca143f57ec20e2d1c188333047ef04` |
| crossbeam-deque | 0.8.8 | MIT OR Apache-2.0 | github.com/crossbeam-rs/crossbeam | `2920c984290229ab4e0ca0452ef09e48a82063f3` | `622f3fc73690be383c7214310406f28a90e6edeadc3cea882f9d71e495b9711a` |
| crossbeam-epoch | 0.9.21 | MIT OR Apache-2.0 | github.com/crossbeam-rs/crossbeam | `2920c984290229ab4e0ca0452ef09e48a82063f3` | `dc74980687109a3b14c72fd458107bf0baa1da1a1a805e178d15501ba9b86d9d` |
| crossbeam-utils | 0.8.23 | MIT OR Apache-2.0 | github.com/crossbeam-rs/crossbeam | `2920c984290229ab4e0ca0452ef09e48a82063f3` | `a31eee39dddec8330830986fcd7625edb5a24ec90ea038215273bbc3adb08ac6` |
| crypto-common | 0.1.7 | MIT OR Apache-2.0 | github.com/RustCrypto/traits | `66a997ccf5776ce873da5a48e2885a9908e940f6` | `78c8292055d1c1df0cce5d180393dc8cce0abec0a7102adb6c7b1eef6016d60a` |
| cssparser | 0.38.0 | MPL-2.0 | github.com/servo/rust-cssparser | `d128a90e1203fc4afb959420d6dc5e24f77a3191` | `11119743ad110e8c1bdccd930d7f5c30c99e5fc76a7b63ec9807e84eef0c5f59` |
| cssparser-macros | 0.7.1 | MPL-2.0 | github.com/servo/rust-cssparser | `d128a90e1203fc4afb959420d6dc5e24f77a3191` | `d045de693cb712d0b22c6a64be5b953f67b3ce00ab5ad3dd5d8b441886ab8e1a` |
| darling | 0.20.11 | MIT | github.com/TedDriggs/darling | `82a51e0b65b158de02ffb5d753ea4cd03529743b` | `fc7f46116c46ff9ab3eb1597a45688b6715c6e628b5c133e288e709a29bcb4ee` |
| darling | 0.24.1 | MIT | github.com/TedDriggs/darling | `6b68cbe74cf5a7684c406f09558afc854022f824` | `ed17f5901b6630b993ca003def43f2f8ef4014fc13b047b57aad617ff32bc2ec` |
| darling_core | 0.20.11 | MIT | github.com/TedDriggs/darling | `82a51e0b65b158de02ffb5d753ea4cd03529743b` | `0d00b9596d185e565c2207a0b01f8bd1a135483d02d9b7b0a54b11da8d53412e` |
| darling_core | 0.24.1 | MIT | github.com/TedDriggs/darling | `6b68cbe74cf5a7684c406f09558afc854022f824` | `6837e2cf7485aaae18f86181d2f0e9a7ed297a025e220aeabf63fdebd3a2ddff` |
| darling_macro | 0.20.11 | MIT | github.com/TedDriggs/darling | `82a51e0b65b158de02ffb5d753ea4cd03529743b` | `fc34b93ccb385b40dc71c6fceac4b2ad23662c7eeb248cf10d529b7e055b6ead` |
| darling_macro | 0.24.1 | MIT | github.com/TedDriggs/darling | `6b68cbe74cf5a7684c406f09558afc854022f824` | `2ac7135c3ef02b2f7833bbeb1be5ba7f966dcde8a87c6b87f65a778d71a02785` |
| data-url | 0.3.2 | MIT OR Apache-2.0 | github.com/servo/rust-url | `91377f48bf35011d042aa5abef9e7f2a0a625aaa` | `be1e0bca6c3637f992fc1cc7cbc52a78c1ef6db076dbf1059c4323d6a2048376` |
| deranged | 0.5.9 | MIT OR Apache-2.0 | github.com/jhpratt/deranged | `aeb03a4be17fe287d5fdd509e9ae134d4b9999fc` | `9e9de72ce2ad1f90dc62fa25f0f430ef85eb4b0d8fa0be4f30373bc40a21d28e` |
| derive_more | 2.1.1 | MIT | github.com/JelteF/derive_more | `f7bb41ac054c060caaf5ff3212e74e42794cb4b4` | `d751e9e49156b02b44f9c1815bcb94b984cdcc4396ecc32521c739452808b134` |
| derive_more-impl | 2.1.1 | MIT | github.com/JelteF/derive_more | `f7bb41ac054c060caaf5ff3212e74e42794cb4b4` | `799a97264921d8623a957f6c3b9011f3b5492f557bbb7a5a19b7fa6d06ba8dcb` |
| digest | 0.10.7 | MIT OR Apache-2.0 | github.com/RustCrypto/traits | `344389411fd9718a0742435152e933a9e71461ee` | `9ed9a281f7bc9b7576e61468ba615a66a5c8cfdff42420a70aa82701a3b1e292` |
| displaydoc | 0.2.7 | MIT OR Apache-2.0 | github.com/yaahc/displaydoc | `20016487354a96b22a7b97974483ae4f51a1bddc` | `c6232dd377dcc64799954cbd3a9bb882e9cdc1308ccd87b1c098f1fb2eaf82a8` |
| dpi | 0.1.2 | Apache-2.0 AND MIT | github.com/rust-windowing/winit | `587ade844dfb0eada3696ba1cb263c66eea80581` | `d8b14ccef22fc6f5a8f4d7d768562a182c04ce9a3b3157b91390b52ddfdf1a76` |
| dtoa | 1.0.11 | MIT OR Apache-2.0 | github.com/dtolnay/dtoa | `4c30faa2d91f167d7c343d13ebc9d30730456d56` | `4c3cf4824e2d5f025c7b531afcb2325364084a16806f6d47fbc1f5fbd9960590` |
| dtoa-short | 0.3.5 | MPL-2.0 | github.com/upsuper/dtoa-short | `2d905cdb8b2e08163dc0d015f529877fe657b4ef` | `cd1511a7b6a56299bd043a9c167a6d2bfb37bf84a6dfceaba651168adfb43c87` |
| either | 1.19.0 | MIT OR Apache-2.0 | github.com/rayon-rs/either | `1e4326df5972eb4a0ced66cb3b67fa0c5aca8493` | `0e9c71c2167ca323c882b99918929403426e2373ea17242ff5653e0d5e1058be` |
| encoding_rs | 0.8.44 | (Apache-2.0 OR MIT) AND BSD-3-Clause | github.com/hsivonen/encoding_rs | `8e8556af3cf976392b993a9dbe07456a5ef58469` | `89253175e704ab869b04f9db224d38630e0211a53841a79181deae0889e169f5` |
| enumn | 0.1.14 | MIT OR Apache-2.0 | github.com/dtolnay/enumn | `06b55bc0990679958d92baec545a6cd79c6e4fa7` | `2f9ed6b3789237c8a0c1c505af1c7eb2c560df6186f01b098c3a1064ea532f38` |
| equivalent | 1.0.3 | Apache-2.0 OR MIT | github.com/indexmap-rs/equivalent | `4ee88130bd4ab3229e785533da2af641be22deac` | `00d174d5400e5e8fd687ad1049e2f578285fa914201b1af7e8b112a4546bd826` |
| euclid | 0.22.14 | MIT OR Apache-2.0 | github.com/servo/euclid | `60f2bd96deec0c0683eeddd737e0dc070a25e9af` | `f1a05365e3b1c6d1650318537c7460c6923f1abdd272ad6842baa2b509957a06` |
| fastrand | 2.5.0 | Apache-2.0 OR MIT | github.com/smol-rs/fastrand | `7a1cc2c69ebde6cb3442809213db3fe19a2cb0ba` | `da7c62ceae207dd37ea5b845da6a0696c799f85e97da1ab5b7910be3c1c80223` |
| fdeflate | 0.3.7 | MIT OR Apache-2.0 | github.com/image-rs/fdeflate | `c365c7e6ffa81feb2e1fb762eed7299f05c9b0ca` | `1e6853b52649d4ac5c0bd02320cddc5ba956bdb407c4b75a2c6b75bf51500f8c` |
| fearless_simd | 0.4.1 | Apache-2.0 OR MIT | github.com/linebender/fearless_simd | `0e8e242aaa8031cdc9708f99334e8998d4725916` | `b97b65636e5b9ef369943878ac74335ba1c55c1cb6adbf1e2c293c624248d693` |
| fixedbitset | 0.5.7 | MIT OR Apache-2.0 | github.com/petgraph/fixedbitset | `f1db5d17dabc4b8f3ba68c1228a3ee7601c7f33c` | `1d674e81391d1e1ab681a28d99df07927c6d4aa5b027d7da16ba32d1d21ecd99` |
| flate2 | 1.1.10 | MIT OR Apache-2.0 | github.com/rust-lang/flate2-rs | `ed93d4fc60eaf876c6aded741bf992d524551930` | `6e634e2e0ebac1ee034020da1ca582e17ffe4e0f5e985823721e168928136dcb` |
| float-cmp | 0.9.0 | MIT | github.com/mikedilger/float-cmp | `4844daf1ad47c9363a96ba8954d3c30d6a008591` | `98de4bbd547a563b716d8dfa9aad1cb19bfab00f4fa09a6a4ed21dbcf44ce9c4` |
| fnv | 1.0.7 | Apache-2.0 / MIT | github.com/servo/rust-fnv | `4b4784ebfd3332dc61f0640764d6f1140e03a9ab` | `3f9eec918d3f24069decb9af1554cad7c880e2da24a9afd88aca000531ab82c1` |
| foldhash | 0.1.5 | Zlib | github.com/orlp/foldhash | `42461756c3760f9165c90d9626a49e36980a1e47` | `d9c4f5dac5e15c24eb999c26181a6ca40b39fe946cbe4c263c7209467bc83af2` |
| fontdb | 0.24.0 | MIT | github.com/RazrFalcon/fontdb | `ecb707e6096b59146b750f66a32c125a43515192` | `2660c5e9157bf76d2db1294e4a9feba604ef610819a3b591088d0d8392a3290f` |
| fontsan | 0.7.0 | BSD-3-Clause | github.com/servo/fontsan | `04d3e6cc12893731bb916ece3afe3bf93cb8089e` | `cff76797a10ce7f8439f6e0a5ed06783e26e60d97e28c8c1c623a608ca67cff9` |
| form_urlencoded | 1.2.2 | MIT OR Apache-2.0 | github.com/servo/rust-url | `91377f48bf35011d042aa5abef9e7f2a0a625aaa` | `cb4cb245038516f5f85277875cdaa4f7d2c9a0fa0468de06ed190163b1581fcf` |
| generic-array | 0.14.7 | MIT | github.com/fizyk20/generic-array | `1397d2bb40c406901b7827161b73b34b056cbe3a` | `85649ca51fd72272d7821adaf274ad91c288277713d9c18820d8499a7ff69e9a` |
| getrandom | 0.2.17 | MIT OR Apache-2.0 | github.com/rust-random/getrandom | `b625985d852600a3eeb68556811e59e7c9a6a098` | `ff2abc00be7fca6ebc474524697ae276ad847ad0a6b3faa4bcb027e9a4614ad0` |
| gif | 0.14.2 | MIT OR Apache-2.0 | github.com/image-rs/image-gif | `a0127e2acbef333398414bb61d097c19c1b09d8a` | `ee8cfcc411d9adbbaba82fb72661cc1bcca13e8bba98b364e62b2dba8f960159` |
| glifo | 0.3.0 | Apache-2.0 OR MIT | github.com/linebender/vello | `7073a85d61ee099d1d3595651ddf92302b074348` | `282a26c1e23de04bdab3e34a21b6f877a479b96737792516b9b8b8f69b6661be` |
| glob | 0.3.4 | MIT OR Apache-2.0 | github.com/rust-lang/glob | `cfa2a58f2e44373573f657ec25b3621e44714dee` | `e4eba85ea1d0a966a983acd07deee566e67395d2d96b6fb39e62b5a833f1eb0b` |
| glow | 0.17.0 | MIT OR Apache-2.0 OR Zlib | github.com/grovesNL/glow | `0983efbc97fa4815f18990ef2a59540ede94f6f6` | `29038e1c483364cc6bb3cf78feee1816002e127c331a1eec55a4d202b9e1adb5` |
| guillotiere | 0.7.0 | MIT/Apache-2.0 | github.com/nical/guillotiere | `11f558ba8a2c80c23e6adb5934d73b352d81eee8` | `6b17e70c989c36bad147b27a58d148c0741c51448aa5653436547323e524d0ab` |
| harfbuzz-sys | 0.8.0 | MIT OR Apache-2.0 | github.com/servo/rust-harfbuzz | `0d3ac4d874ed81a637c05f90d29ce4024b72c745` | `b13f1cead31867a0d5b5e8905c262bc02b77701b8cd45ef402988f33745f9b7c` |
| harfrust | 0.12.0 | MIT | github.com/harfbuzz/harfrust | `60b28ea22b5261710018d69c168a762bcb28794c` | `c03d949a14aa089bbb282f7dd76a498a7f684428e4257202efc119ec010376f9` |
| hashbrown | 0.15.5 | MIT OR Apache-2.0 | github.com/rust-lang/hashbrown | `b751eef8e99ccf3652046ef4a9e1ec47c1bfb78d` | `9229cfe53dfd69f0609a49f65461bd93001ea1ef889cd5529dd176593f5338a1` |
| hashbrown | 0.17.1 | MIT OR Apache-2.0 | github.com/rust-lang/hashbrown | `c62a63a61b7caf2de8f9ecb7b06a66b0ab6bdf3d` | `ed5909b6e89a2db4456e54cd5f673791d7eca6732202bbf2a9cc504fe2f9b84a` |
| headers | 0.4.2 | MIT | github.com/hyperium/headers | `50bfa1e1a184a6328c108edabe3d1d347d3d0e27` | `fc2700e3fe555c3310aa7286cac6167449f2c87e0eb58769c9208a1c58a1d106` |
| headers-core | 0.3.0 | MIT | github.com/hyperium/headers | `b8cf384cc40c75a659240ea7d07898e65db72d4e` | `54b4a22553d4242c49fddb9ba998a99962b5cc6f22cb5a3482bec22522403ce4` |
| heck | 0.5.0 | MIT OR Apache-2.0 | github.com/withoutboats/heck | `070693322aee7c5c7fbee7c9964bf8d7d3a29c96` | `2304e00983f87ffb38b55b444b5e3b60a884b5d30c0fca7d82fe33449bbe55ea` |
| hermit-abi | 0.5.3 | MIT OR Apache-2.0 | github.com/hermit-os/hermit-rs | `c0b97d12a2f1f65610ba84e6f23fbb69fc70c0f7` | `e17592d60ebacc7d5e169f4663c5f84f9161cc90328abcfe8456f41e4dfcb284` |
| html5ever | 0.40.1 | MIT OR Apache-2.0 | github.com/servo/html5ever | `d7232d746d3112f08926d169591576bd12fe2fdf` | `456a1a377e608e555d22ddab27ac0114bc7a7b4199078108e34c2aeae6c9b130` |
| http | 1.5.0 | MIT OR Apache-2.0 | github.com/hyperium/http | `e559023f67e3fad6ecc3ee91307be178e0f13626` | `918d3568bebf352712bc2ef3d46a8bcf1a75b373be6539de198e9105cbbf9ce0` |
| http-body | 1.1.0 | MIT | github.com/hyperium/http-body | `2fb78de9c875c364b7eb1a1a117acc3b83ffb13a` | `ca2a8f2913ee65f60facd6a5905613afaa448497a0230cc41ce022d93290bc2c` |
| httpdate | 1.0.3 | MIT OR Apache-2.0 | github.com/pyfisch/httpdate | `638da761065df8b67282b0a1d139c0a7e4a02429` | `df3b46402a9d5adb4c86a0cf463f42e19994e3ee891101b1841f30a545cb49a9` |
| hyper | 1.12.0 | MIT | github.com/hyperium/hyper | `7f67699ce32d717081deb7018b4da7d679277ac0` | `2c3e324da4c95177d6291d4c8730197c0d1822f8a9766814a4a44fa5ab797c9c` |
| icu_collections | 2.4.0 | Unicode-3.0 | github.com/unicode-org/icu4x | `031d9e89f6a3fd26a14491e090729d46094328fd` | `fe090a42a0d3140fed8c63263ed1fecc02422cb2f4bfe55cd8de2de1b8e105dd` |
| icu_locale_core | 2.4.0 | Unicode-3.0 | github.com/unicode-org/icu4x | `031d9e89f6a3fd26a14491e090729d46094328fd` | `25d29b57c17c6528d0dc4c2141cfa2d08200465acb537a3909c8b9b8a4a984c0` |
| icu_locale_fallback | 2.4.0 | Unicode-3.0 | github.com/unicode-org/icu4x | `031d9e89f6a3fd26a14491e090729d46094328fd` | `406e9ae5b95d8aa37db6293405931b766a6172d1848daee22d70eb44aa78a43c` |
| icu_locale_fallback_data | 2.4.0 | Unicode-3.0 | github.com/unicode-org/icu4x | `031d9e89f6a3fd26a14491e090729d46094328fd` | `3632ebbd0c5c3637ba1a40bdf8288f2cb9d2268e73441a69d5fc73d7cfbaa87c` |
| icu_normalizer | 2.4.0 | Unicode-3.0 | github.com/unicode-org/icu4x | `031d9e89f6a3fd26a14491e090729d46094328fd` | `404a3b8c486f205c87fe7f23ab6894d84a4ad76d6e4671ae1981ba7def87c599` |
| icu_normalizer_data | 2.4.0 | Unicode-3.0 | github.com/unicode-org/icu4x | `031d9e89f6a3fd26a14491e090729d46094328fd` | `20f0088bfb7441f7a37fa0c9e20cb1e9cba1cac6b9ce8cd2a6dc44294f8e8cbc` |
| icu_properties | 2.4.0 | Unicode-3.0 | github.com/unicode-org/icu4x | `031d9e89f6a3fd26a14491e090729d46094328fd` | `2ea83d991c56158f7cf898bf86436c1dfe7c07ee82fe88951f2695c9f37a379f` |
| icu_properties_data | 2.4.0 | Unicode-3.0 | github.com/unicode-org/icu4x | `031d9e89f6a3fd26a14491e090729d46094328fd` | `b98f29af82d02b163fe50161388350e743bb3107cc8e8c2f7314cb505faddfcf` |
| icu_provider | 2.4.0 | Unicode-3.0 | github.com/unicode-org/icu4x | `031d9e89f6a3fd26a14491e090729d46094328fd` | `413099315636c588ee8f011190b329b971a09a8fae9d157e01dcae8379468514` |
| icu_segmenter | 2.4.0 | Unicode-3.0 | github.com/unicode-org/icu4x | `031d9e89f6a3fd26a14491e090729d46094328fd` | `61b195dc605f81aa181bec9b3bd14e26cc71b903e4644f6af578ca9772c7a710` |
| icu_segmenter_data | 2.4.0 | Unicode-3.0 | github.com/unicode-org/icu4x | `031d9e89f6a3fd26a14491e090729d46094328fd` | `913a7d294ad056474749152107309fdee3ec17d2482d1ab70fdc61a81c64b9a3` |
| ident_case | 1.0.1 | MIT/Apache-2.0 | github.com/TedDriggs/ident_case | `bf0d863e3006b40a0d923a81d7b2dd2db1136c2c` | `b9e0384b61958566e926dc50660321d12159025e767c18e043daf26b70104c39` |
| idna | 1.1.0 | MIT OR Apache-2.0 | github.com/servo/rust-url | `91377f48bf35011d042aa5abef9e7f2a0a625aaa` | `3b0875f23caa03898994f6ddc501886a45c7d3d62d04d2d90788d47be1b1e4de` |
| idna_adapter | 1.2.2 | Apache-2.0 OR MIT | github.com/hsivonen/idna_adapter | `1d9782f32c9e29218542b4a9411a82b35ec5c8cb` | `cb68373c0d6620ef8105e855e7745e18b0d00d3bdb07fb532e434244cdb9a714` |
| image | 0.25.10 | MIT OR Apache-2.0 | github.com/image-rs/image | `76e57184f22772dad1138e96954e57945406b15e` | `85ab80394333c02fe689eaf900ab500fbd0c2213da414687ebf995a65d5a6104` |
| image-webp | 0.2.4 | MIT OR Apache-2.0 | github.com/image-rs/image-webp | `08b57d473c90148dd4b37a075df365afc3dfcc2b` | `525e9ff3e1a4be2fbea1fdf0e98686a6d98b4d8f937e1bf7402245af1909e8c3` |
| imagesize | 0.15.0 | MIT | github.com/Roughsketch/imagesize | `7dba0c7ae38ee1b90024f82cbb8fab12d6b1b4eb` | `65b27460c2c92b037f3f94c538ed9a3342f3fdf923606781629ccb35f82d042a` |
| indexmap | 2.14.2 | Apache-2.0 OR MIT | github.com/indexmap-rs/indexmap | `41a870887c4c77adf665886e63df08f406bfe37a` | `cc4e190f5d26ca7051642629da2c52fc03bde85a03197c99408dcd291734c855` |
| inventory | 0.3.25 | MIT OR Apache-2.0 | github.com/dtolnay/inventory | `ffc228473d7aa20ca0790ddccc894a20682489ae` | `6928282826c822ad91bf1c9a1cb90a30ba1c26770749929b4656cd6be829cd7c` |
| ipc-channel | 0.23.1 | MIT OR Apache-2.0 | github.com/servo/ipc-channel | `7448f0ef5cf2754043518f40eacdc9c3eb00c728` | `cf1d9c442a2ec790209da8a49f99f7dae0fad31ba24c9947b01ce5e8de8bddcb` |
| itertools | 0.14.0 | MIT OR Apache-2.0 | github.com/rust-itertools/itertools | `a015a6831525ee1637df747d3f530a627d9741bf` | `2b192c782037fadd9cfa75548310488aabdbf3d2da73885b31bd0abd03351285` |
| itertools | 0.15.0 | MIT OR Apache-2.0 | github.com/rust-itertools/itertools | `37bd72aa6d58e594711d127b52418ca5e58b6091` | `8b4baf93f58d4425749ca49a51c50ebab072c5df6994d08fed93541c331481dc` |
| itoa | 1.0.18 | MIT OR Apache-2.0 | github.com/dtolnay/itoa | `af77385d0daf4d0e949e81f2588be2e44f69f086` | `8f42a60cbdf9a97f5d2305f08a87dc4e09308d1276d28c869c684d7777685682` |
| keyboard-types | 0.8.3 | MIT OR Apache-2.0 | github.com/rust-windowing/keyboard-types | `24446d6fe974bca4303affd07072939f4a7aab6f` | `0fbe853b403ae61a04233030ae8a79d94975281ed9770a1f9e246732b534b28d` |
| kurbo | 0.13.1 | Apache-2.0 OR MIT | github.com/linebender/kurbo | `838b69ef35f3381cefc007d86aa8364f5a6dc673` | `4b60dfc32f652b926df6192e55525b16d186c69d47876c3ead4da5cc9f8450e2` |
| libc | 0.2.191 | MIT OR Apache-2.0 | github.com/rust-lang/libc | `ccc2ff4aaab962f7fde7b8edda7379c045b1ca12` | `65cd87c9a6d60536580b316ad985726104730a5151c99845f2490428ef91240a` |
| libm | 0.2.16 | MIT | github.com/rust-lang/compiler-builtins | `dfd2203a4d6110820ad7bb65cafe1bf331a03a3d` | `b6d2cec3eae94f9f509c767b45932f1ada8350c4bdb85af2fcab4a3c14807981` |
| libz-sys | 1.1.31 | MIT OR Apache-2.0 | github.com/rust-lang/libz-sys | `595fbb7995b2c5e15371963103d33192fd066857` | `ff20308ecabd635a2dc1dd5834a86c462cca20efba1d5d3ddab1f8aea3455cb2` |
| linebender_resource_handle | 0.1.1 | Apache-2.0 OR MIT | github.com/linebender/raw_resource_handle | `bd8694ffca7550e8d569346c50de5fdcb5c51a7f` | `d4a5ff6bcca6c4867b1c4fd4ef63e4db7436ef363e0ad7531d1558856bae64f4` |
| litemap | 0.8.3 | Unicode-3.0 | github.com/unicode-org/icu4x | `a14f2dad852be26bad277ae704abf27a15cbfed1` | `47d9d19d1d6efa0109d2f65ff4c85cddd50bd572e5a00127ab10987290bcefae` |
| lock_api | 0.4.14 | MIT OR Apache-2.0 | github.com/Amanieu/parking_lot | `d7828fff7b5d6327ae608e82db45f888b344449a` | `224399e74b87b5f3557511d98dff8b14089b3dadafcab6bb93eab67d3aace965` |
| log | 0.4.35 | MIT OR Apache-2.0 | github.com/rust-lang/log | `a46450ba763280c29d35b53aa8ad0ff1fc1d78d0` | `5d15f86d12da44cd8d4ec3c73984537eb7f4cadcb9776a575a0a8a05ca11005c` |
| malloc_size_of_derive | 0.1.3 | MIT OR Apache-2.0 | github.com/bholley/malloc_size_of_derive | `9439206419825b9b3687b5a9d9a9a6dd29330854` | `f44db74bde26fdf427af23f1d146c211aed857c59e3be750cf2617f6b0b05c94` |
| markup5ever | 0.40.0 | MIT OR Apache-2.0 | github.com/servo/html5ever | `a193ea7f2492d1e51eb32955a09c241345348dba` | `0ab3dc68ac4a0f5719e560136778c1ee716e296030d75dbd4484e37e39e3a842` |
| memchr | 2.8.3 | Unlicense OR MIT | github.com/BurntSushi/memchr | `5fdb40c054e1fff359a2f7bdf7f87a13b34b465d` | `cf8baf1c55e62ffcace7a9f06f4bd9cd3f0c4beb022d3b367256b91b87513d98` |
| memmap2 | 0.9.12 | MIT OR Apache-2.0 | github.com/RazrFalcon/memmap2-rs | `4692b056b327aa08dace3a7e345a99e4cff77534` | `6b0ca1155a9ecc25aa12e8313a119f32edf0255fea2f44839c4ac0a12b2ac157` |
| mime | 0.3.17 | MIT OR Apache-2.0 | github.com/hyperium/mime | `ce5062d216bf757a0ed3fc70f0fe255d1c8d74ae` | `6877bb514081ee2a7ff5ef9de3281f14a4dd4bceac4c09388074a6b5df8a139a` |
| miniz_oxide | 0.8.9 | MIT OR Zlib OR Apache-2.0 | github.com/Frommi/miniz_oxide/tree/master/miniz_oxide | `44e43c7786e379b2b1a7fde4aa0e63be719e583d` | `1fa76a2c86f704bdb222d66965fb3d63269ce38518b83cb0575fca855ebb6316` |
| miniz_oxide | 0.9.1 | MIT OR Zlib OR Apache-2.0 | github.com/Frommi/miniz_oxide/tree/master/miniz_oxide | `4e582392df3a739d2b0dfd2c537dc33e8942be38` | `b63fbc4a50860e98e7b2aa7804ded1db5cbc3aff9193adaff57a6931bf7c4b4c` |
| moxcms | 0.8.1 | BSD-3-Clause OR Apache-2.0 | github.com/awxkee/moxcms | `c4affa196727d17f21a7f6f148a558867decb470` | `bb85c154ba489f01b25c0d36ae69a87e4a1c73a72631fc6c0eb6dde34a73e44b` |
| multiversion_no_op | 1.0.0 | Apache-2.0 OR MIT | github.com/hsivonen/multiversion_no_op | `5463f2d9272305f21eed5111eaf221a195abf2f4` | `743fb55ba31b18fb1ecef6bdc9aa2743314978ac084044301a7eee33fb99a20d` |
| new_debug_unreachable | 1.0.6 | MIT | github.com/mbrubeck/rust-debug-unreachable | `4b3fa6e0b6d728ed2018690e087a440dd29e7d06` | `650eef8c711430f1a879fdd01d4745a7deea475becfb90269c06775983bbf086` |
| num-complex | 0.4.6 | MIT OR Apache-2.0 | github.com/rust-num/num-complex | `91fdc06356c0c868cb88b5a180859023c57e6e50` | `73f88a1307638156682bada9d7604135552957b7818057dcef22705b4d509495` |
| num-conv | 0.2.2 | MIT OR Apache-2.0 | github.com/jhpratt/num-conv | `b022fe87ac3ad5b0d7606ea7465549eaa92efe97` | `521739c6d2bac4aa25192232afe6841231376b2b26d4d9fae5ecf8ca5772e441` |
| num-derive | 0.4.2 | MIT OR Apache-2.0 | github.com/rust-num/num-derive | `7cc33515dd2ae0eb43c5795c50ce49c554e8ba02` | `ed3955f1a9c7c0c15e092f9c887db08b1fc683305fdf6eb6684f22555355e202` |
| num-integer | 0.1.47 | MIT OR Apache-2.0 | github.com/rust-num/num-integer | `765fd9b3eb9397471d41b9f613e530f6d9d6383f` | `7ce2d95d4b3734dc35aa2f45e1aa22cd416814592a4f9d9205e11affd5b8e10b` |
| num-traits | 0.2.19 | MIT OR Apache-2.0 | github.com/rust-num/num-traits | `7ec3d41d39b28190ec1d42db38021107b3951f3a` | `071dfc062690e90b734c0b2273ce72ad0ffa95f0c74596bc250dcfd960262841` |
| num_cpus | 1.17.0 | MIT OR Apache-2.0 | github.com/seanmonstar/num_cpus | `342af76b486335e5a955c7314fc96cd104e7a17b` | `91df4bbde75afed763b708b7eee1e8e7651e02d97f6d5dd763e89367e957b23b` |
| num_enum | 0.7.6 | BSD-3-Clause OR MIT OR Apache-2.0 | github.com/illicitonion/num_enum | `f11d81c6cb644489f8a0fc24f9eea9a53d66f92e` | `5d0bca838442ec211fa11de3a8b0e0e8f3a4522575b5c4c06ed722e005036f26` |
| num_enum_derive | 0.7.6 | BSD-3-Clause OR MIT OR Apache-2.0 | github.com/illicitonion/num_enum | `f11d81c6cb644489f8a0fc24f9eea9a53d66f92e` | `680998035259dcfcafe653688bf2aa6d3e2dc05e98be6ab46afb089dc84f1df8` |
| ohos-deviceinfo | 0.1.0 | Apache-2.0 | github.com/openharmony-rs/ohos-deviceinfo | `d0f87bc6aa2b9acc9ca1f35605c7284ddcbb1a9a` | `b8e75dc6477dd4a8bafe2e7b1feb9cf10e882d3b15903e51f320a80015a59d93` |
| ohos-deviceinfo-sys | 0.1.8 | Apache-2.0 | github.com/openharmony-rs/ohos-sys | `8b5a34ec5df51be146005a81473f445c289b6011` | `3defe76a7ae47ede1fc8cb52b8c076409fe87181f251f62170bf3455bf5961a2` |
| ordered-channel | 1.2.0 | MIT OR Apache-2.0 | gitlab.com/kornelski/ordered-channel | `9653295d7b5c50e80d07022f4931c9ab22ac05c1` | `95be4d57809897b5a7539fc15a7dfe0e84141bc3dfaa2e9b1b27caa90acf61ab` |
| parking_lot | 0.12.5 | MIT OR Apache-2.0 | github.com/Amanieu/parking_lot | `d7828fff7b5d6327ae608e82db45f888b344449a` | `93857453250e3077bd71ff98b6a65ea6621a19bb0f559a85248955ac12c45a1a` |
| peniko | 0.6.1 | Apache-2.0 OR MIT | github.com/linebender/peniko | `e1bb9ef26282a2dd48058a3a7154691548fe4980` | `839c8299360d2e998bdb106dc0a6cd71dcc5f4df51df1b620361bf50e283cca6` |
| percent-encoding | 2.3.2 | MIT OR Apache-2.0 | github.com/servo/rust-url | `91377f48bf35011d042aa5abef9e7f2a0a625aaa` | `9b4f627cb1b25917193a259e49bdad08f671f8d9708acfd5fe0a8c1455d87220` |
| petgraph | 0.8.3 | MIT OR Apache-2.0 | github.com/petgraph/petgraph | `162903562ce5b00cdba390a0d9c1bb80f1c75bf5` | `8701b58ea97060d5e5b155d383a69952a60943f0e6dfe30b04c287beb0b27455` |
| phf | 0.14.0 | MIT | github.com/rust-phf/rust-phf | `1620d169aad09b02ef02a97b272470152e100bd6` | `010378780309880b08997fae13be7834dba947d36393bd372f2b1556deb2a2f6` |
| phf_codegen | 0.14.0 | MIT | github.com/rust-phf/rust-phf | `1620d169aad09b02ef02a97b272470152e100bd6` | `41b585a510fb76fdebead6897982ef2a03a21d8e6cbcca904999742a4afc6ffe` |
| phf_generator | 0.14.0 | MIT | github.com/rust-phf/rust-phf | `1620d169aad09b02ef02a97b272470152e100bd6` | `aeb62e0959d5a1bebc965f4d15d9e2b7cea002b6b0f5ba8cde6cc26738467100` |
| phf_macros | 0.14.0 | MIT | github.com/rust-phf/rust-phf | `1620d169aad09b02ef02a97b272470152e100bd6` | `5fa8d0ca26d424d27630da600c6624696e7dec8bf7b3b492b383c5dc49e5e085` |
| phf_shared | 0.14.0 | MIT | github.com/rust-phf/rust-phf | `1620d169aad09b02ef02a97b272470152e100bd6` | `c6fd9027e2d9319be6349febd1db4e8d02aa544921200c9b777720ac34a3aa89` |
| pico-args | 0.5.0 | MIT | github.com/RazrFalcon/pico-args | `56e8872fa31f168fd87e84fdc5f24bd60875daea` | `5be167a7af36ee22fe3115051bc51f6e6c7054c9348e28deb4f49bd6f705a315` |
| pin-project-lite | 0.2.17 | Apache-2.0 OR MIT | github.com/taiki-e/pin-project-lite | `3bdf763446aa78f90e3bdac1ef583e014832ab4c` | `a89322df9ebe1c1578d689c92318e070967d1042b512afbe49518723f4e6d5cd` |
| pkg-config | 0.3.34 | MIT OR Apache-2.0 | github.com/rust-lang/pkg-config-rs | `a8852cc148f8f7d504bf3a62444c298dcd3637f0` | `f6b464fbc74e149a392436b17d523f769e057cb6877f6a5c4618bc6f11800548` |
| png | 0.18.1 | MIT OR Apache-2.0 | github.com/image-rs/image-png | `2a3f980245e3ae38b82ade96533e7b450e8477bb` | `60769b8b31b2a9f263dae2776c37b1b28ae246943cf719eb6946a1db05128a61` |
| polycool | 0.4.0 | MIT OR Apache-2.0 | github.com/linebender/kurbo | `0ffc830eca99e68ba6bab54224cfac2c66bde2f9` | `50596ddc09eb5ad5f75cacd40209568e66df71baf86e1499a0e99c4cff12a5a6` |
| postcard | 1.1.3 | MIT OR Apache-2.0 | github.com/jamesmunns/postcard | `718aa6a6850456017c19eeff67303c633f875736` | `6764c3b5dd454e283a30e6dfe78e9b31096d9e32036b5d1eaac7a6119ccb9a24` |
| potential_utf | 0.1.6 | Unicode-3.0 | github.com/unicode-org/icu4x | `a14f2dad852be26bad277ae704abf27a15cbfed1` | `d83eb9bc6d8e5cf568e7a1101d60ee05e81ed50ea106026f3d18deeb046d7661` |
| powerfmt | 0.2.1 | MIT OR Apache-2.0 | github.com/jhpratt/powerfmt | `3dd45162a7e64d6b354675e4282d0550dce8bfc8` | `4a6394b9e965e73d0a289ee54f589087e2c676aedf60885baf52c76b771e4958` |
| precomputed-hash | 0.1.1 | MIT | github.com/emilio/precomputed-hash | not published¹ | `925383efa346730478fb4838dbe9137d2a47675ad789c546d150a6e1dd4ab31c` |
| primal-check | 0.3.4 | MIT OR Apache-2.0 | github.com/huonw/primal | `1a06fdf87b5aad2ec119dad631cd7985667b022d` | `dc0d895b311e3af9902528fbb8f928688abbd95872819320517cc24ca6b2bd08` |
| proc-macro-crate | 3.5.0 | MIT OR Apache-2.0 | github.com/bkchr/proc-macro-crate | `8f3d8b04539f732e09c07907f138a248c8d0eed9` | `e67ba7e9b2b56446f1d419b1d807906278ffa1a658a8a5d8a39dcb1f5a78614f` |
| proc-macro2 | 1.0.107 | MIT OR Apache-2.0 | github.com/dtolnay/proc-macro2 | `ed8a5497669cd63db33bf24646f261b012bbbc4a` | `985e7ec9bb745e6ce6535b544d84d6cd6f7ad8bd711c398938ae983b91a766d9` |
| pxfm | 0.1.30 | BSD-3-Clause OR Apache-2.0 | github.com/awxkee/pxfm | `acf7acd00cfc845307eda7b7b2de19e42303de82` | `d55d956fa96f5ec02be2e13af0e20391a5aa83d6a074e3ad368959d0fab299ea` |
| quick-error | 2.0.1 | MIT/Apache-2.0 | github.com/tailhook/quick-error | `25ab982bb0c43130ae48485151f4a232eeab7d0c` | `a993555f31e5a609f617c12db6250dedcac1b0a85076912c436e6fc9b2c8e6a3` |
| quote | 1.0.47 | MIT OR Apache-2.0 | github.com/dtolnay/quote | `723dcb47d3f0ddc896e17287c8a8d3f2ea2317d5` | `1fbf4db142a473a8d80c26bbf18454ed458bf8d26c8219c331daecfdbd079001` |
| rand | 0.10.3 | MIT OR Apache-2.0 | github.com/rust-random/rand | `9e7d328f60d6372ae22927f37d1a816fc980f25e` | `65c9fb96cbc91e3478eaae79a69fcd3f1ae4ad052e471fe6732fff548984b4af` |
| rand_core | 0.10.1 | MIT OR Apache-2.0 | github.com/rust-random/rand_core | `5cd7df496d0bff1d95534ac8b66a2610f5bf7808` | `63b8176103e19a2643978565ca18b50549f6101881c443590420e4dc998a3c69` |
| raw-window-handle | 0.6.2 | MIT OR Apache-2.0 OR Zlib | github.com/rust-windowing/raw-window-handle | `5fda8e8420b069368e9450e70c2869e32dcdffc1` | `20675572f6f24e9e76ef639bc5552774ed45f1c30e2951e1e99c59888861c539` |
| rayon | 1.12.0 | MIT OR Apache-2.0 | github.com/rayon-rs/rayon | `c9ced185ae3508246a9eb70c8407a1199bb1b77f` | `fb39b166781f92d482534ef4b4b1b2568f42613b53e5b6c160e24cfbfa30926d` |
| rayon-core | 1.13.0 | MIT OR Apache-2.0 | github.com/rayon-rs/rayon | `7af20d7692d5decbcb4adcaa079cd607e3e50814` | `22e18b0f0062d30d4230b2e85ff77fdfe4326feb054b9783a3460d8435c8ab91` |
| read-fonts | 0.41.0 | MIT OR Apache-2.0 | github.com/googlefonts/fontations | `cd4aaef57887dd1d8a5875e38c406843e66c9cab` | `046a7d674daf459825b32f5062056d6882db0d2f5a479fbd76ccfc870ac18709` |
| read-fonts | 0.43.3 | MIT OR Apache-2.0 | github.com/googlefonts/fontations | `a2be65244af08f403bf580f37d246146b58a473f` | `005c8acf251756c478b0bf402885bfd88a1476020c4c7e6060edc1aa68da38ea` |
| realfft | 3.5.1 | MIT | github.com/HEnquist/realfft | `44f36c40413629566566f3165e92ad20ebb0beed` | `538895ccf2006b473aafbe8d72770e21038961ab67aae34f3c577485fc703a7d` |
| regex | 1.13.1 | MIT OR Apache-2.0 | github.com/rust-lang/regex | `2b527599eb9eea0dcc288c704584f242f26a5c61` | `f020237b6c8eed93db2e2cb53c00c60a8e1bc73da7d073199a1180401450218d` |
| regex-automata | 0.4.18 | MIT OR Apache-2.0 | github.com/rust-lang/regex | `e6ad938198e68339a593f85d228f2d44cb54718f` | `ad8553b9b26413251cbf30e620595c7a41b3887f03da04579c0e6b0d6a06b4b2` |
| regex-syntax | 0.8.11 | MIT OR Apache-2.0 | github.com/rust-lang/regex | `140167995737fa11dfe11b8af8b9aa143b790b4e` | `d6f6ff9a378485b298a5286656da665ba74413d36db0979633275d2e708145d4` |
| resvg | 0.48.1 | Apache-2.0 OR MIT | github.com/linebender/resvg | `68b14c4c3bccdb60344c777406486b54c36ec1a4` | `67e3803f97b999e80cbf7c6ecdd07a8102204d92e1633cf48783720c521196bd` |
| rgb | 0.8.53 | MIT | github.com/kornelski/rust-rgb | `f453adedbbd1a672c654239b0eb0576de606d7d2` | `47b34b781b31e5d73e9fbc8689c70551fd1ade9a19e3e28cfec8580a79290cc4` |
| roxmltree | 0.21.2 | MIT OR Apache-2.0 | github.com/RazrFalcon/roxmltree | `b04f75f6a0d17c6248de3eb1b1013bf1bbac44ae` | `9da7f59328b09405b181fee1018925fad50f157afb43c0c0f5c522c256dd5907` |
| rustc-hash | 2.1.3 | Apache-2.0 OR MIT | github.com/rust-lang/rustc-hash | `c13e7ccca705e6255387a2ebc6dca142d6881621` | `6b1e7f9a428571be2dc5bc0505c13fb6bf936822b894ec87abf8a08a4e51742d` |
| rustc_version | 0.4.1 | MIT OR Apache-2.0 | github.com/djc/rustc-version-rs | `eeca449cca83e24150e46739e797aa82e9142809` | `cfcb3a22ef46e85b45de6ee7e79d063319ebb6594faafcf1c225ea92ab6e9b92` |
| rustfft | 6.4.1 | MIT OR Apache-2.0 | github.com/ejmahler/RustFFT | `4758ab0dd6f256c50ac8987c75c9cb96152dc2ca` | `21db5f9893e91f41798c88680037dba611ca6674703c1a18601b01a72c8adb89` |
| rustls | 0.23.46 | Apache-2.0 OR ISC OR MIT | github.com/rustls/rustls | `f04804fa13341a9b1dc02b5d1dd8a746f2c4b7fb` | `48e13bd8c0e9365c43cfa5c9e8f9ad49d3c8444926c9aac819e0e4dc503c8fdf` |
| rustls-pki-types | 1.15.1 | MIT OR Apache-2.0 | github.com/rustls/pki-types | `e29ad15465126acc089b39611bcec85e27ce0817` | `2f4925028c7eb5d1fcdaf196971378ed9d2c1c4efc7dc5d011256f76c99c0a96` |
| rustls-webpki | 0.103.16 | ISC | github.com/rustls/webpki | `8ecdcc76f56b68fda67a6adf7976f5eed5006a4c` | `4443b1b6d39d7c663139942cc4558262562d2c84123bb6282ef177009ca8b281` |
| rustversion | 1.0.23 | MIT OR Apache-2.0 | github.com/dtolnay/rustversion | `3a7c76605450b9a7299c6502a421909de9126a59` | `cf54715a573b99ac80df0bc206da022bcd442c974952c7b9720069370852e21f` |
| same-file | 1.0.6 | Unlicense/MIT | github.com/BurntSushi/same-file | `5799cd323b8eefd17a089c950dac113f66c89c9e` | `93fc1dc3aaa9bfed95e02e6eadabb4baf7e3078b0bd1b4d7b6b0b68378900502` |
| scopeguard | 1.2.0 | MIT OR Apache-2.0 | github.com/bluss/scopeguard | `bb988222848d39b0b9037b29da3a3a1eb05ebf4b` | `94143f37725109f92c262ed2cf5e59bce7498c01bcc1502d7b9afe439a4e9f49` |
| selectors | 0.41.0 | MPL-2.0 | github.com/servo/stylo | `34d25f741765a0bdb894f8271852e633299352d8` | `8322aebea94a6cc288ff884e8a49bdbceef60206c400c6c46d22e62fc4fd91a6` |
| semver | 1.0.28 | MIT OR Apache-2.0 | github.com/dtolnay/semver | `7625c7aa3f0e8ba21e099d1765bcebcb72aa8816` | `8a7852d02fc848982e0c167ef163aaff9cd91dc640ba85e263cb1ce46fae51cd` |
| serde | 1.0.229 | MIT OR Apache-2.0 | github.com/serde-rs/serde | `7fc3b4c30c94f73a96ebd1553f2b090d928fc3a8` | `4148590afebada386688f18773da617792bf2ef03ffc1e4cbd2b1d45b023e0ba` |
| serde_core | 1.0.229 | MIT OR Apache-2.0 | github.com/serde-rs/serde | `7fc3b4c30c94f73a96ebd1553f2b090d928fc3a8` | `67dca2c9c51e58a4791a4b1ed58308b39c64224d349a935ab5039aa360942a48` |
| serde_derive | 1.0.229 | MIT OR Apache-2.0 | github.com/serde-rs/serde | `7fc3b4c30c94f73a96ebd1553f2b090d928fc3a8` | `e7a5d71263a5a7d47b41f6b3f06ba276f10cc18b0931f1799f710578e2309348` |
| serde_json | 1.0.154 | MIT OR Apache-2.0 | github.com/serde-rs/json | `20572a6d066958542d79cdd6cf1a7cd9f0c8e1f8` | `e7e9cc8b1b85264074fbcc02a88680c4096b1e47df8f739dceb03bf482f04bd6` |
| serde_spanned | 1.1.2 | MIT OR Apache-2.0 | github.com/toml-rs/toml | `f72038c2508112dfaaedb227a511f91bf9b2015c` | `4e7523beb55eece201a2356bee0bbca0d1ab466c14c07703b2e0ee6d42cb0c2c` |
| serde_with | 3.24.0 | MIT OR Apache-2.0 | github.com/jonasbb/serde_with | `35ac84c8faf34d6822991bbf2397056994c9208b` | `df9adc193c780ef8f159aee8b61e2d5801aaa555e6eb0947fe45530ec506296f` |
| serde_with_macros | 3.24.0 | MIT OR Apache-2.0 | github.com/jonasbb/serde_with | `35ac84c8faf34d6822991bbf2397056994c9208b` | `3e17bbc68e28663bbbb90df47e058aa7eda4fb445b89fe70457bb94fbccf6e49` |
| servo-background-hang-monitor-api | 0.7.0 | MPL-2.0 | github.com/servo/servo | `aac43a3f31a259f04a574f5ec4e959c943ad7cc7` | `f487641017a8876d31316f298ecf5d34b79357e91356ed10b7ad0c9226c75e52` |
| servo-canvas-traits | 0.7.0 | MPL-2.0 | github.com/servo/servo | `aac43a3f31a259f04a574f5ec4e959c943ad7cc7` | `b7684bbead0695a10a4c859cae6cf62483b0a6912c71db436f8702c862d986f5` |
| servo-config | 0.7.0 | MPL-2.0 | github.com/servo/servo | `aac43a3f31a259f04a574f5ec4e959c943ad7cc7` | `a6620e2508f79b574b08d4ad9117cde953893eb84456941b465ef5303675b4de` |
| servo-config-macro | 0.7.0 | MPL-2.0 | github.com/servo/servo | `aac43a3f31a259f04a574f5ec4e959c943ad7cc7` | `d2cb8c3c207aad414dbd50aa1105a46b5a73f5cb7fa5c60ee5e4f4ab9c7397ab` |
| servo-constellation-traits | 0.7.0 | MPL-2.0 | github.com/servo/servo | `aac43a3f31a259f04a574f5ec4e959c943ad7cc7` | `6b0d7d314268935ee4bcd68e8846f74cf55b2baa06a1c7001a6c8b06cf8a8880` |
| servo-devtools-traits | 0.7.0 | MPL-2.0 | github.com/servo/servo | `aac43a3f31a259f04a574f5ec4e959c943ad7cc7` | `90d55dd38feed48abad439d5c66e8c0881919b66b8f95c492c49120929822c68` |
| servo-geometry | 0.7.0 | MPL-2.0 | github.com/servo/servo | `aac43a3f31a259f04a574f5ec4e959c943ad7cc7` | `2f853b21e7527f15d710eafb000ecc2bb5dbcf4e90b8a736ef6b920a768c54d1` |
| servo-hyper-serde | 0.7.0 | MIT OR Apache-2.0 | github.com/servo/servo | `aac43a3f31a259f04a574f5ec4e959c943ad7cc7` | `2cdacc3d3e98fc78639665af0fe3ce0a3fe0cf0241d14e1eebdf073f7d9d50e6` |
| servo-layout-api | 0.7.0 | MPL-2.0 | github.com/servo/servo | `aac43a3f31a259f04a574f5ec4e959c943ad7cc7` | `0d5587cba8e9361fb0ab97232ab28fa452a761a60bedf3c3b2a1d0baa764db0a` |
| servo-malloc-size-of | 0.7.0 | MIT OR Apache-2.0 | github.com/servo/servo | `aac43a3f31a259f04a574f5ec4e959c943ad7cc7` | `88dd0c1f464e6e360837a1bb610de6e3b4edf4caf446fcb4e8c8f593e167044e` |
| servo-media | 0.7.0 | MPL-2.0 | github.com/servo/servo | `aac43a3f31a259f04a574f5ec4e959c943ad7cc7` | `5cb3ee4f036fb8fd88384808d59bc9ae1e0f2231871907d646aea5f446c67fe5` |
| servo-media-audio | 0.7.0 | MPL-2.0 | github.com/servo/servo | `aac43a3f31a259f04a574f5ec4e959c943ad7cc7` | `562114eb2fadb5774c235b95d2596cb02b918baf4ccfe6c0c5c855974f5d9d29` |
| servo-media-derive | 0.7.0 | MPL-2.0 | github.com/servo/servo | `aac43a3f31a259f04a574f5ec4e959c943ad7cc7` | `3597386ea660c3e01e035f8c93d62293c1461c371c662a2bbd6d5d6ca31c321a` |
| servo-media-player | 0.7.0 | MPL-2.0 | github.com/servo/servo | `aac43a3f31a259f04a574f5ec4e959c943ad7cc7` | `730258a7bf65bf70fb857869fdf97872a8f5bf7a0484c3924ea364c23527e426` |
| servo-media-streams | 0.7.0 | MPL-2.0 | github.com/servo/servo | `aac43a3f31a259f04a574f5ec4e959c943ad7cc7` | `2af646df71d56bfc75caedc5943fc92a1c862f392010d1c04caebc93fcc52cff` |
| servo-media-thread | 0.7.0 | MPL-2.0 | github.com/servo/servo | `aac43a3f31a259f04a574f5ec4e959c943ad7cc7` | `b0836dae0872d2a1f5e46f39d94f925f3580f98e4983b3f9c8f7ee7b48bd9588` |
| servo-media-traits | 0.7.0 | MPL-2.0 | github.com/servo/servo | `aac43a3f31a259f04a574f5ec4e959c943ad7cc7` | `fa53d8b74c9ca081b151c66d09fc70c28b104033b72a6f613e208a21f375582f` |
| servo-media-webrtc | 0.7.0 | MPL-2.0 | github.com/servo/servo | `aac43a3f31a259f04a574f5ec4e959c943ad7cc7` | `b9ae7efb05594a630d758c2934388233c8d49d6ddd21f8114a8470f16f18784f` |
| servo-net-traits | 0.7.0 | MPL-2.0 | github.com/servo/servo | `aac43a3f31a259f04a574f5ec4e959c943ad7cc7` | `33ef7bce5ce3ae9bc966589e5c2ec407e0a679c9308f5ca12e23d1f5c58e576a` |
| servo-pixels | 0.7.0 | MPL-2.0 | github.com/servo/servo | `aac43a3f31a259f04a574f5ec4e959c943ad7cc7` | `29677430eed97c6bd5069c6612c5c1f0e6adfb91c0656d4b5da741a26c07c9a4` |
| servo-profile-traits | 0.7.0 | MPL-2.0 | github.com/servo/servo | `aac43a3f31a259f04a574f5ec4e959c943ad7cc7` | `ebf13d92b6649214e4f949c0ebbd6e0310422f286a8fda3ca3971ff70494681d` |
| servo-script-traits | 0.7.0 | MPL-2.0 | github.com/servo/servo | `aac43a3f31a259f04a574f5ec4e959c943ad7cc7` | `4b9ec4ad62d951a36af1ba07fc49e8e9df4ac6b47b5f60dd7443f21e56434656` |
| servo-storage-traits | 0.7.0 | MPL-2.0 | github.com/servo/servo | `aac43a3f31a259f04a574f5ec4e959c943ad7cc7` | `47291ce72351c3d65212d9b63541810e2200e38ee34674ea4e4d5315aeaea31a` |
| servo-tracing | 0.7.0 | MPL-2.0 | github.com/servo/servo | `aac43a3f31a259f04a574f5ec4e959c943ad7cc7` | `91062d25ac18b38a979b07dc8a6552ee5ca9ea0adc0254eebf51bea65ee622f2` |
| servo-webxr-api | 0.7.0 | MPL-2.0 | github.com/servo/servo | `aac43a3f31a259f04a574f5ec4e959c943ad7cc7` | `460e472b35e76570a1f670e0a936ddcdbf509980d70eddbcb6a3dfc08b6febf8` |
| servo_arc | 0.5.0 | MIT OR Apache-2.0 | github.com/servo/stylo | `34d25f741765a0bdb894f8271852e633299352d8` | `b4eceb519ec34d8290aaf6e46f435102f2bfa63f22b05a144fb16ab266d3b198` |
| sha1 | 0.10.7 | MIT OR Apache-2.0 | github.com/RustCrypto/hashes | `0b32162344ab2bbc3749889370927ae49137d4ae` | `a978451301f4db1d02937a4ab3ccce137717b81826e79b7d49ffe3244a13c3b8` |
| sha1_smol | 1.0.1 | BSD-3-Clause | github.com/mitsuhiko/sha1-smol | `f9d88bd70e9bd577c7c3afdc6acd0313af605324` | `bbfa15b3dddfee50a0fff136974b3e1bde555604ba463834a7eb7deb6417705d` |
| sha2 | 0.10.9 | MIT OR Apache-2.0 | github.com/RustCrypto/hashes | `82c36a428f8d6f05f3bfccdedb243e9d1f85359d` | `a7507d819769d01a365ab707794a4084392c824f54a7a6a7862f8c3d0892b283` |
| simd-adler32 | 0.3.10 | MIT | github.com/mcountryman/simd-adler32 | `d93164f98d7fe2daf883f2e1bec82778dd26199a` | `3a219298ac11a56ea9a6d2120044824d6f01aeb034955e7af7bc16858527deea` |
| simdutf8 | 0.1.5 | MIT OR Apache-2.0 | github.com/rusticstuff/simdutf8 | `87ee8d9d20b849eae1974a82b248ee6fe7491bcc` | `e3a9fe34e3e7a50316060351f37187a3f546bce95496156754b601a5fa71b76e` |
| simplecss | 0.2.2 | Apache-2.0 OR MIT | github.com/linebender/simplecss | `6eacb39ca5c4c2390bfde374ed0651f0e35c9d8d` | `7a9c6883ca9c3c7c90e888de77b7a5c849c779d25d74a1269b0218b14e8b136c` |
| siphasher | 1.0.4 | MIT OR Apache-2.0 | github.com/jedisct1/rust-siphash | `ed028f01a16205b0819fa83b0cdc052e2d5ac3b7` | `33f4fe9184a62d842c9ef383018f3306d8ba224fd9d836f56d7288308847c256` |
| skrifa | 0.44.0 | MIT OR Apache-2.0 | github.com/googlefonts/fontations | `b373cdbc6043a791fb23b3e0f413649458f5e63a` | `819ab7d62b1d3e72d9d9dea5650bac30424f9111364bb94928dbf5ecad1baa68` |
| skrifa | 0.46.2 | MIT OR Apache-2.0 | github.com/googlefonts/fontations | `f886c9069b08eaddcb1caced3a44b0d8291a159a` | `d4febebda507fb889c545a37a135517769d1391941013561292897961d1887d6` |
| slotmap | 1.1.1 | Zlib | github.com/orlp/slotmap | `2d56154b9b628f7d7a506f24ebc4f55a13ec8fed` | `bdd58c3c93c3d278ca835519292445cb4b0d4dc59ccfdf7ceadaab3f8aeb4038` |
| smallbitvec | 2.6.1 | MIT OR Apache-2.0 | github.com/servo/smallbitvec | `ad9b17eace931eb8190fa25f7775932ed48d370f` | `9b0e903ee191d8f7a8fbf0d712c3a1699d19e04ceba5ad1eb673053c7d938a09` |
| smallvec | 1.16.4 | MIT OR Apache-2.0 | github.com/servo/rust-smallvec | `eab77462eda2767d2386f1d251928f474827e1c7` | `d0d5d4b1981b8bbbb7aed204cae5726d683d5d6ac7e74ec54122ea28f315e7c8` |
| speexdsp-resampler | 0.1.0 | BSD-3-Clause | github.com/rust-av/speexdsp-rs | `b543f416aea84140e237bb3747a9d3a01c31fc9c` | `b72d540d5c565dbe1f891d7e21ceb21d2649508306782f1066989fccb0b363d3` |
| stable_deref_trait | 1.2.1 | MIT OR Apache-2.0 | github.com/storyyeller/stable_deref_trait | `30002b4228f7cdee309217b6bf6eee099dda0b00` | `6ce2be8dc25455e1f91df71bfa12ad37d7af1092ae736f3a6cd0e37bc7810596` |
| static_assertions | 1.1.0 | MIT OR Apache-2.0 | github.com/nvzqz/static-assertions-rs | `18bc65a094d890fe1faa5d3ccb70f12b89eabf56` | `a2eb9349b6444b326872e140eb1cf5e7c522154d69e7a0ffb0fb81c06b37543f` |
| strength_reduce | 0.2.4 | MIT OR Apache-2.0 | github.com/ejmahler/strength_reduce | `5b0a43d9d672ee2ac4e471f201fb13417a0a0c71` | `fe895eb47f22e2ddd4dabc02bce419d2e643c8e3b585c78158b349195bc24d82` |
| strict-num | 0.1.1 | MIT | github.com/RazrFalcon/strict-num | `edf5020c215063c3a9eb8fcac463c82eaceba6df` | `6637bab7722d379c8b41ba849228d680cc12d0a45ba1fa2b48f2a30577a06731` |
| string_cache | 0.11.0 | MIT OR Apache-2.0 | github.com/servo/string-cache | `0f6acf1c394f326dbf2adf9dfad2089cbdcf38ad` | `ffa8a5dbe8b3f0bbe29d4c3225daafaeead63afdc1b65fc4c01a1384166038e6` |
| string_cache_codegen | 0.11.2 | MIT OR Apache-2.0 | github.com/servo/string-cache | `5f845e2cdf504e76a53d6827a6f6d4bfcced1c93` | `928dcdf75e47626b3617a976ec205d9f057584c371c1f23b782129268d0e6edc` |
| strsim | 0.11.1 | MIT | github.com/rapidfuzz/strsim-rs | `76c5a900e6e12cfc605eee5ab6e36300384c8682` | `7da8b5736845d9f2fcb837ea5d9e2628564b3b043a70948a3f0b778838c5fb4f` |
| strum | 0.28.0 | MIT | github.com/Peternator7/strum | `7376771128834d28bb9beba5c39846cba62e71ec` | `9628de9b8791db39ceda2b119bbe13134770b56c138ec1d3af810d045c04f9bd` |
| strum_macros | 0.28.0 | MIT | github.com/Peternator7/strum | `7376771128834d28bb9beba5c39846cba62e71ec` | `ab85eea0270ee17587ed4156089e10b9e6880ee688791d45a905f5b1ca36f664` |
| stylo | 0.22.0 | MPL-2.0 | github.com/servo/stylo | `34d25f741765a0bdb894f8271852e633299352d8` | `7a255e46af2478628c48daab3774090caabd7812b1dd3ca98801c2b9c53491ed` |
| stylo_atoms | 0.22.0 | MPL-2.0 | github.com/servo/stylo | `34d25f741765a0bdb894f8271852e633299352d8` | `ee1771588fd747ef16f8b1990061b6f37c7d3251a0f9b74f405a44c0c5fb8966` |
| stylo_derive | 0.22.0 | MPL-2.0 | github.com/servo/stylo | `34d25f741765a0bdb894f8271852e633299352d8` | `4ee5c550c2e00a0b3c046bef84a9ee217e04e7c014142eeb96f9e48781c5cfd3` |
| stylo_dom | 0.22.0 | MPL-2.0 | github.com/servo/stylo | `34d25f741765a0bdb894f8271852e633299352d8` | `9febcb03f2b669ef06b3e8a5209e6280971ed55cecf83fbf3c0562fcf2aa2ec6` |
| stylo_malloc_size_of | 0.22.0 | MIT OR Apache-2.0 | github.com/servo/stylo | `34d25f741765a0bdb894f8271852e633299352d8` | `83ed69d55ee907599f093221004a28371925236f09f20961963020abf38bab6a` |
| stylo_static_prefs | 0.22.0 | MPL-2.0 | github.com/servo/stylo | `34d25f741765a0bdb894f8271852e633299352d8` | `6767c48a1590461db79f8b434ccc5a2d65600a3b716751f7999b35ccead1e024` |
| stylo_traits | 0.22.0 | MPL-2.0 | github.com/servo/stylo | `34d25f741765a0bdb894f8271852e633299352d8` | `645d6bfe40eb6e86ebe9f2d2adadcbced41bfa96175de7cb13d4d178d27bc83d` |
| subtle | 2.6.1 | BSD-3-Clause | github.com/dalek-cryptography/subtle | `5457b5448b021d1da101ababbb854e6657233943` | `13c2bddecc57b384dee18652358fb23172facb8a2c51ccc10d74c157bdea3292` |
| svgtypes | 0.16.1 | Apache-2.0 OR MIT | github.com/linebender/svgtypes | `fa62387aea54d7af49ad65b47bca8d731136dd56` | `695b5790b3131dafa99b3bbfd25a216edb3d216dad9ca208d4657bfb8f2abc3d` |
| syn | 2.0.119 | MIT OR Apache-2.0 | github.com/dtolnay/syn | `3295f9e9841785ac88a5e558c884854d5fb7d67f` | `872831b642d1a07999a962a351ed35b955ea2cfc8f3862091e2a240a84f17297` |
| syn | 3.0.9 | MIT OR Apache-2.0 | github.com/dtolnay/syn | `7c44a5565b4421035101cc16184e9af08ab5fc77` | `d78c8dee4c7bf0e14673097256fed6142ce9d3b85a408189d07482442145823b` |
| synstructure | 0.13.2 | MIT | github.com/mystor/synstructure | `91ead072fa43c55f35880bd7f75a2b0eab72a04e` | `728a70f3dbaf5bab7f0c4b1ac8d7ae5ea60a4b5549c8a5914361c99147a709d2` |
| synstructure | 0.14.0 | MIT | github.com/mystor/synstructure | `30cfafe36e42405e717a8e55da042a2835a698d7` | `901704edd0dfe137f1987838ee4f259e4e063c31371bdb423f7ae38ec6f77f02` |
| sys-locale | 0.3.2 | MIT OR Apache-2.0 | github.com/1Password/sys-locale | `7026899e5723a593a819abdf46e1d4b3794a4d68` | `8eab9a99a024a169fe8a903cf9d4a3b3601109bcc13bd9e3c6fff259138626c4` |
| taffy | 0.14.0 | MIT | github.com/DioxusLabs/taffy | `77f385683c1d698c91a23a259f87fdddf26925fb` | `639627c87f43b9181c811f40a6296409e093a17bc761214cba3c15df74f86b99` |
| tendril | 0.5.1 | MIT OR Apache-2.0 | github.com/servo/html5ever | `d64dfd4c21cf2451649107ade7eaf042d95fbc5a` | `5fed54709c5b3a53d09bb1c113ea4f5ceafd1e772ddcb0030a82e1d56c087b08` |
| thin-vec | 0.2.21 | MIT OR Apache-2.0 | github.com/mozilla/thin-vec | `326a2260b184146bbc3f12653993a115f7ec7d5e` | `d6a4b9ba8738cb4a4f399d37e266becfd475e75eb73425b87a05a2f2039ba63e` |
| thiserror | 1.0.69 | MIT OR Apache-2.0 | github.com/dtolnay/thiserror | `41938bd3a03a70d34ed8e53d99c89c770c7c9c41` | `b6aaf5339b578ea85b50e080feb250a3e8ae8cfcdff9a461c9ec2904bc923f52` |
| thiserror | 2.0.21 | MIT OR Apache-2.0 | github.com/dtolnay/thiserror | `b1827ee06f81a7d7f954e676771e19a97f9f8e3b` | `09e52cb86a36cede5cb101bf8908837b3e4c6e5e59fe7fd85c23fb56200d189e` |
| thiserror-impl | 1.0.69 | MIT OR Apache-2.0 | github.com/dtolnay/thiserror | `41938bd3a03a70d34ed8e53d99c89c770c7c9c41` | `4fee6c4efc90059e10f81e6d42c60a18f76588c3d74cb83a0b242a2b6c7504c1` |
| thiserror-impl | 2.0.21 | MIT OR Apache-2.0 | github.com/dtolnay/thiserror | `b1827ee06f81a7d7f954e676771e19a97f9f8e3b` | `fe5197923287db20a58125f0bc85c062f7f2c892de97b18c356f9efb14b28524` |
| thread_local | 1.1.10 | MIT OR Apache-2.0 | github.com/Amanieu/thread_local-rs | `4724199250d3307824cefe8a0fa5b232d95edae3` | `1ad99c4c6d32803332c548b1af0540b357b3f5fc0be8f6c6bfe8b2e6ae784070` |
| time | 0.3.55 | MIT OR Apache-2.0 | github.com/time-rs/time | `857d9c404c5b6f6cb64d1bfa604c695be3369e12` | `cdb87b95ec50ddfa440816d227a17b2ccbdda963a316a727fda0fc4334f7d134` |
| time-core | 0.1.9 | MIT OR Apache-2.0 | github.com/time-rs/time | `5d8737c39b9170c36c99cfc1f9f49cf0c059c63a` | `9e1c906769ad99c88eaa54e728060edef082f8e358ff32030cb7c7d315e81109` |
| time-macros | 0.2.32 | MIT OR Apache-2.0 | github.com/time-rs/time | `d1c55a454051c83e8fd2ad8431cf37b90dd6ea31` | `7e689342a48d2ea927c87ea50cabf8594854bf940e9310208848d680d668ed85` |
| tiny-skia | 0.12.0 | BSD-3-Clause | github.com/linebender/tiny-skia | `e48b9027762d33731e60e40eb17eaa4f583b0185` | `47ffee5eaaf5527f630fb0e356b90ebdec84d5d18d937c5e440350f88c5a91ea` |
| tiny-skia-path | 0.12.0 | BSD-3-Clause | github.com/linebender/tiny-skia/tree/master/path | `e48b9027762d33731e60e40eb17eaa4f583b0185` | `edca365c3faccca67d06593c5980fa6c57687de727a03131735bb85f01fdeeb9` |
| tinystr | 0.8.5 | Unicode-3.0 | github.com/unicode-org/icu4x | `031d9e89f6a3fd26a14491e090729d46094328fd` | `abc840c238154d58f9e86b2792261edfde565a17696e1cda1a449de27979801a` |
| tinyvec | 1.13.3 | Zlib OR Apache-2.0 OR MIT | github.com/Lokathor/tinyvec | `e343bbbb5a6594c9d57de718e6f232e6c0115dcb` | `fd3ca314f692efd6c868f8408f53fe444634a845f96c028b97d35f6a1f79f0ee` |
| to_shmem | 0.6.0 | MPL-2.0 | github.com/servo/stylo | `34d25f741765a0bdb894f8271852e633299352d8` | `bbf077da8dd01cc18453bd7d9bc3a945c262f5f344562e5f6deb3914b4dd6f14` |
| to_shmem_derive | 0.2.0 | MPL-2.0 | github.com/servo/stylo | `34d25f741765a0bdb894f8271852e633299352d8` | `0f8e91a023810fd2019648410d00eea13a71f21d849e71d6d9ed936fb8061174` |
| tokio | 1.54.1 | MIT | github.com/tokio-rs/tokio | `b53b2001ed2ccfd9b87e32a0c6da1a1eebdc80b8` | `b52fd3255258d26fac7264ab5a76b53c13367136dc011d3d6ad4f8ed070396a3` |
| toml | 1.1.8+spec-1.1.0 | MIT OR Apache-2.0 | github.com/toml-rs/toml | `ceac6e4412420d5a5c19e4f7293ad005cf1a31b2` | `20489e00e4d8741d6be680764cc12e270655e375a20d1011e844a9c3379e678d` |
| toml_datetime | 1.1.2+spec-1.1.0 | MIT OR Apache-2.0 | github.com/toml-rs/toml | `f72038c2508112dfaaedb227a511f91bf9b2015c` | `2b86d767906c6c42421dcba507eb9d203e779497710a47782a224bb871653053` |
| toml_edit | 0.25.17+spec-1.1.0 | MIT OR Apache-2.0 | github.com/toml-rs/toml | `ceac6e4412420d5a5c19e4f7293ad005cf1a31b2` | `e3641d5bbb5349a79e1020a242d251efbc546ad8048d133958323ce9c40a9c9c` |
| toml_parser | 1.1.5+spec-1.1.0 | MIT OR Apache-2.0 | github.com/toml-rs/toml | `ceac6e4412420d5a5c19e4f7293ad005cf1a31b2` | `baa693a8032d7e1cada7d0041e96126df243179ff061456783ac7f12bda4744c` |
| toml_writer | 1.1.3+spec-1.1.0 | MIT OR Apache-2.0 | github.com/toml-rs/toml | `f72038c2508112dfaaedb227a511f91bf9b2015c` | `06bdbd8cfc056b8d2e2e85f29b56a3bdbecb527cef81eb39e3e7b98af4652770` |
| transpose | 0.2.3 | MIT OR Apache-2.0 | github.com/ejmahler/transpose | `134293e7f7f8e13d342ae625e479e6097b5525db` | `1ad61aed86bc3faea4300c7aee358b4c6d0c8d6ccc36524c96e4c92ccf26e77e` |
| typenum | 1.20.1 | MIT OR Apache-2.0 | github.com/paholg/typenum | `0db9a0f731981f29266b63586c29fa07e4477b1a` | `b6f5e870be6c3b371b77fe0ee0bafb859fa4964b4404c27de1d380043c4dda20` |
| uluru | 3.1.0 | MPL-2.0 | github.com/servo/uluru | `e98dc9762e505a3ae580f60f2a35bbd006f43a1e` | `7c8a2469e56e6e5095c82ccd3afb98dad95f7af7929aab6d8ba8d6e0f73657da` |
| unic-char-property | 0.9.0 | MIT/Apache-2.0 | github.com/open-i18n/rust-unic | `5878605364af97a3358368a6eaef02104af2e016` | `a8c57a407d9b6fa02b4795eb81c5b6652060a15a7903ea981f3d723e6c0be221` |
| unic-char-range | 0.9.0 | MIT/Apache-2.0 | github.com/open-i18n/rust-unic | `5878605364af97a3358368a6eaef02104af2e016` | `0398022d5f700414f6b899e10b8348231abf9173fa93144cbc1a43b9793c1fbc` |
| unic-common | 0.9.0 | MIT/Apache-2.0 | github.com/open-i18n/rust-unic | `5878605364af97a3358368a6eaef02104af2e016` | `80d7ff825a6a654ee85a63e80f92f054f904f21e7d12da4e22f9834a4aaa35bc` |
| unic-ucd-ident | 0.9.0 | MIT/Apache-2.0 | github.com/open-i18n/rust-unic | `8a6ce83063d90b91ae2ce59eddb803edd393fca9` | `e230a37c0381caa9219d67cf063aa3a375ffed5bf541a452db16e744bdab6987` |
| unic-ucd-version | 0.9.0 | MIT/Apache-2.0 | github.com/open-i18n/rust-unic | `5878605364af97a3358368a6eaef02104af2e016` | `96bd2f2237fe450fcd0a1d2f5f4e91711124f7857ba2e964247776ebeeb7b0c4` |
| unicode-bidi | 0.3.18 | MIT OR Apache-2.0 | github.com/servo/unicode-bidi | `580b9c6c68fdb689916017e61d5052be3ed349b5` | `5c1cb5db39152898a79168971543b1cb5020dff7fe43c8dc468b0885f5e29df5` |
| unicode-ident | 1.0.27 | (MIT OR Apache-2.0) AND Unicode-3.0 | github.com/dtolnay/unicode-ident | `c1ac3afb721f3cea6527bc2255340a04d0de3ca0` | `a2c754d6c33795a1c324727428e5a7dedb5b06195f9890bdbcba760d3e246563` |
| unicode-properties | 0.1.5 | (MIT OR Apache-2.0) AND Unicode-3.0 | github.com/unicode-rs/unicode-properties | `b5594806088d6492327228a74d05c881b9b3c042` | `993ad287eb20e8ba755312801cfe1413e0f25d604075863acaddca10b04c1833` |
| unicode-script | 0.5.9 | MIT OR Apache-2.0 | github.com/unicode-rs/unicode-script | `e4a3c2e7af8ab8c29f33f7154893608f22eeb2c7` | `aa04c955db05429d464c74d495c1ee677e1866e783a4c3747f9c0cef7756cabf` |
| unicode-segmentation | 1.13.3 | MIT OR Apache-2.0 | github.com/unicode-rs/unicode-segmentation | `66a032fd8d667bc47ac5b640b151dff3f5356d07` | `c6f5d3c3b1bf09027a88a6bc961fc00497d651009560b5463668dc81b0fa87a8` |
| unicode-vo | 0.1.0 | MIT/Apache-2.0 | github.com/RazrFalcon/unicode-vo | `f163f6cc40436cf793742607048273fbb8834fbd` | `b1d386ff53b415b7fe27b50bb44679e2cc4660272694b7b6f3326d8480823a94` |
| unicode_categories | 0.1.1 | MIT OR Apache-2.0 | github.com/swgillespie/unicode-categories | not published¹ | `39ec24b3121d976906ece63c9daad25b85969647682eee313cb5779fdd69e14e` |
| untrusted | 0.9.0 | ISC | github.com/briansmith/untrusted | `89c8b8415f692d3803d183fc5fbf9ec08a6adcf4` | `8ecb6da28b8a351d773b68d5825ac39017e680750f980f3a1a85cd8dd28a47c1` |
| url | 2.5.8 | MIT OR Apache-2.0 | github.com/servo/rust-url | `d6ea13c5f8e7e6e627f6390161b3e185bda5e5ce` | `ff67a8a4397373c3ef660812acab3268222035010ab8680ec4215f38ba3d0eed` |
| urlpattern | 0.3.0 | MIT | github.com/denoland/rust-urlpattern | `ea97a5de9740a7eda311281828b3270d92fb1afb` | `70acd30e3aa1450bc2eece896ce2ad0d178e9c079493819301573dae3c37ba6d` |
| usvg | 0.48.1 | Apache-2.0 OR MIT | github.com/linebender/resvg | `68b14c4c3bccdb60344c777406486b54c36ec1a4` | `977d0a4abdef933f424a99fe09f95576e089b90aebc6f016a3bc813762493e91` |
| utf8_iter | 1.1.0 | Apache-2.0 OR MIT | github.com/hsivonen/utf8_iter | `17ac91ec5c6e18cf31a38dc1c0a704aebd35bb1f` | `af3bd9d3e571c26fcce9d033b87ea5ab061b8d2ffe9b7a5675d810d68f03edf0` |
| utf_types | 1.0.0 | Apache-2.0 OR MIT | github.com/hsivonen/utf_types | `3e8dc643be477f9f0be8cfa21e519b742649fa80` | `3a3b425286980a367278453f3b6d28cdcb134b574e5a35824a6ff5eebad50bb6` |
| uuid | 0.8.2 | Apache-2.0 OR MIT | github.com/uuid-rs/uuid | `b68733dfafb4cc384c50ca24af56221bfd2458d7` | `bc5cf98d8186244414c848017f0e2676b3fcb46807f6668a97dfe67359a3c4b7` |
| uuid | 1.29.0 | Apache-2.0 OR MIT | github.com/uuid-rs/uuid | `4643e607d0ded1f2f95c1c427456e17bd5edf708` | `4a0e2d11620c50695fe12bbbfde96b3477fe606e7284c22d505c12ce51214514` |
| vcpkg | 0.2.15 | MIT/Apache-2.0 | github.com/mcgoo/vcpkg-rs | `e7d37e095ef2aa85c80242aa3850d442832b7b54` | `accd4ea62f7bb7a82fe23066fb0957d48ef677f6eeb8215f372f52e48bb32426` |
| vello_common | 0.2.0 | Apache-2.0 OR MIT | github.com/linebender/vello | `7073a85d61ee099d1d3595651ddf92302b074348` | `bbb2141a2bca6e6d598e471fd4d1d7eed8e020aad6a28187edda07f091a325dd` |
| vello_cpu | 0.2.0 | Apache-2.0 OR MIT | github.com/linebender/vello | `7073a85d61ee099d1d3595651ddf92302b074348` | `ace506ca414548966fcf3c283be94eb1cad8254de488791ee698bf438b0890a9` |
| version_check | 0.9.5 | MIT/Apache-2.0 | github.com/SergioBenitez/version_check | `d77ef9f27cc336719b2d839d09ee6635dd22f758` | `0b928f33d975fc6ad9f86c8f283853ad26bdd5b10b7f1542aa2fa15e2289105a` |
| void | 1.0.2 | MIT | github.com/reem/rust-void | not published¹ | `6a02e4885ed3bc0f2de90ea6dd45ebcbb66dacffe03547fadbb0eeae2770887d` |
| walkdir | 2.5.0 | Unlicense/MIT | github.com/BurntSushi/walkdir | `4f26be4d450910916ea11533b2efc52b9a6483bc` | `29790946404f91d9c5d06f9874efddea1dc06c5efe94541a7d6863108e3a5e4b` |
| web_atoms | 0.3.0 | MIT OR Apache-2.0 | github.com/servo/html5ever | `a193ea7f2492d1e51eb32955a09c241345348dba` | `7572660c8890448ba236b7376f27e389c6a7e1c70195622faced601f855c0ada` |
| webdriver | 0.54.0 | MPL-2.0 | hg.mozilla.org/mozilla-central/file/tip/testing/webdriver | `4e8bec39e43b270342d640476bfef0a6b1b02ef9` | `799eea1e97dc3b3505b178be76e301138ced2af3b62490d8d932ace71168a948` |
| weezl | 0.1.12 | MIT OR Apache-2.0 | github.com/image-rs/weezl | `c75ea1f6d935cc56bad8674396b7cb0dd7275f99` | `a28ac98ddc8b9274cb41bb4d9d4d5c425b6020c50c46f25559911905610b4a88` |
| winapi-util | 0.1.11 | Unlicense OR MIT | github.com/BurntSushi/winapi-util | `803874c57dc1f10ecd42f7c86d9de72f53818432` | `c2a7b1c03c876122aa43f3020e6c3c3ee5c05081c9a00739faf7503aeba10d22` |
| winnow | 1.0.4 | MIT | github.com/winnow-rs/winnow | `7539ec0fc27144bfdcf9a68b0dbbec48bd0d5bae` | `23b97319f7b8343df12cc98938e5c3eb436064524c8d2b4e30a1d3a36eecdf81` |
| writeable | 0.6.4 | Unicode-3.0 | github.com/unicode-org/icu4x | `a14f2dad852be26bad277ae704abf27a15cbfed1` | `3ad82d2a33cdc9674dc7465672f271e096168fcdbe0f799d9e6db8c5892679dc` |
| wuff | 0.2.9 | MIT | github.com/nicoburns/wuff | `78e67f52713c0a2c36bd083fea18ba029565c9ec` | `200a7b806377b65f1ff9a985874332f15357c87e5dd7855f35f06dade95b8b3d` |
| wuff-capi | 0.2.0 | MIT | github.com/nicoburns/wuff | `3351f406f2069d45260eb425f768c38c961ece48` | `d4dbb1eebaa6782421e6aeac4ab5f9f3b9c665eb2f8650073f5235408b3898e2` |
| xmlwriter | 0.1.0 | MIT | github.com/RazrFalcon/xmlwriter | `5fdcff47da7d71896b55a1bc0e4b561171e232d6` | `ec7a2a501ed189703dba8b08142f057e887dfc4b2cc4db2d343ac6376ba3e0b9` |
| yoke | 0.8.4 | Unicode-3.0 | github.com/unicode-org/icu4x | `031d9e89f6a3fd26a14491e090729d46094328fd` | `417ef3fcf6c300b7dbd25c85531a746931c02ad536f3cf4786ed1e88223340b3` |
| yoke-derive | 0.8.4 | Unicode-3.0 | github.com/unicode-org/icu4x | `a59ab860d4bda548e94dfbf992d87fe1f761bc55` | `ec8ebde2db3681e8c9980cc27822030e68752690ddfa9473e739aeb4dbde6d71` |
| zerofrom | 0.1.8 | Unicode-3.0 | github.com/unicode-org/icu4x | `e9575f7e4b5aeddfeeb87ada5d3e715950e8233e` | `0ec05a11813ea801ff6d75110ad09cd0824ddba17dfe17128ea0d5f68e6c5272` |
| zerofrom-derive | 0.1.8 | Unicode-3.0 | github.com/unicode-org/icu4x | `ce235d80cf7cbd028e8bd59920420e1203b205b3` | `f75b4683f6c7f45248d4d64056a24298c6281e0993356d7d1b4a1a962ef10d4a` |
| zeroize | 1.9.1 | Apache-2.0 OR MIT | github.com/RustCrypto/utils | `34a6ebcb9b920ec7ae5809e470fce94fe7c0cb55` | `e13084392c5e4bc371903e2935a5eaeed24905a7511356b883835e18a78f6879` |
| zeroize_derive | 1.5.0 | Apache-2.0 OR MIT | github.com/RustCrypto/utils | `1ea42bb68f560a2909c856b20aa3c62901f6305c` | `3c50655cbb0fe3fc43170059e702f1ce5e19b84cec58dc87b037a09935c2f328` |
| zerotrie | 0.2.5 | Unicode-3.0 | github.com/unicode-org/icu4x | `a14f2dad852be26bad277ae704abf27a15cbfed1` | `4ea269c3bd32f0a32c321907a2ae912ba6f4649bb0fc764a15627e99a7095a3f` |
| zerovec | 0.11.9 | Unicode-3.0 | github.com/unicode-org/icu4x | `031d9e89f6a3fd26a14491e090729d46094328fd` | `f58a2799cfe9a1670c7ea8edbd52ef8edb93cb37a7f83bf31ba9e35d9dae6a40` |
| zerovec-derive | 0.11.7 | Unicode-3.0 | github.com/unicode-org/icu4x | `031d9e89f6a3fd26a14491e090729d46094328fd` | `c3743ddbf47738d929cd1ae2e199d40d0162979dfe31aef8ae646d2bf00bf97a` |
| zmij | 1.0.23 | MIT | github.com/dtolnay/zmij | `7b7cc48b58028e8af7be87e94c0c1c8936f1a57c` | `29666d0abbfad1e3dc4dcf6144730dd3a3ab225bbbdac83319345b1b44ccfc1b` |
| zune-core | 0.5.3 | MIT OR Apache-2.0 OR Zlib | github.com/etemesi254/zune-image | `de5dfc800cfa063116eb695edb6ccd01cb96ea7a` | `d56377fd46368984a170bc5aac5567e52ca5da874caa60bea39fcbca78fb658b` |
| zune-jpeg | 0.5.15 | MIT OR Apache-2.0 OR Zlib | github.com/etemesi254/zune-image/tree/dev/crates/zune-jpeg | `31d81fed7551c8ccea456d9d8e2b1fd8bebb6995` | `27bc9d5b815bc103f142aa054f561d9187d191692ec7c2d1e2b4737f8dbd7296` |
| bs58 (stub) | 0.5.1 | MIT OR Apache-2.0 | github.com/Nullus157/bs58-rs | — (stands in for this repository's crate; no code) | — (a stub, not the published crate) |
| chrono (stub) | 0.4.45 | MIT OR Apache-2.0 | github.com/chronotope/chrono | — (stands in for this repository's crate; no code) | — (a stub, not the published crate) |
| core-foundation (stub) | 0.9.4 | MIT OR Apache-2.0 | github.com/servo/core-foundation-rs | — (stands in for this repository's crate; no code) | — (a stub, not the published crate) |
| core-graphics (stub) | 0.23.2 | MIT OR Apache-2.0 | github.com/servo/core-foundation-rs | — (stands in for this repository's crate; no code) | — (a stub, not the published crate) |
| core-text (stub) | 20.1.0 | MIT OR Apache-2.0 | github.com/servo/core-foundation-rs | — (stands in for this repository's crate; no code) | — (a stub, not the published crate) |
| dwrote (stub) | 0.11.5 | MPL-2.0 | github.com/servo/dwrote-rs | — (stands in for this repository's crate; no code) | — (a stub, not the published crate) |
| embedded-io (stub) | 0.4.0 | MIT OR Apache-2.0 | github.com/embassy-rs/embedded-io | — (stands in for this repository's crate; no code) | — (a stub, not the published crate) |
| embedded-io (stub) | 0.6.1 | MIT OR Apache-2.0 | github.com/rust-embedded/embedded-hal | — (stands in for this repository's crate; no code) | — (a stub, not the published crate) |
| exr (stub) | 1.74.2 | BSD-3-Clause | github.com/johannesvollmer/exrs | — (stands in for this repository's crate; no code) | — (a stub, not the published crate) |
| fontconfig-parser (stub) | 0.5.8 | MIT | github.com/Riey/fontconfig-parser | — (stands in for this repository's crate; no code) | — (a stub, not the published crate) |
| freetype (stub) | 0.8.0 | MIT OR Apache-2.0 | github.com/servo/rust-freetype | — (stands in for this repository's crate; no code) | — (a stub, not the published crate) |
| freetype-sys (stub) | 0.23.0 | MIT | github.com/PistonDevelopers/freetype-sys | — (stands in for this repository's crate; no code) | — (a stub, not the published crate) |
| hermit-abi (stub) | 0.5.3 | MIT OR Apache-2.0 | github.com/hermit-os/hermit-rs | — (stands in for this repository's crate; no code) | — (a stub, not the published crate) |
| hex (stub) | 0.4.3 | MIT OR Apache-2.0 | github.com/KokaKiwi/rust-hex | — (stands in for this repository's crate; no code) | — (a stub, not the published crate) |
| hyper-util (stub) | 0.1.21 | MIT | github.com/hyperium/hyper-util | — (stands in for this repository's crate; no code) | — (a stub, not the published crate) |
| indexmap (stub) | 1.9.3 | Apache-2.0 OR MIT | github.com/bluss/indexmap | — (stands in for this repository's crate; no code) | — (a stub, not the published crate) |
| jiff (stub) | 0.2.39 | Unlicense OR MIT | github.com/BurntSushi/jiff | — (stands in for this repository's crate; no code) | — (a stub, not the published crate) |
| js-sys (stub) | 0.3.107 | MIT OR Apache-2.0 | github.com/wasm-bindgen/wasm-bindgen/tree/master/crates/js-sys | — (stands in for this repository's crate; no code) | — (a stub, not the published crate) |
| libc (patched) | 0.2.191 | MIT OR Apache-2.0 | github.com/rust-lang/libc | `ccc2ff4aaab962f7fde7b8edda7379c045b1ca12` | `65cd87c9a6d60536580b316ad985726104730a5151c99845f2490428ef91240a` |
| mach2 (stub) | 0.7.0 | BSD-2-Clause OR MIT OR Apache-2.0 | github.com/JohnTitor/mach2 | — (stands in for this repository's crate; no code) | — (a stub, not the published crate) |
| mio (stub) | 1.2.4 | MIT | github.com/tokio-rs/mio | — (stands in for this repository's crate; no code) | — (a stub, not the published crate) |
| num_threads (stub) | 0.1.8 | MIT OR Apache-2.0 | github.com/jhpratt/num_threads | — (stands in for this repository's crate; no code) | — (a stub, not the published crate) |
| objc (stub) | 0.2.7 | MIT | github.com/SSheldon/rust-objc | — (stands in for this repository's crate; no code) | — (a stub, not the published crate) |
| objc2 (stub) | 0.6.5 | MIT | github.com/madsmtm/objc2 | — (stands in for this repository's crate; no code) | — (a stub, not the published crate) |
| objc2-core-foundation (stub) | 0.3.2 | Zlib OR Apache-2.0 OR MIT | github.com/madsmtm/objc2 | — (stands in for this repository's crate; no code) | — (a stub, not the published crate) |
| objc2-core-graphics (stub) | 0.3.2 | Zlib OR Apache-2.0 OR MIT | github.com/madsmtm/objc2 | — (stands in for this repository's crate; no code) | — (a stub, not the published crate) |
| objc2-core-text (stub) | 0.3.2 | Zlib OR Apache-2.0 OR MIT | github.com/madsmtm/objc2 | — (stands in for this repository's crate; no code) | — (a stub, not the published crate) |
| parking_lot_core (patched) | 0.9.12 | MIT OR Apache-2.0 | github.com/Amanieu/parking_lot | `d7828fff7b5d6327ae608e82db45f888b344449a` | `2621685985a2ebf1c516881c026032ac7deafcda1a2c9b7850dc81e3dfcb64c1` |
| r-efi (stub) | 6.0.0 | MIT OR Apache-2.0 OR LGPL-2.1-or-later | github.com/r-efi/r-efi | — (stands in for this repository's crate; no code) | — (a stub, not the published crate) |
| ravif (stub) | 0.13.0 | BSD-3-Clause | github.com/kornelski/cavif-rs | — (stands in for this repository's crate; no code) | — (a stub, not the published crate) |
| ring (stub) | 0.17.14 | Apache-2.0 AND ISC | github.com/briansmith/ring | — (stands in for this repository's crate; no code) | — (a stub, not the published crate) |
| rustc_version (stub) | 0.4.1 | MIT OR Apache-2.0 | github.com/djc/rustc-version-rs | — (stands in for this repository's crate; no code) | — (a stub, not the published crate) |
| schemars (stub) | 0.9.0 | MIT | github.com/GREsau/schemars | — (stands in for this repository's crate; no code) | — (a stub, not the published crate) |
| schemars (stub) | 1.2.3 | MIT | github.com/GREsau/schemars | — (stands in for this repository's crate; no code) | — (a stub, not the published crate) |
| servo-allocator (patched) | 0.7.0 | MPL-2.0 | github.com/servo/servo | `aac43a3f31a259f04a574f5ec4e959c943ad7cc7` | `380a2edcbdd9d91d59149f96433a7f1b2d3b128cd0c4055474bca1b76cb22fc1` |
| servo-base (patched) | 0.7.0 | MPL-2.0 | github.com/servo/servo | `aac43a3f31a259f04a574f5ec4e959c943ad7cc7` | `c1bbfbafceeefb49538b928ea7a31c798d15c058ef90d9e2af233f54a4d15fa6` |
| servo-embedder-traits (patched) | 0.7.0 | MPL-2.0 | github.com/servo/servo | `aac43a3f31a259f04a574f5ec4e959c943ad7cc7` | `5f14bbb2593cb2ee9d076b53672703064f30593d6bdb4fab34cd5c0dc6d6da5c` |
| servo-fonts (patched) | 0.7.0 | MPL-2.0 | github.com/servo/servo | `aac43a3f31a259f04a574f5ec4e959c943ad7cc7` | `871ff64ee0d5269a3caeae5c0e758d8e0579987e0187edf86ae0307103359051` |
| servo-fonts-traits (patched) | 0.7.0 | MPL-2.0 | github.com/servo/servo | `aac43a3f31a259f04a574f5ec4e959c943ad7cc7` | `d15da891032b58554795f50b914877cc166eb54665b469cc2df4579307e98d6a` |
| servo-layout (patched) | 0.7.0 | MPL-2.0 | github.com/servo/servo | `aac43a3f31a259f04a574f5ec4e959c943ad7cc7` | `675bf3cb0127b58dde2d072f783c2c57d92c271abd037bc76c781a778af285f4` |
| servo-paint-api (patched) | 0.7.0 | MPL-2.0 | github.com/servo/servo | `aac43a3f31a259f04a574f5ec4e959c943ad7cc7` | `fe5e606d34334b4baccb14d912172e997a3d7dd1c0d19cb2bbd2bb38e5a6c369` |
| servo-script (stub) | 0.7.0 | MPL-2.0 | github.com/servo/servo | — (stands in for this repository's crate; no code) | — (a stub, not the published crate) |
| servo-url (patched) | 0.7.0 | MPL-2.0 | github.com/servo/servo | `aac43a3f31a259f04a574f5ec4e959c943ad7cc7` | `0b34391568393f40ab8a3b3b7cf41d1fff397ece39e7b0c651a3b513852bb6f5` |
| stylo (patched) | 0.22.0 | MPL-2.0 | github.com/servo/stylo | `34d25f741765a0bdb894f8271852e633299352d8` | `7a255e46af2478628c48daab3774090caabd7812b1dd3ca98801c2b9c53491ed` |
| surfman (stub) | 0.14.0 | MIT OR Apache-2.0 OR MPL-2.0 | github.com/servo/surfman | — (stands in for this repository's crate; no code) | — (a stub, not the published crate) |
| tempfile (stub) | 3.27.0 | MIT OR Apache-2.0 | github.com/Stebalien/tempfile | — (stands in for this repository's crate; no code) | — (a stub, not the published crate) |
| wasi (stub) | 0.11.1+wasi-snapshot-preview1 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | github.com/bytecodealliance/wasi | — (stands in for this repository's crate; no code) | — (a stub, not the published crate) |
| wasm-bindgen (stub) | 0.2.130 | MIT OR Apache-2.0 | github.com/wasm-bindgen/wasm-bindgen | — (stands in for this repository's crate; no code) | — (a stub, not the published crate) |
| web-sys (stub) | 0.3.107 | MIT OR Apache-2.0 | github.com/wasm-bindgen/wasm-bindgen/tree/master/crates/web-sys | — (stands in for this repository's crate; no code) | — (a stub, not the published crate) |
| winapi (stub) | 0.3.9 | MIT OR Apache-2.0 | github.com/retep998/winapi-rs | — (stands in for this repository's crate; no code) | — (a stub, not the published crate) |
| winapi-util (stub) | 0.1.11 | Unlicense OR MIT | github.com/BurntSushi/winapi-util | — (stands in for this repository's crate; no code) | — (a stub, not the published crate) |
| windows (stub) | 0.62.2 | MIT OR Apache-2.0 | github.com/microsoft/windows-rs | — (stands in for this repository's crate; no code) | — (a stub, not the published crate) |
| windows-sys (stub) | 0.61.2 | MIT OR Apache-2.0 | github.com/microsoft/windows-rs | — (stands in for this repository's crate; no code) | — (a stub, not the published crate) |
| xml (stub) | 1.4.0 | MIT | github.com/kornelski/xml-rs | — (stands in for this repository's crate; no code) | — (a stub, not the published crate) |
| yeslogic-fontconfig-sys (stub) | 6.0.1 | MIT | github.com/yeslogic/fontconfig-rs | — (stands in for this repository's crate; no code) | — (a stub, not the published crate) |
| zlib-rs (stub) | 0.6.8 | Zlib | github.com/trifectatechfoundation/zlib-rs | — (stands in for this repository's crate; no code) | — (a stub, not the published crate) |

¹ Published without `.cargo_vcs_info.json` (precomputed-hash 0.1.1, unicode_categories 0.1.1, void 1.0.2); its integrity is the crates.io checksum.

## Code that runs at build time

`web_atoms`' build script (it generates the atom tables with `string_cache_codegen` and `phf_codegen`),
`parking_lot_core`'s (a tsan probe), `proc-macro2`'s and `quote`'s (compiler probes), and the proc macros
`cssparser-macros`, `derive_more-impl` and `phf_macros`. Per J.c., these first ran on the CI runners.

**Stylo's build script runs Python** (`stylo-0.22.0/build.rs` → `properties/build.py`): it renders the property code
from Mako templates. J.c. allowed build-time Python on 2026-10-31, with no network fetch (CLAUDE.md, `69550a3c`;
C2.4 survey §3.7). Its first run is on the CI runners: the closure workflow's `stylo-build` job, with
`PIP_NO_INDEX=1`. The Python it imports is bundled inside the published crate, so nothing is fetched:

| Package | Version | Licence | Form in `properties/vendored_python/` | SHA-256 | Repository, last commit (2026-10-31) |
|---|---|---|---|---|---|
| Mako | 1.3.10 | MIT | `mako-1.3.10-py3-none-any.whl` | `baef24a52fc4fc514a0887ac600f9f1cff3d82c61d4d700a1fa84d597b88db59` | github.com/sqlalchemy/mako, 2026-09-22 `14889bc` |
| toml | 0.10.2 | MIT | `toml-0.10.2-py2.py3-none-any.whl` | `806143ae5bfb6a3c6e736a764057db0e6a0e05e338b5630894a5f779cabb4f9b` | github.com/uiri/toml, 2023-10-11 `65bab75` ⚠ |
| MarkupSafe | pallets/markupsafe at `1251593f6b0e3b45f2cc8aba662622bc22d6a5e2` | BSD-3-Clause (`markupsafe/LICENSE.txt`) | `markupsafe/__init__.py`, one file (Stylo's copy, with `_native.py` folded in) | `13d7034c5c70cf23c6e2200e3e7bdb05f5218a4953c99243da0ee00876519d40` | github.com/pallets/markupsafe, 2026-10-02 `4e11127` |

⚠ toml's repository has had no commit in about three years; it is a finished parser here, run only on the build
host. Versions and licences are from each wheel's `METADATA`; the SHA-256s are of the files as vendored (they are
also covered by `stylo`'s `.cargo-checksum.json`).

## Obligations

MIT and Apache-2.0 notices for the linked crates go into `NOTICE` when a browser binary ships (each crate's
licence files are in its directory here; `void`'s is in `licenses/`), and so does the Unicode-3.0 notice for the
`icu4x` data Stylo links. MPL-2.0 files (`selectors`, `cssparser`, `dtoa-short`, `stylo` and its MPL crates) stay
MPL-2.0. The Python bundled in `stylo` runs only on the build host and isn't distributed.

## Updating

Bump the versions in `userland/servo-closure-smoke/Cargo.toml`, re-run `cargo vendor --versioned-dirs` from a
scratch copy with the `[patch]` table removed, re-apply the parking_lot_core patch if that crate moved, replace
`vendor/`, regenerate the `[patch]` tables (`gen-patch-table.py`) and this file's table
(`gen-vendored-table.py`), drop any vendored crate no lockfile names, and re-read the spike's breakage list (§5)
and the C2.4 survey's §3 against the new sources.
