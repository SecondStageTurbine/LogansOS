<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# `image-decode-fuzz` — Lane 2c host fuzzing

Deterministic, in-tree fuzzing for the Lane 2 decoders:

| Target | Code under test |
|---|---|
| `inflate-zlib` | `miniz_oxide::inflate::decompress_to_vec_zlib_with_limit` |
| `inflate-raw` | `miniz_oxide::inflate::decompress_to_vec_with_limit` |
| `png` | `neo_image_decode::decode_with_limit` (PNG) — the API `libneo::image_decode` re-exports |
| `jpeg` | `neo_image_decode::decode_with_limit` (JPEG) — the same API |

The seed corpora are the KAT vectors already in the repository: Lane 2a's two
zlib streams (`tools/miniz-oxide-logic-tests/tests/data/zlib/`), Lane 2b's six
PNGs and three JPEGs (`tools/image-decode-logic-tests/tests/data/`), plus
DEFLATE/zlib streams compressed from deterministic inputs. Raw-DEFLATE seeds are
the zlib streams' payloads (header and Adler-32 stripped), so the raw target
also starts from real compressed data.

## Why not `cargo-fuzz`?

`cargo-fuzz` is not installed in the workspace and needs the libFuzzer/sanitizer
toolchain. The guardrail's requirement is a fixed budget in ordinary CI on the
pinned nightly, so this harness is a small deterministic mutational fuzzer:
same seed and iteration count produce the same input sequence, a crash is
reproducible from `(target, seed, iteration)`, and the exact crashing bytes are
written out before the process aborts.

## Running

```sh
# the CI shape: 60 s per target
cargo run -p image-decode-fuzz -- --target all --seconds 60

# one target, a fixed number of inputs (what a crash report gives you)
cargo run -p image-decode-fuzz -- --target png --seed 234 --iterations 500000

# replay one file
cargo run -p image-decode-fuzz -- --check path/to/input.bin --target png

# reduce a crash to a small regression input
cargo run -p image-decode-fuzz -- --minimize path/to/crash.bin --target png
```

Options: `--target`, `--seconds`, `--iterations`, `--seed`, `--cap` (decode and
single-allocation cap, default 16 MiB), `--max-input`, `--crash-dir`. `--help`
lists them all.

## Guards

- **Crash / panic** — a panic aborts the process (the workspace uses
  `panic = "abort"`), so the CI job fails. The panic hook writes
  `crash-<target>-seed<seed>-iter<n>.bin` under `--crash-dir` and prints the
  replay and minimise commands.
- **Timeout** — the campaign stops itself at `--seconds`; the CI job also wraps
  the process in `timeout(1)`, so a decoder that never returns fails the job.
- **Allocation above the decode size cap** — a counting global allocator aborts
  on any single allocation larger than `--cap`, a total-live guard aborts on
  runaway growth, and every decode goes through a capped API
  (`decode_with_limit`, `decompress_to_vec{,_zlib}_with_limit`) with the returned
  buffer checked against the cap.

## CI

`.github/workflows/ci.yml`'s `decoder-fuzz` job runs four parallel
GitHub-hosted arms, one per target, each with a 60-second budget and a
`timeout(1)` backstop. It is a blocking host job: a crash, a panic, a hang or an
over-cap allocation reddens the workflow.

When a crash is found: download the `decoder-fuzz-crash-*` artifact (or take the
path the hook printed), minimise it, add the minimised input as a regression KAT
in `tools/image-decode-logic-tests` (or `tools/miniz-oxide-logic-tests`), and
fix the bug in our wrapper — or, for an upstream bug, record it in that codec's
`VENDORED.md` and guard it in the wrapper.
