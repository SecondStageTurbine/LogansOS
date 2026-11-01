<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — Relay 230, Lane 2b: vendored `png` + `zune-jpeg` and the image-decode API

**Seat:** Dexter. **Answers:** `Plans/DISPATCH-2026-11-01-m910q-reviews-and-l2b.md`, §"Section 230: Dexter,
Lane 2b (PNG and JPEG decoding)". **Branch:** `dexter/l2b-image-decode`. **Base:** `origin/main` @ `26b59b71`
(ff-clean at close). **Head (code; this RETURN's parent):** `86a718b4`. **Kernel-touching:** no. The only commit
after that head is this RETURN file under `Plans/` (docs-only), so the close-gate result transfers under the
tree-identity rule; no code path differs.

**Stacking note.** The branch was cut from `origin/dexter/l2a-miniz-oxide` as the dispatch says, then rebased onto
current `origin/main` to satisfy the close gate's rebase-freshness rule. It therefore contains Lane 2a's two
commits (rebased, `1d371d7a` + `5abb906f`) plus this lane's `86a718b4`. `origin/dexter/l2a-miniz-oxide` itself is
unchanged. Merge 2a first and cherry-pick `86a718b4`, or merge the whole stack; the PM should not treat the
rebased 2a copies as new work. The full branch diff vs `main` is 144 files, 45,294 insertions(+), 3 deletions(-);
**this lane's own commit is 114 files, 37,093 insertions(+), 2 deletions(-)** (`git show --stat 86a718b4`).

## What landed

| Item | Value |
|---|---|
| `png` upstream | https://github.com/image-rs/image-png, branch `master`, commit `b2f10c6a7e3127d4aaf231f3eae731a33e6eb0bc` (2026-09-23), crate `png` 0.18.1 — the dispatch's `b2f10c6`, re-checked on 2026-11-01 |
| `png` licence | **MIT OR Apache-2.0** in the crate manifest; this copy is used under **MIT**. `LICENSE-MIT` and `LICENSE-APACHE` are kept |
| `zune-jpeg` upstream | https://github.com/etemesi254/zune-image, branch `dev`, commit `2f9086b07cdf5a36621ce18f79d310f99b751de9` (2026-10-24), crate `zune-jpeg` 0.5.16-rc3 — the dispatch's `2f9086b`, re-checked on 2026-11-01 |
| `zune-jpeg` licence | **MIT OR Apache-2.0 OR Zlib** in the crate manifest; this copy is used under **MIT**. All three licence files are kept |
| `zune-core` | Required by `zune-jpeg`'s relative path dependency, so it is vendored too at the same commit (`crate zune-core 0.5.3`, triple licence, used under MIT). Recorded in `third_party/zune-core/VENDORED.md` |
| Vendored at | `third_party/png/`, `third_party/zune-jpeg/`, `third_party/zune-core/`, each with `Cargo.toml.upstream`, `VENDORED.md` and licence files |
| Source delta | `src/` is upstream byte-for-byte plus one first line per `.rs` file (`// SPDX-License-Identifier: …`), verified with a `tail -n +2` + `cmp` loop — 0 diffs on all three trees. `Cargo.toml.upstream` is byte-identical to upstream's manifest in all three |
| Manifest delta | Each `Cargo.toml` differs only in `publish = false`, `license = "MIT"` (the option used), and the removed non-vendored bench/test blocks. `zune-jpeg`'s `../zune-core` path dependency is unchanged because the core crate is vendored beside it |
| Workspace | All three are in `exclude` (first-party fmt/clippy do not apply; each zune crate carries a `rustfmt.toml` that disables formatting of the vendored source). `userland/neo-image-decode` and `tools/image-decode-logic-tests` are members |
| Consumer / kernel | Host-only. **The kernel and OS userland never link the codecs**: the `image-decode` feature is off by default, and libneo rejects it for `target_os = "none"` with a `compile_error!` |

The survey rows in `docs/THINK-2026-10-31-lived-in-os-lanes.md` §2 were re-checked on the day: `png` is still
`b2f10c6` / MIT OR Apache-2.0, `zune-image` is still `2f9086b` / MIT OR Apache-2.0 OR Zlib. Both are active, so
an update is a re-vendor at a newer commit plus a re-run of the KATs.

## The API (bytes in, RGBA + size out)

`userland/neo-image-decode` (package `neo-image-decode`) is the one small API:

- `decode(bytes) -> Result<DecodedImage, ImageDecodeError>` — sniffs PNG vs JPEG and uses the default 64 MiB cap.
- `decode_with_limit(bytes, max_rgba_bytes)` — the caller's budget.
- `DecodedImage { width: u32, height: u32, rgba: Vec<u8> }`, always row-major RGBA8, `rgba.len() == width * height * 4`.
- `ImageFormat::from_magic`, and `ImageDecodeError` (`EmptyInput`, `UnknownFormat`, `InvalidDimensions`,
  `TooLarge`, `UnsupportedColorType`, `DecoderOutput`, `Png`, `Jpeg`).

**Why a separate crate, and why the API is still "in libneo".** The dispatch says "add one small decode API in
`libneo`"; `libneo` re-exports this crate as `pub use neo_image_decode as image_decode` behind its host-only
`image-decode` feature, so callers use `libneo::image_decode::decode`. The implementation is a separate std crate
because **`png` 0.18 is std-only**, and a Cargo feature on `libneo` that links std is workspace-global under
feature unification: a host KAT that enabled it made every `no_std` userland binary that links libneo link std
too, and `neo-arthur`'s `panic_impl` became a duplicate lang item. Keeping the std implementation out of
`libneo`'s compilation keeps the OS target unchanged. This is the one place the lane departs from a literal
reading of "in libneo"; the public path is unchanged.

**Size cap (the guard against a hostile image).** `width * height * 4` is checked with `u64` arithmetic before the
pixel buffer is allocated. PNG additionally pre-checks the `IHDR` dimensions straight from the bytes, before the
decoder is asked to parse anything, and its own allocation budget is bounded (`limit.max(64 KiB)` so a tiny image
with an exactly-fitting cap can still parse). JPEG decodes headers first, checks the declared dimensions, and only
then decodes the scan. A 1,000,000 × 1,000,000 PNG header is a KAT: it returns `TooLarge` without an allocation.

## Host KATs

New crate `tools/image-decode-logic-tests` (15 tests). All fixtures were fetched **raw** with `curl` at pinned
commits; `tests/data/PROVENANCE.md` records URL, commit, file SHA-256, size, native type/JPEG mode and the
expected RGBA SHA-256.

| Fixture | Source (commit) | Size | Expected RGBA SHA-256 |
|---|---|---|---|
| `png/basn0g08.png` | image-rs/image-png `b2f10c6` `tests/pngsuite/` | 32x32 gray 8 | `982faa27…620eb158` |
| `png/basn2c08.png` | same | 32x32 RGB 8 | `23a53c67…e0f0c99e` |
| `png/basn2c16.png` | same | 32x32 RGB 16 (stripped to 8) | `7c4b73e8…d726c17a` |
| `png/basn3p08.png` | same | 32x32 palette 8 | `b1c3302e…d29acc32cc` |
| `png/basn4a08.png` | same | 32x32 gray+alpha 8 | `76b94a71…dbb002509` |
| `png/basn6a08.png` | same | 32x32 RGBA 8 | `2eb6a2cb…3b7431d2` |
| `jpeg/tiny_non_interleaved_444.jpg` | etemesi254/zune-image `2f9086b` `test-images/jpeg/` | 16x16 baseline 4:4:4 | `59bfa308…1bea6db` |
| `jpeg/non_interleaved_422_64x64.jpg` | same | 64x64 baseline 4:2:2 | `4500aa14…602ef8f` |
| `jpeg/progressive_restart_420.jpg` | same | 256x256 progressive 4:2:0 | `df194d15…57b6c00` |

The six PNG hashes were produced by `tests/data/reference_png_rgba.py`, an independent decoder using only Python's
`zlib` and the PNG spec's filters; it matches the vendored decoder byte-for-byte on all six (16-bit samples
compare at the high byte, which is what `STRIP_16` keeps). The JPEG hashes pin the vendored `zune-jpeg` output:
the 16x16 4:4:4 one was independently reproduced exactly with the `jpeg-decoder` 0.3 crate, while the 4:2:2 and
progressive ones differ from that independent decoder by a few LSBs in IDCT/chroma upsampling — the same limit
`zune-jpeg`'s own libjpeg parity test records (max channel delta 2). `PROVENANCE.md` states which is which; the
JPEG hash is a regression pin, not a cross-decoder equality claim.

