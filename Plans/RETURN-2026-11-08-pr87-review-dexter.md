<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — relay 354: #87 SpiderMonkey vendor + generator + frozen outputs

**Seat:** Dexter. **Model:** DeepSeek, high. **Answers:**
`Plans/DISPATCH-2026-11-08-relays-335-339-reroute.md` §354.

**Reviewed:** PR #87 head `fd1ec760808333064fcc8297f7a2fcff50180f59` ("OT-61 C3c-1 generator: check
normalizer_glue.h against cbindgen 0.29.4"), the head the dispatch names. Branch `claude/ot61-c3c1-mozjs-vendor`,
merge-base `e6b3d665cb509c45b63a0fdce0f53e737f62fe8b`, 23 commits, 10,087 files, +3,873,415. `origin/main` at review
time is `65af2ff45184af05122a028869c7953904038f97`.

**Overall: FAIL at the dispatched head.** Item 1 fails on its second half — the three GPL-only files the dispatch
names are all still in the vendored tree. Items 2 and 5 pass. Item 3 fails narrowly: the workflow pins the Rust
toolchain and cbindgen but not the clang/python/make that actually produce the frozen set. Item 4 fails narrowly:
the linked-part licence records hold, but `js/src/dtoa.c` is a linked part (included by
`js/src/util/DoubleToString.cpp:70`, and it has its own compile entry) whose Lucent notice is not on CLAUDE.md's
allowed list, and it is **not** flagged as pending J.c. anywhere — the `js/src` row calls it MPL-2.0.

**The branch moved while I reviewed it.** At `90a4decdc` (2026-11-08 21:28Z, five commits past the dispatched head)
the coder landed "drop the GPL/LGPL files from the vendored SpiderMonkey tree", which cuts all three named files and
more; my re-check of that tree is in the annex. That commit's generator run was still queued at review time, so the
dispatched head's green run is the only green evidence in hand. Items 3 and 4 are still open at the tip.

## Checklist, in the dispatch's order

| # | Dispatch item | Verdict | Enforcing refs (all at `fd1ec760`) |
|---|---|---|---|
| 1 | Vendor matches `servo/mozjs` @ `138c9470` except the recorded cut-list and patches; the GPL-only files are cut | **FAIL** — the hash half passes: all 9,964 vendored blobs identical to upstream, 0 differing, 0 extra, 806 missing and all 806 in the cut-list; all three patches apply cleanly. The GPL half fails: yamllint, `make_unicode.py` and `run_complete` are all vendored, plus pylru (GPL-2.0+) and setuptools' `_vendor/autocommand` (LGPLv3) | `VENDORED.md:107-115`; `.../third_party/python/yamllint/yamllint-1.37.1.dist-info/METADATA:6`; `.../js/src/util/make_unicode.py:9`; `.../js/src/devtools/rootAnalysis/run_complete:8`; `.../third_party/python/pylru/pylru.py:8`; `.../setuptools/_vendor/autocommand-2.2.2.dist-info/METADATA:7` |
| 2 | The generator job runs with the network off, and fails when its output differs from the committed frozen files | **PASS** (one noted exception) | `ot61-mozjs-generate.yml:60-106,110-153,170-173,203-228`; run `42519658102` green at this head |
| 3 | The workflow takes no secrets, writes nothing to `main`, and runs on a pinned toolchain | **FAIL (narrow)** — no secrets and no writes pass; the toolchain is not pinned | `ot61-mozjs-generate.yml:18-21,30,50-56,121-122,238`; contrast `ot61-closure.yml:193,218-229` |
| 4 | Licence records cover every linked part; `dtoa.c` is flagged as pending J.c. | **FAIL (narrow)** — the directory records hold; `dtoa.c` is unrecorded and its row is wrong | `VENDORED.md:127`; `.../js/src/util/DoubleToString.cpp:70`; `.../js/src/dtoa.c:4-18`; `generated/compile_commands.json` (`@SRC@/js/src/util/DoubleToString.cpp`) |
| 5 | Nothing in the default product image changes | **PASS** | `Cargo.toml:4-96`; `.github/workflows/ci.yml:1052-1056,1432-1463`; the PR touches no kernel/userland/tools/scripts/Cargo file |

## 1. Vendor tree and the GPL cut — FAIL

