SPDX-License-Identifier: CC-BY-4.0

# OT-61 C2.4 — Servo style/layout dependency closure at the pinned versions (Relay #200)

Date: 2026-10-31. Author: Prometheus (relay-runner).
Inputs: Servo `928d9accdffedaafb14a242ff61c2bcb6a56a345` (`origin/main`), Stylo rev `890aa121c6aa930c1a3c52f307fd4ba5d9410068`, `Cargo.lock` as ground truth, crates.io API + tarballs, GitHub API, `third_party/ot61-browser/` on `origin/claude/ot61-c2.3b-closure`, PAL/overlay sources.
Method: same as the C2.3b spike (`docs/RESEARCH-2026-10-29-ot61-c2.3b-html5ever-stylo-closure-SPIKE.md`): parse `Cargo.lock` with `tomllib`, walk transitive deps from the roots, fetch crates.io metadata + tarball per crate, scan sources for OS-surface use, classify build-time code and native C. No closure was built; this is research for J.c.'s supply-chain decision (ADR-078 seat D).

## 1. Headline numbers

| Closure | Crates | Breakdown |
|---|---|---|
| `stylo` only (C2.4 scope) | **152** | 141 crates.io + 11 Stylo-git |
| `servo-layout` full (browser graph) | **828** | 770 crates.io + 47 Servo-workspace + 11 Stylo-git |
| Already vendored at C2.3b | 44 | 39 of them are exactly in the stylo closure at the pinned versions |

The layout closure is effectively the whole browser: it pulls `mozjs_sys` (SpiderMonkey 153.6), WebRender/wgpu/naga, tokio/hyper/warp, the media stack (rav1e, wuff, speexdsp), and the full image/SVG stack. **C2.4 should be scoped to the stylo closure (152), not the layout closure (828).**

## 2. Stylo closure (152 crates)

