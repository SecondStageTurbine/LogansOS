<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# `ot61-browser`: the vendored html5ever + selectors closure (OT-61 C2.3b)

The browser's first third-party crates: upstream **html5ever** (the HTML parser) and **selectors** (Stylo's
selector matching, published from `servo/stylo`), with **cssparser** and everything they depend on. Pinned here
as sources, per J.c.'s decision on the C2.3b spike (#200, ledger 2026-10-29): *vendor the sources*, and *the first
build runs on the CI runners* (`.github/workflows/ot61-closure.yml`). The closure and its reasoning are in
`docs/RESEARCH-2026-10-29-ot61-c2.3b-html5ever-stylo-closure-SPIKE.md`.

## Layout

- `vendor/<crate>-<version>/` — each crate exactly as published on crates.io, unpacked by `cargo vendor`. The
  `.cargo-checksum.json` beside each holds the SHA-256 of the published `.crate` (`package`) and of every file,
  so the copy can be checked against crates.io at any time.
- `patched/parking_lot_core-0.9.12/` — the one patched crate. The diff against the published crate is
  `parking_lot_core-0.9.12-neoplutarch.patch`.
- Only `userland/servo-closure-smoke` uses this tree, through `[patch.crates-io]` path entries in its
  `Cargo.toml`. (Not `[source]` replacement: `-Z build-std` resolves std's own dependencies through the same
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

## The crates

Licences from each crate's `Cargo.toml`. "Upstream commit" is the commit the release was published from
(`.cargo_vcs_info.json`). Release and repository last-commit dates, read on 2026-10-29, are in the spike's §2 for
these exact versions. Every licence is MPL-2.0, MIT, Apache-2.0 or Unlicense-or-MIT; `unicode-ident` adds the
Unicode-3.0 data licence and runs only on the build host. No GPL, LGPL or AGPL.

| Crate | Version | Licence | Repository | Upstream commit | crates.io `.crate` SHA-256 |
|---|---|---|---|---|---|
| bitflags | 2.13.2 | MIT OR Apache-2.0 | github.com/bitflags/bitflags | `80ce9b545acb0bd42150695fc351889cac1d8eb4` | `3ded4057c258ba199e2d26386d3af3780957ecaee6c4ef4041c6b4b8b97c0b06` |
| cfg-if | 1.0.5 | MIT OR Apache-2.0 | github.com/rust-lang/cfg-if | `8b0a2b0fc1fa3b3741b9e72895f390cf499fe36f` | `4e7648175b45a9a48536d676f68d918270699102aa8dab5496df06904c914600` |
| cssparser | 0.38.0 | MPL-2.0 | github.com/servo/rust-cssparser | `d128a90e1203fc4afb959420d6dc5e24f77a3191` | `11119743ad110e8c1bdccd930d7f5c30c99e5fc76a7b63ec9807e84eef0c5f59` |
| cssparser-macros | 0.7.1 | MPL-2.0 | github.com/servo/rust-cssparser | `d128a90e1203fc4afb959420d6dc5e24f77a3191` | `d045de693cb712d0b22c6a64be5b953f67b3ce00ab5ad3dd5d8b441886ab8e1a` |
| derive_more | 2.1.1 | MIT | github.com/JelteF/derive_more | `f7bb41ac054c060caaf5ff3212e74e42794cb4b4` | `d751e9e49156b02b44f9c1815bcb94b984cdcc4396ecc32521c739452808b134` |
| derive_more-impl | 2.1.1 | MIT | github.com/JelteF/derive_more | `f7bb41ac054c060caaf5ff3212e74e42794cb4b4` | `799a97264921d8623a957f6c3b9011f3b5492f557bbb7a5a19b7fa6d06ba8dcb` |
| dtoa | 1.0.11 | MIT OR Apache-2.0 | github.com/dtolnay/dtoa | `4c30faa2d91f167d7c343d13ebc9d30730456d56` | `4c3cf4824e2d5f025c7b531afcb2325364084a16806f6d47fbc1f5fbd9960590` |
| dtoa-short | 0.3.5 | MPL-2.0 | github.com/upsuper/dtoa-short | `2d905cdb8b2e08163dc0d015f529877fe657b4ef` | `cd1511a7b6a56299bd043a9c167a6d2bfb37bf84a6dfceaba651168adfb43c87` |
| fastrand | 2.5.0 | Apache-2.0 OR MIT | github.com/smol-rs/fastrand | `7a1cc2c69ebde6cb3442809213db3fe19a2cb0ba` | `da7c62ceae207dd37ea5b845da6a0696c799f85e97da1ab5b7910be3c1c80223` |
| html5ever | 0.40.1 | MIT OR Apache-2.0 | github.com/servo/html5ever | `d7232d746d3112f08926d169591576bd12fe2fdf` | `456a1a377e608e555d22ddab27ac0114bc7a7b4199078108e34c2aeae6c9b130` |
| itoa | 1.0.18 | MIT OR Apache-2.0 | github.com/dtolnay/itoa | `af77385d0daf4d0e949e81f2588be2e44f69f086` | `8f42a60cbdf9a97f5d2305f08a87dc4e09308d1276d28c869c684d7777685682` |
| lock_api | 0.4.14 | MIT OR Apache-2.0 | github.com/Amanieu/parking_lot | `d7828fff7b5d6327ae608e82db45f888b344449a` | `224399e74b87b5f3557511d98dff8b14089b3dadafcab6bb93eab67d3aace965` |
| log | 0.4.34 | MIT OR Apache-2.0 | github.com/rust-lang/log | `8034743dd9d7f7583bd9a670271483d176130911` | `f9f8bd3e56ce4dfc153cf470fffbfa98c7620958b312ca5c3a4b8d5181fd13c6` |
| markup5ever | 0.40.0 | MIT OR Apache-2.0 | github.com/servo/html5ever | `a193ea7f2492d1e51eb32955a09c241345348dba` | `0ab3dc68ac4a0f5719e560136778c1ee716e296030d75dbd4484e37e39e3a842` |
| memchr | 2.8.3 | Unlicense OR MIT | github.com/BurntSushi/memchr | `5fdb40c054e1fff359a2f7bdf7f87a13b34b465d` | `cf8baf1c55e62ffcace7a9f06f4bd9cd3f0c4beb022d3b367256b91b87513d98` |
| new_debug_unreachable | 1.0.6 | MIT | github.com/mbrubeck/rust-debug-unreachable | `4b3fa6e0b6d728ed2018690e087a440dd29e7d06` | `650eef8c711430f1a879fdd01d4745a7deea475becfb90269c06775983bbf086` |
| parking_lot | 0.12.5 | MIT OR Apache-2.0 | github.com/Amanieu/parking_lot | `d7828fff7b5d6327ae608e82db45f888b344449a` | `93857453250e3077bd71ff98b6a65ea6621a19bb0f559a85248955ac12c45a1a` |
| phf | 0.14.0 | MIT | github.com/rust-phf/rust-phf | `1620d169aad09b02ef02a97b272470152e100bd6` | `010378780309880b08997fae13be7834dba947d36393bd372f2b1556deb2a2f6` |
| phf_codegen | 0.14.0 | MIT | github.com/rust-phf/rust-phf | `1620d169aad09b02ef02a97b272470152e100bd6` | `41b585a510fb76fdebead6897982ef2a03a21d8e6cbcca904999742a4afc6ffe` |
| phf_generator | 0.14.0 | MIT | github.com/rust-phf/rust-phf | `1620d169aad09b02ef02a97b272470152e100bd6` | `aeb62e0959d5a1bebc965f4d15d9e2b7cea002b6b0f5ba8cde6cc26738467100` |
| phf_macros | 0.14.0 | MIT | github.com/rust-phf/rust-phf | `1620d169aad09b02ef02a97b272470152e100bd6` | `5fa8d0ca26d424d27630da600c6624696e7dec8bf7b3b492b383c5dc49e5e085` |
| phf_shared | 0.14.0 | MIT | github.com/rust-phf/rust-phf | `1620d169aad09b02ef02a97b272470152e100bd6` | `c6fd9027e2d9319be6349febd1db4e8d02aa544921200c9b777720ac34a3aa89` |
| precomputed-hash | 0.1.1 | MIT | github.com/emilio/precomputed-hash | not published¹ | `925383efa346730478fb4838dbe9137d2a47675ad789c546d150a6e1dd4ab31c` |
| proc-macro2 | 1.0.107 | MIT OR Apache-2.0 | github.com/dtolnay/proc-macro2 | `ed8a5497669cd63db33bf24646f261b012bbbc4a` | `985e7ec9bb745e6ce6535b544d84d6cd6f7ad8bd711c398938ae983b91a766d9` |
| quote | 1.0.47 | MIT OR Apache-2.0 | github.com/dtolnay/quote | `723dcb47d3f0ddc896e17287c8a8d3f2ea2317d5` | `1fbf4db142a473a8d80c26bbf18454ed458bf8d26c8219c331daecfdbd079001` |
| rustc-hash | 2.1.3 | Apache-2.0 OR MIT | github.com/rust-lang/rustc-hash | `c13e7ccca705e6255387a2ebc6dca142d6881621` | `6b1e7f9a428571be2dc5bc0505c13fb6bf936822b894ec87abf8a08a4e51742d` |
| rustc_version | 0.4.1 | MIT OR Apache-2.0 | github.com/djc/rustc-version-rs | `eeca449cca83e24150e46739e797aa82e9142809` | `cfcb3a22ef46e85b45de6ee7e79d063319ebb6594faafcf1c225ea92ab6e9b92` |
| scopeguard | 1.2.0 | MIT OR Apache-2.0 | github.com/bluss/scopeguard | `bb988222848d39b0b9037b29da3a3a1eb05ebf4b` | `94143f37725109f92c262ed2cf5e59bce7498c01bcc1502d7b9afe439a4e9f49` |
| selectors | 0.41.0 | MPL-2.0 | github.com/servo/stylo | `34d25f741765a0bdb894f8271852e633299352d8` | `8322aebea94a6cc288ff884e8a49bdbceef60206c400c6c46d22e62fc4fd91a6` |
| semver | 1.0.28 | MIT OR Apache-2.0 | github.com/dtolnay/semver | `7625c7aa3f0e8ba21e099d1765bcebcb72aa8816` | `8a7852d02fc848982e0c167ef163aaff9cd91dc640ba85e263cb1ce46fae51cd` |
| serde | 1.0.229 | MIT OR Apache-2.0 | github.com/serde-rs/serde | `7fc3b4c30c94f73a96ebd1553f2b090d928fc3a8` | `4148590afebada386688f18773da617792bf2ef03ffc1e4cbd2b1d45b023e0ba` |
| serde_core | 1.0.229 | MIT OR Apache-2.0 | github.com/serde-rs/serde | `7fc3b4c30c94f73a96ebd1553f2b090d928fc3a8` | `67dca2c9c51e58a4791a4b1ed58308b39c64224d349a935ab5039aa360942a48` |
| serde_derive | 1.0.229 | MIT OR Apache-2.0 | github.com/serde-rs/serde | `7fc3b4c30c94f73a96ebd1553f2b090d928fc3a8` | `e7a5d71263a5a7d47b41f6b3f06ba276f10cc18b0931f1799f710578e2309348` |
| servo_arc | 0.5.0 | MIT OR Apache-2.0 | github.com/servo/stylo | `34d25f741765a0bdb894f8271852e633299352d8` | `b4eceb519ec34d8290aaf6e46f435102f2bfa63f22b05a144fb16ab266d3b198` |
| siphasher | 1.0.4 | MIT OR Apache-2.0 | github.com/jedisct1/rust-siphash | `ed028f01a16205b0819fa83b0cdc052e2d5ac3b7` | `33f4fe9184a62d842c9ef383018f3306d8ba224fd9d836f56d7288308847c256` |
| smallvec | 1.16.4 | MIT OR Apache-2.0 | github.com/servo/rust-smallvec | `eab77462eda2767d2386f1d251928f474827e1c7` | `d0d5d4b1981b8bbbb7aed204cae5726d683d5d6ac7e74ec54122ea28f315e7c8` |
| stable_deref_trait | 1.2.1 | MIT OR Apache-2.0 | github.com/storyyeller/stable_deref_trait | `30002b4228f7cdee309217b6bf6eee099dda0b00` | `6ce2be8dc25455e1f91df71bfa12ad37d7af1092ae736f3a6cd0e37bc7810596` |
| string_cache | 0.11.0 | MIT OR Apache-2.0 | github.com/servo/string-cache | `0f6acf1c394f326dbf2adf9dfad2089cbdcf38ad` | `ffa8a5dbe8b3f0bbe29d4c3225daafaeead63afdc1b65fc4c01a1384166038e6` |
| string_cache_codegen | 0.11.2 | MIT OR Apache-2.0 | github.com/servo/string-cache | `5f845e2cdf504e76a53d6827a6f6d4bfcced1c93` | `928dcdf75e47626b3617a976ec205d9f057584c371c1f23b782129268d0e6edc` |
| syn | 2.0.119 | MIT OR Apache-2.0 | github.com/dtolnay/syn | `3295f9e9841785ac88a5e558c884854d5fb7d67f` | `872831b642d1a07999a962a351ed35b955ea2cfc8f3862091e2a240a84f17297` |
| syn | 3.0.9 | MIT OR Apache-2.0 | github.com/dtolnay/syn | `7c44a5565b4421035101cc16184e9af08ab5fc77` | `d78c8dee4c7bf0e14673097256fed6142ce9d3b85a408189d07482442145823b` |
| tendril | 0.5.1 | MIT OR Apache-2.0 | github.com/servo/html5ever | `d64dfd4c21cf2451649107ade7eaf042d95fbc5a` | `5fed54709c5b3a53d09bb1c113ea4f5ceafd1e772ddcb0030a82e1d56c087b08` |
| unicode-ident | 1.0.27 | (MIT OR Apache-2.0) AND Unicode-3.0 | github.com/dtolnay/unicode-ident | `c1ac3afb721f3cea6527bc2255340a04d0de3ca0` | `a2c754d6c33795a1c324727428e5a7dedb5b06195f9890bdbcba760d3e246563` |
| web_atoms | 0.3.0 | MIT OR Apache-2.0 | github.com/servo/html5ever | `a193ea7f2492d1e51eb32955a09c241345348dba` | `7572660c8890448ba236b7376f27e389c6a7e1c70195622faced601f855c0ada` |
| parking_lot_core (patched) | 0.9.12 | MIT OR Apache-2.0 | github.com/Amanieu/parking_lot | `d7828fff7b5d6327ae608e82db45f888b344449a` | `2621685985a2ebf1c516881c026032ac7deafcda1a2c9b7850dc81e3dfcb64c1` |

¹ `precomputed-hash` 0.1.1 (2017) predates `.cargo_vcs_info.json`; its integrity is the crates.io checksum.

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

## Obligations

MIT and Apache-2.0 notices for the linked crates go into `NOTICE` when a browser binary ships (each crate's
licence files are in its directory here). MPL-2.0 files (`selectors`, `cssparser`, `dtoa-short`) stay MPL-2.0.

## Updating

Bump the versions in `userland/servo-closure-smoke/Cargo.toml`, re-run `cargo vendor --versioned-dirs` from a
scratch copy with the `[patch]` table removed, re-apply the parking_lot_core patch if that crate moved, replace
`vendor/`, regenerate the `[patch]` table and this file's table, and re-read the spike's breakage list (§5)
against the new sources.
