<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — Relay 234, Lane 2c: host fuzzing for inflate, PNG and JPEG

**Seat:** Dexter. **Answers:** `Plans/DISPATCH-2026-11-01-l2c-decoder-fuzz-dexter.md`, Relay 234.
**Branch:** `dexter/l2c-decoder-fuzz`. **Base:** `origin/main` @ `10e10ee9` (ff-clean at close).
**Head (code; this RETURN's parent):** `f32979ff`. **Kernel-touching:** no (host-only: `tools/`,
`userland/neo-image-decode`, CI). This RETURN is the only commit after the code head, so the close-gate result
transfers under the tree-identity rule (only `Plans/` differs).

**Stacking note.** The branch was cut from `origin/dexter/l2-codecs` as the dispatch says (it needs Lane 2a/2b),
then rebased onto current `origin/main` to satisfy the close gate's rebase-freshness rule. It therefore contains
Lane 2a's and 2b's four commits (rebased: `2695e897`, `110c0927`, `71c57db8`, `9a7558d7`) plus this lane's
`f32979ff`. `origin/dexter/l2-codecs` itself is unchanged. Merge 2a/2b first and cherry-pick `f32979ff`, or merge
the whole stack; the PM should not treat the rebased 2a/2b copies as new work. The stacked branch diff vs `main`
is 151 files, 46,839 insertions(+), 3 deletions(-); **this lane's own commit is 11 files, 1,409 insertions(+),
13 deletions(-)**.

## What landed

| Item | Value |
|---|---|
| Harness | `tools/image-decode-fuzz` (workspace member; std host binary) |
| Targets | `inflate-zlib`, `inflate-raw`, `png`, `jpeg` |
| Seeds | Lane 2a's two zlib KAT streams and deterministic compressed inputs; Lane 2b's six PNGs and three JPEGs. Raw-DEFLATE seeds are the zlib streams' payloads (header + Adler-32 stripped), not random bytes |
| Engine | deterministic SplitMix64 mutational fuzzer; `(seed, iteration)` reproduces an input; the corpus grows on accepted inputs; format dictionaries for PNG chunks, JPEG markers and DEFLATE/zlib headers |
| Guards | panic aborts (workspace `panic = "abort"`); panic hook writes the exact input + replay/minimise commands; `--seconds` budget plus CI `timeout(1)`; counting allocator aborts on any single allocation over the cap or runaway live bytes; every decode goes through a capped API |
| Minimiser | `--minimize` delta-debugs a crash by subprocess replay (`--check` replays one file) |
| CI | `.github/workflows/ci.yml` job `decoder-fuzz`: four GitHub-hosted arms, 60 s each, `timeout(1)` hang backstop, crash-input artifact on failure |
| Decode cap | 16 MiB per fuzz input (stricter than the API's 64 MiB default, to keep one decode bounded; the cap-enforcement path is still exercised) |

The image targets call `neo_image_decode::decode_with_limit`, the implementation `libneo::image_decode`
re-exports. The harness links the implementation crate directly because `libneo` is `no_std` and the harness is a
std host binary — the same split the Lane 2b KAT crate uses.

**Why an in-tree fuzzer, not `cargo-fuzz`.** `cargo-fuzz` is not installed and needs the libFuzzer/sanitizer
toolchain; the close condition is a fixed budget in ordinary CI on the pinned nightly. The in-tree harness makes
the budget, seed and mutation schedule explicit, and it runs in the normal workspace `test`/`clippy` jobs too.

## Fuzz budget and measured run

Command (the CI shape):

```
./target/debug/image-decode-fuzz --target all --seconds 60 --seed 234
```

Host: Linux 6.8.0 x86_64, pinned `rustc 1.97.0-nightly (e8e4541ff 2026-04-15)`, 2026-11-01. Debug build, so
overflow checks are on (the found bug is a debug panic; see below).

| Target | Iterations | Accepted | Rejected | Exec/s | Max decoded output |
|---|---|---|---|---|---|
| `inflate-zlib` | 1,418,225 | 566,872 | 851,353 | 23,637 | 22,528 B |
| `inflate-raw` | 5,329,740 | 3,656,978 | 1,672,762 | 88,829 | 89,057 B |
| `png` | 522,056 | 101,910 | 420,146 | 8,701 | 4,096 B |
| `jpeg` | 4,387 | 3,599 | 788 | 73 | 16,776,704 B |

Total **240 s** of target time (4 × 60 s), 7,274,408 inputs. Peak live memory reached 43,513,200 B, under the
harness's 6 × cap total-live guard; no single allocation exceeded the 16 MiB cap (the largest decode output was
16,776,704 B, under 16,777,216 B), and the total-live guard never fired. The CI arms run the same four budgets in
parallel; the local run above is the same harness and configuration.

## Found and fixed

**PNG IHDR size overflow in our API wrapper (`neo_image_decode::checked_rgba_len`).**

