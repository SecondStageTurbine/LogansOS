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
- `stubs/<crate>-<version>/` (40 more, OT-61 L0) — empty stand-ins for crates `userland/layout-smoke`'s lockfile
  names only behind other platforms' `cfg`s, or behind edges option A switches off (`servo-script`, `surfman`):
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

`userland/layout-smoke` (L0's closure probe) names 40 more stubs in its `[patch.crates-io]` table, in
`stubs/<crate>-<version>/`. Each is built the same way as the `windows-sys` stub above: the published crate's
name, version, licence, `[lib]` name and `[features]` table, and an empty `#![no_std]` `src/lib.rs`. No upstream
code is in them. Feature entries that point at dependencies (`dep:x`, `x/feature`) are dropped, since a stub has
no dependencies; optional dependencies that were implicit features are kept as empty features, so every feature
a dependent asks for still exists. The manifests are generated from each crate's published `Cargo.toml`, never
typed by hand.

- **Why they're in the lockfile.** Cargo resolves every platform's dependencies into one lockfile. Each stub
  below is reached only through a dependency edge whose `cfg` is false for both our target and the build
  host's build scripts (macOS, Windows, wasm, Android, hermit, UEFI, `fuzzing`), or through an edge option A
  switched off for our target: `servo-layout`'s `script` dependency and `servo-paint-api`'s `surfman`, both
  under `cfg(not(target_os = "neoplutarch"))`.
- **Why it's safe: checked, not assumed.** With the stubs patched in, `cargo build -Zunstable-options
  --unit-graph` for `x86_64-unknown-neoplutarch` gives the same 507 units over the same 387 packages, with the
  same features, as without them (host build-script units included), and the build stops at the same point (the
  `-lstdc++` link that waits on `neo-crt`, #2). None of the 40 is compiled. If one ever were, its dependent would
  fail to compile against the empty crate: a loud failure, never a silently wrong build.
- **What they save.** Without them the lockfile has 668 packages, including mozjs (Servo's SpiderMonkey
  bindings), `aws-lc-sys`, `bindgen` and the Apple/Windows/wasm binding crates. With them it has 489, of which
  431 come from crates.io: exactly the crates the target build compiles. Those 431 are what L0 vendors.
- **The `cfg(unix)` / `cfg(target_os = "linux")` edges.** Some edges below (`iana-time-zone`, `num_threads`,
  `mio`, `tempfile`, `freetype`, `fontconfig-parser`, `yeslogic-fontconfig-sys`) are true on a Linux host. They
  are safe only because no crate behind them is built for the host here; the unit graph above is the check. The
  stubs apply to `layout-smoke`'s workspace only: `neo-layout-dom` and `neo-static-page` are separate
  workspaces with no `[patch]` table, so their host tests resolve the real crates.
- **Licences.** Each stub keeps its upstream licence expression, in its manifest's `license` field and its SPDX
  line, and the upstream licence files where the published crate ships them. Seven published crates ship none
  (`dwrote`, the four `objc2` crates, `r-efi`, `servo-script`); their stubs carry a `LICENSE-NOTE.md` saying so.
  `winapi` 0.3.9 declares the old `MIT/Apache-2.0` form; its stub writes it as `MIT OR Apache-2.0`. #25 carries
  its own `winapi` 0.3.9 stub (under `ot61-swgl`); when #55 rebases onto it, the two converge on one copy.
- **`servo-script`.** Its stub holds the published 0.7.0's name, version, `[lib] name = "script"` and features.
  Option A (`Plans/DECISION-2026-11-02-ot61-first-page-option-a.md`) makes `servo-layout` not depend on it for our
  target, so the closure behind it (mozjs and its build tooling, about 420 MB of sources) is never vendored.

| Stub | Licence | Reached only from (dependent, under) |
|---|---|---|
| `arbitrary` 1.5.0 | MIT OR Apache-2.0 | rav1e 0.8.1, `cfg(fuzzing)` |
| `core-foundation` 0.9.4 | MIT OR Apache-2.0 | wr_glyph_rasterizer 0.70.1, `cfg(any(target_os = "macos", target_os = "ios"))` |
| `core-graphics` 0.23.2 | MIT OR Apache-2.0 | wr_glyph_rasterizer 0.70.1, `cfg(any(target_os = "macos", target_os = "ios"))` |
| `core-text` 20.1.0 | MIT OR Apache-2.0 | wr_glyph_rasterizer 0.70.1, `cfg(any(target_os = "macos", target_os = "ios"))` |
| `crunchy` 0.2.4 | MIT | half 2.7.1, `cfg(target_arch = "spirv")` |
| `dwrote` 0.11.5 | MPL-2.0 | servo-fonts 0.7.0, `cfg(target_os = "windows")`; servo-fonts-traits 0.7.0, `cfg(target_os = "windows")`; wr_glyph_rasterizer 0.70.1, `cfg(target_os = "windows")` |
| `fontconfig-parser` 0.5.8 | MIT | fontdb 0.24.0, `cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))` |
| `freetype` 0.8.0 | MIT OR Apache-2.0 | wr_glyph_rasterizer 0.70.1, `cfg(any(target_os = "android", all(unix, not(any(target_os = "macos", target_os = "ios")))))` |
| `hermit-abi` 0.5.3 | MIT OR Apache-2.0 | num_cpus 1.17.0, `cfg(target_os = "hermit")` |
| `iana-time-zone` 0.1.65 | MIT OR Apache-2.0 | chrono 0.4.45, `cfg(unix)` |
| `interpolate_name` 0.2.4 | MIT | rav1e 0.8.1, `cfg(fuzzing)` |
| `jiff-static` 0.2.39 | Unlicense OR MIT | jiff 0.2.39, `cfg(any())` |
| `jiff-tzdb-platform` 0.1.3 | Unlicense OR MIT | jiff 0.2.39, `cfg(any(windows, target_family = "wasm"))` |
| `js-sys` 0.3.107 | MIT OR Apache-2.0 | glow 0.17.0, `cfg(target_arch = "wasm32")`; uuid 1.29.0, `cfg(all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none"), target_feature = "atomics"))` |
| `libfuzzer-sys` 0.4.13 | (MIT OR Apache-2.0) AND NCSA | rav1e 0.8.1, `cfg(fuzzing)` |
| `mach2` 0.7.0 | BSD-2-Clause OR MIT OR Apache-2.0 | servo-base 0.7.0, `cfg(any(target_os = "macos", target_os = "ios"))` |
| `mio` 1.2.4 | MIT | ipc-channel 0.23.1, `cfg(any(target_os = "linux", target_os = "openbsd", target_os = "freebsd", target_os = "illumos"))` |
| `num_threads` 0.1.8 | MIT OR Apache-2.0 | time 0.3.55, `cfg(target_family = "unix")` |
| `objc` 0.2.7 | MIT | wr_glyph_rasterizer 0.70.1, `cfg(any(target_os = "macos", target_os = "ios"))` |
| `objc2` 0.6.5 | MIT | servo-fonts 0.7.0, `cfg(target_os = "macos")` |
| `objc2-core-foundation` 0.3.2 | Zlib OR Apache-2.0 OR MIT | servo-fonts 0.7.0, `cfg(target_os = "macos")` |
| `objc2-core-graphics` 0.3.2 | Zlib OR Apache-2.0 OR MIT | harfbuzz-sys 0.8.0, `cfg(target_vendor = "apple")`; servo-fonts 0.7.0, `cfg(target_os = "macos")` |
| `objc2-core-text` 0.3.2 | Zlib OR Apache-2.0 OR MIT | harfbuzz-sys 0.8.0, `cfg(target_vendor = "apple")`; servo-fonts 0.7.0, `cfg(target_os = "macos")` |
| `portable-atomic` 1.15.0 | Apache-2.0 OR MIT | jiff 0.2.39, `cfg(not(target_has_atomic = "ptr"))` |
| `portable-atomic-util` 0.2.8 | Apache-2.0 OR MIT | jiff 0.2.39, `cfg(not(target_has_atomic = "ptr"))` |
| `r-efi` 6.0.0 | MIT OR Apache-2.0 OR LGPL-2.1-or-later | getrandom 0.4.3, `cfg(all(target_os = "uefi", getrandom_backend = "efi_rng"))` |
| `rand` 0.9.5 | MIT OR Apache-2.0 | rav1e 0.8.1, `cfg(fuzzing)` |
| `rand_chacha` 0.9.0 | MIT OR Apache-2.0 | rav1e 0.8.1, `cfg(fuzzing)` |
| `servo-script` 0.7.0 | MPL-2.0 | servo-layout 0.7.0, `cfg(not(target_os = "neoplutarch"))` |
| `surfman` 0.14.0 | MIT OR Apache-2.0 OR MPL-2.0 | servo-paint-api 0.7.0, `cfg(not(target_os = "neoplutarch"))` |
| `tempfile` 3.27.0 | MIT OR Apache-2.0 | ipc-channel 0.23.1, `cfg(any(target_os = "linux", target_os = "openbsd", target_os = "freebsd", target_os = "illumos"))` |
| `wasi` 0.11.1+wasi-snapshot-preview1 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | getrandom 0.2.17, `cfg(target_os = "wasi")` |
| `web-sys` 0.3.107 | MIT OR Apache-2.0 | glow 0.17.0, `cfg(target_arch = "wasm32")` |
| `winapi` 0.3.9 | MIT OR Apache-2.0 | servo-fonts 0.7.0, `cfg(target_os = "windows")`; zeitstempel 0.2.2, `cfg(windows)` |
| `winapi-util` 0.1.11 | Unlicense OR MIT | same-file 1.0.6, `cfg(windows)`; walkdir 2.5.0, `cfg(windows)` |
| `windows` 0.62.2 | MIT OR Apache-2.0 | harfbuzz-sys 0.8.0, `cfg(target_family = "windows")`; ipc-channel 0.23.1, `cfg(target_os = "windows")` |
| `windows-link` 0.2.1 | MIT OR Apache-2.0 | chrono 0.4.45, `cfg(windows)`; jiff 0.2.39, `cfg(windows)` |
| `windows-sys` 0.52.0 | MIT OR Apache-2.0 | ring 0.17.14, `cfg(all(all(target_arch = "aarch64", target_endian = "little"), target_os = "windows"))` |
| `xml` 1.4.0 | MIT | servo-fonts 0.7.0, `cfg(target_os = "android")` |
| `yeslogic-fontconfig-sys` 6.0.1 | MIT | servo-fonts 0.7.0, `cfg(all(any(target_os = "linux", target_os = "freebsd"), not(target_env = "ohos")))` |

## The crates

Generated by `gen-vendored-table.py`, never typed by hand: licences from each crate's `Cargo.toml`, "Upstream
commit" from `.cargo_vcs_info.json` (the commit the release was published from), the SHA-256 of the published
`.crate` from `.cargo-checksum.json`. Release and last-commit dates are in the C2.3b spike's §2 and the C2.4
survey's §4. Every licence is MPL-2.0, MIT, Apache-2.0, Unlicense-or-MIT, BSD-3-Clause or Unicode-3.0 (the data
licence of `icu4x` and `unicode-ident`; on CLAUDE.md's vendoring list by J.c.'s decision of 2026-10-31, `69550a3c`).
No GPL, LGPL or AGPL.