| Crate | Version | License | Released | Build flags | OS-surface hits |
|---|---|---|---|---|---|
| allocator-api2 | 0.2.21 | MIT OR Apache-2.0 | 2024-12-01 | - | - |
| app_units | 0.7.8 | MPL-2.0 | 2025-03-10 | - | - |
| arrayvec | 0.7.8 | MIT OR Apache-2.0 | 2026-07-02 | - | - |
| atomic_refcell | 0.1.14 | Apache-2.0 OR MIT | 2026-04-20 | - | atomics |
| autocfg | 1.5.1 | Apache-2.0 OR MIT | 2026-05-22 | - | atomics |
| bitflags | 2.13.2 | MIT OR Apache-2.0 | 2026-09-10 | - | - |
| byteorder | 1.5.0 | Unlicense OR MIT | 2023-10-06 | - | - |
| cfg-if | 1.0.5 | MIT OR Apache-2.0 | 2026-09-16 | - | - |
| core_detect | 1.0.0 | MIT/Apache-2.0 | 2021-05-09 | - | atomics |
| core_maths | 0.1.1 | MIT | 2025-02-01 | - | - |
| crossbeam-deque | 0.8.8 | MIT OR Apache-2.0 | 2026-09-05 | build.rs | atomics |
| crossbeam-epoch | 0.9.21 | MIT OR Apache-2.0 | 2026-09-05 | build.rs | thread, sync, atomics |
| crossbeam-utils | 0.8.23 | MIT OR Apache-2.0 | 2026-09-05 | build.rs | thread, sync, atomics, monoclock |
| cssparser | 0.38.0 | MPL-2.0 | 2026-09-07 | - | - |
| cssparser-macros | 0.7.1 | MPL-2.0 | 2026-09-07 | proc-macro | - |
| darling | 0.20.11 | MIT | 2025-03-28 | - | - |
| darling_core | 0.20.11 | MIT | 2025-03-28 | - | thread, atomics |
| darling_macro | 0.20.11 | MIT | 2025-03-28 | proc-macro | - |
| derive_more | 2.1.1 | MIT | 2025-12-22 | - | - |
| derive_more-impl | 2.1.1 | MIT | 2025-12-22 | proc-macro | sync |
| displaydoc | 0.2.7 | MIT OR Apache-2.0 | 2026-07-28 | proc-macro | - |
| dtoa | 1.0.11 | MIT OR Apache-2.0 | 2025-12-27 | - | - |
| dtoa-short | 0.3.5 | MPL-2.0 | 2024-06-07 | - | - |
| either | 1.19.0 | MIT OR Apache-2.0 | 2026-10-06 | - | - |
| encoding_rs | 0.8.42 | (Apache-2.0 OR MIT) AND BSD-3-Clause | 2026-09-24 | build.rs | - |
| equivalent | 1.0.3 | Apache-2.0 OR MIT | 2026-10-13 | - | - |
| euclid | 0.22.14 | MIT OR Apache-2.0 | 2026-03-18 | - | - |
| fastrand | 2.5.0 | Apache-2.0 OR MIT | 2026-07-19 | - | thread, monoclock, getrandom |
| fnv | 1.0.7 | Apache-2.0 / MIT | 2020-05-14 | - | - |
| foldhash | 0.2.0 | Zlib | 2025-08-23 | - | atomics |
| form_urlencoded | 1.2.2 | MIT OR Apache-2.0 | 2025-08-21 | - | - |
| getrandom | 0.2.17 | MIT OR Apache-2.0 | 2026-01-11 | - | atomics, libc |
| hashbrown | 0.17.1 | MIT OR Apache-2.0 | 2026-05-09 | - | thread, atomics, libc |
| heck | 0.5.0 | MIT OR Apache-2.0 | 2024-03-12 | - | - |
| hermit-abi | 0.5.3 | MIT OR Apache-2.0 | 2026-08-31 | - | - |
| icu_collections | 2.1.1 | Unicode-3.0 | 2025-10-28 | - | - |
| icu_locale | 2.1.1 | Unicode-3.0 | 2025-10-28 | - | - |
| icu_locale_core | 2.3.0 | Unicode-3.0 | 2026-08-13 | - | - |
| icu_locale_data | 2.1.2 | Unicode-3.0 | 2026-01-12 | build.rs | - |
| icu_normalizer | 2.1.1 | Unicode-3.0 | 2025-10-28 | - | - |
| icu_normalizer_data | 2.1.1 | Unicode-3.0 | 2025-10-28 | build.rs | - |
| icu_properties | 2.1.2 | Unicode-3.0 | 2025-12-09 | - | - |
| icu_properties_data | 2.1.2 | Unicode-3.0 | 2025-12-09 | build.rs | - |
| icu_provider | 2.3.1 | Unicode-3.0 | 2026-08-20 | - | - |
| icu_segmenter | 2.1.2 | Unicode-3.0 | 2025-12-22 | - | - |
| icu_segmenter_data | 2.1.1 | Unicode-3.0 | 2025-10-28 | build.rs | - |
| ident_case | 1.0.1 | MIT/Apache-2.0 | 2019-03-18 | - | - |
| idna | 1.1.0 | MIT OR Apache-2.0 | 2025-08-21 | - | - |
| idna_adapter | 1.2.1 | Apache-2.0 OR MIT | 2025-05-08 | - | - |
| indexmap | 2.14.2 | Apache-2.0 OR MIT | 2026-09-05 | - | - |
| itertools | 0.14.0 | MIT OR Apache-2.0 | 2024-12-31 | - | atomics |
| itoa | 1.0.18 | MIT OR Apache-2.0 | 2026-03-20 | - | - |
| libc | 0.2.191 | MIT OR Apache-2.0 | 2026-10-16 | build.rs | sync, wallclock, libc |
| libm | 0.2.16 | MIT | 2026-01-24 | build.rs | atomics |
| litemap | 0.8.3 | Unicode-3.0 | 2026-08-13 | - | - |
| lock_api | 0.4.14 | MIT OR Apache-2.0 | 2025-10-03 | - | sync, atomics |
| log | 0.4.34 | MIT OR Apache-2.0 | 2026-08-22 | - | atomics, net |
| malloc_size_of_derive | 0.1.3 | MIT OR Apache-2.0 | 2024-08-01 | proc-macro | - |
| memchr | 2.8.3 | Unlicense OR MIT | 2026-07-08 | - | atomics |
| mime | 0.3.17 | MIT OR Apache-2.0 | 2023-03-20 | - | - |
| multiversion | 0.9.2 | MIT OR Apache-2.0 | 2026-10-22 | - | - |
| multiversion-macros | 0.9.2 | MIT OR Apache-2.0 | 2026-10-22 | proc-macro | atomics |
| multiversion_no_op | 1.0.0 | Apache-2.0 OR MIT | 2026-07-30 | proc-macro | - |
| new_debug_unreachable | 1.0.6 | MIT | 2024-03-15 | - | - |
| num-derive | 0.4.2 | MIT OR Apache-2.0 | 2024-02-06 | proc-macro | - |
| num-integer | 0.1.47 | MIT OR Apache-2.0 | 2026-08-12 | - | - |
| num-traits | 0.2.19 | MIT OR Apache-2.0 | 2024-05-03 | build.rs | - |
| num_cpus | 1.17.0 | MIT OR Apache-2.0 | 2025-05-30 | - | atomics, fs, env, libc |
| parking_lot | 0.12.5 | MIT OR Apache-2.0 | 2025-10-03 | - | thread, sync, atomics, monoclock |
| parking_lot_core | 0.9.12 | MIT OR Apache-2.0 | 2025-10-03 | build.rs | thread, atomics, monoclock, libc |
| percent-encoding | 2.3.2 | MIT OR Apache-2.0 | 2025-08-21 | - | - |
| phf | 0.14.0 | MIT | 2026-06-21 | - | - |
| phf_codegen | 0.14.0 | MIT | 2026-06-21 | - | - |
| phf_generator | 0.14.0 | MIT | 2026-06-21 | - | - |
| phf_macros | 0.14.0 | MIT | 2026-06-21 | proc-macro | - |
| phf_shared | 0.14.0 | MIT | 2026-06-21 | - | - |
| potential_utf | 0.1.6 | Unicode-3.0 | 2026-08-13 | - | - |
| ppv-lite86 | 0.2.21 | MIT OR Apache-2.0 | 2025-03-09 | - | - |
| precomputed-hash | 0.1.1 | MIT | 2017-09-29 | - | - |
| proc-macro2 | 1.0.107 | MIT OR Apache-2.0 | 2026-07-19 | build.rs | thread, atomics |
| quote | 1.0.47 | MIT OR Apache-2.0 | 2026-07-19 | build.rs | - |
| rand | 0.8.7 | MIT OR Apache-2.0 | 2026-07-11 | - | thread, atomics, libc |
| rand_chacha | 0.3.1 | MIT OR Apache-2.0 | 2021-06-09 | - | - |
| rand_core | 0.6.4 | MIT OR Apache-2.0 | 2022-09-15 | - | getrandom |
| rayon | 1.12.0 | MIT OR Apache-2.0 | 2026-04-14 | - | thread, sync, atomics |
| rayon-core | 1.13.0 | MIT OR Apache-2.0 | 2025-08-12 | build.rs | thread, sync, atomics, monoclock |
| redox_syscall | 0.5.18 | MIT | 2025-10-04 | - | thread, sync, atomics |
| rustc-hash | 2.1.3 | Apache-2.0 OR MIT | 2026-07-02 | - | thread |
| rustc_version | 0.4.1 | MIT OR Apache-2.0 | 2024-08-28 | - | - |
| rustversion | 1.0.23 | MIT OR Apache-2.0 | 2026-07-07 | proc-macro | wallclock |
| same-file | 1.0.6 | Unlicense/MIT | 2020-01-11 | - | atomics, fs |
| scopeguard | 1.2.0 | MIT OR Apache-2.0 | 2023-07-17 | - | thread |
| selectors | 0.41.0 | MPL-2.0 | stylo@890aa121 | see Stylo-git table | see notes |
| semver | 1.0.28 | MIT OR Apache-2.0 | 2026-04-04 | - | - |
| serde | 1.0.229 | MIT OR Apache-2.0 | 2026-07-18 | build.rs | sync, atomics, wallclock |
| serde_core | 1.0.229 | MIT OR Apache-2.0 | 2026-07-18 | build.rs | sync, atomics, wallclock |
| serde_derive | 1.0.229 | MIT OR Apache-2.0 | 2026-07-18 | proc-macro | thread |
| serde_spanned | 1.1.2 | MIT OR Apache-2.0 | 2026-10-08 | - | - |
| servo_arc | 0.5.0 | MPL-2.0 | stylo@890aa121 | see Stylo-git table | see notes |
| simdutf8 | 0.1.5 | MIT OR Apache-2.0 | 2024-09-22 | - | atomics |
| siphasher | 1.0.4 | MIT OR Apache-2.0 | 2026-09-25 | - | - |
| smallbitvec | 2.6.1 | MIT OR Apache-2.0 | 2026-05-06 | - | - |
| smallvec | 1.16.3 | MIT OR Apache-2.0 | 2026-10-10 | - | - |
| stable_deref_trait | 1.2.1 | MIT OR Apache-2.0 | 2025-10-09 | - | - |
| static_assertions | 1.1.0 | MIT OR Apache-2.0 | 2019-11-03 | - | - |
| string_cache | 0.11.0 | MIT OR Apache-2.0 | 2026-08-21 | - | sync, atomics |
| string_cache_codegen | 0.11.2 | MIT OR Apache-2.0 | 2026-09-14 | - | fs |
| strum | 0.28.0 | MIT | 2026-02-22 | - | - |
| strum_macros | 0.28.0 | MIT | 2026-02-22 | proc-macro | - |
| stylo | 0.22.0 | MPL-2.0 | stylo@890aa121 | see Stylo-git table | see notes |
| stylo_atoms | 0.22.0 | MPL-2.0 | stylo@890aa121 | see Stylo-git table | see notes |
| stylo_derive | 0.22.0 | MPL-2.0 | stylo@890aa121 | see Stylo-git table | see notes |
| stylo_dom | 0.22.0 | MPL-2.0 | stylo@890aa121 | see Stylo-git table | see notes |
| stylo_malloc_size_of | 0.22.0 | MPL-2.0 | stylo@890aa121 | see Stylo-git table | see notes |
| stylo_static_prefs | 0.22.0 | MPL-2.0 | stylo@890aa121 | see Stylo-git table | see notes |
| stylo_traits | 0.22.0 | MPL-2.0 | stylo@890aa121 | see Stylo-git table | see notes |
| syn | 2.0.119 | MIT OR Apache-2.0 | 2026-07-15 | - | thread |
| syn | 3.0.4 | MIT OR Apache-2.0 | 2026-08-24 | - | thread |
| synstructure | 0.13.2 | MIT | 2025-04-30 | - | - |
| thin-vec | 0.2.21 | MIT OR Apache-2.0 | 2026-10-07 | - | - |
| tinystr | 0.8.4 | Unicode-3.0 | 2026-08-13 | - | - |
| to_shmem | 0.6.0 | MPL-2.0 | stylo@890aa121 | see Stylo-git table | see notes |
| to_shmem_derive | 0.2.0 | MPL-2.0 | stylo@890aa121 | see Stylo-git table | see notes |
| toml | 1.1.8+spec-1.1.0 | MIT OR Apache-2.0 | 2026-10-09 | - | atomics |
| toml_datetime | 1.1.2+spec-1.1.0 | MIT OR Apache-2.0 | 2026-10-08 | - | - |
| toml_parser | 1.1.5+spec-1.1.0 | MIT OR Apache-2.0 | 2026-10-09 | - | atomics |
| toml_writer | 1.1.3+spec-1.1.0 | MIT OR Apache-2.0 | 2026-10-08 | - | - |
| uluru | 3.1.0 | MPL-2.0 | 2024-04-08 | - | - |
| unicode-bidi | 0.3.18 | MIT OR Apache-2.0 | 2024-12-16 | - | - |
| unicode-ident | 1.0.27 | (MIT OR Apache-2.0) AND Unicode-3.0 | 2026-10-12 | - | - |
| url | 2.5.8 | MIT OR Apache-2.0 | 2026-01-05 | - | atomics, net |
| utf16_iter | 1.0.5 | Apache-2.0 OR MIT | 2023-12-01 | - | - |
| utf8_iter | 1.0.4 | Apache-2.0 OR MIT | 2023-12-01 | - | - |
| void | 1.0.2 | MIT | 2016-04-18 | - | - |
| walkdir | 2.5.0 | Unlicense/MIT | 2024-03-01 | C:1 | fs |
| wasi | 0.11.1+wasi-snapshot-preview1 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | 2025-06-10 | - | - |
| web_atoms | 0.3.0 | MIT OR Apache-2.0 | 2026-09-11 | build.rs | - |
| winapi-util | 0.1.11 | Unlicense OR MIT | 2025-09-07 | - | fs |
| windows-link | 0.2.1 | MIT OR Apache-2.0 | 2025-10-06 | - | - |
| windows-sys | 0.61.2 | MIT OR Apache-2.0 | 2025-10-06 | - | wallclock |
| winnow | 1.0.4 | MIT | 2026-07-13 | - | atomics, env |
| write16 | 1.0.0 | Apache-2.0 OR MIT | 2022-07-18 | - | - |
| writeable | 0.6.4 | Unicode-3.0 | 2026-08-13 | - | - |
| yoke | 0.8.3 | Unicode-3.0 | 2026-06-03 | - | thread |
| yoke-derive | 0.8.2 | Unicode-3.0 | 2026-04-01 | proc-macro | - |
| zerocopy | 0.8.63 | BSD-2-Clause OR Apache-2.0 OR MIT | 2026-10-13 | build.rs | atomics |
| zerocopy-derive | 0.8.63 | BSD-2-Clause OR Apache-2.0 OR MIT | 2026-10-13 | proc-macro | fs, env |
| zerofrom | 0.1.8 | Unicode-3.0 | 2026-05-12 | - | - |
| zerofrom-derive | 0.1.7 | Unicode-3.0 | 2026-04-01 | proc-macro | - |
| zerotrie | 0.2.5 | Unicode-3.0 | 2026-08-13 | - | - |
| zerovec | 0.11.8 | Unicode-3.0 | 2026-08-19 | - | - |
| zerovec-derive | 0.11.7 | Unicode-3.0 | 2026-10-22 | proc-macro | - |