- Found by the `png` target at seed 234, iteration 3235: `attempt to multiply with overflow` at
  `userland/neo-image-decode/src/lib.rs:153`.
- Minimised to **24 bytes** with the harness's own `--minimize`: PNG signature + a 13-byte `IHDR` whose width
  and height are both `0xFFFFFFFF` (the file is now
  `tools/image-decode-logic-tests/tests/data/png/hostile_ihdr_overflow.png`, SHA-256
  `d6aabc50a389205489fbc975fd3fea236dc90463cc12ffe636bcd38e36016545`).
- `u64::from(width) * u64::from(height) * 4` overflows `u64` for two maximal `u32` dimensions. In the fuzz/debug
  build that is a panic; in a release build it wraps, and a wrapped small result would pass the cap check — a
  size-cap bypass as well as a crash. The fix uses checked multiplication and answers
  `ImageDecodeError::TooLarge`; no allocation is attempted for the claim. The 1,000,000 × 1,000,000 KAT still
  passes, and the new KAT pins the `u32::MAX` case.
- Regression KAT: `png_ihdr_at_u32_max_is_too_large_not_an_overflow` in `tools/image-decode-logic-tests`, with
  the fixture and its origin recorded in that crate's `PROVENANCE.md`.
- After the fix, the full 60 s × 4 campaign found **no** further crash, panic, hang or over-cap allocation. No
  upstream (`png`, `zune-jpeg`, `miniz_oxide`) bug was found, so no `VENDORED.md` entry was needed.

## Verification

| Check | Result |
|---|---|
| `cargo test -p image-decode-fuzz` | **6 passed, 0 failed** |
| `cargo test -p image-decode-logic-tests` | **16 passed, 0 failed** |
| `cargo clippy -p image-decode-fuzz -p image-decode-logic-tests -p neo-image-decode --all-targets -- -D warnings` | **exit 0** |
| `cargo fmt --all --check` | **exit 0** |
| `scripts/worker-close-gate.sh` | **CLOSE-GATE SATISFIED (host side)** — Step 1 ff-clean; **37/37 Step-2 checks OK, 0 FAIL** (fmt, workspace host tests, ai-ops both feature sets, neo-atrium three feature sets, workspace clippy, blocking kernel clippy, `!xsave-fpu` kernel build, `!mt7601u` compile KAT, and the `ci.yml` product-feature userland set); Step 3 SPDX OK |
| Step 4 self-stat | 11 files changed, 1,409 insertions(+), 13 deletions(-) (this lane's commit) |
| 60 s × 4 campaign | all four targets `status=ok`; no crash, panic, timeout or over-cap allocation |

```
Branch:        dexter/l2c-decoder-fuzz @ f32979ff
Rebased on:    origin/main @ 10e10ee9  (ff-clean)
Kernel-touching: no
Push CI:       <not run — see "CI / PR status" below>
Self-stat:     11 files changed, 1409 insertions(+), 13 deletions(-)
```

## CI / PR status — no green CI URL from this box

**This is an infrastructure gap, not a code claim.** The branch is pushed at `f32979ff`, but this relay box has
only an SSH write deploy key for `SecondStageTurbine/LogansOS` — no GitHub API token (`GITHUB_TOKEN`, `GH_TOKEN`,
`WORKER_GIT_TOKEN` are unset) and no `gh`. A deploy key can push a branch but cannot open a PR or call
`workflow_dispatch`. `ci.yml` runs on `pull_request` and `workflow_dispatch`; this repo's branch pushes do not run
CI, so there is no run to cite. I am **not** claiming green CI; the local close gate above ran the exact Step-2
host set on the pinned `nightly-2026-04-16` toolchain.

**PM action to close the gate:** open the PR here —

https://github.com/SecondStageTurbine/LogansOS/pull/new/dexter/l2c-decoder-fuzz

(or dispatch the `CI` workflow on `dexter/l2c-decoder-fuzz`). That `pull_request` run is the push CI; the new
`decoder-fuzz` job is one of its four arms.

## Notes

1. **Close-gate container.** `scripts/worker-close-gate.sh` defaults to `docker compose run --rm neo`, but this
   worker box's Docker daemon does not bind-mount this tree (the container sees an empty `/workspace`), so the gate
   was run with `CLOSE_GATE_DOCKER=env`, i.e. every Step-2 check ran on the host toolchain pinned by
   `rust-toolchain.toml` (`nightly-2026-04-16`, the one CI's checkout uses). Every check passed.
2. **CI cost.** Four short GitHub-hosted arms per run; they do not touch the QEMU shard pool.
3. **No personal details.** The new files contain only upstream KAT data provenance already in the repo (the
   vendored crate authors' metadata is not touched) and the public upstream URLs; no Windows/`/home` path, LAN
   address, MAC or project email was added.
4. **RAG.** Not available on the always-on box, as the dispatch says; used grep and direct reads.
5. **Disk reclaim.** After the PM confirms the merge: `cargo clean` in `~/workers/dexter` (do not delete the
   worktree).