### The hash check passes: upstream minus exactly the recorded cut-list

I fetched `servo/mozjs` at `138c9470151b8dcbb335f249cb83f9404857baf9` with
`git fetch --depth 1 --filter=blob:none` and compared **tree entries**, not contents (the dispatch says not to audit
upstream's contents):

- upstream `git ls-tree -r` at that commit: **10,770** blobs.
- `third_party/ot61-mozjs/vendor/mozjs-138c9470/` at `fd1ec760`: **9,964** blobs.
- **All 9,964 have the same (mode, blob SHA) as upstream: 0 differ, 0 are extra.** That also covers the CRLF
  files in upstream's index — the blobs are byte-identical, so `vendor/.gitattributes`' `* -text` is doing its job.
- **806 absent**, and all 806 are in `VENDORED.md:107-115`: 536 `nsprpub/`, 129 `third_party/xsimd/`, 101
  `js/src/zydis/`, 12 `third_party/gemmology/`, 11 `third_party/function2/`, 6 repo-metadata files
  (`.gitattributes`, `android-build`, `Cross.toml`, `shell.nix`, `publish_crates_io.py`,
  `PULL_REQUEST_TEMPLATE.md`) and 11 binaries (8 setuptools `*.exe`, `icudt78l.dat`, `test.manifest.jar`,
  `fetched_artifact.zip`). No unrecorded path.
- `mozjs-extracted-crates/` is present and is **204** files, as `VENDORED.md` says, all hash-identical.
- The three patches are applied to a copy, never to `vendor/` (`ot61-mozjs-generate.yml:37-48`). I checked them
  against upstream at that commit: `git apply --check` then apply of `0001` → `0002` → `0003` in order, **all
  clean**, 29 files touched.

`VENDORED.md:115` says "10 binaries" but the row lists 11 (8 + dat + jar + zip). Count note, not a finding.

### The GPL half fails: the three named files are all present

The dispatch names three. All three are vendored at `fd1ec760`, unmodified:

| File | Evidence |
|---|---|
| `.../third_party/python/yamllint/` (40 files) | `yamllint-1.37.1.dist-info/METADATA:6` — `License: GPL-3.0-or-later`; `:14` — `GNU General Public License v3 (GPLv3)` |
| `.../js/src/util/make_unicode.py` | `:9-17` — "This program is free software … GNU General Public License … version 3 or later" |
| `.../js/src/devtools/rootAnalysis/run_complete` | `:8-15` — same GPL-3.0-or-later notice (Sixgill, Stanford) |

A sweep of the same copy finds at least two more, so a cut limited to the three would still not satisfy CLAUDE.md:

- `.../third_party/python/pylru/pylru.py:8` — GPL-**2.0**-or-later (`Copyright (C) 2006-2022 Jay Hutchinson`).
- `.../third_party/python/setuptools/setuptools/_vendor/autocommand-2.2.2.dist-info/METADATA:7` — `License:
  LGPLv3`.

CLAUDE.md's survey rule is unambiguous: "**GPL, LGPL and AGPL code is never copied.** Read it for the principle
only." `VENDORED.md`'s defence — "the generator-time trees … are never linked" — is true but not the rule; and item
1's own test is that these files are cut, not that they are inert.

**The two halves of the item are in tension, and the fix is the recorded cut-list.** The tree is byte-identical to
upstream *because* nothing was removed. Cutting these files necessarily breaks byte-identity for those paths, so the
cut-list has to grow and the "matches upstream except the cut-list" check has to be the one that proves the removal
(vendor an explicit list, or keep a per-file absent-list with its licence). That is exactly what `90a4decdc` now
does — see the annex.

## 2. Network off and drift — PASS

**Network isolation.** Every step that executes third-party build machinery runs under `sudo unshare -n …`:
`configure` for the host target (`:60-66`), `make pre-export export` (`:95-98`), the NeoPlutarch
`configure`/`export` (`:119-125`), and the compile probe (`:170-173`). `unshare -n` gives the step a namespace with
no interfaces, so a fetch fails rather than silently succeeding.

**Drift fails the job.** The freeze step (`:203-228`) writes a fresh set into `$RUNNER_TEMP/frozen`, then
`diff -r "$committed" "$frozen"`; any difference prints the diff and `exit 1` (`:213-219`). The committed
`generated/` exists in this PR, so the check runs rather than falling into the emit-the-tarball branch.

**Evidence at this head.** Run
[42519658102](https://github.com/SecondStageTurbine/LogansOS/actions/runs/42519658102) is green on `fd1ec760`, and
its step list includes `freeze the outputs, and check them against the committed copy` **success** and
`normalizer_glue.h against cbindgen` **success**. The previous run at the freeze commit (`42519019972`, `55ff15aa`)
is green too.

**The frozen set is internally consistent** with `VENDORED.md`: `compile_commands.json` has 167 entries, 81 of them
`Unified_*`; `generated/obj/` has 96 files; `dist_include.txt` 396 links; `system_wrappers.txt` 1,061 names; no
absolute path (`/home/`, `/tmp/`, `/github/`) survives in any frozen file. `freeze.py` sorts the walks and entries,
tokenises `@SRC@`/`@OBJ@`/`@REPO@`, and fails on a leaked root (`freeze.py:50-69,147-160,219-232`).

**One noted exception.** `cargo install cbindgen --version 0.29.4 --locked` (`:238`) is a **network-on** step: it
installs from crates.io, then runs cbindgen under `unshare -n` (`:241-243`). The workflow's own comment says so
(`:230-234`). It does not feed `generated/` — it only checks the hand-written `normalizer_glue.h` — so the frozen set
is still produced offline. But the job is not wholly network-off, and that dependency is worth naming: if crates.io
or the pinned Rust toolchain is unavailable, the header check (not the freeze) fails. If the PM's rule is "the whole
job, no network at all", this is the one step to move out or vendor cbindgen for.

Related, not item 2: the compile probe is `continue-on-error: true` (`:159`) — by design a probe, not a gate — so the
job can be green with the engine failing to compile. The drift check is unaffected.

## 3. Secrets, writes, pinned toolchain — FAIL (narrow)

**Takes no secrets: pass.** No `secrets.*`, no `GITHUB_TOKEN`, no `permissions:` block, no authenticated action. The
only repository config it reads is `vars.CI_RUNNER_LABELS` (`:30`), a repository variable.

**Writes nothing to `main`: pass.** The workflow has no `git push`, `git commit`, PR or release step; it writes only
under `$RUNNER_TEMP` and uploads an artifact (`:270-290`). `on:` is `workflow_dispatch` plus `push` to
`claude/ot61-**` only (`:15-21`), so it cannot fire on `main`.

**Pinned toolchain: fail.** The Rust side is pinned — `rust-toolchain.toml` is `nightly-2026-04-16` (which
`cargo install` picks up at the repo root), and cbindgen is `--version 0.29.4 --locked` (`:238`). The toolchain that
actually produces the frozen set is not:

- `runs-on: … 'ubuntu-latest'` (`:30`) — a moving image;
- `NEO_CLANG=clang`, `HOST_CC=clang`, `HOST_CXX=clang++` (`:121-122`) — bare PATH names, plus `clang` again in the
  resource-dir and cbindgen steps (`:170-172`, `:241-243`); the "Tools present" step only **prints** the versions
  (`:50-56`);
- `python3` and `make` are likewise the runner's.

This matters because `js-confdefs.h` and the compile database are configure's compiler probes:
`HAVE_VISIBILITY_ATTRIBUTE`, `HAVE_MALLOC_USABLE_SIZE`, `TARGET_XPCOM_ABI` and the rest are written by tests that
compile with clang, and the drift check compares that header byte for byte. A runner-image bump that changes the
clang major can redden the generator with a diff that is not a source change — or, worse, silently change the frozen
ABI defines when the committed set is regenerated.

The repo already has the pattern: `ot61-closure.yml:193` pins `NEO_CRT_CLANG_MAJOR: "19"`, `:218-229` installs
`clang-19`, asserts `-dumpversion` matches, and checks it against the Dockerfile, so "the compiler can't move
silently". This workflow should do the same (`NEO_CLANG=clang-19`, `HOST_CC=clang-19`, `HOST_CXX=clang++-19`, and the
`:50-56` health check against it), or state why the frozen set is clang-version-independent.

## 4. Licence records and `dtoa.c` — FAIL (narrow)

**The linked-part records hold.** I checked each row against the vendored files:

| Part | Record | Check |
|---|---|---|
| `js/src`, `js/public`, `mfbt`, `mozglue/misc` | MPL-2.0 | per-file MPL headers; correct for these, except `dtoa.c` below |
| `mfbt/double-conversion` | BSD-3-Clause | `mfbt/double-conversion/LICENSE` present, V8 authors |
| `js/src/irregexp` | BSD-3-Clause | `js/src/irregexp/LICENSE.v8` present (`LICENSE` alone does not exist there; cosmetic) |
| `modules/fdlibm` | BSD-2-Clause style | `modules/fdlibm/src/LICENSE` present |
| `third_party/fmt` | MIT | `third_party/fmt/LICENSE` present |
| zlib | zlib, headers only | `modules/zlib/src/{zlib.h,zconf.h}`, licence in the header |
| `*_glue` crates | MPL-2.0 | `Cargo.toml: license = "MPL-2.0"` + MPL headers (`normalizer_glue`, `unicode_bidi_ffi`) |
| `icu_*` | Unicode-3.0 | `license = "Unicode-3.0"`; `icu_normalizer`/`icu_collections` also ship `LICENSE` |
| `utf16_iter`, `utf8_iter` | Apache-2.0 OR MIT | both `LICENSE-APACHE` and `LICENSE-MIT` present |
| `encoding_c_mem-0.2.6` | Apache-2.0 OR MIT | `LICENSE-APACHE`, `LICENSE-MIT`, `COPYRIGHT` present |

**`dtoa.c` is not flagged, and its row is wrong.** `js/src/dtoa.c:4-18` carries the David M. Gay / Lucent notice
("Permission to use, copy, modify, and distribute this software for any purpose without fee is hereby granted …"),
which is permissive but **not** on CLAUDE.md's allowed list (MIT, Apache-2.0, BSD, zlib, ISC, MPL-2.0, Unicode-3.0,
Unlicense/CC0). It is not decoration: `js/src/util/DoubleToString.cpp:70` `#include`s it, and
`generated/compile_commands.json` has an entry for `@SRC@/js/src/util/DoubleToString.cpp` — so Gay's code is
compiled into the engine and linked. Yet `VENDORED.md:127` folds the whole of `js/src` into "MPL-2.0", the PR body
never mentions it, and THINK #10 §2.2 (`refs/pr/10` `c2d5f3cc`, the full table `VENDORED.md` defers to) has the same
blanket row and no `dtoa` note. There is nowhere the "pending J.c." status lives.

