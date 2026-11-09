<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — relay 346: #77 (WebRender 0.70 on SWGL)

**Seat:** Dexter. **Model:** DeepSeek, high. **Answers:** `Plans/DISPATCH-2026-11-08-relays-335-339-reroute.md` §346, which runs the **#77** items and the **All four** items of `Plans/DISPATCH-2026-11-08-relays-329-332.md` §329, for #77 only.
**Reviewed:** PR #77 head `c8a065744a779d7d95e06dbcb8299d889718fd90` (`claude/ot61-r4b-webrender-v2`), base/merge-base `13eed2c54caf76cbef78dad762467df68611546a` (the PR's `main` base). `origin/main` is now `e6b3d665c`; the three shared code files moved since the base (see the base-drift note at the end).
**Overall: PASS on all six checklist items.** The vendored closure is complete and its 99 crates' licence and commit records match the crates themselves; all three patches reproduce `patched/` exactly; the CI job's closure check rejects any crates.io resolution; the pixel oracle is real and a wrong frame exits non-zero; the feature is opt-in; and the THINK/PR carry the prior-art survey. Five non-blocking notes at the end (base drift, the PR body's "not kernel-touching" sentence, the inherited rustup bootstrap, two stale THINK/VENDORED counts, and r-efi's LGPL alternative). The head's full 4-shard run and the WebRender closure job are green.

## Checklist, in the dispatch's order

| # | Dispatch item | Verdict | Enforcing lines |
|---|---|---|---|
| #77.1 | `VENDORED.md` lists every vendored crate (version, licence, source); every licence is on the allowed list | **PASS** | `third_party/ot61-swgl/VENDORED.md:64-172`; r-efi note `:68`, row `:135`; font row `:178` |
| #77.2 | The patches match `patched/`; `gen-patch-table.py` reproduces the table | **PASS** | `swgl-0.70.0-force-clear.patch`, `webrender-0.70.0-frame-time.patch`, `wr_glyph_rasterizer-0.70.1-neoplutarch.patch`; `gen-patch-table.py:1-48`; `userland/webrender-smoke/Cargo.toml:40-139` |
| #77.3 | `ot61-closure.yml` fetches nothing at build time that isn't pinned by hash or version | **PASS** (closure; note 3) | workflow `:348-357`; lock has zero `source =` entries; bootstrap caveat `:320` |
| #77.4 | The `test_harness.rs` scenario fails on a wrong frame (pixel/digest check, not only "no panic") | **PASS** | `userland/webrender-smoke/src/main.rs:157-169,408-426`; `kernel/src/test_harness.rs:3054-3097`; `tools/xtask/src/main.rs:18792-18806` |
| All.1 | The feature is off by default, and the off build is today's image | **PASS** | `kernel/Cargo.toml:81` (default), `:495` (feature); `Cargo.toml:101` (excluded standalone crate); `kernel/src/test_harness.rs:175,3054` |
| All.2 | Prior art: the PR body or THINK carries the survey with licences and dates | **PASS** | `docs/THINK-2026-10-31-ot61-r4-swgl-floor.md:60-71`; `VENDORED.md:64-178`; PR #77 body, "Prior art / licences" |

## #77.1 PASS — the vendored closure is complete, and its licence/source records match the crates

**Completeness.** The tree has 102 top-level crate directories: 98 under `vendor/`, 3 under `patched/` and 1 under `stubs/`. Three names exist in both `vendor/` and `patched/` (`swgl-0.70.0`, `webrender-0.70.0`, `wr_glyph_rasterizer-0.70.1`), so there are **99 distinct crates** — exactly the 99 rows of `VENDORED.md:72-172`.

I cross-checked both directions against `userland/webrender-smoke/Cargo.lock`:

- every third-party package in the lock has a vendored directory; and
- every vendored directory is in the lock.

The only lock packages outside the tree are the three first-party path packages (`webrender-smoke`, `neo-std-pal`, `neo-crt`), which is expected. `dirs − lock = ∅`.

**Licence records.** I parsed the `license` field from every crate's own `Cargo.toml` and compared it with the table: **99/99 match, no mismatches.** The resulting set is:

- 49 `MIT OR Apache-2.0`, 11 `MIT/Apache-2.0`, 11 `MPL-2.0`, 10 `MIT`;
- 2 `Apache-2.0`, 3 `Apache-2.0 OR MIT`, 1 `Apache-2.0/MIT`;
- 1 `BSD-3-Clause`, 2 `Zlib OR Apache-2.0 OR MIT`, 1 `Zlib`;
- 2 `Unlicense OR MIT`, 4 `Unicode-3.0`, 1 `(MIT OR Apache-2.0) AND Unicode-3.0`;
- 1 `MIT OR Apache-2.0 OR LGPL-2.1-or-later` (`r-efi`).