Also covered: format sniffing, empty/unknown input, the exact-cap boundary for both formats, a hostile oversized
PNG `IHDR`, and a fixed-budget deterministic mutation smoke test (64 mutations per fixture; errors or a valid
capped image, never a panic or an over-cap allocation). The mutation test is the THINK's "fuzz each decoder on the
host for a fixed budget" floor, **not** coverage-guided fuzzing — that remains a follow-up if the PM wants a real
fuzz target wired into CI.

## Verification

| Check | Result |
|---|---|
| `cargo test -p image-decode-logic-tests` | **15 passed, 0 failed** |
| `cargo test -p neo-image-decode --doc` | **1 passed** |
| `cargo test --workspace --exclude neoplutarch-kernel` | **exit 0** |
| `cargo clippy --workspace --all-targets -- -D warnings` | **exit 0** |
| `scripts/worker-close-gate.sh` | **exit 0 — CLOSE-GATE SATISFIED (host side)** |
| Step 2: fmt, workspace host tests, ai-ops (both feature sets), neo-atrium (three feature sets), workspace clippy, blocking kernel clippy, `!xsave-fpu` kernel build, `!mt7601u` compile KAT, `ci.yml` product-feature userland set | all **OK** |
| Step 3: SPDX on changed source files | **OK** (vendored `.rs` carry a header; vendored `Cargo.toml`/`rustfmt.toml` carry one) |
| Step 4 self-stat | 144 files, 45,294 insertions(+), 3 deletions(-) for the stacked branch; this lane's commit is 114 files, 37,093 insertions(+), 2 deletions(-) |
| Vendored source check | `tail -n +2` + `cmp` against the upstream clones: **0 diffs** in all three trees; `Cargo.toml.upstream` byte-identical |