The fix is a record edit, not a cut: `VENDORED.md`'s `js/src` row gets "MPL-2.0 except `js/src/dtoa.c` (David Gay,
Lucent permissive — **pending J.c.**)", with `DoubleToString.cpp:70` cited as the inclusion point, and THINK #10 §2.2
matches. Item 4 passes once that is written and J.c. has ruled.

**A related integrity note (not a checklist item).** `encoding_c_mem-0.2.6/` has no `.cargo-checksum.json`, so
`cargo xtask vendor-verify` skips it (`tools/xtask/src/vendor_verify.rs:118-139` only collects crates that have one;
CI runs it at `ci.yml:1052-1056`). Its only integrity record is the `.crate` sha256 in `VENDORED.md`. The mozjs tree's
record is the hash check above, which is strong; the extra vendor dirs have prose only.

## 5. Default product image — PASS

- The PR's non-vendor changes are only `.github/workflows/ot61-mozjs-generate.yml`, `VENDORED.md`,
  `generator/{freeze.py,neo-cc,neo-c++,neo-crt-symbols.txt,include/normalizer_glue.h}`, `patches/000{1,2,3}` and
  `vendor/.gitattributes`. No `Cargo.toml`, `Cargo.lock`, `kernel/`, `userland/`, `tools/`, `scripts/`, `Dockerfile`
  or `boot/` change.