| Crate | Version | Licence | Repository | Upstream commit | crates.io `.crate` SHA-256 |
|---|---|---|---|---|---|
| app_units | 0.7.8 | MPL-2.0 | github.com/servo/app_units | `2e4e0e0ad8b43bf49a1d3d84e3ca4c55367314a7` | `467b60e4ee6761cd6fd4e03ea58acefc8eec0d1b1def995c1b3b783fa7be8a60` |
| arrayvec | 0.7.8 | MIT OR Apache-2.0 | github.com/bluss/arrayvec | `0cb664cf505844348538230479b0040b44f3faf1` | `d3fb67a6e08acf24fdeccbac2cb6ac4305825bd1f117462e0e6f2f193345ad56` |
| atomic_refcell | 0.1.14 | Apache-2.0 OR MIT | github.com/mozilla/atomic_refcell | `216bba408cf6df886dca5b0fb4b1d410d46db03c` | `21e4227379beff4205943696e6c3e0cd809bacdf3f0edd6e3dd153e2269571a4` |
| autocfg | 1.5.1 | Apache-2.0 OR MIT | github.com/cuviper/autocfg | `2799b09c24e6632f8e653c5cd8fc303e85a906ba` | `f2032f911046de80f0a198e0901378627c33f59ea0ac00e363d481118bd70a53` |
| bitflags | 2.13.2 | MIT OR Apache-2.0 | github.com/bitflags/bitflags | `80ce9b545acb0bd42150695fc351889cac1d8eb4` | `3ded4057c258ba199e2d26386d3af3780957ecaee6c4ef4041c6b4b8b97c0b06` |
| byteorder | 1.5.0 | Unlicense OR MIT | github.com/BurntSushi/byteorder | `ec068eefa042d494475db125c4b034bd8e9e34dd` | `1fd0f2584146f6f2ef48085050886acf353beff7305ebd1ae69500e27c67f64b` |
| cfg-if | 1.0.5 | MIT OR Apache-2.0 | github.com/rust-lang/cfg-if | `8b0a2b0fc1fa3b3741b9e72895f390cf499fe36f` | `4e7648175b45a9a48536d676f68d918270699102aa8dab5496df06904c914600` |
| core_detect | 1.0.0 | MIT/Apache-2.0 | github.com/thomcc/core_detect | `a0f957776563806e7d20978471676e0db72892ba` | `7f8f80099a98041a3d1622845c271458a2d73e688351bf3cb999266764b81d48` |
| core_maths | 0.1.1 | MIT | github.com/robertbastian/core_maths | `d8ea96acac3893ad9a7e16cd7526936fbfc51021` | `77745e017f5edba1a9c1d854f6f3a52dac8a12dd5af5d2f54aecf61e43d80d30` |
| crossbeam-deque | 0.8.8 | MIT OR Apache-2.0 | github.com/crossbeam-rs/crossbeam | `2920c984290229ab4e0ca0452ef09e48a82063f3` | `622f3fc73690be383c7214310406f28a90e6edeadc3cea882f9d71e495b9711a` |
| crossbeam-epoch | 0.9.21 | MIT OR Apache-2.0 | github.com/crossbeam-rs/crossbeam | `2920c984290229ab4e0ca0452ef09e48a82063f3` | `dc74980687109a3b14c72fd458107bf0baa1da1a1a805e178d15501ba9b86d9d` |
| crossbeam-utils | 0.8.23 | MIT OR Apache-2.0 | github.com/crossbeam-rs/crossbeam | `2920c984290229ab4e0ca0452ef09e48a82063f3` | `a31eee39dddec8330830986fcd7625edb5a24ec90ea038215273bbc3adb08ac6` |
| cssparser | 0.38.0 | MPL-2.0 | github.com/servo/rust-cssparser | `d128a90e1203fc4afb959420d6dc5e24f77a3191` | `11119743ad110e8c1bdccd930d7f5c30c99e5fc76a7b63ec9807e84eef0c5f59` |
| cssparser-macros | 0.7.1 | MPL-2.0 | github.com/servo/rust-cssparser | `d128a90e1203fc4afb959420d6dc5e24f77a3191` | `d045de693cb712d0b22c6a64be5b953f67b3ce00ab5ad3dd5d8b441886ab8e1a` |
| darling | 0.20.11 | MIT | github.com/TedDriggs/darling | `82a51e0b65b158de02ffb5d753ea4cd03529743b` | `fc7f46116c46ff9ab3eb1597a45688b6715c6e628b5c133e288e709a29bcb4ee` |
| darling_core | 0.20.11 | MIT | github.com/TedDriggs/darling | `82a51e0b65b158de02ffb5d753ea4cd03529743b` | `0d00b9596d185e565c2207a0b01f8bd1a135483d02d9b7b0a54b11da8d53412e` |
| darling_macro | 0.20.11 | MIT | github.com/TedDriggs/darling | `82a51e0b65b158de02ffb5d753ea4cd03529743b` | `fc34b93ccb385b40dc71c6fceac4b2ad23662c7eeb248cf10d529b7e055b6ead` |
| derive_more | 2.1.1 | MIT | github.com/JelteF/derive_more | `f7bb41ac054c060caaf5ff3212e74e42794cb4b4` | `d751e9e49156b02b44f9c1815bcb94b984cdcc4396ecc32521c739452808b134` |
| derive_more-impl | 2.1.1 | MIT | github.com/JelteF/derive_more | `f7bb41ac054c060caaf5ff3212e74e42794cb4b4` | `799a97264921d8623a957f6c3b9011f3b5492f557bbb7a5a19b7fa6d06ba8dcb` |
| displaydoc | 0.2.7 | MIT OR Apache-2.0 | github.com/yaahc/displaydoc | `20016487354a96b22a7b97974483ae4f51a1bddc` | `c6232dd377dcc64799954cbd3a9bb882e9cdc1308ccd87b1c098f1fb2eaf82a8` |
| dtoa | 1.0.11 | MIT OR Apache-2.0 | github.com/dtolnay/dtoa | `4c30faa2d91f167d7c343d13ebc9d30730456d56` | `4c3cf4824e2d5f025c7b531afcb2325364084a16806f6d47fbc1f5fbd9960590` |
| dtoa-short | 0.3.5 | MPL-2.0 | github.com/upsuper/dtoa-short | `2d905cdb8b2e08163dc0d015f529877fe657b4ef` | `cd1511a7b6a56299bd043a9c167a6d2bfb37bf84a6dfceaba651168adfb43c87` |
| either | 1.19.0 | MIT OR Apache-2.0 | github.com/rayon-rs/either | `1e4326df5972eb4a0ced66cb3b67fa0c5aca8493` | `0e9c71c2167ca323c882b99918929403426e2373ea17242ff5653e0d5e1058be` |
| encoding_rs | 0.8.44 | (Apache-2.0 OR MIT) AND BSD-3-Clause | github.com/hsivonen/encoding_rs | `8e8556af3cf976392b993a9dbe07456a5ef58469` | `89253175e704ab869b04f9db224d38630e0211a53841a79181deae0889e169f5` |
| equivalent | 1.0.3 | Apache-2.0 OR MIT | github.com/indexmap-rs/equivalent | `4ee88130bd4ab3229e785533da2af641be22deac` | `00d174d5400e5e8fd687ad1049e2f578285fa914201b1af7e8b112a4546bd826` |
| euclid | 0.22.14 | MIT OR Apache-2.0 | github.com/servo/euclid | `60f2bd96deec0c0683eeddd737e0dc070a25e9af` | `f1a05365e3b1c6d1650318537c7460c6923f1abdd272ad6842baa2b509957a06` |
| fastrand | 2.5.0 | Apache-2.0 OR MIT | github.com/smol-rs/fastrand | `7a1cc2c69ebde6cb3442809213db3fe19a2cb0ba` | `da7c62ceae207dd37ea5b845da6a0696c799f85e97da1ab5b7910be3c1c80223` |
| fnv | 1.0.7 | Apache-2.0 / MIT | github.com/servo/rust-fnv | `4b4784ebfd3332dc61f0640764d6f1140e03a9ab` | `3f9eec918d3f24069decb9af1554cad7c880e2da24a9afd88aca000531ab82c1` |
| form_urlencoded | 1.2.2 | MIT OR Apache-2.0 | github.com/servo/rust-url | `91377f48bf35011d042aa5abef9e7f2a0a625aaa` | `cb4cb245038516f5f85277875cdaa4f7d2c9a0fa0468de06ed190163b1581fcf` |
| hashbrown | 0.17.1 | MIT OR Apache-2.0 | github.com/rust-lang/hashbrown | `c62a63a61b7caf2de8f9ecb7b06a66b0ab6bdf3d` | `ed5909b6e89a2db4456e54cd5f673791d7eca6732202bbf2a9cc504fe2f9b84a` |
| heck | 0.5.0 | MIT OR Apache-2.0 | github.com/withoutboats/heck | `070693322aee7c5c7fbee7c9964bf8d7d3a29c96` | `2304e00983f87ffb38b55b444b5e3b60a884b5d30c0fca7d82fe33449bbe55ea` |
| hermit-abi | 0.5.3 | MIT OR Apache-2.0 | github.com/hermit-os/hermit-rs | `c0b97d12a2f1f65610ba84e6f23fbb69fc70c0f7` | `e17592d60ebacc7d5e169f4663c5f84f9161cc90328abcfe8456f41e4dfcb284` |
| html5ever | 0.40.1 | MIT OR Apache-2.0 | github.com/servo/html5ever | `d7232d746d3112f08926d169591576bd12fe2fdf` | `456a1a377e608e555d22ddab27ac0114bc7a7b4199078108e34c2aeae6c9b130` |
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
| indexmap | 2.14.2 | Apache-2.0 OR MIT | github.com/indexmap-rs/indexmap | `41a870887c4c77adf665886e63df08f406bfe37a` | `cc4e190f5d26ca7051642629da2c52fc03bde85a03197c99408dcd291734c855` |
| itertools | 0.14.0 | MIT OR Apache-2.0 | github.com/rust-itertools/itertools | `a015a6831525ee1637df747d3f530a627d9741bf` | `2b192c782037fadd9cfa75548310488aabdbf3d2da73885b31bd0abd03351285` |
| itoa | 1.0.18 | MIT OR Apache-2.0 | github.com/dtolnay/itoa | `af77385d0daf4d0e949e81f2588be2e44f69f086` | `8f42a60cbdf9a97f5d2305f08a87dc4e09308d1276d28c869c684d7777685682` |
| libc | 0.2.191 | MIT OR Apache-2.0 | github.com/rust-lang/libc | `ccc2ff4aaab962f7fde7b8edda7379c045b1ca12` | `65cd87c9a6d60536580b316ad985726104730a5151c99845f2490428ef91240a` |
| libm | 0.2.16 | MIT | github.com/rust-lang/compiler-builtins | `dfd2203a4d6110820ad7bb65cafe1bf331a03a3d` | `b6d2cec3eae94f9f509c767b45932f1ada8350c4bdb85af2fcab4a3c14807981` |
| litemap | 0.8.3 | Unicode-3.0 | github.com/unicode-org/icu4x | `a14f2dad852be26bad277ae704abf27a15cbfed1` | `47d9d19d1d6efa0109d2f65ff4c85cddd50bd572e5a00127ab10987290bcefae` |
| lock_api | 0.4.14 | MIT OR Apache-2.0 | github.com/Amanieu/parking_lot | `d7828fff7b5d6327ae608e82db45f888b344449a` | `224399e74b87b5f3557511d98dff8b14089b3dadafcab6bb93eab67d3aace965` |
| log | 0.4.35 | MIT OR Apache-2.0 | github.com/rust-lang/log | `a46450ba763280c29d35b53aa8ad0ff1fc1d78d0` | `5d15f86d12da44cd8d4ec3c73984537eb7f4cadcb9776a575a0a8a05ca11005c` |
| malloc_size_of_derive | 0.1.3 | MIT OR Apache-2.0 | github.com/bholley/malloc_size_of_derive | `9439206419825b9b3687b5a9d9a9a6dd29330854` | `f44db74bde26fdf427af23f1d146c211aed857c59e3be750cf2617f6b0b05c94` |
| markup5ever | 0.40.0 | MIT OR Apache-2.0 | github.com/servo/html5ever | `a193ea7f2492d1e51eb32955a09c241345348dba` | `0ab3dc68ac4a0f5719e560136778c1ee716e296030d75dbd4484e37e39e3a842` |
| memchr | 2.8.3 | Unlicense OR MIT | github.com/BurntSushi/memchr | `5fdb40c054e1fff359a2f7bdf7f87a13b34b465d` | `cf8baf1c55e62ffcace7a9f06f4bd9cd3f0c4beb022d3b367256b91b87513d98` |
| mime | 0.3.17 | MIT OR Apache-2.0 | github.com/hyperium/mime | `ce5062d216bf757a0ed3fc70f0fe255d1c8d74ae` | `6877bb514081ee2a7ff5ef9de3281f14a4dd4bceac4c09388074a6b5df8a139a` |
| multiversion_no_op | 1.0.0 | Apache-2.0 OR MIT | github.com/hsivonen/multiversion_no_op | `5463f2d9272305f21eed5111eaf221a195abf2f4` | `743fb55ba31b18fb1ecef6bdc9aa2743314978ac084044301a7eee33fb99a20d` |
| new_debug_unreachable | 1.0.6 | MIT | github.com/mbrubeck/rust-debug-unreachable | `4b3fa6e0b6d728ed2018690e087a440dd29e7d06` | `650eef8c711430f1a879fdd01d4745a7deea475becfb90269c06775983bbf086` |
| num-derive | 0.4.2 | MIT OR Apache-2.0 | github.com/rust-num/num-derive | `7cc33515dd2ae0eb43c5795c50ce49c554e8ba02` | `ed3955f1a9c7c0c15e092f9c887db08b1fc683305fdf6eb6684f22555355e202` |
| num-integer | 0.1.47 | MIT OR Apache-2.0 | github.com/rust-num/num-integer | `765fd9b3eb9397471d41b9f613e530f6d9d6383f` | `7ce2d95d4b3734dc35aa2f45e1aa22cd416814592a4f9d9205e11affd5b8e10b` |
| num-traits | 0.2.19 | MIT OR Apache-2.0 | github.com/rust-num/num-traits | `7ec3d41d39b28190ec1d42db38021107b3951f3a` | `071dfc062690e90b734c0b2273ce72ad0ffa95f0c74596bc250dcfd960262841` |
| num_cpus | 1.17.0 | MIT OR Apache-2.0 | github.com/seanmonstar/num_cpus | `342af76b486335e5a955c7314fc96cd104e7a17b` | `91df4bbde75afed763b708b7eee1e8e7651e02d97f6d5dd763e89367e957b23b` |
| parking_lot | 0.12.5 | MIT OR Apache-2.0 | github.com/Amanieu/parking_lot | `d7828fff7b5d6327ae608e82db45f888b344449a` | `93857453250e3077bd71ff98b6a65ea6621a19bb0f559a85248955ac12c45a1a` |
| percent-encoding | 2.3.2 | MIT OR Apache-2.0 | github.com/servo/rust-url | `91377f48bf35011d042aa5abef9e7f2a0a625aaa` | `9b4f627cb1b25917193a259e49bdad08f671f8d9708acfd5fe0a8c1455d87220` |
| phf | 0.14.0 | MIT | github.com/rust-phf/rust-phf | `1620d169aad09b02ef02a97b272470152e100bd6` | `010378780309880b08997fae13be7834dba947d36393bd372f2b1556deb2a2f6` |
| phf_codegen | 0.14.0 | MIT | github.com/rust-phf/rust-phf | `1620d169aad09b02ef02a97b272470152e100bd6` | `41b585a510fb76fdebead6897982ef2a03a21d8e6cbcca904999742a4afc6ffe` |
| phf_generator | 0.14.0 | MIT | github.com/rust-phf/rust-phf | `1620d169aad09b02ef02a97b272470152e100bd6` | `aeb62e0959d5a1bebc965f4d15d9e2b7cea002b6b0f5ba8cde6cc26738467100` |
| phf_macros | 0.14.0 | MIT | github.com/rust-phf/rust-phf | `1620d169aad09b02ef02a97b272470152e100bd6` | `5fa8d0ca26d424d27630da600c6624696e7dec8bf7b3b492b383c5dc49e5e085` |
| phf_shared | 0.14.0 | MIT | github.com/rust-phf/rust-phf | `1620d169aad09b02ef02a97b272470152e100bd6` | `c6fd9027e2d9319be6349febd1db4e8d02aa544921200c9b777720ac34a3aa89` |
| potential_utf | 0.1.6 | Unicode-3.0 | github.com/unicode-org/icu4x | `a14f2dad852be26bad277ae704abf27a15cbfed1` | `d83eb9bc6d8e5cf568e7a1101d60ee05e81ed50ea106026f3d18deeb046d7661` |
| precomputed-hash | 0.1.1 | MIT | github.com/emilio/precomputed-hash | not published¹ | `925383efa346730478fb4838dbe9137d2a47675ad789c546d150a6e1dd4ab31c` |
| proc-macro2 | 1.0.107 | MIT OR Apache-2.0 | github.com/dtolnay/proc-macro2 | `ed8a5497669cd63db33bf24646f261b012bbbc4a` | `985e7ec9bb745e6ce6535b544d84d6cd6f7ad8bd711c398938ae983b91a766d9` |
| quote | 1.0.47 | MIT OR Apache-2.0 | github.com/dtolnay/quote | `723dcb47d3f0ddc896e17287c8a8d3f2ea2317d5` | `1fbf4db142a473a8d80c26bbf18454ed458bf8d26c8219c331daecfdbd079001` |
| rayon | 1.12.0 | MIT OR Apache-2.0 | github.com/rayon-rs/rayon | `c9ced185ae3508246a9eb70c8407a1199bb1b77f` | `fb39b166781f92d482534ef4b4b1b2568f42613b53e5b6c160e24cfbfa30926d` |
| rayon-core | 1.13.0 | MIT OR Apache-2.0 | github.com/rayon-rs/rayon | `7af20d7692d5decbcb4adcaa079cd607e3e50814` | `22e18b0f0062d30d4230b2e85ff77fdfe4326feb054b9783a3460d8435c8ab91` |
| rustc-hash | 2.1.3 | Apache-2.0 OR MIT | github.com/rust-lang/rustc-hash | `c13e7ccca705e6255387a2ebc6dca142d6881621` | `6b1e7f9a428571be2dc5bc0505c13fb6bf936822b894ec87abf8a08a4e51742d` |
| rustc_version | 0.4.1 | MIT OR Apache-2.0 | github.com/djc/rustc-version-rs | `eeca449cca83e24150e46739e797aa82e9142809` | `cfcb3a22ef46e85b45de6ee7e79d063319ebb6594faafcf1c225ea92ab6e9b92` |
| rustversion | 1.0.23 | MIT OR Apache-2.0 | github.com/dtolnay/rustversion | `3a7c76605450b9a7299c6502a421909de9126a59` | `cf54715a573b99ac80df0bc206da022bcd442c974952c7b9720069370852e21f` |
| same-file | 1.0.6 | Unlicense/MIT | github.com/BurntSushi/same-file | `5799cd323b8eefd17a089c950dac113f66c89c9e` | `93fc1dc3aaa9bfed95e02e6eadabb4baf7e3078b0bd1b4d7b6b0b68378900502` |
| scopeguard | 1.2.0 | MIT OR Apache-2.0 | github.com/bluss/scopeguard | `bb988222848d39b0b9037b29da3a3a1eb05ebf4b` | `94143f37725109f92c262ed2cf5e59bce7498c01bcc1502d7b9afe439a4e9f49` |
| selectors | 0.41.0 | MPL-2.0 | github.com/servo/stylo | `34d25f741765a0bdb894f8271852e633299352d8` | `8322aebea94a6cc288ff884e8a49bdbceef60206c400c6c46d22e62fc4fd91a6` |
| semver | 1.0.28 | MIT OR Apache-2.0 | github.com/dtolnay/semver | `7625c7aa3f0e8ba21e099d1765bcebcb72aa8816` | `8a7852d02fc848982e0c167ef163aaff9cd91dc640ba85e263cb1ce46fae51cd` |
| serde | 1.0.229 | MIT OR Apache-2.0 | github.com/serde-rs/serde | `7fc3b4c30c94f73a96ebd1553f2b090d928fc3a8` | `4148590afebada386688f18773da617792bf2ef03ffc1e4cbd2b1d45b023e0ba` |
| serde_core | 1.0.229 | MIT OR Apache-2.0 | github.com/serde-rs/serde | `7fc3b4c30c94f73a96ebd1553f2b090d928fc3a8` | `67dca2c9c51e58a4791a4b1ed58308b39c64224d349a935ab5039aa360942a48` |
| serde_derive | 1.0.229 | MIT OR Apache-2.0 | github.com/serde-rs/serde | `7fc3b4c30c94f73a96ebd1553f2b090d928fc3a8` | `e7a5d71263a5a7d47b41f6b3f06ba276f10cc18b0931f1799f710578e2309348` |
| serde_spanned | 1.1.2 | MIT OR Apache-2.0 | github.com/toml-rs/toml | `f72038c2508112dfaaedb227a511f91bf9b2015c` | `4e7523beb55eece201a2356bee0bbca0d1ab466c14c07703b2e0ee6d42cb0c2c` |
| servo_arc | 0.5.0 | MIT OR Apache-2.0 | github.com/servo/stylo | `34d25f741765a0bdb894f8271852e633299352d8` | `b4eceb519ec34d8290aaf6e46f435102f2bfa63f22b05a144fb16ab266d3b198` |
| simdutf8 | 0.1.5 | MIT OR Apache-2.0 | github.com/rusticstuff/simdutf8 | `87ee8d9d20b849eae1974a82b248ee6fe7491bcc` | `e3a9fe34e3e7a50316060351f37187a3f546bce95496156754b601a5fa71b76e` |
| siphasher | 1.0.4 | MIT OR Apache-2.0 | github.com/jedisct1/rust-siphash | `ed028f01a16205b0819fa83b0cdc052e2d5ac3b7` | `33f4fe9184a62d842c9ef383018f3306d8ba224fd9d836f56d7288308847c256` |
| smallbitvec | 2.6.1 | MIT OR Apache-2.0 | github.com/servo/smallbitvec | `ad9b17eace931eb8190fa25f7775932ed48d370f` | `9b0e903ee191d8f7a8fbf0d712c3a1699d19e04ceba5ad1eb673053c7d938a09` |
| smallvec | 1.16.4 | MIT OR Apache-2.0 | github.com/servo/rust-smallvec | `eab77462eda2767d2386f1d251928f474827e1c7` | `d0d5d4b1981b8bbbb7aed204cae5726d683d5d6ac7e74ec54122ea28f315e7c8` |
| stable_deref_trait | 1.2.1 | MIT OR Apache-2.0 | github.com/storyyeller/stable_deref_trait | `30002b4228f7cdee309217b6bf6eee099dda0b00` | `6ce2be8dc25455e1f91df71bfa12ad37d7af1092ae736f3a6cd0e37bc7810596` |
| static_assertions | 1.1.0 | MIT OR Apache-2.0 | github.com/nvzqz/static-assertions-rs | `18bc65a094d890fe1faa5d3ccb70f12b89eabf56` | `a2eb9349b6444b326872e140eb1cf5e7c522154d69e7a0ffb0fb81c06b37543f` |
| string_cache | 0.11.0 | MIT OR Apache-2.0 | github.com/servo/string-cache | `0f6acf1c394f326dbf2adf9dfad2089cbdcf38ad` | `ffa8a5dbe8b3f0bbe29d4c3225daafaeead63afdc1b65fc4c01a1384166038e6` |
| string_cache_codegen | 0.11.2 | MIT OR Apache-2.0 | github.com/servo/string-cache | `5f845e2cdf504e76a53d6827a6f6d4bfcced1c93` | `928dcdf75e47626b3617a976ec205d9f057584c371c1f23b782129268d0e6edc` |
| strum | 0.28.0 | MIT | github.com/Peternator7/strum | `7376771128834d28bb9beba5c39846cba62e71ec` | `9628de9b8791db39ceda2b119bbe13134770b56c138ec1d3af810d045c04f9bd` |
| strum_macros | 0.28.0 | MIT | github.com/Peternator7/strum | `7376771128834d28bb9beba5c39846cba62e71ec` | `ab85eea0270ee17587ed4156089e10b9e6880ee688791d45a905f5b1ca36f664` |
| stylo | 0.22.0 | MPL-2.0 | github.com/servo/stylo | `34d25f741765a0bdb894f8271852e633299352d8` | `7a255e46af2478628c48daab3774090caabd7812b1dd3ca98801c2b9c53491ed` |
| stylo_atoms | 0.22.0 | MPL-2.0 | github.com/servo/stylo | `34d25f741765a0bdb894f8271852e633299352d8` | `ee1771588fd747ef16f8b1990061b6f37c7d3251a0f9b74f405a44c0c5fb8966` |
| stylo_derive | 0.22.0 | MPL-2.0 | github.com/servo/stylo | `34d25f741765a0bdb894f8271852e633299352d8` | `4ee5c550c2e00a0b3c046bef84a9ee217e04e7c014142eeb96f9e48781c5cfd3` |
| stylo_dom | 0.22.0 | MPL-2.0 | github.com/servo/stylo | `34d25f741765a0bdb894f8271852e633299352d8` | `9febcb03f2b669ef06b3e8a5209e6280971ed55cecf83fbf3c0562fcf2aa2ec6` |
| stylo_malloc_size_of | 0.22.0 | MIT OR Apache-2.0 | github.com/servo/stylo | `34d25f741765a0bdb894f8271852e633299352d8` | `83ed69d55ee907599f093221004a28371925236f09f20961963020abf38bab6a` |
| stylo_static_prefs | 0.22.0 | MPL-2.0 | github.com/servo/stylo | `34d25f741765a0bdb894f8271852e633299352d8` | `6767c48a1590461db79f8b434ccc5a2d65600a3b716751f7999b35ccead1e024` |
| stylo_traits | 0.22.0 | MPL-2.0 | github.com/servo/stylo | `34d25f741765a0bdb894f8271852e633299352d8` | `645d6bfe40eb6e86ebe9f2d2adadcbced41bfa96175de7cb13d4d178d27bc83d` |
| syn | 2.0.119 | MIT OR Apache-2.0 | github.com/dtolnay/syn | `3295f9e9841785ac88a5e558c884854d5fb7d67f` | `872831b642d1a07999a962a351ed35b955ea2cfc8f3862091e2a240a84f17297` |
| syn | 3.0.9 | MIT OR Apache-2.0 | github.com/dtolnay/syn | `7c44a5565b4421035101cc16184e9af08ab5fc77` | `d78c8dee4c7bf0e14673097256fed6142ce9d3b85a408189d07482442145823b` |
| synstructure | 0.13.2 | MIT | github.com/mystor/synstructure | `91ead072fa43c55f35880bd7f75a2b0eab72a04e` | `728a70f3dbaf5bab7f0c4b1ac8d7ae5ea60a4b5549c8a5914361c99147a709d2` |
| synstructure | 0.14.0 | MIT | github.com/mystor/synstructure | `30cfafe36e42405e717a8e55da042a2835a698d7` | `901704edd0dfe137f1987838ee4f259e4e063c31371bdb423f7ae38ec6f77f02` |
| tendril | 0.5.1 | MIT OR Apache-2.0 | github.com/servo/html5ever | `d64dfd4c21cf2451649107ade7eaf042d95fbc5a` | `5fed54709c5b3a53d09bb1c113ea4f5ceafd1e772ddcb0030a82e1d56c087b08` |
| thin-vec | 0.2.21 | MIT OR Apache-2.0 | github.com/mozilla/thin-vec | `326a2260b184146bbc3f12653993a115f7ec7d5e` | `d6a4b9ba8738cb4a4f399d37e266becfd475e75eb73425b87a05a2f2039ba63e` |
| tinystr | 0.8.5 | Unicode-3.0 | github.com/unicode-org/icu4x | `031d9e89f6a3fd26a14491e090729d46094328fd` | `abc840c238154d58f9e86b2792261edfde565a17696e1cda1a449de27979801a` |
| to_shmem | 0.6.0 | MPL-2.0 | github.com/servo/stylo | `34d25f741765a0bdb894f8271852e633299352d8` | `bbf077da8dd01cc18453bd7d9bc3a945c262f5f344562e5f6deb3914b4dd6f14` |
| to_shmem_derive | 0.2.0 | MPL-2.0 | github.com/servo/stylo | `34d25f741765a0bdb894f8271852e633299352d8` | `0f8e91a023810fd2019648410d00eea13a71f21d849e71d6d9ed936fb8061174` |
| toml | 1.1.8+spec-1.1.0 | MIT OR Apache-2.0 | github.com/toml-rs/toml | `ceac6e4412420d5a5c19e4f7293ad005cf1a31b2` | `20489e00e4d8741d6be680764cc12e270655e375a20d1011e844a9c3379e678d` |
| toml_datetime | 1.1.2+spec-1.1.0 | MIT OR Apache-2.0 | github.com/toml-rs/toml | `f72038c2508112dfaaedb227a511f91bf9b2015c` | `2b86d767906c6c42421dcba507eb9d203e779497710a47782a224bb871653053` |
| toml_parser | 1.1.5+spec-1.1.0 | MIT OR Apache-2.0 | github.com/toml-rs/toml | `ceac6e4412420d5a5c19e4f7293ad005cf1a31b2` | `baa693a8032d7e1cada7d0041e96126df243179ff061456783ac7f12bda4744c` |
| toml_writer | 1.1.3+spec-1.1.0 | MIT OR Apache-2.0 | github.com/toml-rs/toml | `f72038c2508112dfaaedb227a511f91bf9b2015c` | `06bdbd8cfc056b8d2e2e85f29b56a3bdbecb527cef81eb39e3e7b98af4652770` |
| uluru | 3.1.0 | MPL-2.0 | github.com/servo/uluru | `e98dc9762e505a3ae580f60f2a35bbd006f43a1e` | `7c8a2469e56e6e5095c82ccd3afb98dad95f7af7929aab6d8ba8d6e0f73657da` |
| unicode-ident | 1.0.27 | (MIT OR Apache-2.0) AND Unicode-3.0 | github.com/dtolnay/unicode-ident | `c1ac3afb721f3cea6527bc2255340a04d0de3ca0` | `a2c754d6c33795a1c324727428e5a7dedb5b06195f9890bdbcba760d3e246563` |
| url | 2.5.8 | MIT OR Apache-2.0 | github.com/servo/rust-url | `d6ea13c5f8e7e6e627f6390161b3e185bda5e5ce` | `ff67a8a4397373c3ef660812acab3268222035010ab8680ec4215f38ba3d0eed` |
| utf8_iter | 1.1.0 | Apache-2.0 OR MIT | github.com/hsivonen/utf8_iter | `17ac91ec5c6e18cf31a38dc1c0a704aebd35bb1f` | `af3bd9d3e571c26fcce9d033b87ea5ab061b8d2ffe9b7a5675d810d68f03edf0` |
| utf_types | 1.0.0 | Apache-2.0 OR MIT | github.com/hsivonen/utf_types | `3e8dc643be477f9f0be8cfa21e519b742649fa80` | `3a3b425286980a367278453f3b6d28cdcb134b574e5a35824a6ff5eebad50bb6` |
| void | 1.0.2 | MIT | github.com/reem/rust-void | not published¹ | `6a02e4885ed3bc0f2de90ea6dd45ebcbb66dacffe03547fadbb0eeae2770887d` |
| walkdir | 2.5.0 | Unlicense/MIT | github.com/BurntSushi/walkdir | `4f26be4d450910916ea11533b2efc52b9a6483bc` | `29790946404f91d9c5d06f9874efddea1dc06c5efe94541a7d6863108e3a5e4b` |
| web_atoms | 0.3.0 | MIT OR Apache-2.0 | github.com/servo/html5ever | `a193ea7f2492d1e51eb32955a09c241345348dba` | `7572660c8890448ba236b7376f27e389c6a7e1c70195622faced601f855c0ada` |
| winapi-util | 0.1.11 | Unlicense OR MIT | github.com/BurntSushi/winapi-util | `803874c57dc1f10ecd42f7c86d9de72f53818432` | `c2a7b1c03c876122aa43f3020e6c3c3ee5c05081c9a00739faf7503aeba10d22` |
| winnow | 1.0.4 | MIT | github.com/winnow-rs/winnow | `7539ec0fc27144bfdcf9a68b0dbbec48bd0d5bae` | `23b97319f7b8343df12cc98938e5c3eb436064524c8d2b4e30a1d3a36eecdf81` |
| writeable | 0.6.4 | Unicode-3.0 | github.com/unicode-org/icu4x | `a14f2dad852be26bad277ae704abf27a15cbfed1` | `3ad82d2a33cdc9674dc7465672f271e096168fcdbe0f799d9e6db8c5892679dc` |
| yoke | 0.8.4 | Unicode-3.0 | github.com/unicode-org/icu4x | `031d9e89f6a3fd26a14491e090729d46094328fd` | `417ef3fcf6c300b7dbd25c85531a746931c02ad536f3cf4786ed1e88223340b3` |
| yoke-derive | 0.8.4 | Unicode-3.0 | github.com/unicode-org/icu4x | `a59ab860d4bda548e94dfbf992d87fe1f761bc55` | `ec8ebde2db3681e8c9980cc27822030e68752690ddfa9473e739aeb4dbde6d71` |
| zerofrom | 0.1.8 | Unicode-3.0 | github.com/unicode-org/icu4x | `e9575f7e4b5aeddfeeb87ada5d3e715950e8233e` | `0ec05a11813ea801ff6d75110ad09cd0824ddba17dfe17128ea0d5f68e6c5272` |
| zerofrom-derive | 0.1.8 | Unicode-3.0 | github.com/unicode-org/icu4x | `ce235d80cf7cbd028e8bd59920420e1203b205b3` | `f75b4683f6c7f45248d4d64056a24298c6281e0993356d7d1b4a1a962ef10d4a` |
| zerotrie | 0.2.5 | Unicode-3.0 | github.com/unicode-org/icu4x | `a14f2dad852be26bad277ae704abf27a15cbfed1` | `4ea269c3bd32f0a32c321907a2ae912ba6f4649bb0fc764a15627e99a7095a3f` |
| zerovec | 0.11.9 | Unicode-3.0 | github.com/unicode-org/icu4x | `031d9e89f6a3fd26a14491e090729d46094328fd` | `f58a2799cfe9a1670c7ea8edbd52ef8edb93cb37a7f83bf31ba9e35d9dae6a40` |
| zerovec-derive | 0.11.7 | Unicode-3.0 | github.com/unicode-org/icu4x | `031d9e89f6a3fd26a14491e090729d46094328fd` | `c3743ddbf47738d929cd1ae2e199d40d0162979dfe31aef8ae646d2bf00bf97a` |
| parking_lot_core (patched) | 0.9.12 | MIT OR Apache-2.0 | github.com/Amanieu/parking_lot | `d7828fff7b5d6327ae608e82db45f888b344449a` | `2621685985a2ebf1c516881c026032ac7deafcda1a2c9b7850dc81e3dfcb64c1` |
| stylo (patched) | 0.22.0 | MPL-2.0 | github.com/servo/stylo | `34d25f741765a0bdb894f8271852e633299352d8` | `7a255e46af2478628c48daab3774090caabd7812b1dd3ca98801c2b9c53491ed` |
| windows-sys (stub) | 0.61.2 | MIT OR Apache-2.0 | github.com/microsoft/windows-rs | — (stands in for this repository's crate; no code) | — (a stub, not the published crate) |

¹ Published without `.cargo_vcs_info.json` (precomputed-hash 0.1.1, void 1.0.2); its integrity is the crates.io checksum.

**`void` 1.0.2** was published with `license = "MIT"` and no licence text: its repository had none at the 1.0.2
release (`ab2f4dbb`). `licenses/void-1.0.2-LICENSE-MIT` is the MIT text the same project added when it relicensed
(github.com/reem/rust-void, `ea5a2526d7a81ff45960525b62a3dbfd34c1703c`, file `LICENSE-MIT`, SHA-256
`cdc52ff9219a75ecc314567c862e31a369afe276ee75884f4211cf8cbacd2667`), fetched with `git`.

C2.3b's crates, by use:

**Linked into the binary** (the spike's §2.1): html5ever, markup5ever, tendril, web_atoms, selectors, servo_arc,
cssparser, string_cache, smallvec, parking_lot, parking_lot_core, lock_api, scopeguard, phf, phf_shared,
siphasher, precomputed-hash, new_debug_unreachable, stable_deref_trait, memchr, log, rustc-hash, cfg-if, bitflags,
derive_more, dtoa, dtoa-short, itoa. **Build host only:** cssparser-macros, derive_more-impl, phf_macros,
phf_generator, phf_codegen, string_cache_codegen, fastrand, proc-macro2, quote, syn (both majors), unicode-ident,
rustc_version, semver. `serde`, `serde_core` and `serde_derive` are vendored only because `phf`'s optional
`serde` feature puts them in the lockfile; nothing enables it, so they are neither built nor linked.

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