```
Branch:        dexter/l2b-image-decode @ 86a718b4
Rebased on:    origin/main @ 26b59b7  (ff-clean)
Kernel-touching: no
Push CI:       <not run — see "CI / PR status" below>
Self-stat:     144 files changed, 45294 insertions(+), 3 deletions(-)
```

## CI / PR status — no green CI URL from this box

**This is an infrastructure gap, not a code claim.** The branch is pushed at `86a718b4`, but this relay box has
only an SSH **write deploy key** for `SecondStageTurbine/LogansOS` — no GitHub API token (`GITHUB_TOKEN`,
`GH_TOKEN`, `WORKER_GIT_TOKEN` are unset) and no `gh`. A deploy key can push a branch but cannot open a PR or call
`workflow_dispatch`. `ci.yml` runs on `pull_request` and `workflow_dispatch`; this repo's branch pushes do not run
CI, so there is no run to cite. I am **not** claiming green CI; the local close-gate above ran the exact Step-2
host set on the pinned `nightly-2026-04-16` toolchain.

**PM action to close the gate:** open the PR here —

https://github.com/SecondStageTurbine/LogansOS/pull/new/dexter/l2b-image-decode

(or dispatch the `CI` workflow on `dexter/l2b-image-decode`). That `pull_request` run is the push CI.

## Notes

1. **Close-gate container.** `scripts/worker-close-gate.sh` defaults to `docker compose run --rm neo`, but this
   worker tree is mode 700 owned by `logans` and the image's `neo` user cannot read it (the same gap Lane 2a
   recorded). The gate was run with `CLOSE_GATE_DOCKER` pointing at a one-line `exec "$@"` wrapper, i.e. on the
   host toolchain pinned by `rust-toolchain.toml` (`nightly-2026-04-16`, the one CI's checkout uses). Every Step-2
   check passed. If the PM wants the container path exercised too, the tree permissions are the only blocker.
2. **Feature-unification trap (found and fixed here).** The first KAT wiring put the API behind a `libneo`
   feature and depended on `sha2` with default features. `cargo clippy --workspace --all-targets` then unified
   `std` into `crypto-common` and into `libneo`, and the kernel and `neo-arthur` both hit duplicate
   `panic_impl` lang items. The fix is the separate `neo-image-decode` crate (implementation) plus
   `sha2 = { default-features = false }`; both the workspace tests and the workspace clippy are green now. This
   is why the implementation is not literally inside `libneo`'s crate; `libneo::image_decode` still resolves.
3. **Personal-details scan.** The categories the dispatch names were scanned across the added files. The only
   hits are the upstream crate authors' email addresses in the vendored `Cargo.toml` / `Cargo.toml.upstream`
   (upstream metadata, same precedent as the vendored `fatfs` and `miniz_oxide` manifests); no Windows user path,
   `/home/<user>` path, LAN address, MAC, project hostname or project email is in the diff.
4. **RAG.** Not available on the always-on box, as the dispatch says; used grep and direct reads.
5. **Disk reclaim.** After the PM confirms the merge: `cargo clean` in `~/workers/dexter` (do not delete the
   worktree).