Every one is on CLAUDE.md's allowed list (MIT, Apache-2.0, BSD, zlib, ISC, MPL-2.0, Unicode-3.0, Unlicense/CC0). **The only crate that needs naming is `r-efi 6.0.0`**: its declaration offers LGPL-2.1-or-later as an alternative. It is tri-licensed, and the table records it as **taken under MIT** (`VENDORED.md:68,135`); no GPL-only, LGPL-only or AGPL crate is present, and the THINK says the same (`THINK:40-43`). Taken under MIT, it is allowed. `unicode-ident`'s `(MIT OR Apache-2.0) AND Unicode-3.0` and the four `Unicode-3.0` crates are also allowed under the 2026-10-31 addition.

**Source records.** All 99 rows checked: the 95 crates with a `.cargo_vcs_info.json` have a `git.sha1` that matches the table's upstream commit, and the four `(crates.io checksum)` rows — `malloc_buf`, `topological-sort`, `wio`, the `winapi` stub — have no VCS info and are recorded by checksum, as the table says. Every `vendor/` directory has its `.cargo-checksum.json`; the patched copies and the stub do not, which matches the layout section (`VENDORED.md:15-22`). The font row's SHA-256 is also correct: `sha256sum third_party/ot61-swgl/fonts/robotoslab/RobotoSlab-wght.ttf` is `786ae192…5b16`, the value in `VENDORED.md:178`; its `LICENSE.txt` is Apache-2.0 and `METADATA.pb` says `APACHE2`.

## #77.2 PASS — all three patches reproduce `patched/`; the generated table matches the smoke

The dispatch names two patches; the tree has a third (the S2 font backend), so I checked all three.

For each patch I copied the corresponding `vendor/` crate to a scratch tree, applied the patch with `patch -p0` (the patches are `diff -ruN` output rooted at `third_party/ot61-swgl`), and diffed the result against `patched/` with `diff -r -x .cargo-checksum.json -I 'SPDX-License-Identifier'`:

- `swgl-0.70.0-force-clear.patch` → `patched/swgl-0.70.0`: clean, **no differences**;
- `webrender-0.70.0-frame-time.patch` → `patched/webrender-0.70.0`: clean, **no differences**;
- `wr_glyph_rasterizer-0.70.1-neoplutarch.patch` → `patched/wr_glyph_rasterizer-0.70.1`: clean, **no differences**.

All three applied without fuzz. The `-I SPDX-License-Identifier` is the rule `VENDORED.md:22` states: the patched `.rs`/`.toml` files carry an added SPDX line that is deliberately not in the patch, so the patch stays applicable upstream. The patched files I inspected do carry it (e.g. `patched/webrender-0.70.0/src/internal_types.rs:1`, `patched/wr_glyph_rasterizer-0.70.1/src/platform/neoplutarch/font.rs:1`); `patched/swgl-0.70.0/src/gl.cc:1` carries upstream's MPL header.

**The table.** `python3 third_party/ot61-swgl/gen-patch-table.py ../../third_party/ot61-swgl Cargo.lock`, run from `userland/webrender-smoke`, produces a 100-line table that is **byte-identical** to the `[patch.crates-io]` block at `userland/webrender-smoke/Cargo.toml:40-139` (`diff -u` empty). Its 99 entries are exactly the 99 `VENDORED.md` crates, with the patched copies replacing the vendor copies and `winapi` pointing at `stubs/`.

