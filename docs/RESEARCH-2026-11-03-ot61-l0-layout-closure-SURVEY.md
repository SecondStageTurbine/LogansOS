<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# OT-61 L0: the layout closure's new crates, a licence and freshness survey (2026-11-03)

The prior-art survey rule (CLAUDE.md, 2026-10-27) asks, for every vendored crate, its licence and how alive it is. L0 (#55,
`userland/layout-smoke`; THINK `docs/THINK-2026-11-01-ot61-first-page-layout.md`) vendors **195 crates** into
`third_party/ot61-browser/vendor` that the tree didn't have: the crates.io crates Servo's layout compiles for
`x86_64-unknown-neoplutarch` beyond Stylo's closure (C2.4 survey, `RESEARCH-2026-10-30-ot61-c2.4-stylo-closure-SURVEY.md`)
and the SWGL/WebRender trees (#66, #25). This is the same table the C2.4 survey §4 gives for Stylo's.

## How the columns were read

- **Licence:** each crate's own `Cargo.toml` (`license`), as vendored.
- **Released:** the version's `created_at` from the crates.io API (`/api/v1/crates/<name>/<version>`, raw JSON), read 2026-11-03.
- **Repository last commit:** the head commit of the repository's default branch, fetched on 2026-11-03 with
  `git fetch --depth=1 --filter=tree:0 <repo> HEAD` (one commit object, no trees or blobs), with its committer date.
  For a monorepo (Servo's, image-rs', linebender's) that date is the monorepo's, not the crate's directory's.
- **Not checked:** whether a GitHub repository is archived. This session's GitHub API reaches only LogansOS, so the
  last-commit date is the liveness signal. Two repositories aren't on GitHub and have no date here (below).

## Findings

- **Licences:** all on CLAUDE.md's vendoring list, alone or as a choice: MIT, Apache-2.0, MPL-2.0, BSD-3-Clause,
  BSD-2-Clause, ISC, Zlib, 0BSD, Unlicense-or-MIT, Unicode-3.0. No GPL, LGPL or AGPL, and none uses only a licence file.
- **Freshness:** 178 of the 195 have a repository commit within the last two years. **15** don't (no
  commit since before 2024-11-03). Each is small and finished, or a data table. Two sit under TLS: `subtle`
  (constant-time primitives, under rustls) and `untrusted` (input parsing, under rustls-webpki). Both are widely
  used and audited crates whose consumers (rustls 0.23.46, rustls-webpki 0.103.16) are current, and neither has an
  open RustSec advisory to our knowledge. That is not a fresh audit, and the AAG's memory-safety lens should see
  them. None of the 15 runs on a static first page except `unicode_categories` (servo-layout's own dependency) and
  the image/SVG helpers when a page has such content:

| Crate | Version | Released | Repository | Last commit | What it is | Used by |
|---|---|---|---|---|---|---|
| `arrayref` | 0.3.9 | 2024-09-14 | github.com/droundy/arrayref | `f8d0299` 2024-09-14 | macros over array slices | tiny-skia, tiny-skia-path |
| `byteorder-lite` | 0.1.0 | 2024-04-07 | github.com/image-rs/byteorder-lite | `7d44da0` 2024-04-07 | image-rs's cut-down byteorder | image, image-webp |
| `enumn` | 0.1.14 | 2024-07-30 | github.com/dtolnay/enumn | `7fd813a` 2024-07-29 | a small derive (dtolnay) | accesskit |
| `pico-args` | 0.5.0 | 2022-06-04 | github.com/RazrFalcon/pico-args | `ee9e334` 2023-10-19 | a command-line argument parser | resvg, usvg |
| `primal-check` | 0.3.4 | 2024-06-07 | github.com/huonw/primal | `140650b` 2024-09-23 | a primality check | rustfft |
| `quick-error` | 2.0.1 | 2021-05-07 | github.com/tailhook/quick-error | `936a068` 2023-01-04 | an error-type macro | image-webp |
| `sha1_smol` | 1.0.1 | 2024-07-21 | github.com/mitsuhiko/sha1-smol | `f9d88bd` 2024-07-21 | SHA-1 (uuid v5 names), no unsafe | uuid |
| `strict-num` | 0.1.1 | 2023-05-27 | github.com/RazrFalcon/strict-num | `76d36fe` 2024-04-25 | checked float wrappers | tiny-skia-path, usvg |
| `subtle` | 2.6.1 | 2024-06-24 | github.com/dalek-cryptography/subtle | `369e746` 2024-06-19 | constant-time primitives (dalek-cryptography) | rustls |
| `transpose` | 0.2.3 | 2024-02-20 | github.com/ejmahler/transpose | `134293e` 2024-02-19 | matrix transpose | rustfft |
| `unicode-vo` | 0.1.0 | 2019-06-11 | github.com/RazrFalcon/unicode-vo | `f163f6c` 2019-05-20 | the Unicode vertical-orientation table (UAX #50), data only | usvg |
| `unicode_categories` | 0.1.1 | 2016-10-22 | github.com/swgillespie/unicode-categories | `ef659e9` 2021-11-05 | a Unicode general-category table, data only | servo-layout |
| `untrusted` | 0.9.0 | 2021-07-13 | github.com/briansmith/untrusted | `b7c4aea` 2021-07-13 | safe parsing of untrusted input (briansmith) | rustls-webpki |
| `version_check` | 0.9.5 | 2024-07-25 | github.com/SergioBenitez/version_check | `d77ef9f` 2024-07-25 | a rustc version probe | build scripts of cookie, generic-array, slotmap (host only) |
| `xmlwriter` | 0.1.0 | 2019-08-12 | github.com/RazrFalcon/xmlwriter | `a40b0aa` 2023-10-25 | an XML writer | usvg |

- **RustSec advisories: no known vulnerability affects any of the 195.**
  - **How it was checked:** a depth-1 clone of `rustsec/advisory-db` at `263a4d9` (2026-11-02). Each crate's
    advisories were matched against its exact vendored version, by each advisory's `patched` and `unaffected`
    ranges; withdrawn advisories were skipped.
  - **What it found:** five informational **"unmaintained"** notices, all for the `unic-*` crates:
    `unic-char-property` (RUSTSEC-2025-0081), `unic-char-range` (RUSTSEC-2025-0075), `unic-common`
    (RUSTSEC-2025-0080), `unic-ucd-ident` (RUSTSEC-2025-0100) and `unic-ucd-version` (RUSTSEC-2025-0098).
    - They come in through `urlpattern` 0.3.0, a dependency of `servo-malloc-size-of`, and are Unicode
      identifier tables and their helpers, from 2019 releases.
    - Their repository's head commit is from 2026-06-22, which is exactly why a last-commit date alone isn't the
      liveness signal: RustSec still calls the crates unmaintained.
    - They hold no open vulnerability. Replacing them is Servo's/`urlpattern`'s move (RustSec suggests
      `unicode-ident` or ICU4X); we take it when we re-vendor a release that has it.
  - **`arrayref` 0.3.9: never update this crate to 0.3.10.** RUSTSEC-2026-0260 is about 0.3.10, which was
    published with a malicious dependency and removed from crates.io; 0.3.9 and earlier are unaffected.
  - Every other advisory filed against these crates' names is for versions older than ours.
- **Not on GitHub:** `ordered-channel` 1.2.0 (gitlab.com/kornelski, used by vello's CPU renderer) and `webdriver` 0.54.0
  (in mozilla-central, `testing/webdriver`; Servo's embedder traits name its types). Both are maintained in those
  homes; their release dates are in the table below.
- **Who maintains our copies:** the vendored copies are unmodified (each `.cargo-checksum.json` matches crates.io).
  The OT-61 browser lane (Seat D) owns them: a security fix upstream is taken by re-vendoring the fixed release
  (VENDORED.md, "Updating"); for a crate with no live upstream, a fix is made in `patched/` with its `.patch` beside it.

## All 195

| Crate | Version | Licence | Released | Repository | Last commit |
|---|---|---|---|---|---|
| `accesskit` | 0.24.1 | MIT OR Apache-2.0 | 2026-06-12 | github.com/AccessKit/accesskit | `f302622` 2026-10-25 |
| `adler2` | 2.0.1 | 0BSD OR MIT OR Apache-2.0 | 2025-06-09 | github.com/oyvindln/adler2 | `89a031a` 2025-06-10 |
| `aho-corasick` | 1.1.5 | Unlicense OR MIT | 2026-08-03 | github.com/BurntSushi/aho-corasick | `6c0abf5` 2026-08-10 |
| `alloc-no-stdlib` | 2.0.4 | BSD-3-Clause | 2022-09-12 | github.com/dropbox/rust-alloc-no-stdlib | `0a81fd6` 2026-06-14 |
| `arrayref` | 0.3.9 | BSD-2-Clause | 2024-09-14 | github.com/droundy/arrayref | macros over array slices | tiny-skia, tiny-skia-path |
| `base64` | 0.22.1 | MIT OR Apache-2.0 | 2024-04-30 | github.com/marshallpierce/rust-base64 | `5b98ee1` 2026-08-18 |
| `base64` | 0.23.1 | MIT OR Apache-2.0 | 2026-08-04 | github.com/marshallpierce/rust-base64 | `5b98ee1` 2026-08-18 |
| `block-buffer` | 0.10.4 | MIT OR Apache-2.0 | 2023-03-09 | github.com/RustCrypto/utils | `ddefdca` 2026-11-02 |
| `brotli-decompressor` | 5.0.3 | BSD-3-Clause/MIT | 2026-06-14 | github.com/dropbox/rust-brotli-decompressor | `61dbe68` 2026-10-31 |
| `byte-slice-cast` | 1.2.3 | MIT | 2025-03-03 | github.com/sdroege/bytes-num-slice-cast | `a7f61c2` 2025-03-03 |
| `byteorder-lite` | 0.1.0 | Unlicense OR MIT | 2024-04-07 | github.com/image-rs/byteorder-lite | image-rs's cut-down byteorder | image, image-webp |
| `bytes` | 1.12.1 | MIT | 2026-07-08 | github.com/tokio-rs/bytes | `6220790` 2026-10-31 |
| `cc` | 1.11.0 | MIT OR Apache-2.0 | 2026-11-02 | github.com/rust-lang/cc-rs | `995643d` 2026-11-02 |
| `chacha20` | 0.10.2 | MIT OR Apache-2.0 | 2026-08-27 | github.com/RustCrypto/stream-ciphers | `a2024dd` 2026-08-31 |
| `cobs` | 0.3.0 | MIT OR Apache-2.0 | 2025-01-31 | github.com/jamesmunns/cobs.rs | `03c874a` 2026-10-12 |
| `color` | 0.3.3 | Apache-2.0 OR MIT | 2026-05-05 | github.com/linebender/color | `943c533` 2026-10-21 |
| `color_quant` | 1.1.0 | MIT | 2020-10-09 | github.com/image-rs/color_quant.git | `0e2e48b` 2026-05-09 |
| `content-security-policy` | 0.9.0 | MIT OR Apache-2.0 | 2026-09-10 | github.com/rust-ammonia/rust-content-security-policy | `a75b255` 2026-09-10 |
| `cookie` | 0.16.2 | MIT OR Apache-2.0 | 2022-12-16 | github.com/SergioBenitez/cookie-rs | `8e97000` 2026-04-19 |
| `cookie` | 0.18.2 | MIT OR Apache-2.0 | 2026-08-08 | github.com/SergioBenitez/cookie-rs | `8e97000` 2026-04-19 |
| `cpufeatures` | 0.2.17 | MIT OR Apache-2.0 | 2025-01-25 | github.com/RustCrypto/utils | `ddefdca` 2026-11-02 |
| `cpufeatures` | 0.3.1 | MIT OR Apache-2.0 | 2026-08-26 | github.com/RustCrypto/utils | `ddefdca` 2026-11-02 |
| `crc32fast` | 1.5.4 | MIT OR Apache-2.0 | 2026-11-01 | github.com/srijs/rust-crc32fast | `118796a` 2026-11-01 |
| `crypto-common` | 0.1.7 | MIT OR Apache-2.0 | 2025-11-12 | github.com/RustCrypto/traits | `ec30ea6` 2026-11-02 |
| `darling` | 0.24.1 | MIT | 2026-08-20 | github.com/TedDriggs/darling | `4237d61` 2026-10-29 |
| `darling_core` | 0.24.1 | MIT | 2026-08-20 | github.com/TedDriggs/darling | `4237d61` 2026-10-29 |
| `darling_macro` | 0.24.1 | MIT | 2026-08-20 | github.com/TedDriggs/darling | `4237d61` 2026-10-29 |
| `data-url` | 0.3.2 | MIT OR Apache-2.0 | 2025-08-21 | github.com/servo/rust-url | `13b385b` 2026-10-30 |
| `deranged` | 0.5.9 | MIT OR Apache-2.0 | 2026-10-13 | github.com/jhpratt/deranged | `aeb03a4` 2026-10-13 |
| `digest` | 0.10.7 | MIT OR Apache-2.0 | 2023-05-19 | github.com/RustCrypto/traits | `ec30ea6` 2026-11-02 |
| `dpi` | 0.1.2 | Apache-2.0 AND MIT | 2025-05-02 | github.com/rust-windowing/winit | `e1ad70b` 2026-11-02 |
| `enumn` | 0.1.14 | MIT OR Apache-2.0 | 2024-07-30 | github.com/dtolnay/enumn | a small derive (dtolnay) | accesskit |
| `fdeflate` | 0.3.7 | MIT OR Apache-2.0 | 2024-12-06 | github.com/image-rs/fdeflate | `a713d02` 2026-05-31 |
| `fearless_simd` | 0.4.1 | Apache-2.0 OR MIT | 2026-05-16 | github.com/linebender/fearless_simd | `8b18b22` 2026-10-28 |
| `fixedbitset` | 0.5.7 | MIT OR Apache-2.0 | 2024-04-14 | github.com/petgraph/fixedbitset | `111fb25` 2026-07-04 |
| `flate2` | 1.1.10 | MIT OR Apache-2.0 | 2026-08-28 | github.com/rust-lang/flate2-rs | `d05c7c4` 2026-11-02 |
| `float-cmp` | 0.9.0 | MIT | 2021-07-21 | github.com/mikedilger/float-cmp | `35c6a34` 2026-05-03 |
| `foldhash` | 0.1.5 | Zlib | 2025-03-15 | github.com/orlp/foldhash | `77d8e3d` 2026-06-30 |
| `fontdb` | 0.24.0 | MIT | 2026-07-29 | github.com/RazrFalcon/fontdb | `ecb707e` 2026-07-29 |
| `fontsan` | 0.7.0 | BSD-3-Clause | 2026-07-15 | github.com/servo/fontsan | `04d3e6c` 2026-07-15 |
| `generic-array` | 0.14.7 | MIT | 2023-03-28 | github.com/fizyk20/generic-array.git | `6b6c76f` 2026-08-13 |
| `getrandom` | 0.2.17 | MIT OR Apache-2.0 | 2026-01-11 | github.com/rust-random/getrandom | `6a04900` 2026-10-30 |
| `gif` | 0.14.2 | MIT OR Apache-2.0 | 2026-04-09 | github.com/image-rs/image-gif | `a5f89dd` 2026-05-31 |
| `glifo` | 0.3.0 | Apache-2.0 OR MIT | 2026-08-07 | github.com/linebender/vello | `5016411` 2026-11-03 |
| `glob` | 0.3.4 | MIT OR Apache-2.0 | 2026-07-21 | github.com/rust-lang/glob | `057cef2` 2026-10-08 |
| `glow` | 0.17.0 | MIT OR Apache-2.0 OR Zlib | 2026-03-07 | github.com/grovesNL/glow | `0e9ad33` 2026-10-22 |
| `guillotiere` | 0.7.0 | MIT/Apache-2.0 | 2026-03-18 | github.com/nical/guillotiere | `9c8933a` 2026-03-18 |
| `harfbuzz-sys` | 0.8.0 | MIT OR Apache-2.0 | 2026-08-04 | github.com/servo/rust-harfbuzz | `0d3ac4d` 2026-07-29 |
| `harfrust` | 0.12.0 | MIT | 2026-07-03 | github.com/harfbuzz/harfrust | `bdd55a5` 2026-11-01 |
| `hashbrown` | 0.15.5 | MIT OR Apache-2.0 | 2025-08-07 | github.com/rust-lang/hashbrown | `f7bae3d` 2026-10-31 |
| `headers` | 0.4.2 | MIT | 2026-09-24 | github.com/hyperium/headers | `6a091bf` 2026-10-01 |
| `headers-core` | 0.3.0 | MIT | 2023-11-24 | github.com/hyperium/headers | `6a091bf` 2026-10-01 |
| `http` | 1.5.0 | MIT OR Apache-2.0 | 2026-07-29 | github.com/hyperium/http | `3dd9f5e` 2026-10-12 |
| `http-body` | 1.1.0 | MIT | 2026-07-13 | github.com/hyperium/http-body | `c6ca645` 2026-11-02 |
| `httpdate` | 1.0.3 | MIT OR Apache-2.0 | 2023-08-13 | github.com/pyfisch/httpdate | `53df520` 2024-12-22 |
| `hyper` | 1.12.0 | MIT | 2026-10-06 | github.com/hyperium/hyper | `cf563e8` 2026-11-02 |
| `image` | 0.25.10 | MIT OR Apache-2.0 | 2026-03-10 | github.com/image-rs/image | `075e65d` 2026-09-15 |
| `image-webp` | 0.2.4 | MIT OR Apache-2.0 | 2025-08-27 | github.com/image-rs/image-webp | `f4d80bd` 2026-04-07 |
| `imagesize` | 0.15.0 | MIT | 2026-07-09 | github.com/Roughsketch/imagesize | `9d0a7de` 2026-07-09 |
| `inventory` | 0.3.25 | MIT OR Apache-2.0 | 2026-10-07 | github.com/dtolnay/inventory | `e26963a` 2026-10-25 |
| `ipc-channel` | 0.23.1 | MIT OR Apache-2.0 | 2026-10-24 | github.com/servo/ipc-channel | `f121405` 2026-11-01 |
| `itertools` | 0.15.0 | MIT OR Apache-2.0 | 2026-06-16 | github.com/rust-itertools/itertools | `a409cdd` 2026-10-30 |
| `keyboard-types` | 0.8.3 | MIT OR Apache-2.0 | 2025-10-02 | github.com/rust-windowing/keyboard-types | `b2c511a` 2025-12-16 |
| `kurbo` | 0.13.1 | Apache-2.0 OR MIT | 2026-05-13 | github.com/linebender/kurbo | `a5f97ff` 2026-10-30 |
| `libz-sys` | 1.1.31 | MIT OR Apache-2.0 | 2026-10-28 | github.com/rust-lang/libz-sys | `595fbb7` 2026-10-28 |
| `linebender_resource_handle` | 0.1.1 | Apache-2.0 OR MIT | 2025-09-16 | github.com/linebender/raw_resource_handle | `28076cd` 2026-03-31 |
| `memmap2` | 0.9.12 | MIT OR Apache-2.0 | 2026-10-21 | github.com/RazrFalcon/memmap2-rs | `4692b05` 2026-10-21 |
| `miniz_oxide` | 0.8.9 | MIT OR Zlib OR Apache-2.0 | 2025-06-09 | github.com/Frommi/miniz_oxide/tree/master/miniz_oxide | `b28c237` 2026-11-02 |
| `miniz_oxide` | 0.9.1 | MIT OR Zlib OR Apache-2.0 | 2026-03-13 | github.com/Frommi/miniz_oxide/tree/master/miniz_oxide | `b28c237` 2026-11-02 |
| `moxcms` | 0.8.1 | BSD-3-Clause OR Apache-2.0 | 2026-03-07 | github.com/awxkee/moxcms.git | `097dccf` 2026-10-28 |
| `num-complex` | 0.4.6 | MIT OR Apache-2.0 | 2024-05-07 | github.com/rust-num/num-complex | `fb5dca6` 2026-07-07 |
| `num-conv` | 0.2.2 | MIT OR Apache-2.0 | 2026-05-18 | github.com/jhpratt/num-conv | `67f41e3` 2026-08-10 |
| `num_enum` | 0.7.6 | BSD-3-Clause OR MIT OR Apache-2.0 | 2026-03-15 | github.com/illicitonion/num_enum | `77b8441` 2026-04-19 |
| `num_enum_derive` | 0.7.6 | BSD-3-Clause OR MIT OR Apache-2.0 | 2026-03-15 | github.com/illicitonion/num_enum | `77b8441` 2026-04-19 |
| `ohos-deviceinfo` | 0.1.0 | Apache-2.0 | 2025-03-06 | github.com/openharmony-rs/ohos-deviceinfo | `d0f87bc` 2025-03-06 |
| `ohos-deviceinfo-sys` | 0.1.8 | Apache-2.0 | 2026-09-30 | github.com/openharmony-rs/ohos-sys | `dc537cc` 2026-10-19 |
| `ordered-channel` | 1.2.0 | MIT OR Apache-2.0 | 2025-03-09 | gitlab.com/kornelski/ordered-channel | `not` on |
| `peniko` | 0.6.1 | Apache-2.0 OR MIT | 2026-05-15 | github.com/linebender/peniko | `a8ad89c` 2026-08-27 |
| `petgraph` | 0.8.3 | MIT OR Apache-2.0 | 2025-09-30 | github.com/petgraph/petgraph | `6b4ee17` 2026-10-31 |
| `pico-args` | 0.5.0 | MIT | 2022-06-04 | github.com/RazrFalcon/pico-args | a command-line argument parser | resvg, usvg |
| `pin-project-lite` | 0.2.17 | Apache-2.0 OR MIT | 2026-02-27 | github.com/taiki-e/pin-project-lite | `a5d904d` 2026-11-02 |
| `pkg-config` | 0.3.34 | MIT OR Apache-2.0 | 2026-08-14 | github.com/rust-lang/pkg-config-rs | `a8852cc` 2026-08-14 |
| `png` | 0.18.1 | MIT OR Apache-2.0 | 2026-02-14 | github.com/image-rs/image-png | `b2f10c6` 2026-09-23 |
| `polycool` | 0.4.0 | MIT OR Apache-2.0 | 2025-10-23 | github.com/linebender/kurbo | `a5f97ff` 2026-10-30 |
| `postcard` | 1.1.3 | MIT OR Apache-2.0 | 2025-07-24 | github.com/jamesmunns/postcard | `ea583e4` 2026-10-21 |
| `powerfmt` | 0.2.1 | MIT OR Apache-2.0 | 2026-10-05 | github.com/jhpratt/powerfmt | `3dd4516` 2026-10-05 |
| `primal-check` | 0.3.4 | MIT OR Apache-2.0 | 2024-06-07 | github.com/huonw/primal | a primality check | rustfft |
| `proc-macro-crate` | 3.5.0 | MIT OR Apache-2.0 | 2026-03-04 | github.com/bkchr/proc-macro-crate | `0857037` 2026-03-05 |
| `pxfm` | 0.1.30 | BSD-3-Clause OR Apache-2.0 | 2026-07-06 | github.com/awxkee/pxfm | `acf7acd` 2026-07-03 |
| `quick-error` | 2.0.1 | MIT/Apache-2.0 | 2021-05-07 | github.com/tailhook/quick-error | an error-type macro | image-webp |
| `rand` | 0.10.3 | MIT OR Apache-2.0 | 2026-09-20 | github.com/rust-random/rand | `d1cbc13` 2026-11-02 |
| `rand_core` | 0.10.1 | MIT OR Apache-2.0 | 2026-04-13 | github.com/rust-random/rand_core | `fdea405` 2026-07-02 |
| `raw-window-handle` | 0.6.2 | MIT OR Apache-2.0 OR Zlib | 2024-05-17 | github.com/rust-windowing/raw-window-handle | `1b85948` 2026-04-05 |
| `read-fonts` | 0.41.0 | MIT OR Apache-2.0 | 2026-07-03 | github.com/googlefonts/fontations | `a49bbb5` 2026-11-02 |
| `read-fonts` | 0.43.3 | MIT OR Apache-2.0 | 2026-08-25 | github.com/googlefonts/fontations | `a49bbb5` 2026-11-02 |
| `realfft` | 3.5.1 | MIT | 2026-10-26 | github.com/HEnquist/realfft | `44f36c4` 2026-10-26 |
| `regex` | 1.13.1 | MIT OR Apache-2.0 | 2026-07-15 | github.com/rust-lang/regex | `94ac45b` 2026-10-28 |
| `regex-automata` | 0.4.18 | MIT OR Apache-2.0 | 2026-08-04 | github.com/rust-lang/regex | `94ac45b` 2026-10-28 |
| `regex-syntax` | 0.8.11 | MIT OR Apache-2.0 | 2026-06-09 | github.com/rust-lang/regex | `94ac45b` 2026-10-28 |
| `resvg` | 0.48.1 | Apache-2.0 OR MIT | 2026-08-02 | github.com/linebender/resvg | `cd64e51` 2026-10-31 |
| `rgb` | 0.8.53 | MIT | 2026-02-25 | github.com/kornelski/rust-rgb | `7915bd4` 2026-02-15 |
| `roxmltree` | 0.21.2 | MIT OR Apache-2.0 | 2026-10-24 | github.com/RazrFalcon/roxmltree | `37931a2` 2026-10-31 |
| `rustfft` | 6.4.1 | MIT OR Apache-2.0 | 2025-09-18 | github.com/ejmahler/RustFFT | `26a8ae7` 2026-10-28 |
| `rustls` | 0.23.46 | Apache-2.0 OR ISC OR MIT | 2026-10-14 | github.com/rustls/rustls | `f10706b` 2026-11-02 |
| `rustls-pki-types` | 1.15.1 | MIT OR Apache-2.0 | 2026-07-23 | github.com/rustls/pki-types | `d6981e7` 2026-11-02 |
| `rustls-webpki` | 0.103.16 | ISC | 2026-10-20 | github.com/rustls/webpki | `4657021` 2026-10-20 |
| `serde_json` | 1.0.154 | MIT OR Apache-2.0 | 2026-10-11 | github.com/serde-rs/json | `8c535a2` 2026-10-16 |
| `serde_with` | 3.24.0 | MIT OR Apache-2.0 | 2026-09-26 | github.com/jonasbb/serde_with/ | `24996b1` 2026-10-31 |
| `serde_with_macros` | 3.24.0 | MIT OR Apache-2.0 | 2026-09-26 | github.com/jonasbb/serde_with/ | `24996b1` 2026-10-31 |
| `servo-background-hang-monitor-api` | 0.7.0 | MPL-2.0 | 2026-10-05 | github.com/servo/servo | `3d75f73` 2026-11-03 |
| `servo-canvas-traits` | 0.7.0 | MPL-2.0 | 2026-10-05 | github.com/servo/servo | `3d75f73` 2026-11-03 |
| `servo-config` | 0.7.0 | MPL-2.0 | 2026-10-05 | github.com/servo/servo | `3d75f73` 2026-11-03 |
| `servo-config-macro` | 0.7.0 | MPL-2.0 | 2026-10-05 | github.com/servo/servo | `3d75f73` 2026-11-03 |
| `servo-constellation-traits` | 0.7.0 | MPL-2.0 | 2026-10-05 | github.com/servo/servo | `3d75f73` 2026-11-03 |
| `servo-devtools-traits` | 0.7.0 | MPL-2.0 | 2026-10-05 | github.com/servo/servo | `3d75f73` 2026-11-03 |
| `servo-geometry` | 0.7.0 | MPL-2.0 | 2026-10-05 | github.com/servo/servo | `3d75f73` 2026-11-03 |
| `servo-hyper-serde` | 0.7.0 | MIT OR Apache-2.0 | 2026-10-05 | github.com/servo/servo | `3d75f73` 2026-11-03 |
| `servo-layout-api` | 0.7.0 | MPL-2.0 | 2026-10-05 | github.com/servo/servo | `3d75f73` 2026-11-03 |
| `servo-malloc-size-of` | 0.7.0 | MIT OR Apache-2.0 | 2026-10-05 | github.com/servo/servo | `3d75f73` 2026-11-03 |
| `servo-media` | 0.7.0 | MPL-2.0 | 2026-10-05 | github.com/servo/servo | `3d75f73` 2026-11-03 |
| `servo-media-audio` | 0.7.0 | MPL-2.0 | 2026-10-05 | github.com/servo/servo | `3d75f73` 2026-11-03 |
| `servo-media-derive` | 0.7.0 | MPL-2.0 | 2026-10-05 | github.com/servo/servo | `3d75f73` 2026-11-03 |
| `servo-media-player` | 0.7.0 | MPL-2.0 | 2026-10-05 | github.com/servo/servo | `3d75f73` 2026-11-03 |
| `servo-media-streams` | 0.7.0 | MPL-2.0 | 2026-10-05 | github.com/servo/servo | `3d75f73` 2026-11-03 |
| `servo-media-thread` | 0.7.0 | MPL-2.0 | 2026-10-05 | github.com/servo/servo | `3d75f73` 2026-11-03 |
| `servo-media-traits` | 0.7.0 | MPL-2.0 | 2026-10-05 | github.com/servo/servo | `3d75f73` 2026-11-03 |
| `servo-media-webrtc` | 0.7.0 | MPL-2.0 | 2026-10-05 | github.com/servo/servo | `3d75f73` 2026-11-03 |
| `servo-net-traits` | 0.7.0 | MPL-2.0 | 2026-10-05 | github.com/servo/servo | `3d75f73` 2026-11-03 |
| `servo-pixels` | 0.7.0 | MPL-2.0 | 2026-10-05 | github.com/servo/servo | `3d75f73` 2026-11-03 |
| `servo-profile-traits` | 0.7.0 | MPL-2.0 | 2026-10-05 | github.com/servo/servo | `3d75f73` 2026-11-03 |
| `servo-script-traits` | 0.7.0 | MPL-2.0 | 2026-10-05 | github.com/servo/servo | `3d75f73` 2026-11-03 |
| `servo-storage-traits` | 0.7.0 | MPL-2.0 | 2026-10-05 | github.com/servo/servo | `3d75f73` 2026-11-03 |
| `servo-tracing` | 0.7.0 | MPL-2.0 | 2026-10-05 | github.com/servo/servo | `3d75f73` 2026-11-03 |
| `servo-webxr-api` | 0.7.0 | MPL-2.0 | 2026-10-05 | github.com/servo/servo | `3d75f73` 2026-11-03 |
| `sha1` | 0.10.7 | MIT OR Apache-2.0 | 2026-07-10 | github.com/RustCrypto/hashes | `23d8e40` 2026-10-27 |
| `sha1_smol` | 1.0.1 | BSD-3-Clause | 2024-07-21 | github.com/mitsuhiko/sha1-smol | SHA-1 (uuid v5 names), no unsafe | uuid |
| `sha2` | 0.10.9 | MIT OR Apache-2.0 | 2025-04-30 | github.com/RustCrypto/hashes | `23d8e40` 2026-10-27 |
| `simd-adler32` | 0.3.10 | MIT | 2026-07-14 | github.com/mcountryman/simd-adler32 | `d93164f` 2026-07-14 |
| `simplecss` | 0.2.2 | Apache-2.0 OR MIT | 2025-01-06 | github.com/linebender/simplecss | `8a29c42` 2025-12-21 |
| `skrifa` | 0.44.0 | MIT OR Apache-2.0 | 2026-07-04 | github.com/googlefonts/fontations | `a49bbb5` 2026-11-02 |
| `skrifa` | 0.46.2 | MIT OR Apache-2.0 | 2026-08-21 | github.com/googlefonts/fontations | `a49bbb5` 2026-11-02 |
| `slotmap` | 1.1.1 | Zlib | 2025-12-06 | github.com/orlp/slotmap | `eccedef` 2026-09-04 |
| `speexdsp-resampler` | 0.1.0 | BSD-3-Clause | 2020-02-14 | github.com/rust-av/speexdsp-rs | `7c67def` 2026-02-02 |
| `strength_reduce` | 0.2.4 | MIT OR Apache-2.0 | 2022-11-08 | github.com/ejmahler/strength_reduce | `10c1545` 2025-06-23 |
| `strict-num` | 0.1.1 | MIT | 2023-05-27 | github.com/RazrFalcon/strict-num | checked float wrappers | tiny-skia-path, usvg |
| `strsim` | 0.11.1 | MIT | 2024-04-02 | github.com/rapidfuzz/strsim-rs | `dacc84c` 2025-11-27 |
| `subtle` | 2.6.1 | BSD-3-Clause | 2024-06-24 | github.com/dalek-cryptography/subtle | constant-time primitives (dalek-cryptography) | rustls |
| `svgtypes` | 0.16.1 | Apache-2.0 OR MIT | 2026-01-09 | github.com/linebender/svgtypes | `cf6f6da` 2026-10-31 |
| `sys-locale` | 0.3.2 | MIT OR Apache-2.0 | 2024-11-01 | github.com/1Password/sys-locale | `f465ca1` 2026-08-10 |
| `taffy` | 0.14.0 | MIT | 2026-08-24 | github.com/DioxusLabs/taffy | `c6421c1` 2026-10-30 |
| `thiserror` | 1.0.69 | MIT OR Apache-2.0 | 2024-11-10 | github.com/dtolnay/thiserror | `3b29f01` 2026-10-15 |
| `thiserror` | 2.0.21 | MIT OR Apache-2.0 | 2026-09-23 | github.com/dtolnay/thiserror | `3b29f01` 2026-10-15 |
| `thiserror-impl` | 1.0.69 | MIT OR Apache-2.0 | 2024-11-10 | github.com/dtolnay/thiserror | `3b29f01` 2026-10-15 |
| `thiserror-impl` | 2.0.21 | MIT OR Apache-2.0 | 2026-09-23 | github.com/dtolnay/thiserror | `3b29f01` 2026-10-15 |
| `thread_local` | 1.1.10 | MIT OR Apache-2.0 | 2026-07-10 | github.com/Amanieu/thread_local-rs | `69fb050` 2026-07-10 |
| `time` | 0.3.55 | MIT OR Apache-2.0 | 2026-08-01 | github.com/time-rs/time | `3ce1048` 2026-10-16 |
| `time-core` | 0.1.9 | MIT OR Apache-2.0 | 2026-06-12 | github.com/time-rs/time | `3ce1048` 2026-10-16 |
| `time-macros` | 0.2.32 | MIT OR Apache-2.0 | 2026-07-20 | github.com/time-rs/time | `3ce1048` 2026-10-16 |
| `tiny-skia` | 0.12.0 | BSD-3-Clause | 2026-02-02 | github.com/linebender/tiny-skia | `5d47547` 2026-08-20 |
| `tiny-skia-path` | 0.12.0 | BSD-3-Clause | 2026-02-02 | github.com/linebender/tiny-skia/tree/master/path | `5d47547` 2026-08-20 |
| `tinyvec` | 1.13.3 | Zlib OR Apache-2.0 OR MIT | 2026-09-13 | github.com/Lokathor/tinyvec | `93f6af5` 2026-09-16 |
| `tokio` | 1.54.1 | MIT | 2026-10-21 | github.com/tokio-rs/tokio | `b499257` 2026-10-31 |
| `toml_edit` | 0.25.17+spec-1.1.0 | MIT OR Apache-2.0 | 2026-10-09 | github.com/toml-rs/toml | `7024319` 2026-11-02 |
| `transpose` | 0.2.3 | MIT OR Apache-2.0 | 2024-02-20 | github.com/ejmahler/transpose | matrix transpose | rustfft |
| `typenum` | 1.20.1 | MIT OR Apache-2.0 | 2026-05-29 | github.com/paholg/typenum | `0db9a0f` 2026-05-29 |
| `unic-char-property` | 0.9.0 | MIT/Apache-2.0 | 2019-03-03 | github.com/open-i18n/rust-unic/ | `37bd705` 2026-06-22 |
| `unic-char-range` | 0.9.0 | MIT/Apache-2.0 | 2019-03-03 | github.com/open-i18n/rust-unic/ | `37bd705` 2026-06-22 |
| `unic-common` | 0.9.0 | MIT/Apache-2.0 | 2019-03-03 | github.com/open-i18n/rust-unic/ | `37bd705` 2026-06-22 |
| `unic-ucd-ident` | 0.9.0 | MIT/Apache-2.0 | 2019-03-03 | github.com/open-i18n/rust-unic/ | `37bd705` 2026-06-22 |
| `unic-ucd-version` | 0.9.0 | MIT/Apache-2.0 | 2019-03-03 | github.com/open-i18n/rust-unic/ | `37bd705` 2026-06-22 |
| `unicode-bidi` | 0.3.18 | MIT OR Apache-2.0 | 2024-12-16 | github.com/servo/unicode-bidi | `9a8c7b0` 2026-09-15 |
| `unicode-properties` | 0.1.5 | (MIT OR Apache-2.0) AND Unicode-3.0 | 2026-10-31 | github.com/unicode-rs/unicode-properties | `b559480` 2026-10-31 |
| `unicode-script` | 0.5.9 | MIT OR Apache-2.0 | 2026-11-02 | github.com/unicode-rs/unicode-script | `e4a3c2e` 2026-11-01 |
| `unicode-segmentation` | 1.13.3 | MIT OR Apache-2.0 | 2026-06-01 | github.com/unicode-rs/unicode-segmentation | `4444c95` 2026-11-02 |
| `unicode-vo` | 0.1.0 | MIT/Apache-2.0 | 2019-06-11 | github.com/RazrFalcon/unicode-vo | the Unicode vertical-orientation table (UAX #50), data only | usvg |
| `unicode_categories` | 0.1.1 | MIT OR Apache-2.0 | 2016-10-22 | github.com/swgillespie/unicode-categories | a Unicode general-category table, data only | servo-layout |
| `untrusted` | 0.9.0 | ISC | 2021-07-13 | github.com/briansmith/untrusted | safe parsing of untrusted input (briansmith) | rustls-webpki |
| `urlpattern` | 0.3.0 | MIT | 2024-07-31 | github.com/denoland/rust-urlpattern | `e29804d` 2026-02-12 |
| `usvg` | 0.48.1 | Apache-2.0 OR MIT | 2026-08-02 | github.com/linebender/resvg | `cd64e51` 2026-10-31 |
| `uuid` | 0.8.2 | Apache-2.0 OR MIT | 2021-01-11 | github.com/uuid-rs/uuid | `7c7326d` 2026-10-29 |
| `uuid` | 1.29.0 | Apache-2.0 OR MIT | 2026-10-23 | github.com/uuid-rs/uuid | `7c7326d` 2026-10-29 |
| `vcpkg` | 0.2.15 | MIT/Apache-2.0 | 2021-06-19 | github.com/mcgoo/vcpkg-rs | `fa42994` 2025-02-01 |
| `vello_common` | 0.2.0 | Apache-2.0 OR MIT | 2026-08-07 | github.com/linebender/vello | `5016411` 2026-11-03 |
| `vello_cpu` | 0.2.0 | Apache-2.0 OR MIT | 2026-08-07 | github.com/linebender/vello | `5016411` 2026-11-03 |
| `version_check` | 0.9.5 | MIT/Apache-2.0 | 2024-07-25 | github.com/SergioBenitez/version_check | a rustc version probe | build scripts of cookie, generic-array, slotmap (host only) |
| `webdriver` | 0.54.0 | MPL-2.0 | 2026-06-02 | hg.mozilla.org/mozilla-central/file/tip/testing/webdriver | `not` on |
| `weezl` | 0.1.12 | MIT OR Apache-2.0 | 2025-11-11 | github.com/image-rs/weezl | `606f9c7` 2026-05-15 |
| `wuff` | 0.2.9 | MIT | 2026-09-01 | github.com/nicoburns/wuff | `02a1f49` 2026-09-08 |
| `wuff-capi` | 0.2.0 | MIT | 2026-07-13 | github.com/nicoburns/wuff | `02a1f49` 2026-09-08 |
| `xmlwriter` | 0.1.0 | MIT | 2019-08-12 | github.com/RazrFalcon/xmlwriter | an XML writer | usvg |
| `zeroize` | 1.9.1 | Apache-2.0 OR MIT | 2026-10-06 | github.com/RustCrypto/utils | `ddefdca` 2026-11-02 |
| `zeroize_derive` | 1.5.0 | Apache-2.0 OR MIT | 2026-06-12 | github.com/RustCrypto/utils | `ddefdca` 2026-11-02 |
| `zmij` | 1.0.23 | MIT | 2026-07-13 | github.com/dtolnay/zmij | `3535525` 2026-08-05 |
| `zune-core` | 0.5.3 | MIT OR Apache-2.0 OR Zlib | 2026-08-07 | github.com/etemesi254/zune-image | `2f9086b` 2026-10-24 |
| `zune-jpeg` | 0.5.15 | MIT OR Apache-2.0 OR Zlib | 2026-03-26 | github.com/etemesi254/zune-image/tree/dev/crates/zune-jpeg | `2f9086b` 2026-10-24 |
