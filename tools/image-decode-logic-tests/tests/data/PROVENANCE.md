<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# KAT fixture provenance (Lane 2b image decode)

Every fixture was fetched **raw** with `curl` from a pinned upstream commit, never re-encoded. The expected
RGBA SHA-256 is over row-major RGBA8: `width * height * 4` bytes, alpha 255 where the source has no alpha.

## PNG fixtures

Source: [image-rs/image-png](https://github.com/image-rs/image-png) at
`b2f10c6a7e3127d4aaf231f3eae731a33e6eb0bc` (2026-09-23), paths under `tests/pngsuite/`. PngSuite is Willem van
Schaik's PNG test suite; the upstream `tests/pngsuite/PngSuite.LICENSE` is the applicable licence.

| Fixture | Upstream path | File SHA-256 | Size | Native type | Expected RGBA SHA-256 |
|---|---|---|---|---|---|
| `png/basn0g08.png` | `tests/pngsuite/basn0g08.png` | `268d061075d1dd2eeec62b31303d09f6998549e1bfb447a5f09c80a2b0978ac3` | 32x32 | grayscale 8 | `982faa277e83f73ca15b491e67eb41fa25526418ed23e057a9986c4f620eb158` |
| `png/basn2c08.png` | `tests/pngsuite/basn2c08.png` | `c90e86090a625661b19960cafdde6e347d6e32d73837aaae533f66dd3f099506` | 32x32 | RGB 8 | `23a53c674ec50d5a5eb9c3f679b6b19ba5304ae99dff76801bec4939e0f0c99e` |
| `png/basn2c16.png` | `tests/pngsuite/basn2c16.png` | `1e1c28ec0ee5da6224404b4005076dd30e2335a1a80c0ca722046a245c4a2b3b` | 32x32 | RGB 16 | `7c4b73e829f02793549b4480e25f0c0b332abcb24ac059dbad855fd1d726c17a` |
| `png/basn3p08.png` | `tests/pngsuite/basn3p08.png` | `d58256cd2eb16b5740d4c1403d25ce43d8dd03e270627ab709d2fb141e3d904c` | 32x32 | palette 8 | `b1c3302eceae6738c36edafa98c8054824d9440f3ba53a3f17cc81d29acc32cc` |
| `png/basn4a08.png` | `tests/pngsuite/basn4a08.png` | `7be24b618bfd437b921ae0caf9341d112338c6290cc62466deb19d2c7a512968` | 32x32 | gray+alpha 8 | `76b94a71d3c183a362c2cf6a46ebb50adc9d3a25a89bc0afc46fda6dbb002509` |
| `png/basn6a08.png` | `tests/pngsuite/basn6a08.png` | `559c594166eb156f461c9beff0f053196730dc998fdb0d2b801c89e6680860a5` | 32x32 | RGBA 8 | `2eb6a2cb3166e9c188add371157e9f81caa18fdf34d218844ed930b53b7431d2` |

The expected hashes were produced by `tests/data/reference_png_rgba.py`, a decoder written independently of the
vendored `png` crate: it uses only Python's `zlib` and the PNG spec's per-row filters. Its output matched the
vendored decoder byte for byte on all six fixtures (the 16-bit fixture is compared at its high byte, which is what
`Transformations::normalize_to_color8` / `STRIP_16` keeps). Re-run it on the same files to reproduce every value:

```
python3 tools/image-decode-logic-tests/tests/data/reference_png_rgba.py \
  tools/image-decode-logic-tests/tests/data/png/*.png
```

## JPEG fixtures

Source: [etemesi254/zune-image](https://github.com/etemesi254/zune-image) at
`2f9086b07cdf5a36621ce18f79d310f99b751de9` (2026-10-24), paths under `test-images/jpeg/`. The repository's
`LICENSE-MIT`, `LICENSE-APACHE` and `LICENSE-ZLIB` cover them.

| Fixture | Upstream path | File SHA-256 | Size | Mode | Expected RGBA SHA-256 (vendored `zune-jpeg`, RGBA output) |
|---|---|---|---|---|---|
| `jpeg/tiny_non_interleaved_444.jpg` | `test-images/jpeg/tiny_non_interleaved_444.jpg` | `f30a5ed588fc14e1f1922c46fe07614abbc10460693a93f376d7ffcee099f55e` | 16x16 | baseline 4:4:4 | `59bfa308579730d52c4ec6c9366102ecaf570e6a7cd03aba307d568bd1bea6db` |
| `jpeg/non_interleaved_422_64x64.jpg` | `test-images/jpeg/non_interleaved_422_64x64.jpg` | `1e056357d27f5b0fcf29a3b0b644298a32575b7b58aebe7012b3944604c5a7d5` | 64x64 | baseline 4:2:2 | `4500aa1466440ae556a15710b241a21ba43e89b49f198aadac4c6744b602ef8f` |
| `jpeg/progressive_restart_420.jpg` | `test-images/jpeg/progressive_restart_420.jpg` | `f5ae8100566d0c52f8902fe538a4893363efcc990d999a909832479bc7e948b0` | 256x256 | progressive 4:2:0 | `df194d1583400202406fd9894bd39349e753c818ea8ebf0d00f967ed757b6c00` |

**What the JPEG hash pins, and what it does not.** These values are the vendored `zune-jpeg` output; they are a
regression pin for the vendored copy, not a cross-decoder equality claim. JPEG decoding is not bit-exact across
independent IDCT/upsampling implementations:

- The 16x16 4:4:4 fixture's hash was independently reproduced exactly with the `jpeg-decoder` 0.3 crate
  (RGB output plus alpha 255), so that one has a second implementation behind it.
- For the 4:2:2 and progressive 4:2:0 fixtures, `jpeg-decoder` 0.3 produced different hashes
  (`3c99b902…` and `8305dcfb…` respectively) at the same dimensions: chroma upsampling and IDCT rounding differ
  by a few LSBs. This is the same limit the vendored crate's own parity test records against libjpeg-turbo
  (`crates/zune-jpeg/tests/arithmetic_progressive.rs`, max channel delta 2). The KAT therefore checks the
  dimensions and the exact vendored output, while `zune-jpeg`'s own upstream tests remain the cross-decoder
  check.

The `sha2` crate in the KAT crate is a host-only test dependency (MIT OR Apache-2.0); it is not vendored.