- The workspace `members` list is explicit (`Cargo.toml:4-96`) and does not include any `third_party` tree; nothing
  outside the new directory or its workflow mentions `ot61-mozjs` (checked by `git grep` at the head). So `cargo
  metadata`, `fmt`, `clippy` and the image build do not see it.
- The `Dockerfile` mounts the repo as a volume rather than baking it (`.dockerignore` comment), so the 10k files do
  not enter the dev image either.
- The CI guards that could choke on 10,000 new files do not: `vendor-verify` skips a crate with no checksum file
  (above), and the SPDX step scans only `kernel/ userland/ tools/` `.rs` and `-maxdepth 3` `*.toml` (`ci.yml:1432-1463`)
  — none of the new paths. `VENDORED.md:23-24` says "the SPDX check skips `third_party/*/vendor/*`"; the check
  doesn't scan `third_party` at all, so the outcome holds and only the stated reason is imprecise.

`VENDORED.md`'s own statement — "Nothing builds from this tree yet … the default kernel, image and userland are
unchanged" (§Layout) — matches what I can verify.

## Annex: the branch tip at review time (`90a4decdc`)

The branch does not sit at the dispatched head. After `fd1ec760` it has:

| Commit | Subject | Generator run |
|---|---|---|
| `5088aeee7` | bindgen for jsapi and jsglue (THINK #89 §1) | `42523300778` cancelled |
| `db24be390` | a drift failure also prints the fresh frozen set | `42523325631` failure |
| `7854527e3c` | libclang from the same LLVM as clang | `42524544804` failure (bindgen step) |
| `7ad972ab` | patch 0004, jsglue.cpp's malloc_usable_size | `42525150414` in progress |
| `90a4decdc` | **drop the GPL/LGPL files from the vendored SpiderMonkey tree** | `42525394638` pending |

What I verified at `90a4decdc` (not the dispatched head, so not folded into the verdicts above):

- **Item 1's GPL half is fixed.** `yamllint/`, `js/src/util/make_unicode.py`, `run_complete`, `pylru/` are absent
  (0 files each), as are `mozbuild/action/tooltool.py`, `setuptools/_vendor/autocommand/` and ICU's unused
  `config.sub`/`config.guess`/`aclocal.m4`. `VENDORED.md:118` records all of them with the CLAUDE.md reason, and
  `:121-127` flags the two remaining GPL-with-exception / dual-licence cases (`build/autoconf/config.{sub,guess}`,
  `js/src/vtune` + `ittnotify.h` as BSD) with a fallback.
- The tree is still upstream minus a recorded cut-list: **9,906** blobs, **864** missing (the original 806 plus the
  58 GPL/ICU paths), **0 extra, 0 differing**; every extra missing path is in the new `VENDORED.md` row.
- **Item 4 is still open at the tip**: the `js/src` row still says MPL-2.0 and there is still no `dtoa` mention.
- **Item 3 is still open at the tip**: `clang` remains a bare PATH name (the new bindgen step adds
  `clang -print-resource-dir` and `LIBCLANG_PATH`, still unpinned).
- **The tip has no green generator run yet** — `90a4decdc` is queued. The runs before it are red:
  `7854527e3c`'s job log shows the bindgen step failing, and `db24be390` also failed (I did not open its log).
  The GPL cut itself is not a build input (mach's `.pth` entries skip missing dirs; `tooltool` and `autocommand`
  serve commands the generator never runs), so the freeze should be unchanged, but that is a prediction, not
  evidence until a run passes at the cut head.

## What would clear the two open items

1. **Pin clang** as `ot61-closure.yml:218-229` does: install `clang-${NEO_CRT_CLANG_MAJOR}`, assert the major, and
   pass it as `NEO_CLANG` / `HOST_CC` / `HOST_CXX` (and the cbindgen step's `PATH`). If the frozen set is meant to be
   clang-independent, say so and pin `python3`/`make` instead — but then a clang bump must not be able to red the
   drift check.
2. **Record `dtoa.c`**: one line in `VENDORED.md`'s `js/src` row (and THINK #10 §2.2) marking the David Gay / Lucent
   code as pending J.c., citing `js/src/util/DoubleToString.cpp:70`.

Neither is a redesign; both are record/pin changes. Item 1 is already fixed on the branch.

## What I did not check

I did not run the generator locally (it needs clang, `unshare -n` and the pinned toolchain, and the dispatch scopes
this to the records and our own pieces). I did not read upstream's file contents — the vendor check is tree-entry
hashes, as the dispatch says. The green-run evidence is the GitHub run's step list, not a re-execution.