OS-surface legend: thread=`std::thread`, sync=Mutex/RwLock/Condvar/Once, atomics, wallclock=`SystemTime`, monoclock=`Instant`, fs=`std::fs`, net=`std::net`, env=`std::env`, getrandom=`getrandom::`, libc=`libc::`. Hits are raw source counts excluding tests/build scripts; several are target-gated (see section 6).

## 3. Stylo-git crates (11, rev 890aa121, all MPL-2.0)

| Crate | build.rs | proc-macro | Notes |
|---|---|---|---|
| stylo 0.22.0 | yes | — | `style/build.rs` |
| stylo_dom 0.22.0 | no | — | |
| stylo_atoms 0.22.0 | yes | — | codegen build.rs |
| stylo_traits 0.22.0 | no | — | dir `style_traits` |
| stylo_static_prefs 0.22.0 | yes | — | build.rs reads `preferences.toml` at build time |
| stylo_malloc_size_of 0.22.0 | no | — | dir `malloc_size_of` |
| stylo_derive 0.22.0 | no | yes | dir `style_derive` |
| to_shmem 0.6.0 | no | — | |
| to_shmem_derive 0.2.0 | no | yes | |
| selectors 0.41.0 | no | — | already vendored at C2.3b |
| servo_arc 0.5.0 | no | — | already vendored at C2.3b |

## 4. Servo workspace path-crates in the layout closure (47, all 0.7.0)

License: MPL-2.0 inherited from `[workspace.package]` except `servo-hyper-serde` and `servo-malloc-size-of` (MIT OR Apache-2.0) and `servo-script-bindings` (MPL-2.0 AND BSD-3-Clause).
build.rs: `servo-allocator`, `servo-embedder-traits`, `servo-script`, `servo-script-bindings`, `servo-script-webcrypto`, `servo-script-webgpu`.
proc-macro: `servo-config-macro`, `servo-deny-public-fields`, `servo-dom-struct`, `servo-jstraceable-derive`, `servo-media-derive`, `servo-tracing`.
Only `servo-layout`, `servo-layout-api`, `servo-paint-api`, `servo-geometry`, `servo-malloc-size-of`, `servo-url`, `servo-pixels`, `servo-base`, `servo-config` are plausibly needed for a C2.4-style layout slice; the rest (media/script/webgpu/webxr/bluetooth/devtools) come along only because `servo-layout` depends on the whole browser graph.

## 5. Layout extension beyond stylo (why 828, component view)

| Component pull | Key crates (license) | Native/toolchain |
|---|---|---|
| SpiderMonkey | mozjs_sys 153.6.0-1 (MPL-2.0) + 11 mozjs_icu/glue crates (Unicode-3.0/MIT) | prebuilt lib + C glue |
| WebRender | webrender 0.70.1 (MPL-2.0, build.rs), webrender_api, wr_glyph_rasterizer | GPU |
| wgpu/naga | wgpu-core/hal 30.0.1, naga 30.0.1 (MIT/Apache) | ash, khronos-egl, x11-dl, wayland-sys, objc2 (target-gated) |
| GL | gleam 0.15.1 (build.rs), glutin_wgl_sys, cgl | target-gated |
| ANGLE | mozangle 0.7.1 (BSD-3, build.rs, bindgen) | **1125 C/C++ files** |
| Fonts | freetype 0.8.0 + freetype-sys 0.23.0 (build.rs, **598 C files**), harfbuzz-sys 0.8.0 (**169 C**), harfrust 0.12.0 (pure Rust), fontsan 0.7.0 (**99 C**), yeslogic-fontconfig-sys (build.rs), dwrote (Windows-gated) | C toolchain required |
| Allocator | tikv-jemallocator/jemalloc-sys 0.6.1 (**355 C**) | C toolchain |
| Network | tokio 1.54.1, hyper 1.12.0, warp 0.4.3, h2, rustls, aws-lc-sys 0.46.0, ring 0.17.14, libz-sys 1.1.31 | C/CMake for aws-lc |
| Media | servo-media*, rav1e 0.8.1 (BSD-2), ravif, wuff 0.2.9, speexdsp-resampler (BSD-3) | |
| Images/SVG | image 0.25.10, resvg/usvg 0.48.1, fontdb, tiny-skia, exr, tiff, gif, png, zune-jpeg | |
| IPC | ipc-channel 0.23.0, bincode, postcard, swapper | |
| Crypto (WebCrypto) | p256/p384/p521, curve25519-dalek, rsa, sha2, aes-gcm | |
| DevTools/WebDriver | webdriver 0.54.0 (MPL-2.0), warp | |

Native-C scan covered 277 crates (stylo closure + priority layout crates); the remaining 493 layout crates were metadata-only.

## 6. OS needs vs PAL/overlay (tools/ot61-pal-manifest.toml)

Provided and sufficient: threads+futex (`thread_neoplutarch.rs`), Mutex/RwLock/Condvar/Once, atomics, monotonic `Instant` (10 ms tick), entropy `SYS_GETRANDOM`, sealed-fs reads, heap `SYS_MAP_REGION`.

