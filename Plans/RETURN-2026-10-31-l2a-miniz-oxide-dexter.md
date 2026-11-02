<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — Relay 226, Lane 2a: vendored `miniz_oxide` + host KATs

**Seat:** Dexter. **Answers:** `Plans/DISPATCH-2026-10-31-seats-resume-on-public.md`, §"Relay 226: Dexter, Lane 2a
(miniz_oxide)". **Branch:** `dexter/l2a-miniz-oxide`. **Base:** `origin/main` @ `07d7f6ef` (ff-clean at close).
**Head (code; this RETURN's parent):** `d65087189321fa85768d6ae216b71c71c2450218`. **Kernel-touching:** no.
The only commit after that head is this RETURN file under `Plans/` (docs-only), so the close-gate result transfers
under the tree-identity rule; no code path differs.

## What landed

| Item | Value |
|---|---|
| Upstream | https://github.com/Frommi/miniz_oxide, branch `master` |
| Commit | `7fc2dd45810e1722272ff77c4238a8f8e0d0da22` (2026-10-29), the dispatch's `7fc2dd4`, re-checked on the day |
| Crate | `miniz_oxide` 0.9.1, the `miniz_oxide/` subdirectory (the repository root is the separate `miniz_oxide_c_api`, not vendored) |
| Licence | **MIT OR Zlib OR Apache-2.0** in the crate manifest; this copy is used under **MIT**. `LICENSE`, `LICENSE-MIT.md`, `LICENSE-APACHE.md` and `LICENSE-ZLIB.md` are all kept |
| Vendored at | `third_party/miniz_oxide/` (`src/`, `Readme.md`, the four licence files, `Cargo.toml.upstream`, `Cargo.toml`, `VENDORED.md`) |
| Source delta | `src/` is upstream byte-for-byte plus one first line per `.rs`, `// SPDX-License-Identifier: MIT`; verified with a `tail -n +2` plus `cmp` loop — 0 diffs. `Cargo.toml.upstream` is byte-identical to upstream's |
| Manifest delta | `Cargo.toml` differs only in `publish = false`, the removed `[[bench]]`, the removed (comment-only) `[dev-dependencies]`, and `license = "MIT"` naming the option used |
| Workspace | `third_party/miniz_oxide` is in `exclude` (first-party fmt/clippy do not apply); `tools/miniz-oxide-logic-tests` is a member and its path dependency keeps the vendored copy compiled on the host |
| Consumer / kernel | Host KAT crate only. **The kernel does not link it**; this lane is the vendoring plus host proof, not a codec wiring |

The survey row in `docs/THINK-2026-10-31-lived-in-os-lanes.md` §2 says "MIT (crate `Cargo.toml`)". Re-checked here:
the crate declares the triple, and MIT is one of the three, so the lane's licensing conclusion holds; all three
licence texts ship rather than only the chosen one.

## Host KATs

New crate `tools/miniz-oxide-logic-tests` (8 tests). Every compressed vector was fetched **raw** with `curl` at a
pinned commit; `tests/data/PROVENANCE.md` records the URL, commit, compressed SHA-256, decoded bytes, decoded
SHA-256 and the upstream project's own expected value for each.

| Vector | Source (commit) | Decodes to | Independent oracle |
|---|---|---|---|
| `gzip/zlib-rs-example.gz` | `trifectatechfoundation/zlib-rs` `5dbef501…` | `zlib-rs gzip example` | same commit's `example.txt` |
| `gzip/flate2-multi.gz` | `rust-lang/flate2-rs` `54f49cd9…` | `first\n` + `second\n`, two members | same commit's `multi.txt` |
| `zlib/ratarmount-simple.zlib` | `mxmlnkn/ratarmount` `7179828a…` | `foo fighter\n` | same commit's `create-fixed-archives-list.sh` records MD5 `19696f24a91fc4e8950026f9c801a0d0` for the decompressed `simple` |
| `zlib/dexador-test.zlib` | `fukamachi/dexador` `6289b8a3…` | `Deflate test string.` | same commit's `t/dexador.lisp` asserts that exact body |

`miniz_oxide` implements DEFLATE and the zlib wrapper, not the gzip container, so the gzip KAT parses RFC 1952,
raw-inflates each member, and checks the member's CRC32 and ISIZE. The multi-member fixture fails a decoder that
stops after the first member. The zlib vectors cover both header variants (`0x785e`, `0x789c`); the corrupt-adler
test flips the ratarmount stream's last byte and expects an error. Round trips cover raw DEFLATE and zlib over
empty, text, zero-filled, all-byte-values, repeated text and deterministic random input at levels 0, 1, 3, 6, 9, 10.
The `dexador` fixture has one trailing byte after the zlib stream (upstream's HTTP body); the KAT keeps it and
documents that decoding ignores it.

## Verification

| Check | Result |
|---|---|
| `cargo test -p miniz-oxide-logic-tests` | **8 passed, 0 failed** |
| `scripts/worker-close-gate.sh` (host toolchain; the box's tree is not readable by the dev container, see note) | **exit 0 — CLOSE-GATE SATISFIED (host side)** |
| Step 2: fmt, workspace host tests, ai-ops tests (real-quant/AVX2 and soft-f32-only), neo-atrium feature tests, workspace clippy, blocking kernel clippy, `!xsave-fpu` kernel build, `!mt7601u` compile KAT, and the `ci.yml` product-feature userland clippy set | all **OK** |
| `cargo test --workspace --exclude neoplutarch-kernel` (separate full run) | **exit 0** |
| Step 3: SPDX on changed source files | **OK** (all changed files carry a header) |
| Step 4 self-stat | 31 files changed, 8102 insertions(+), 1 deletion(-), no phantom deletions |

```
Branch:        dexter/l2a-miniz-oxide @ d6508718
Rebased on:    origin/main @ 07d7f6e  (ff-clean)
Kernel-touching: no
Push CI:       <not run — see "CI / PR status" below>
Self-stat:     31 files changed, 8102 insertions(+), 1 deletion(-)
```

## CI / PR status — no green CI URL from this box

**This is an infrastructure gap, not a code claim.** The branch is pushed at `d6508718`, but this relay box has only
an SSH **write deploy key** for `SecondStageTurbine/LogansOS` — no GitHub API token (`GITHUB_TOKEN`, `GH_TOKEN` and
`WORKER_GIT_TOKEN` are all unset) and no `gh`. A deploy key can push a branch but cannot open a PR or call
`workflow_dispatch`. `ci.yml` runs on `pull_request` (opened/reopened/ready_for_review) and `workflow_dispatch`, and
this repo's branch pushes deliberately do not run CI, so there is no run to cite.

**PM action to close the gate:** open the PR here —

https://github.com/SecondStageTurbine/LogansOS/pull/new/dexter/l2a-miniz-oxide

(or dispatch the `CI` workflow on `dexter/l2a-miniz-oxide`). The `pull_request` run is the push CI. The local
close-gate above ran the exact Step-2 host set, but per the standing rule that is not a substitute for the CI URL;
I am **not** claiming green CI.

## Notes

1. **Personal-details scan (rule 1).** The list the dispatch says was in the session prompt was not present in this
   session, so I scanned the categories rule 1 names across every added file: Windows user paths, `/home/<user>`
   paths, LAN addresses, MAC addresses, hostnames, serial numbers and email addresses. The only hits are the
   upstream crate authors' email addresses in the vendored `Cargo.toml` / `Cargo.toml.upstream` (upstream metadata,
   same precedent as the vendored `fatfs` manifest); no project or machine detail is in the diff.
2. **Close-gate container.** `scripts/worker-close-gate.sh` defaults to `docker compose run --rm neo`, but this
   worker tree is mode 700 owned by `logans` and the image's `neo` user cannot read it (it also cannot see
   `rust-toolchain.toml`, so it had fallen back to a newer default nightly). I ran the close-gate with
   `CLOSE_GATE_DOCKER` pointing at a one-line `exec "$@"` wrapper, i.e. on the host toolchain pinned by
   `rust-toolchain.toml` (`nightly-2026-04-16`, the one CI's checkout uses). Every Step-2 check passed. If the PM
   wants the container path exercised too, the tree permissions are the only blocker.
3. **RAG.** Not available on the always-on box, as the dispatch says; used grep and direct reads.
4. **Disk reclaim.** After the PM confirms the merge: `cargo clean` in `~/workers/dexter` (do not delete the
   worktree).