One wording gap: `VENDORED.md` itself has **no** `[patch.crates-io]` table (the script's docstring says it prints "the `[patch.crates-io]` table that points a crate at this vendored tree", `gen-patch-table.py:3-10`). The item's second half is therefore satisfied against the smoke's `Cargo.toml`, not a table in `VENDORED.md`; the crate **set** still matches `VENDORED.md` exactly.

## #77.3 PASS — the closure build resolves only vendored path entries; the one unpinned fetch is the inherited rustup bootstrap

**The closure is hermetic.** `userland/webrender-smoke/Cargo.lock` has **zero** `source =` lines: all 99 third-party packages are path entries into `third_party/ot61-swgl`, so Cargo has no registry or git source to fetch for the build. The new job checks exactly that at `.github/workflows/ot61-closure.yml:348-357`: it greps the lock for `source = "registry"` and fails if any crate resolves to crates.io. I also grepped every `vendor/*/build.rs` and `patched/*/build.rs`: none contains `curl`, `wget`, `git clone`, `git fetch`, `reqwest` or `ureq`; the build scripts only compile/translate local sources. The head's `webrender-build` job ran and passed (run [`42454662105`](https://github.com/SecondStageTurbine/LogansOS/actions/runs/42454662105), job [`128651010499`](https://github.com/SecondStageTurbine/LogansOS/actions/runs/42454662105/job/128651010499)), so this path was exercised.

**Caveat (note 3).** The job also contains the self-hosted-runner rustup bootstrap, which curls `https://sh.rustup.rs` and pipes it to `sh` at `:320-321` — an unpinned script. It is copied verbatim from the four pre-existing jobs on `main` (`origin/main:.github/workflows/ot61-closure.yml:74,141,204,254`), so it is not introduced by #77's logic, and it is guarded by `if ! command -v rustup` so it does not run on a runner that already has rustup. If the checklist's "fetches" is read to include toolchain provisioning (not just the crate closure), this is the one line that fails the literal test; the fix is to pin that installer by hash or drop the bootstrap for the GitHub-hosted path, as a separate change.

## #77.4 PASS — a wrong frame exits non-zero, and the harness requires the pixel-check lines

`userland/webrender-smoke/src/main.rs` builds the expected frame by hand (`expected()` at `:129-148`; the module doc at `:1-34` derives the scene items and the gradient/text bounds). At `:408-426` it calls `first_mismatch(&fb, true)`, which walks every pixel outside the gradient and text boxes and compares it exactly with `expected()` (`:157-169`); `check(4, …, bad.is_none())` prints `FAIL(4)` and exits 4 on any mismatch. The gradient (`:428-443`) and text (`:445-459`) checks also fail the process, and the final `PASS` line is at `:461`.

On the kernel side, `kernel/src/test_harness.rs:3054-3097` spawns `webrender-smoke`, waits for it to exit, and fails on any non-zero code (`:3089-3090`) or on a frame-accounting underflow (`:3092-3093`). `tools/xtask/src/main.rs:18792-18806` additionally requires these exact serial lines: `ok(4) solid, clipped, transformed and image pixels == the hand-derived scene`, `ok(5)` gradient, `ok(6)` glyph lookup, `ok(7)` text, and `PASS`. So a wrong frame cannot pass as "no panic": it exits 4, and the harness turns that into a failed scenario.

The head's full-tier run is green (run [`42454686047`](https://github.com/SecondStageTurbine/LogansOS/actions/runs/42454686047), all four QEMU shards, host/build, RNG and fuzz); `webrender_smoke` is in the default suite (`ignored: false`, `tools/xtask/src/main.rs:2312-2320`) and is not hardware-only or explicit-only, so it is in that run's default selection.

## All.1 PASS — the feature is opt-in and the off build is unchanged

`kernel/Cargo.toml:495` defines `neo-test-webrender-smoke = ["neo-test", "browser-runtime", "mem-quota", "kernel-rng"]`; it is **not** in the default set at `kernel/Cargo.toml:81` (`default = ["kernel-diagnostic", "accel-sim", "inference-preemptible", "user-copy-fixup"]`). The standalone crate is excluded from the root workspace at `Cargo.toml:101`, so `cargo build` does not build WebRender. The harness `SCENARIO` and the scenario block are both `#[cfg(feature = "neo-test-webrender-smoke")]` (`kernel/src/test_harness.rs:174-175,3054`), and the xtask entry is data that only builds the smoke when the scenario is selected (`tools/xtask/src/main.rs:2312-2320`, `:8240-8241`). No production kernel or userland path references the feature, and the only other appearance is the new CI job, which is separate from the image build. Off is today's image.

## All.2 PASS — the prior-art survey is present, with licences and dates

The PR body's "Prior art / licences" paragraph points at the rung-4 THINK and `third_party/ot61-swgl/VENDORED.md`. The THINK's **Prior-art survey for the font backend** at `docs/THINK-2026-10-31-ot61-r4-swgl-floor.md:60-71` is a real survey: eight candidates, each with what it does, its licence, its last commit SHA and date (checked 2026-10-31), fit, and what was taken. It records the two taken (`skrifa`, `zeno`) and the rejected ones, including FreeType's FTL/GPL-2.0 licence and rusttype's abandonment. `VENDORED.md:64-178` records every vendored crate's licence, repository and upstream commit, plus the font's date and SHA-256. That satisfies "licences and dates for what's borrowed".

The THINK's S2 crate count and licence histogram (`THINK:38-40`: "about 87", 45/13/11/9) predate the final table's 99 crates / 49-11-11-10; they are documentation drift, not a missing survey (note 4). `VENDORED.md:66`'s "the first 18" is also misleading because the table is alphabetical, not dependency-ordered; the actual S1 lock has 18 third-party crates spread through the table.

## Verification performed

- `git fetch origin +pull/77/head:refs/pr/77`; head `c8a06574` (matches the dispatch), base `13eed2c5` (PR's `main` base).
- `git diff --stat origin/main...refs/pr/77`: 3,527 files, +1,261,539/−29 — almost all of it the vendored tree. Reviewed the first-party files and the vendored **records** only, per the dispatch.
- Parsed all 99 `VENDORED.md` rows and every crate's `Cargo.toml` licence field; 0 mismatches.
- Compared the lock's 102 packages to the 102 crate directories (99 distinct crates); `dirs − lock = ∅`, `lock − dirs` = the three first-party path packages.
- Verified every crate's `.cargo_vcs_info.json` SHA against its table row; 0 mismatches.
- Applied all three patches to scratch copies of `vendor/` and diffed them to `patched/` with `diff -r -x .cargo-checksum.json -I SPDX-License-Identifier`; 0 differences.
- Ran `gen-patch-table.py` against the smoke's `Cargo.lock`; output byte-identical to `Cargo.toml:40-139` (99 entries).
- Checked the font's SHA-256 and licence; matched `VENDORED.md:178`.
- Grepped the workflow, build scripts and lock for network fetches/sources.
- Read the harness block, the xtask scenario registration and needles, and `main.rs`'s oracle.
- GitHub check-runs at `c8a06574`: full-tier run `42454686047` green (4 QEMU shards, host/build, RNG, fuzz); OT-61 closure run `42454662105` green, including `WebRender S2 smoke on SWGL over neo-crt` (`128651010499`). The full run is a `pull_request` event on `claude/ot61-r4b-webrender-v2`, i.e. the PR's own full-alone run.
- RAG: not needed for this review (the work is record/tree comparison, not symbol search); used git/show/grep/read.
- Working tree: only this RETURN on `dexter/pr77-review`; the pre-existing untracked `pr59-review/` directory is untouched.

## Non-blocking notes

1. **Base drift.** The PR is based on `13eed2c5`; `origin/main` is now `e6b3d665c`. The shared code files moved: `git diff --stat 13eed2c5..origin/main -- kernel/Cargo.toml kernel/src/test_harness.rs tools/xtask/src/main.rs` is **+1,048/−17**. This review is of the PR's own diff against its stated base, as §329 asks; the PM's batch assembly/rebase and the gate on the assembled tree are still the merge evidence. The head's green full run is the PR's own `pull_request` full-alone run on the old base — close-gate evidence, not a batch run on the assembled tree.
2. **The PR body's "not kernel-touching" is wrong at this head.** The diff touches `kernel/Cargo.toml`, `kernel/src/test_harness.rs` and `tools/xtask/src/main.rs`, and `scripts/worker-close-gate.sh:153` classifies any `kernel/` or `tools/xtask/` change as kernel-touching. The head's full-tier run is green anyway, so this is a wording correction, not a gate gap — but the PR body sentence should not be relied on.
3. **Inherited unpinned rustup bootstrap.** `.github/workflows/ot61-closure.yml:320` curls `https://sh.rustup.rs` with no hash/version. It is copied from four existing jobs on `main`, is conditional on rustup being absent, and is outside the crate closure. If item #77.3 is read literally, that is the one failing line; fix it separately rather than bouncing this PR for it.
4. **Stale counts in the THINK/VENDORED prose.** `THINK:38-40` says "about 87" crates and gives 45/13/11/9 licence counts; the final table is 99 crates and 49/11/11/10. `VENDORED.md:66` says the S1 closure is "the first 18", but the table is alphabetical and S1's 18 are spread through it. The records themselves are correct; only the summaries need a refresh.
5. **`r-efi`'s LGPL alternative.** Named for completeness: it is the only row whose licence expression includes LGPL (`MIT OR Apache-2.0 OR LGPL-2.1-or-later`, `VENDORED.md:135`). The project takes it under MIT, which is allowed; it is not an LGPL-only crate.

## Recommendation to the PM

**PASS — #77 clears this review's six items.** The closure is complete and its licence/source records are internally consistent, the three patches are faithful, the CI job rejects crates.io resolution, the oracle is a real pixel check that fails a wrong frame, the feature is opt-in, and the prior-art survey is present. Merge on the PM's batch/full-tier evidence for the assembled tree, with note 1 (base drift) as the reason to gate the assembly rather than this old-base run, and correct the PR body's "not kernel-touching" sentence.