Gaps and how the stylo closure touches them:
- **`num_cpus` 1.17.0** — `global_style_data.rs:192` sizes the style pool as `num_cpus::get()*3/4`. No CPU-count syscall; `available_parallelism` errors, so num_cpus falls back to **1** -> single style thread. Correct but slow; a PAL `SYS_CPU_COUNT` (or a config override) is the fix.
- **`parking_lot` 0.12.5 / `parking_lot_core` 0.9.12** — used by stylo (`global_style_data.rs:13`). Already patched for neoplutarch at C2.3b (`parking_lot_core-0.9.12-neoplutarch.patch`); same patch applies.
- **`rand` 0.8.7** — stylo uses `default-features=false, features=["small_rng"]` only: `SmallRng::seed_from_u64(hasher.finish())` (`style/values/computed/mod.rs:554`). **No entropy needed at runtime.** `getrandom` 0.2.17 in the closure is target-gated (unix/wasi paths) and not linked on `x86_64-unknown-neoplutarch`.
- **`fastrand` 2.5.0** — getrandom/monoclock hits are in its default-seed path; check feature usage before vendoring.
- **`libc` 0.2.191** — in the closure via num_cpus/parking_lot_core/rand, all target-gated; not linked on our target (same conclusion as C2.3b).
- **`SystemTime`** — hits in serde/serde_core (test paths), rustversion (build), windows-sys (target-gated). No runtime wallclock need in stylo.
- **`std::env`** — winnow/toml hits are optional features; stylo itself does not read env at runtime.
- **`std::fs`** — `string_cache_codegen`/`walkdir` are build-time codegen only; `num_cpus` fs hits are target-gated.
- **`std::net`** — only `url`'s optional feature and `log` false-positive; no runtime net in stylo.

## 7. Build-time code execution surface (host toolchain)

- build.rs in the stylo closure: `crossbeam-deque/epoch/utils`, `encoding_rs`, `icu_locale_data`, `icu_normalizer_data`, `icu_properties_data`, `icu_segmenter_data`, `libc`, `libm`, `num-traits`, `parking_lot_core`, `proc-macro2`, `quote`, `rayon-core`, `serde`/`serde_core`, `web_atoms`, `zerocopy` + stylo's own 3 (see section 3).
- proc-macros: `cssparser-macros`, `darling_macro`, `derive_more-impl`, `displaydoc`, `malloc_size_of_derive`, `multiversion-macros`, `multiversion_no_op`, `num-derive`, `phf_macros`, `serde_derive`, `strum_macros`, `stylo_derive`, `to_shmem_derive`, `yoke-derive`, `zerofrom-derive`, `zerovec-derive`, `zerocopy-derive`.
- Vendoring does not remove these; they run on the host at build time (same stance as C2.3b).

## 8. Native C in the layout closure (C3+ blockers, not C2.4)

mozangle 1125, freetype-sys 598, tikv-jemalloc-sys 355, harfbuzz-sys 169, fontsan 99 C/C++ files; plus aws-lc-sys (CMake), ring (asm), libz-sys, mozjs_sys (prebuilt). harfrust 0.12.0 is a pure-Rust harfbuzz replacement worth noting for the font path.

## 9. License posture

- No GPL/LGPL/EPL/AGPL anywhere in the 828-crate closure (checked crates.io license fields; `r-efi` is MIT/Apache/ISC despite the name).
- MPL-2.0 core: stylo, selectors, servo_arc, cssparser, dtoa-short, uluru, app_units, swapper, dwrote, webdriver, webrender, all 47 Servo workspace crates (except the 3 noted). File-level copyleft; fine for linking, modifications to MPL files must be published — consistent with ADR-078.
- Unicode-3.0: 50 crates (icu4x family + mozjs_icu + unicode-ident dual). Permissive-style but a named license; keep license texts in vendored trees.
- Everything else MIT/Apache-2.0/BSD/ISC/Zlib/Unlicense. `aws-lc-sys` is a long AND-of-permissive-licenses — fine.

## 10. C2.3b vendored overlap (third_party/ot61-browser/vendor/)

44 vendored crates; **39 are in the stylo closure at exactly the pinned versions** (bitflags, cfg-if, cssparser, cssparser-macros, derive_more(+impl), dtoa, dtoa-short, fastrand, itoa, lock_api, log, memchr, new_debug_unreachable, parking_lot, phf*, precomputed-hash, proc-macro2, quote, rustc-hash, rustc_version, scopeguard, selectors, semver, serde, serde_core, serde_derive, servo_arc, siphasher, stable_deref_trait, string_cache(+codegen), syn-2.0.119, unicode-ident, web_atoms).
5 vendored crates are NOT in the stylo closure: html5ever, markup5ever, tendril (parser-only), and two version drifts: **smallvec vendored 1.16.4 vs pinned 1.16.3**, **syn vendored 3.0.9 vs pinned 3.0.4**. C2.4 must add ~113 new crates (152 - 39) and reconcile the two drifts.

## 11. Decisions for J.c. (seat D, ADR-078)

1. Scope C2.4 to the **stylo closure (152 crates)**; do not vendor the 828-crate layout graph.
2. Vendoring delta: +113 crates, fix smallvec/syn drift, reuse the C2.3b parking_lot_core patch.
3. PAL: add `SYS_CPU_COUNT` (or config override) so the style pool scales; everything else stylo needs is already provided.
4. No license blockers. Keep Unicode-3.0 and MPL notices in vendored trees.
5. Native C (fonts/ANGLE/jemalloc/SpiderMonkey) is a C3+ conversation; harfrust is the pure-Rust escape hatch for fonts.

## Appendix A — full 828-crate layout closure (name version (source))

```
accesskit 0.24.0 (io)
addr2line 0.25.1 (io)
adler2 2.0.1 (io)
aead 0.6.1 (io)
aes 0.9.3 (io)
aes-gcm 0.11.1 (io)
aes-kw 0.3.1 (io)
aho-corasick 1.1.5 (io)
aligned 0.4.3 (io)
aligned-vec 0.6.4 (io)
alloc-no-stdlib 2.0.4 (io)
alloc-stdlib 0.2.4 (io)
alloca 0.4.0 (io)
allocator-api2 0.2.21 (io)
android_system_properties 0.1.6 (io)
anes 0.1.6 (io)
anstream 0.6.21 (io)
anstream 1.0.0 (io)
anstyle 1.0.14 (io)
anstyle-parse 0.2.7 (io)
anstyle-parse 1.0.0 (io)
anstyle-query 1.1.5 (io)
anstyle-wincon 3.0.11 (io)
anyhow 1.0.104 (io)
app_units 0.7.8 (io)
arbitrary 1.5.0 (io)
arg_enum_proc_macro 0.3.4 (io)
argon2 0.6.0 (io)
arrayref 0.3.9 (io)
arraystring 0.3.0 (io)
arrayvec 0.7.8 (io)
as-slice 0.2.1 (io)
ash 0.38.0+1.3.281 (io)
atomic-waker 1.1.2 (io)
atomic_refcell 0.1.14 (io)
autocfg 1.5.1 (io)
av-scenechange 0.14.1 (io)
av1-grain 0.2.5 (io)
avif-serialize 0.8.9 (io)
aws-lc-rs 1.18.2 (io)
aws-lc-sys 0.46.0 (io)
backtrace 0.3.76 (io)
base16ct 1.0.0 (io)
base64 0.22.1 (io)
base64 0.23.1 (io)
base64ct 1.8.3 (io)
bincode 1.3.3 (io)
bindgen 0.72.1 (io)
bit-set 0.10.0 (io)
bit-vec 0.9.1 (io)
bit_field 0.10.3 (io)
bitflags 1.3.2 (io)
bitflags 2.13.2 (io)
bitstream-io 4.10.0 (io)
blake2 0.11.0 (io)
block-buffer 0.10.4 (io)
block-buffer 0.12.1 (io)
block-padding 0.4.2 (io)
block2 0.6.2 (io)
brotli 8.0.4 (io)
brotli-decompressor 5.0.3 (io)
bs58 0.5.1 (io)
buf-read-ext 0.4.0 (io)
build-parallel 0.1.2 (io)
built 0.8.1 (io)
bumpalo 3.20.3 (io)
byte-slice-cast 1.2.3 (io)
bytemuck 1.25.2 (io)
bytemuck_derive 1.12.1 (io)
byteorder 1.5.0 (io)
byteorder-lite 0.1.0 (io)
bytes 1.12.1 (io)
calendrical_calculations 0.2.4 (io)
cast 0.3.0 (io)
cbc 0.2.1 (io)
cbindgen 0.29.4 (io)
cc 1.9.0 (io)
cexpr 0.6.0 (io)
cfg-if 1.0.5 (io)
cfg_aliases 0.2.2 (io)
cgl 0.3.2 (io)
chacha20 0.10.2 (io)
chacha20poly1305 0.11.0 (io)
chardetng 1.0.0 (io)
chrono 0.4.45 (io)
ciborium 0.2.2 (io)
ciborium-io 0.2.2 (io)
ciborium-ll 0.2.2 (io)
cipher 0.5.2 (io)
clang-sys 1.9.1 (io)
clap 4.6.7 (io)
clap_builder 4.6.7 (io)
clap_lex 1.1.1 (io)
cmake 0.1.58 (io)
cmov 0.5.4 (io)
cobs 0.3.0 (io)
codespan-reporting 0.13.1 (io)
color 0.3.3 (io)
color_quant 1.1.0 (io)
colorchoice 1.0.5 (io)
const-oid 0.10.2 (io)
content-security-policy 0.9.0 (io)
cookie 0.16.2 (io)
cookie 0.18.2 (io)
core-foundation 0.9.4 (io)
core-foundation-sys 0.8.7 (io)
core-graphics 0.23.2 (io)
core-graphics-types 0.1.3 (io)
core-text 20.1.0 (io)
core_detect 1.0.0 (io)
core_maths 0.1.1 (io)
cpubits 0.1.1 (io)
cpufeatures 0.2.17 (io)
cpufeatures 0.3.1 (io)
crc32fast 1.5.3 (io)
criterion 0.8.2 (io)
criterion-plot 0.8.2 (io)
crossbeam-channel 0.5.17 (io)
crossbeam-deque 0.8.8 (io)
crossbeam-epoch 0.9.21 (io)
crossbeam-utils 0.8.23 (io)
crunchy 0.2.4 (io)
crypto-bigint 0.7.6 (io)
crypto-common 0.1.7 (io)
crypto-common 0.2.2 (io)
crypto-primes 0.7.2 (io)
cshake 0.2.1 (io)
cssparser 0.38.0 (io)
cssparser-macros 0.7.1 (io)
ctr 0.10.1 (io)
ctutils 0.4.5 (io)
curve25519-dalek 5.0.0 (io)
curve25519-dalek-derive 0.1.1 (io)
darling 0.20.11 (io)
darling 0.24.1 (io)
darling_core 0.20.11 (io)
darling_core 0.24.1 (io)
darling_macro 0.20.11 (io)
darling_macro 0.24.1 (io)
data-url 0.3.2 (io)
dbl 0.5.0 (io)
defmt 1.2.0 (io)
defmt-macros 1.2.0 (io)
defmt-parser 1.1.0 (io)
der 0.8.2 (io)
deranged 0.5.9 (io)
derive_more 2.1.1 (io)
derive_more-impl 2.1.1 (io)
digest 0.10.7 (io)
digest 0.11.3 (io)
diplomat 0.14.0 (io)
diplomat-runtime 0.14.0 (io)
diplomat_core 0.14.0 (io)
dispatch2 0.3.1 (io)
displaydoc 0.2.7 (io)
dlib 0.5.3 (io)
document-features 0.2.12 (io)
dpi 0.1.2 (io)
dtoa 1.0.11 (io)
dtoa-short 0.3.5 (io)
dunce 1.0.5 (io)
dwrote 0.11.5 (io)
dyn-clone 1.0.20 (io)
ecdsa 0.17.0 (io)
ed25519 3.0.0 (io)
ed25519-dalek 3.0.0 (io)
ed448 0.5.0 (io)
ed448-goldilocks 0.14.0-pre.15 (io)
either 1.19.0 (io)
elliptic-curve 0.14.1 (io)
embedded-io 0.4.0 (io)
embedded-io 0.6.1 (io)
encoding_c 0.9.8 (io)
encoding_c_mem 0.2.6 (io)
encoding_rs 0.8.42 (io)
enumn 0.1.14 (io)
env_filter 0.1.4 (io)
env_logger 0.11.8 (io)
equator 0.4.2 (io)
equator-macro 0.4.2 (io)
equivalent 1.0.3 (io)
errno 0.3.14 (io)
etagere 0.2.15 (io)
euclid 0.22.14 (io)
exr 1.74.2 (io)
fastrand 2.5.0 (io)
fax 0.2.7 (io)
fdeflate 0.3.7 (io)
fearless_simd 0.4.1 (io)
ff 0.14.0 (io)
fft-convolver 0.4.0 (io)
fiat-crypto 0.3.0 (io)
filetime 0.2.29 (io)
find-msvc-tools 0.1.15 (io)
fixed_decimal 0.7.3 (io)
fixedbitset 0.5.7 (io)
flate2 1.1.9 (io)
float-cmp 0.9.0 (io)
fnv 1.0.7 (io)
foldhash 0.1.5 (io)
foldhash 0.2.0 (io)
font-types 0.12.7 (io)
fontconfig-parser 0.5.8 (io)
fontdb 0.24.0 (io)
fontsan 0.7.0 (io)
foreign-types 0.5.0 (io)
foreign-types-macros 0.2.4 (io)
foreign-types-shared 0.3.1 (io)
form_urlencoded 1.2.2 (io)
freetype 0.8.0 (io)
freetype-sys 0.23.0 (io)
fs_extra 1.3.0 (io)
futures-channel 0.3.34 (io)
futures-core 0.3.34 (io)
futures-io 0.3.34 (io)
futures-macro 0.3.34 (io)
futures-sink 0.3.34 (io)
futures-task 0.3.34 (io)
futures-util 0.3.34 (io)
generic-array 0.14.7 (io)
getrandom 0.2.17 (io)
getrandom 0.3.4 (io)
getrandom 0.4.1 (io)
ghash 0.6.0 (io)
gif 0.14.2 (io)
gimli 0.32.3 (io)
gl_generator 0.14.0 (io)
gleam 0.15.1 (io)
glifo 0.3.0 (io)
glob 0.3.4 (io)
glow 0.17.0 (io)
glslopt 0.1.14 (io)
glutin_wgl_sys 0.6.1 (io)
gpu-allocator 0.28.0 (io)
group 0.14.0 (io)
guillotiere 0.7.0 (io)
h2 0.4.20 (io)
half 2.7.1 (io)
harfbuzz-sys 0.8.0 (io)
harfrust 0.12.0 (io)
hash2curve 0.14.0 (io)
hashbrown 0.12.3 (io)
hashbrown 0.15.5 (io)
hashbrown 0.16.1 (io)
hashbrown 0.17.1 (io)
hashlink 0.11.1 (io)
headers 0.4.2 (io)
headers-core 0.3.0 (io)
heck 0.5.0 (io)
hermit-abi 0.5.3 (io)
hex 0.4.3 (io)
hkdf 0.13.0 (io)
hmac 0.13.0 (io)
html5ever 0.40.1 (io)
http 1.5.0 (io)
http-body 1.1.0 (io)
http-body-util 0.1.5 (io)
httparse 1.10.1 (io)
httpdate 1.0.3 (io)
hybrid-array 0.4.15 (io)
hyper 1.12.0 (io)
hyper-util 0.1.21 (io)
iana-time-zone 0.1.65 (io)
iana-time-zone-haiku 0.1.2 (io)
icu_calendar 2.1.1 (io)
icu_calendar_data 2.1.1 (io)
icu_capi 2.1.2 (io)
icu_casemap 2.1.1 (io)
icu_casemap_data 2.1.1 (io)
icu_collator 2.1.2 (io)
icu_collator_data 2.1.1 (io)
icu_collections 2.1.1 (io)
icu_datetime 2.1.1 (io)
icu_datetime_data 2.1.2 (io)
icu_decimal 2.1.1 (io)
icu_decimal_data 2.1.1 (io)
icu_experimental 0.4.0 (io)
icu_experimental_data 0.4.1 (io)
icu_list 2.1.1 (io)
icu_list_data 2.1.1 (io)
icu_locale 2.1.1 (io)
icu_locale_core 2.3.0 (io)
icu_locale_data 2.1.2 (io)
icu_normalizer 2.1.1 (io)
icu_normalizer_data 2.1.1 (io)
icu_pattern 0.4.2 (io)
icu_plurals 2.1.1 (io)
icu_plurals_data 2.1.1 (io)
icu_properties 2.1.2 (io)
icu_properties_data 2.1.2 (io)
icu_provider 2.3.1 (io)
icu_provider_adapters 2.1.1 (io)
icu_segmenter 2.1.2 (io)
icu_segmenter_data 2.1.1 (io)
icu_time 2.1.1 (io)
icu_time_data 2.1.1 (io)
id-arena 2.3.0 (io)
ident_case 1.0.1 (io)
idna 1.1.0 (io)
idna_adapter 1.2.1 (io)
image 0.25.10 (io)
image-webp 0.2.4 (io)
imagesize 0.15.0 (io)
imgref 1.12.3 (io)
indexmap 1.9.3 (io)
indexmap 2.14.2 (io)
inout 0.2.5 (io)
interpolate_name 0.2.4 (io)
inventory 0.3.25 (io)
ipc-channel 0.23.0 (io)
ipnet 2.12.2 (io)
is_terminal_polyfill 1.70.2 (io)
itertools 0.13.0 (io)
itertools 0.14.0 (io)
itertools 0.15.0 (io)
itoa 1.0.18 (io)
ixdtf 0.6.6 (io)
jiff 0.2.38 (io)
jiff-core 0.1.1 (io)
jiff-static 0.2.38 (io)
jiff-tzdb 0.1.9 (io)
jiff-tzdb-platform 0.1.3 (io)
jni-sys 0.3.1 (io)
jni-sys 0.4.1 (io)
jni-sys-macros 0.4.1 (io)
jobserver 0.1.35 (io)
js-sys 0.3.106 (io)
k12 0.5.1 (io)
keccak 0.2.2 (io)
kem 0.3.0 (io)
keyboard-types 0.8.3 (io)
khronos-egl 6.0.0 (io)
khronos_api 3.1.0 (io)
kurbo 0.13.1 (io)
lazy_static 1.5.1 (io)
leb128fmt 0.1.0 (io)
lebe 0.5.3 (io)
libc 0.2.191 (io)
libfuzzer-sys 0.4.13 (io)
libloading 0.8.9 (io)
libm 0.2.16 (io)
libz-sys 1.1.31 (io)
linebender_resource_handle 0.1.1 (io)
linux-raw-sys 0.12.1 (io)
litemap 0.8.3 (io)
litrs 1.0.1 (io)
lock_api 0.4.14 (io)
log 0.4.34 (io)
loop9 0.1.5 (io)
mach2 0.7.0 (io)
macro_rules_attribute 0.2.2 (io)
macro_rules_attribute-proc_macro 0.2.2 (io)
malloc_buf 0.0.6 (io)
malloc_size_of_derive 0.1.3 (io)
markup5ever 0.40.0 (io)
maybe-rayon 0.1.1 (io)
memchr 2.8.3 (io)
memmap2 0.9.12 (io)
mime 0.3.17 (io)
mime_guess 2.0.5 (io)
minimal-lexical 0.2.1 (io)
miniz_oxide 0.8.9 (io)
mio 1.2.4 (io)
ml-dsa 0.1.1 (io)
ml-kem 0.3.3 (io)
module-lattice 0.2.3 (io)
moxcms 0.8.1 (io)
mozangle 0.7.1 (io)
mozjs 0.26.13 (io)
mozjs_collator_glue 153.6.0 (io)
mozjs_icu_collator 153.6.0 (io)
mozjs_icu_collator_data 153.6.0 (io)
mozjs_icu_collections 153.6.0 (io)
mozjs_icu_normalizer 153.6.0 (io)
mozjs_icu_normalizer_data 153.6.0 (io)
mozjs_locale_glue 153.6.0 (io)
mozjs_normalizer_glue 153.6.0 (io)
mozjs_properties_glue 153.6.0 (io)
mozjs_sys 153.6.0-1 (io)
mozjs_unicode_bidi_ffi 153.6.0 (io)
mozjs_utf16_iter 153.6.0 (io)
mozjs_utf8_iter 153.6.0 (io)
multiversion 0.9.2 (io)
multiversion-macros 0.9.2 (io)
multiversion_no_op 1.0.0 (io)
naga 30.0.1 (io)
naga-types 30.0.1 (io)
ndk-sys 0.6.0+11769913 (io)
new_debug_unreachable 1.0.6 (io)
no_std_io2 0.9.4 (io)
nom 7.1.3 (io)
nom 8.0.0 (io)
nom-language 0.1.0 (io)
nom-rfc8288 0.4.0 (io)
noop_proc_macro 0.3.0 (io)
num-bigint 0.4.8 (io)
num-complex 0.4.6 (io)
num-conv 0.2.2 (io)
num-derive 0.4.2 (io)
num-integer 0.1.47 (io)
num-rational 0.4.2 (io)
num-traits 0.2.19 (io)
num_cpus 1.17.0 (io)
num_enum 0.7.6 (io)
num_enum_derive 0.7.6 (io)
num_threads 0.1.8 (io)
objc 0.2.7 (io)
objc2 0.6.5 (io)
objc2-app-kit 0.3.2 (io)
objc2-core-foundation 0.3.2 (io)
objc2-core-graphics 0.3.2 (io)
objc2-core-text 0.3.2 (io)
objc2-core-video 0.3.2 (io)
objc2-encode 4.1.0 (io)
objc2-foundation 0.3.2 (io)
objc2-io-surface 0.3.2 (io)
objc2-metal 0.3.2 (io)
objc2-quartz-core 0.3.2 (io)
object 0.37.3 (io)
ocb3 0.2.0 (io)
ohos-deviceinfo 0.1.0 (io)
ohos-deviceinfo-sys 0.1.8 (io)
once_cell 1.21.4 (io)
once_cell_polyfill 1.70.2 (io)
oorandom 11.1.5 (io)
ordered-channel 1.2.0 (io)
ordered-float 5.5.0 (io)
p256 0.14.0 (io)
p384 0.14.0 (io)
p521 0.14.0 (io)
page_size 0.6.0 (io)
parking_lot 0.12.5 (io)
parking_lot_core 0.9.12 (io)
password-hash 0.6.1 (io)
paste 1.0.15 (io)
pastey 0.1.1 (io)
pbkdf2 0.13.0 (io)
peek-poke 0.3.0 (io)
peek-poke-derive 0.3.0 (io)
pem-rfc7468 1.0.0 (io)
peniko 0.6.1 (io)
percent-encoding 2.3.2 (io)
petgraph 0.8.3 (io)
phc 0.6.1 (io)
phf 0.14.0 (io)
phf_codegen 0.14.0 (io)
phf_generator 0.14.0 (io)
phf_macros 0.14.0 (io)
phf_shared 0.14.0 (io)
pico-args 0.5.0 (io)
pin-project 1.1.13 (io)
pin-project-internal 1.1.13 (io)
pin-project-lite 0.2.17 (io)
pkcs1 0.8.0-rc.5 (io)
pkcs8 0.11.0 (io)
pkg-config 0.3.34 (io)
plane-split 0.18.0 (io)
plotters 0.3.7 (io)
plotters-backend 0.3.7 (io)
plotters-svg 0.3.7 (io)
png 0.18.1 (io)
poly1305 0.9.1 (io)
polycool 0.4.0 (io)
polyval 0.7.3 (io)
portable-atomic 1.15.0 (io)
portable-atomic-util 0.2.8 (io)
postcard 1.1.3 (io)
potential_utf 0.1.6 (io)
powerfmt 0.2.1 (io)
ppv-lite86 0.2.21 (io)
precomputed-hash 0.1.1 (io)
presser 0.3.1 (io)
prettyplease 0.2.37 (io)
primal-check 0.3.4 (io)
primefield 0.14.0 (io)
primeorder 0.14.0 (io)
proc-macro-crate 3.5.0 (io)
proc-macro2 1.0.107 (io)
profiling 1.0.18 (io)
profiling-procmacros 1.0.18 (io)
pulp 0.22.3 (io)
pulp-wasm-simd-flag 0.1.1 (io)
pxfm 0.1.30 (io)
quick-error 2.0.1 (io)
quickcheck 1.1.0 (io)
quote 1.0.47 (io)
r-efi 5.3.0 (io)
rand 0.10.3 (io)
rand 0.8.7 (io)
rand 0.9.5 (io)
rand_chacha 0.3.1 (io)
rand_chacha 0.9.0 (io)
rand_core 0.10.1 (io)
rand_core 0.6.4 (io)
rand_core 0.9.5 (io)
range-alloc 0.1.5 (io)
rav1e 0.8.1 (io)
ravif 0.13.0 (io)
raw-cpuid 11.6.0 (io)
raw-window-handle 0.6.2 (io)
raw-window-metal 1.1.0 (io)
rayon 1.12.0 (io)
rayon-core 1.13.0 (io)
read-fonts 0.41.0 (io)
read-fonts 0.43.3 (io)
realfft 3.5.0 (io)
reborrow 0.5.5 (io)
redox_syscall 0.5.18 (io)
ref-cast 1.0.27 (io)
ref-cast-impl 1.0.27 (io)
regex 1.13.1 (io)
regex-automata 0.4.18 (io)
regex-syntax 0.8.11 (io)
renderdoc-sys 1.1.0 (io)
resvg 0.48.1 (io)
rfc6979 0.6.0 (io)
rgb 0.8.53 (io)
ring 0.17.14 (io)
roxmltree 0.20.0 (io)
roxmltree 0.21.1 (io)
rsa 0.10.0-rc.19 (io)
rtsan-standalone 0.3.0 (io)
rtsan-standalone-macros 0.3.0 (io)
rtsan-standalone-sys 0.3.0 (io)
rustc-demangle 0.1.28 (io)
rustc-hash 1.1.0 (io)
rustc-hash 2.1.3 (io)
rustc_version 0.4.1 (io)
rustfft 6.4.1 (io)
rustix 1.1.5 (io)
rustls 0.23.46 (io)
rustls-pki-types 1.15.1 (io)
rustls-webpki 0.103.16 (io)
rustversion 1.0.23 (io)
ryu 1.0.23 (io)
same-file 1.0.6 (io)
schemars 0.9.0 (io)
schemars 1.2.3 (io)
scoped-tls 1.0.1 (io)
scopeguard 1.2.0 (io)
sec1 0.8.1 (io)
selectors 0.41.0 (stylo)
semver 1.0.28 (io)
serde 1.0.229 (io)
serde_bytes 0.11.19 (io)
serde_core 1.0.229 (io)
serde_derive 1.0.229 (io)
serde_json 1.0.154 (io)
serde_spanned 1.1.2 (io)
serde_test 1.0.177 (io)
serde_urlencoded 0.7.1 (io)
serde_with 3.24.0 (io)
serde_with_macros 3.24.0 (io)
serdect 0.4.3 (io)
servo-allocator 0.7.0 (ws)
servo-background-hang-monitor-api 0.7.0 (ws)
servo-base 0.7.0 (ws)
servo-bluetooth-traits 0.7.0 (ws)
servo-canvas-traits 0.7.0 (ws)
servo-config 0.7.0 (ws)
servo-config-macro 0.7.0 (ws)
servo-constellation-traits 0.7.0 (ws)
servo-default-resources 0.7.0 (ws)
servo-deny-public-fields 0.7.0 (ws)
servo-devtools-traits 0.7.0 (ws)
servo-dom-struct 0.7.0 (ws)
servo-embedder-traits 0.7.0 (ws)
servo-fonts 0.7.0 (ws)
servo-fonts-traits 0.7.0 (ws)
servo-geometry 0.7.0 (ws)
servo-hyper-serde 0.7.0 (ws)
servo-jstraceable-derive 0.7.0 (ws)
servo-layout 0.7.0 (ws)
servo-layout-api 0.7.0 (ws)
servo-malloc-size-of 0.7.0 (ws)
servo-media 0.7.0 (ws)
servo-media-audio 0.7.0 (ws)
servo-media-derive 0.7.0 (ws)
servo-media-player 0.7.0 (ws)
servo-media-streams 0.7.0 (ws)
servo-media-thread 0.7.0 (ws)
servo-media-traits 0.7.0 (ws)
servo-media-webrtc 0.7.0 (ws)
servo-metrics 0.7.0 (ws)
servo-net-traits 0.7.0 (ws)
servo-paint-api 0.7.0 (ws)
servo-pixels 0.7.0 (ws)
servo-profile-traits 0.7.0 (ws)
servo-script 0.7.0 (ws)
servo-script-bindings 0.7.0 (ws)
servo-script-traits 0.7.0 (ws)
servo-script-webcrypto 0.7.0 (ws)
servo-script-webgpu 0.7.0 (ws)
servo-storage-traits 0.7.0 (ws)
servo-timers 0.7.0 (ws)
servo-tracing 0.7.0 (ws)
servo-url 0.7.0 (ws)
servo-webgpu-traits 0.7.0 (ws)
servo-webvtt 0.7.0 (ws)
servo-webxr-api 0.7.0 (ws)
servo-xpath 0.7.0 (ws)
servo_arc 0.5.0 (stylo)
sha1 0.10.6 (io)
sha1 0.11.0 (io)
sha1_smol 1.0.1 (io)
sha2 0.10.9 (io)
sha2 0.11.1 (io)
sha3 0.12.0 (io)
shake 0.1.0 (io)
shlex 1.3.0 (io)
shlex 2.0.1 (io)
signal-hook-registry 1.4.8 (io)
signature 3.0.0 (io)
simd-adler32 0.3.10 (io)
simd_helpers 0.1.0 (io)
simdutf8 0.1.5 (io)
simplecss 0.2.2 (io)
siphasher 1.0.4 (io)
skrifa 0.44.0 (io)
skrifa 0.46.2 (io)
slab 0.4.12 (io)
slotmap 1.1.1 (io)
smallbitvec 2.6.1 (io)
smallvec 1.16.3 (io)
socket2 0.6.5 (io)
speexdsp-resampler 0.1.0 (io)
spirv 0.4.0+sdk-1.4.341.0 (io)
spki 0.8.1 (io)
sponge-cursor 0.1.0 (io)
stable_deref_trait 1.2.1 (io)
static_assertions 1.1.0 (io)
strck 1.0.0 (io)
strength_reduce 0.2.4 (io)
strict-num 0.1.1 (io)
string_cache 0.11.0 (io)
string_cache_codegen 0.11.2 (io)
strsim 0.11.1 (io)
strum 0.28.0 (io)
strum_macros 0.28.0 (io)
stylo 0.22.0 (stylo)
stylo_atoms 0.22.0 (stylo)
stylo_derive 0.22.0 (stylo)
stylo_dom 0.22.0 (stylo)
stylo_malloc_size_of 0.22.0 (stylo)
stylo_static_prefs 0.22.0 (stylo)
stylo_traits 0.22.0 (stylo)
subtle 2.6.1 (io)
surfman 0.14.0 (io)
svg_fmt 0.4.5 (io)
svgtypes 0.16.1 (io)
swapper 0.1.0 (io)
syn 2.0.119 (io)
syn 3.0.4 (io)
synstructure 0.13.2 (io)
sys-locale 0.3.2 (io)
taffy 0.15.1 (io)
tar 0.4.46 (io)
tempfile 3.23.0 (io)
tendril 0.5.1 (io)
termcolor 1.4.1 (io)
thin-vec 0.2.21 (io)
thiserror 1.0.69 (io)
thiserror 2.0.21 (io)
thiserror-impl 1.0.69 (io)
thiserror-impl 2.0.21 (io)
thread_local 1.1.10 (io)
tiff 0.11.3 (io)
tikv-jemalloc-sys 0.6.1+5.3.0-1-ge13ca993e8ccb9ba9847cc330696e02839f328f7 (io)
tikv-jemallocator 0.6.1 (io)
time 0.3.55 (io)
time-core 0.1.9 (io)
time-macros 0.2.32 (io)
tiny-skia 0.12.0 (io)
tiny-skia-path 0.12.0 (io)
tinystr 0.8.4 (io)
tinytemplate 1.2.1 (io)
tinyvec 1.13.3 (io)
to_shmem 0.6.0 (stylo)
to_shmem_derive 0.2.0 (stylo)
tokio 1.54.1 (io)
tokio-macros 2.7.2 (io)
tokio-util 0.7.20 (io)
toml 0.9.12+spec-1.1.0 (io)
toml 1.1.8+spec-1.1.0 (io)
toml_datetime 0.7.5+spec-1.1.0 (io)
toml_datetime 1.1.2+spec-1.1.0 (io)
toml_edit 0.25.17+spec-1.1.0 (io)
toml_parser 1.1.5+spec-1.1.0 (io)
toml_writer 1.1.3+spec-1.1.0 (io)
topological-sort 0.1.0 (io)
tower-service 0.3.3 (io)
tracing 0.1.44 (io)
tracing-attributes 0.1.31 (io)
tracing-core 0.1.36 (io)
tracy-rs 0.1.2 (io)
trait-set 0.5.0 (io)
transpose 0.2.3 (io)
try-lock 0.2.5 (io)
typenum 1.20.1 (io)
uluru 3.1.0 (io)
unic-char-property 0.9.0 (io)
unic-char-range 0.9.0 (io)
unic-common 0.9.0 (io)
unic-ucd-ident 0.9.0 (io)
unic-ucd-version 0.9.0 (io)
unicase 2.10.0 (io)
unicode-bidi 0.3.18 (io)
unicode-ident 1.0.27 (io)
unicode-properties 0.1.4 (io)
unicode-script 0.5.8 (io)
unicode-segmentation 1.13.3 (io)
unicode-vo 0.1.0 (io)
unicode-width 0.2.2 (io)
unicode-xid 0.2.6 (io)
unicode_categories 0.1.1 (io)
universal-hash 0.6.1 (io)
untrusted 0.9.0 (io)
url 2.5.8 (io)
urlpattern 0.3.0 (io)
usvg 0.48.1 (io)
utf16_iter 1.0.5 (io)
utf8_iter 1.0.4 (io)
utf8parse 0.2.2 (io)
uuid 1.26.1 (io)
v_frame 0.3.9 (io)
valuable 0.1.1 (io)
vcpkg 0.2.15 (io)
vello_common 0.2.0 (io)
vello_cpu 0.2.0 (io)
version_check 0.9.5 (io)
void 1.0.2 (io)
walkdir 2.5.0 (io)
want 0.3.2 (io)
warp 0.4.3 (io)
wasi 0.11.1+wasi-snapshot-preview1 (io)
wasip2 1.0.2+wasi-0.2.9 (io)
wasip3 0.4.0+wasi-0.3.0-rc-2026-01-06 (io)
wasm-bindgen 0.2.129 (io)
wasm-bindgen-macro 0.2.129 (io)
wasm-bindgen-macro-support 0.2.129 (io)
wasm-bindgen-shared 0.2.129 (io)
wasm-encoder 0.244.0 (io)
wasm-metadata 0.244.0 (io)
wasmparser 0.244.0 (io)
wayland-sys 0.31.11 (io)
web-sys 0.3.106 (io)
web_atoms 0.3.0 (io)
webdriver 0.54.0 (io)
webrender 0.70.1 (io)
webrender_api 0.70.0 (io)
webrender_build 0.70.0 (io)
weezl 0.1.12 (io)
wgpu-core 30.0.1 (io)
wgpu-core-deps-apple 30.0.1 (io)
wgpu-core-deps-emscripten 30.0.1 (io)
wgpu-core-deps-windows-linux-android 30.0.1 (io)
wgpu-hal 30.0.1 (io)
wgpu-naga-bridge 30.0.1 (io)
wgpu-types 30.0.1 (io)
winapi 0.3.9 (io)
winapi-i686-pc-windows-gnu 0.4.0 (io)
winapi-util 0.1.11 (io)
winapi-x86_64-pc-windows-gnu 0.4.0 (io)
windows 0.62.2 (io)
windows-collections 0.3.2 (io)
windows-core 0.62.2 (io)
windows-future 0.3.2 (io)
windows-implement 0.60.2 (io)
windows-interface 0.59.3 (io)
windows-link 0.2.1 (io)
windows-numerics 0.3.1 (io)
windows-result 0.4.1 (io)
windows-strings 0.5.1 (io)
windows-sys 0.52.0 (io)
windows-sys 0.61.2 (io)
windows-targets 0.52.6 (io)
windows-threading 0.2.1 (io)
windows_aarch64_gnullvm 0.52.6 (io)
windows_aarch64_msvc 0.52.6 (io)
windows_i686_gnu 0.52.6 (io)
windows_i686_gnullvm 0.52.6 (io)
windows_i686_msvc 0.52.6 (io)
windows_x86_64_gnu 0.52.6 (io)
windows_x86_64_gnullvm 0.52.6 (io)
windows_x86_64_msvc 0.52.6 (io)
winnow 0.7.15 (io)
winnow 1.0.4 (io)
wio 0.2.2 (io)
wit-bindgen 0.51.0 (io)
wit-bindgen-core 0.51.0 (io)
wit-bindgen-rust 0.51.0 (io)
wit-bindgen-rust-macro 0.51.0 (io)
wit-component 0.244.0 (io)
wit-parser 0.244.0 (io)
wnaf 0.14.1 (io)
wr_glyph_rasterizer 0.70.1 (io)
wr_malloc_size_of 0.2.2 (io)
write16 1.0.0 (io)
writeable 0.6.4 (io)
wuff 0.2.9 (io)
wuff-capi 0.2.0 (io)
x-wing 0.1.1 (io)
x11-dl 2.21.0 (io)
x25519-dalek 3.0.0 (io)
x448 0.14.0-pre.12 (io)
xattr 1.6.1 (io)
xml 1.4.0 (io)
xml-rs 0.8.29 (io)
xml5ever 0.40.0 (io)
xmlwriter 0.1.0 (io)
y4m 0.8.0 (io)
yeslogic-fontconfig-sys 6.0.1 (io)
yoke 0.8.3 (io)
yoke-derive 0.8.2 (io)
zeitstempel 0.2.2 (io)
zerocopy 0.8.63 (io)
zerocopy-derive 0.8.63 (io)
zerofrom 0.1.8 (io)
zerofrom-derive 0.1.7 (io)
zeroize 1.9.1 (io)
zeroize_derive 1.5.0 (io)
zerotrie 0.2.5 (io)
zerovec 0.11.8 (io)
zerovec-derive 0.11.7 (io)
zmij 1.0.23 (io)
zune-core 0.5.3 (io)
zune-inflate 0.2.54 (io)
zune-jpeg 0.5.15 (io)
```
